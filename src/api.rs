//! The page's JSON API under `/api/`, reached through the panel's bridge.
//!
//! | request                      | what it does                                         |
//! |------------------------------|------------------------------------------------------|
//! | `GET /api/state`             | folders, links to the index shards, progress, favourites |
//! | `POST /api/links`            | links to thumbnails, previews and originals          |
//! | `GET /api/stream/<id>?mode=` | an HLS playlist of a video: `copy` or `h264`         |
//! | `PUT /api/favorites/<id>`    | marks a favourite; `DELETE` unmarks it               |
//! | `POST /api/favorites`        | marks or unmarks many: `{ids, on}`                   |
//! | `DELETE /api/files/<id>`     | moves the file to the user's trash                   |
//! | `POST /api/files/delete`     | moves many to the trash: `{ids}`                     |
//! | `POST /api/uploads`          | links to upload files to, past the plugin ([`upload`]) |
//! | `PUT /api/upload-folder/<id>`| remembers the folder uploads went to last            |
//! | `POST /api/rebuild`          | walks the folders again, drops what is no longer given |
//! | `PUT /api/faces`             | turns faces on or off: `{enabled}`                   |
//! | `DELETE /api/faces`          | turns faces off and deletes faces, people and names  |
//! | `GET /api/faces/<id>`        | a photo's faces: boxes and people                    |
//! | `PUT /api/people/<id>`       | names, hides or shows a person: `{name?, hidden?}`   |
//! | `POST /api/people/merge`     | merges people: `{from, into}`                        |
//!
//! The index itself never passes through the plugin: the page fetches its
//! shards by links, and the browser keeps them while they don't change.

use std::collections::HashSet;

use serde::Deserialize;
use serde_json::json;

use crate::index;
use crate::media::{self, preview_path, thumb_path};
use crate::rustcloud::app::jobs::{self as queue, NewJob};
use crate::rustcloud::app::links::{self, StreamMode, Target};
use crate::rustcloud::app::{appdata, files, use_ffmpeg};
use crate::upload::{self, UploadRequest};
use crate::web::{error, json, ok, query_param, valid_id};
use crate::Response;
use crate::{faces, people};

/// Most links one request asks for (the server's limit for `links.create`).
const MAX_LINKS: usize = 1000;
/// Most files one request deletes or marks (the server's limit for `jobs.add`).
const MAX_IDS: usize = 1000;
const FAVORITES: &str = "favorites.json";
const UPLOAD_FOLDER: &str = "upload-folder.txt";

pub fn handle(method: &str, path: &str, query: &str, body: &[u8]) -> Response {
    match (method, path) {
        ("GET", "/state") => state(),
        ("POST", "/links") => match serde_json::from_slice::<LinksRequest>(body) {
            Ok(request) => links_for(&request.items),
            Err(err) => error(400, &err.to_string()),
        },
        ("POST", "/uploads") => match serde_json::from_slice::<UploadRequest>(body) {
            Ok(request) => upload::links_for(&request),
            Err(err) => error(400, &err.to_string()),
        },
        ("POST", "/favorites") => match serde_json::from_slice::<FavoritesRequest>(body) {
            Ok(request) => favorite_many(&request.ids, request.on),
            Err(err) => error(400, &err.to_string()),
        },
        ("POST", "/files/delete") => match serde_json::from_slice::<IdsRequest>(body) {
            Ok(request) => delete_many(&request.ids),
            Err(err) => error(400, &err.to_string()),
        },
        ("POST", "/rebuild") => rebuild(),
        ("PUT", "/faces") => match serde_json::from_slice::<FacesRequest>(body) {
            Ok(request) => faces::set_enabled(request.enabled),
            Err(err) => error(400, &err.to_string()),
        },
        ("DELETE", "/faces") => faces::forget_all(),
        ("POST", "/people/merge") => match serde_json::from_slice::<MergeRequest>(body) {
            Ok(request) => people::merge(&request.from, &request.into),
            Err(err) => error(400, &err.to_string()),
        },
        _ => {
            if let Some(id) = path.strip_prefix("/stream/") {
                return match method {
                    "GET" => stream(id, &query_param(query, "mode")),
                    _ => error(405, "method not allowed"),
                };
            }
            if let Some(id) = path.strip_prefix("/files/") {
                return match method {
                    "DELETE" => delete(id),
                    _ => error(405, "method not allowed"),
                };
            }
            if let Some(id) = path.strip_prefix("/upload-folder/") {
                return match method {
                    "PUT" => remember_upload_folder(id),
                    _ => error(405, "method not allowed"),
                };
            }
            if let Some(id) = path.strip_prefix("/faces/") {
                return match method {
                    "GET" => faces::of_file(id),
                    _ => error(405, "method not allowed"),
                };
            }
            if let Some(id) = path.strip_prefix("/people/") {
                return match (method, serde_json::from_slice::<PersonRequest>(body)) {
                    ("PUT", Ok(request)) => people::change(id, request.name.as_deref(), request.hidden),
                    ("PUT", Err(err)) => error(400, &err.to_string()),
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
    // People by link (with every photo of each, it can be large); names here.
    let people = match links::create(&[Target::Appdata(people::PEOPLE.into())]).pop() {
        Some(Ok(url)) => Some(url),
        _ => None,
    };
    ok(json!({
        "faces": faces::status(),
        "people": people,
        "names": people::Names::read(),
        "folders": folders,
        "shards": shard_links,
        "progress": progress,
        "favorites": favorites(),
        "uploadFolder": appdata::read(UPLOAD_FOLDER).ok().and_then(|id| String::from_utf8(id).ok()),
        "ffmpeg": use_ffmpeg::get_capabilities().available,
    }))
}

#[derive(Deserialize)]
struct FacesRequest {
    enabled: bool,
}

#[derive(Deserialize)]
struct PersonRequest {
    name: Option<String>,
    hidden: Option<bool>,
}

#[derive(Deserialize)]
struct MergeRequest {
    from: Vec<String>,
    into: String,
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

#[derive(Deserialize)]
struct FavoritesRequest {
    ids: Vec<String>,
    on: bool,
}

#[derive(Deserialize)]
struct IdsRequest {
    ids: Vec<String>,
}

fn favorite(id: &str, on: bool) -> Response {
    favorite_many(&[id.to_owned()], on)
}

fn favorite_many(ids: &[String], on: bool) -> Response {
    if let Err(response) = check_ids(ids) {
        return response;
    }
    match set_favorites(ids, on) {
        Ok(ids) => ok(json!({ "favorites": ids })),
        Err(err) => error(500, &format!("{err:?}")),
    }
}

/// Marks or unmarks favourites; returns them all.
fn set_favorites(changed: &[String], on: bool) -> Result<Vec<String>, appdata::Error> {
    let mut ids = favorites();
    let before = ids.len();
    if on {
        let mut known: HashSet<String> = ids.iter().cloned().collect();
        ids.extend(changed.iter().filter(|id| known.insert((*id).clone())).cloned());
    } else {
        let gone: HashSet<&String> = changed.iter().collect();
        ids.retain(|id| !gone.contains(id));
    }
    if ids.len() != before {
        appdata::write(FAVORITES, &serde_json::to_vec(&ids).expect("ids serialize"))?;
    }
    Ok(ids)
}

fn check_ids(ids: &[String]) -> Result<(), Response> {
    if ids.len() > MAX_IDS {
        return Err(error(400, &format!("at most {MAX_IDS} files at once")));
    }
    if !ids.iter().all(|id| valid_id(id)) {
        return Err(error(400, "not a file id"));
    }
    Ok(())
}

/// The page can't keep it: its sandbox has no storage of its own.
fn remember_upload_folder(id: &str) -> Response {
    if !valid_id(id) {
        return error(400, "not a folder id");
    }
    match appdata::write(UPLOAD_FOLDER, id.as_bytes()) {
        Ok(_) => ok(json!({})),
        Err(err) => error(500, &format!("{err:?}")),
    }
}

/// Moves a user's file to their trash.
fn delete(id: &str) -> Response {
    if !valid_id(id) {
        return error(400, "not a file id");
    }
    match delete_files(&[id.to_owned()]) {
        Ok(done) if !done.read_only.is_empty() => {
            // The folder was given for reading only: the page asks for more.
            json(403, json!({ "error": "the folder is given for reading only", "code": "read_only" }))
        }
        Ok(done) => match done.failed.into_iter().next() {
            Some((_, why)) => error(500, &why),
            None => ok(json!({ "favorites": done.favorites })),
        },
        Err(response) => response,
    }
}

/// Moves files to the trash, as many as it can: `deleted`, `readOnly` (in
/// folders given for reading only), `failed`, and the favourites left.
fn delete_many(ids: &[String]) -> Response {
    if let Err(response) = check_ids(ids) {
        return response;
    }
    match delete_files(ids) {
        Ok(done) => {
            let failed: Vec<_> = done.failed.iter().map(|(id, why)| json!({ "id": id, "error": why })).collect();
            ok(json!({
                "deleted": done.deleted,
                "readOnly": done.read_only,
                "failed": failed,
                "favorites": done.favorites,
            }))
        }
        Err(response) => response,
    }
}

struct Deleted {
    deleted: Vec<String>,
    read_only: Vec<String>,
    failed: Vec<(String, String)>,
    favorites: Vec<String>,
}

/// The server doesn't wake the plugin for its own changes, so a `media` job
/// is added for each file: it finds the file gone and takes it out of the
/// index with its pictures (only jobs write the index).
fn delete_files(ids: &[String]) -> Result<Deleted, Response> {
    let mut done = Deleted { deleted: Vec::new(), read_only: Vec::new(), failed: Vec::new(), favorites: Vec::new() };
    for id in ids {
        match files::delete(id) {
            // Already gone: the index catches up all the same.
            Ok(()) | Err(files::Error::NotFound) => done.deleted.push(id.clone()),
            Err(files::Error::ReadOnly) => done.read_only.push(id.clone()),
            Err(files::Error::NoUser) => return Err(error(401, "not logged in")),
            Err(err) => done.failed.push((id.clone(), format!("{err:?}"))),
        }
    }
    let forget: Vec<NewJob> = done
        .deleted
        .iter()
        .map(|id| NewJob { kind: "media".into(), key: id.clone(), data: "deleted".into() })
        .collect();
    if !forget.is_empty() {
        queue::add(&forget).map_err(|err| error(500, &format!("{err:?}")))?;
    }
    done.favorites = set_favorites(&done.deleted, false).map_err(|err| error(500, &format!("{err:?}")))?;
    Ok(done)
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

/// The folders the user gave the plugin, each once: a folder given again with
/// write access has two grants.
fn granted_folders() -> Result<Vec<files::Node>, files::Error> {
    let mut seen = HashSet::new();
    Ok(files::granted()?.into_iter().filter(|node| node.is_dir && seen.insert(node.id.clone())).collect())
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
