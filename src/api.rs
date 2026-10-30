//! The page's JSON API under `/api/`, reached through the panel's bridge.
//!
//! | request                      | what it does                                         |
//! |------------------------------|------------------------------------------------------|
//! | `GET /api/state`             | folders, links to the index shards, progress, favourites |
//! | `POST /api/links`            | links to thumbnails, previews and originals          |
//! | `GET /api/stream/<id>?mode=` | an HLS playlist of a video: `copy` or `h264`         |
//! | `PUT /api/favorites/<id>`    | marks a favourite; `DELETE` unmarks it               |
//! | `POST /api/rebuild`          | walks the folders again, drops what is no longer given |
//!
//! The index itself never passes through the plugin: the page fetches its
//! shards by links, and the browser keeps them while they don't change.

use serde::Deserialize;
use serde_json::json;

use crate::index;
use crate::media::{self, preview_path, thumb_path};
use crate::rustcloud::app::jobs::{self as queue, NewJob};
use crate::rustcloud::app::links::{self, StreamMode, Target};
use crate::rustcloud::app::{appdata, files, use_ffmpeg};
use crate::web::{error, ok, query_param, valid_id};
use crate::Response;

/// Most links one request asks for (the server's limit for `links.create`).
const MAX_LINKS: usize = 1000;
const FAVORITES: &str = "favorites.json";

pub fn handle(method: &str, path: &str, query: &str, body: &[u8]) -> Response {
    match (method, path) {
        ("GET", "/state") => state(),
        ("POST", "/links") => match serde_json::from_slice::<LinksRequest>(body) {
            Ok(request) => links_for(&request.items),
            Err(err) => error(400, &err.to_string()),
        },
        ("POST", "/rebuild") => rebuild(),
        _ => {
            if let Some(id) = path.strip_prefix("/stream/") {
                return match method {
                    "GET" => stream(id, &query_param(query, "mode")),
                    _ => error(405, "method not allowed"),
                };
            }
            if let Some(id) = path.strip_prefix("/favorites/") {
                return match method {
                    "PUT" => favorite(id, true),
                    "DELETE" => favorite(id, false),
                    _ => error(405, "method not allowed"),
                };
            }
            error(404, "not found")
        }
    }
}

/// Everything the page needs to start, in one answer.
fn state() -> Response {
    let folders = match granted_folders() {
        Ok(folders) => folders,
        Err(err) => return files_error(err),
    };
    let shards = match appdata::list(index::DIR) {
        Ok(entries) => entries.into_iter().filter(|e| !e.is_dir && e.name.ends_with(".json")).collect(),
        Err(appdata::Error::NotFound) => Vec::new(),
        Err(err) => return error(500, &format!("{err:?}")),
    };
    let targets: Vec<Target> = shards.iter().map(|e| Target::Appdata(format!("{}/{}", index::DIR, e.name))).collect();
    let mut shard_links = Vec::new();
    for link in links::create(&targets) {
        match link {
            Ok(url) => shard_links.push(url),
            // Changed between the listing and the link: the next poll gets it.
            Err(links::Error::NotFound) => {}
            Err(err) => return links_error(err),
        }
    }
    let progress: serde_json::Map<String, serde_json::Value> = queue::progress()
        .unwrap_or_default()
        .into_iter()
        .map(|kind| (kind.kind, json!({ "waiting": kind.waiting, "failed": kind.failed })))
        .collect();
    let folders: Vec<_> = folders.iter().map(|f| json!({ "id": f.id, "name": f.name })).collect();
    ok(json!({
        "folders": folders,
        "shards": shard_links,
        "progress": progress,
        "favorites": favorites(),
        "ffmpeg": use_ffmpeg::get_capabilities().available,
    }))
}

#[derive(Deserialize)]
struct LinksRequest {
    items: Vec<LinkItem>,
}

#[derive(Deserialize)]
struct LinkItem {
    id: String,
    /// `thumb`, `preview` or `original`.
    kind: String,
}

/// Links in the order asked for; `null` for what isn't there (a thumbnail
/// not made yet, a file gone).
fn links_for(items: &[LinkItem]) -> Response {
    if items.len() > MAX_LINKS {
        return error(400, &format!("at most {MAX_LINKS} links at once"));
    }
    let mut targets = Vec::with_capacity(items.len());
    for item in items {
        if !valid_id(&item.id) {
            return error(400, "not a file id");
        }
        targets.push(match item.kind.as_str() {
            "thumb" => Target::Appdata(thumb_path(&item.id)),
            "preview" => Target::Appdata(preview_path(&item.id)),
            "original" => Target::File(item.id.clone()),
            _ => return error(400, "kind is thumb, preview or original"),
        });
    }
    let mut urls = Vec::with_capacity(items.len());
    for link in links::create(&targets) {
        match link {
            Ok(url) => urls.push(json!(url)),
            Err(links::Error::NotFound) => urls.push(json!(null)),
            Err(err) => return links_error(err),
        }
    }
    ok(json!({ "urls": urls }))
}

/// A playlist for a video the browser can't play as it is.
fn stream(id: &str, mode: &str) -> Response {
    if !valid_id(id) {
        return error(400, "not a file id");
    }
    let mode = match mode {
        "copy" => StreamMode::Copy,
        "h264" | "" => StreamMode::H264,
        _ => return error(400, "mode is copy or h264"),
    };
    match links::video_stream(&Target::File(id.to_owned()), mode) {
        Ok(url) => ok(json!({ "url": url })),
        Err(err) => links_error(err),
    }
}

/// Favourites: file ids in AppData, in the order they were marked.
fn favorites() -> Vec<String> {
    appdata::read(FAVORITES).ok().and_then(|data| serde_json::from_slice(&data).ok()).unwrap_or_default()
}

fn favorite(id: &str, on: bool) -> Response {
    if !valid_id(id) {
        return error(400, "not a file id");
    }
    let mut ids = favorites();
    let had = ids.iter().any(|known| known == id);
    if on && !had {
        ids.push(id.to_owned());
    } else if !on && had {
        ids.retain(|known| known != id);
    } else {
        return ok(json!({ "favorites": ids }));
    }
    match appdata::write(FAVORITES, &serde_json::to_vec(&ids).expect("ids serialize")) {
        Ok(_) => ok(json!({ "favorites": ids })),
        Err(err) => error(500, &format!("{err:?}")),
    }
}

/// Walks the granted folders again and drops what is in no granted folder.
/// The index itself is changed by the jobs, one batch at a time.
fn rebuild() -> Response {
    let folders = match granted_folders() {
        Ok(folders) => folders,
        Err(err) => return files_error(err),
    };
    let ids: Vec<String> = folders.into_iter().map(|f| f.id).collect();
    let prune = NewJob { kind: "prune".into(), key: "all".into(), data: String::new() };
    match queue::add(&[prune]).and_then(|()| media::rescan(&ids)) {
        Ok(()) => ok(json!({ "folders": ids.len() })),
        Err(err) => error(500, &format!("{err:?}")),
    }
}

fn granted_folders() -> Result<Vec<files::Node>, files::Error> {
    Ok(files::granted()?.into_iter().filter(|node| node.is_dir).collect())
}

fn files_error(err: files::Error) -> Response {
    match err {
        files::Error::NoUser => error(401, "not logged in"),
        err => error(500, &format!("{err:?}")),
    }
}

fn links_error(err: links::Error) -> Response {
    match err {
        links::Error::NoSession => error(400, "links need a browser session"),
        links::Error::NotFound => error(404, "not found"),
        links::Error::Other(why) => error(500, &why),
    }
}
