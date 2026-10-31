//! Uploads from the page: `POST /api/uploads` gives one link per file, and
//! the page sends each file to its link, straight to the server — any size,
//! never through the plugin. The server starts the `media` job of each new
//! file itself: the file comes from the user.

use std::collections::{HashMap, HashSet};

use serde::Deserialize;
use serde_json::json;

use crate::rustcloud::app::files;
use crate::rustcloud::app::links;
use crate::web::{error, json, ok, valid_id};
use crate::Response;

/// Most files one request asks links for; the page asks in parts.
pub const MAX_FILES: usize = 200;

#[derive(Deserialize)]
pub struct UploadRequest {
    /// A granted folder.
    folder: String,
    items: Vec<UploadItem>,
}

#[derive(Deserialize)]
struct UploadItem {
    /// The subfolder inside `folder` (`a/b`) when a folder was dropped; empty
    /// for files.
    #[serde(default)]
    dir: String,
    name: String,
}

/// Why a link could not be made.
enum Refusal {
    /// The folder was given for reading only: nothing is uploaded.
    ReadOnly,
    /// This file only: its answer says why.
    File(&'static str, String),
    Failed(String),
}

/// Links for the files, in order: `{url, name}` with the name the file gets
/// (`IMG_1 (1).jpg` when `IMG_1.jpg` is there), or `{error, code}`. Folders
/// that were dropped are made inside the granted one.
pub fn links_for(request: &UploadRequest) -> Response {
    if !valid_id(&request.folder) {
        return error(400, "not a folder id");
    }
    if request.items.len() > MAX_FILES {
        return error(400, &format!("at most {MAX_FILES} files at once"));
    }
    let mut folders = Folders::default();
    let mut answers = Vec::with_capacity(request.items.len());
    for item in &request.items {
        match folders.link(&request.folder, item) {
            Ok((url, name)) => answers.push(json!({ "url": url, "name": name })),
            Err(Refusal::ReadOnly) => {
                return json(403, json!({ "error": "the folder is given for reading only", "code": "read_only" }));
            }
            Err(Refusal::File(code, why)) => answers.push(json!({ "error": why, "code": code })),
            Err(Refusal::Failed(why)) => return error(500, &why),
        }
    }
    ok(json!({ "items": answers }))
}

/// The folders this request uploads into: their ids by path, the names taken
/// in each.
#[derive(Default)]
struct Folders {
    ids: HashMap<String, String>,
    taken: HashMap<String, HashSet<String>>,
}

impl Folders {
    fn link(&mut self, root: &str, item: &UploadItem) -> Result<(String, String), Refusal> {
        let folder = self.folder(root, &item.dir)?;
        let taken = self.taken(&folder)?;
        let name = free_name(&item.name, |name| taken.contains(name));
        match links::upload(&folder, &name, false) {
            Ok(url) => {
                taken.insert(name.clone());
                Ok((url, name))
            }
            Err(links::Error::NoSession) => Err(Refusal::Failed("uploads need a browser session".into())),
            Err(links::Error::NotFound) => Err(Refusal::File("not_found", "the folder is gone".into())),
            // The server says why in words only.
            Err(links::Error::Other(why)) if why.contains("only read") => Err(Refusal::ReadOnly),
            Err(links::Error::Other(why)) => Err(Refusal::File("refused", why)),
        }
    }

    /// The id of `dir` inside `root`, made if it isn't there.
    fn folder(&mut self, root: &str, dir: &str) -> Result<String, Refusal> {
        let mut id = root.to_owned();
        let mut path = String::new();
        for part in dir.split('/').filter(|part| !part.is_empty()) {
            path = if path.is_empty() { part.to_owned() } else { format!("{path}/{part}") };
            if let Some(known) = self.ids.get(&path) {
                id = known.clone();
                continue;
            }
            let existing = files::children(&id).map_err(refusal)?.into_iter().find(|node| node.name == part);
            id = match existing {
                Some(node) if node.is_dir => node.id,
                Some(_) => return Err(Refusal::File("conflict", format!("“{part}” is a file"))),
                None => {
                    let made = files::create_folder(&id, part).map_err(refusal)?.id;
                    self.taken.entry(id.clone()).or_default().insert(part.to_owned());
                    made
                }
            };
            self.ids.insert(path.clone(), id.clone());
        }
        Ok(id)
    }

    fn taken(&mut self, folder: &str) -> Result<&mut HashSet<String>, Refusal> {
        if !self.taken.contains_key(folder) {
            let names = files::children(folder).map_err(refusal)?.into_iter().map(|node| node.name).collect();
            self.taken.insert(folder.to_owned(), names);
        }
        Ok(self.taken.get_mut(folder).expect("just put"))
    }
}

fn refusal(err: files::Error) -> Refusal {
    match err {
        files::Error::ReadOnly => Refusal::ReadOnly,
        files::Error::InvalidName => Refusal::File("invalid_name", "the name can't be used".into()),
        files::Error::NotFound => Refusal::File("not_found", "the folder is gone".into()),
        err => Refusal::Failed(format!("{err:?}")),
    }
}

/// `name`, or `name (1).ext`, `name (2).ext`… — the first one not taken.
pub fn free_name(name: &str, taken: impl Fn(&str) -> bool) -> String {
    if !taken(name) {
        return name.to_owned();
    }
    let (stem, ext) = match name.rfind('.') {
        Some(dot) if dot > 0 => name.split_at(dot),
        _ => (name, ""),
    };
    (1..).map(|n| format!("{stem} ({n}){ext}")).find(|candidate| !taken(candidate)).expect("some number is free")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_names_get_a_number_before_the_extension() {
        let taken = ["IMG_1.jpg", "IMG_1 (1).jpg", "notes", ".hidden"];
        let is_taken = |name: &str| taken.contains(&name);
        assert_eq!(free_name("IMG_2.jpg", is_taken), "IMG_2.jpg");
        assert_eq!(free_name("IMG_1.jpg", is_taken), "IMG_1 (2).jpg");
        assert_eq!(free_name("notes", is_taken), "notes (1)");
        assert_eq!(free_name(".hidden", is_taken), ".hidden (1)");
    }
}
