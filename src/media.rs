//! Background jobs: describing photos and videos (`media`, one job per file,
//! added by the server on file events) and walking folders again for
//! "Rebuild" (`scan`, one job per folder).
//!
//! Describing a file: its metadata and a thumbnail from the server's ffmpeg;
//! for formats browsers don't show (HEIC, RAW, TIFF) also a large preview.
//! Pictures go to AppData (`thumbs/<id>.webp`, `previews/<id>.jpg`), the
//! description to the [index](crate::index).

use std::collections::HashMap;

use crate::exif;
use crate::faces;
use crate::index::{Entry, Shards, Store};
use crate::rustcloud::app::jobs::{self as queue, Job, NewJob, Outcome};
use crate::rustcloud::app::use_ffmpeg::{self, Fit, ImageFormat, Metadata, PictureOptions, Source};
use crate::rustcloud::app::{appdata, files};

/// Grid tiles: rows are about 240 px high on a 2× screen.
const THUMB: PictureOptions = PictureOptions {
    max_width: 720,
    max_height: 480,
    fit: Fit::Inside,
    format: ImageFormat::Webp,
    quality: 75,
    at_ms: None,
};

/// What the viewer shows for a photo the browser can't show itself.
const PREVIEW: PictureOptions = PictureOptions {
    max_width: 2560,
    max_height: 2560,
    fit: Fit::Inside,
    format: ImageFormat::Jpeg,
    quality: 85,
    at_ms: None,
};

pub fn thumb_path(id: &str) -> String {
    format!("thumbs/{id}.webp")
}

pub fn preview_path(id: &str) -> String {
    format!("previews/{id}.jpg")
}

/// Why a job didn't get done.
#[derive(Clone, Debug, PartialEq)]
pub enum Problem {
    /// Try again later: ffmpeg busy, a server error.
    Later(String),
    /// It won't work: a broken file, a format ffmpeg doesn't read.
    Broken(String),
}

pub fn outcome(result: Result<(), Problem>) -> Outcome {
    match result {
        Ok(()) => Outcome::Done,
        Err(Problem::Later(why)) => Outcome::Retry(why),
        Err(Problem::Broken(why)) => Outcome::Failed(why),
    }
}

pub fn later(err: impl std::fmt::Debug) -> Problem {
    Problem::Later(format!("{err:?}"))
}

/// The user's AppData of the plugin, as the index's store.
pub struct AppData;

impl Store for AppData {
    type Error = appdata::Error;

    fn read(&self, path: &str) -> Result<Option<Vec<u8>>, appdata::Error> {
        match appdata::read(path) {
            Ok(data) => Ok(Some(data)),
            Err(appdata::Error::NotFound) => Ok(None),
            Err(err) => Err(err),
        }
    }

    fn write(&self, path: &str, data: &[u8]) -> Result<(), appdata::Error> {
        appdata::write(path, data).map(drop)
    }

    fn delete(&self, path: &str) -> Result<(), appdata::Error> {
        match appdata::delete(path) {
            Ok(()) | Err(appdata::Error::NotFound) => Ok(()),
            Err(err) => Err(err),
        }
    }
}

/// Runs a batch of `media` jobs: the keys are file ids.
pub fn run_media(jobs: &[Job]) -> Vec<Outcome> {
    let granted = match files::granted() {
        Ok(granted) => granted,
        Err(err) => return jobs.iter().map(|_| Outcome::Retry(format!("{err:?}"))).collect(),
    };
    let mut places = Places::new(granted.iter().filter(|n| n.is_dir).map(|n| (n.id.clone(), n.name.clone())));
    let mut shards: Shards<AppData> = Shards::new(AppData);
    let results: Vec<_> = jobs.iter().map(|job| describe(&job.key, &mut shards, &mut places)).collect();
    // Nothing counts as done until the index is written.
    if let Err(err) = shards.save() {
        return jobs.iter().map(|_| Outcome::Retry(format!("index: {err:?}"))).collect();
    }
    // Their faces and analysis follow: found, found again, or forgotten.
    let ids: Vec<String> = jobs.iter().map(|job| job.key.clone()).collect();
    faces::index_changed(&ids);
    crate::analysis::index_changed(&ids);
    results.into_iter().map(outcome).collect()
}

/// Brings the index up to date for one file: describes it, notes a move, or
/// forgets it if it is gone.
fn describe(id: &str, shards: &mut Shards<AppData>, places: &mut Places) -> Result<(), Problem> {
    let node = match files::get(id) {
        Ok(node) if is_media(&node) => node,
        // Deleted, moved out of the granted folders, or no longer a picture.
        Ok(_) | Err(files::Error::NotFound) => return forget(id, shards),
        Err(err) => return Err(later(err)),
    };
    let Some(place) = places.of(node.parent_id.as_deref(), lookup)? else {
        return forget(id, shards);
    };
    let etag = node.etag.clone().unwrap_or_default();
    let date = file_date(&node);
    if let Some(known) = shards.get(id).map_err(later)? {
        if known.etag == etag && known.error.is_none() {
            // Same content: at most renamed or moved.
            let moved = Entry {
                name: node.name.clone(),
                folder: place.path.clone(),
                root: place.root.clone(),
                modified: date.0,
                dated: date.1,
                ..known.clone()
            };
            return shards.put(moved).map_err(later);
        }
    }

    let source = Source::File(id.to_owned());
    let metadata = match use_ffmpeg::get_metadata(&source) {
        Ok(metadata) => metadata,
        Err(use_ffmpeg::Error::NotFound) => return forget(id, shards),
        Err(err) => {
            let problem = ffmpeg_problem(err);
            if let Problem::Broken(why) = &problem {
                // Still in the gallery, as a tile without a picture.
                let mut entry = entry(&node, &place, &no_metadata(), date.clone());
                entry.error = Some(why.clone());
                shards.put(entry).map_err(later)?;
            }
            return Err(problem);
        }
    };

    let mut entry = entry(&node, &place, &metadata, date);
    let mut broken = None;
    match picture(&source, THUMB, &thumb_path(id)) {
        Ok(()) => entry.thumb = true,
        Err(Problem::Broken(why)) => broken = Some(why),
        Err(later) => return Err(later),
    }
    if !entry.video && !shown_by_browsers(&entry.mime) && entry.thumb {
        match picture(&source, PREVIEW, &preview_path(id)) {
            Ok(()) => entry.preview = true,
            Err(Problem::Broken(why)) => broken = Some(why),
            Err(later) => return Err(later),
        }
    }
    entry.error = broken.clone();
    shards.put(entry).map_err(later)?;
    broken.map_or(Ok(()), |why| Err(Problem::Broken(why)))
}

/// Makes a picture of the file and keeps it in AppData.
fn picture(source: &Source, options: PictureOptions, path: &str) -> Result<(), Problem> {
    let bytes = use_ffmpeg::thumbnail(source, options).map_err(ffmpeg_problem)?;
    appdata::write(path, &bytes).map_err(later)?;
    Ok(())
}

fn ffmpeg_problem(err: use_ffmpeg::Error) -> Problem {
    use use_ffmpeg::Error::*;
    match err {
        // The administrator may approve use.ffmpeg later; the user may log in again.
        Busy | Timeout | NotAllowed | NoUser | NotFound => Problem::Later(format!("{err:?}")),
        Unsupported(why) | InvalidOptions(why) | Failed(why) => Problem::Broken(why),
    }
}

/// Takes a file out of the gallery, with its pictures.
fn forget(id: &str, shards: &mut Shards<AppData>) -> Result<(), Problem> {
    if shards.remove(id).map_err(later)?.is_some() {
        AppData.delete(&thumb_path(id)).map_err(later)?;
        AppData.delete(&preview_path(id)).map_err(later)?;
    }
    Ok(())
}

/// Runs a batch of `scan` jobs: each walks one folder, adding `scan` jobs
/// for its folders and `media` jobs for its photos and videos.
pub fn run_scan(jobs: &[Job]) -> Vec<Outcome> {
    jobs.iter().map(|job| outcome(scan(&job.key))).collect()
}

fn scan(folder: &str) -> Result<(), Problem> {
    let children = match files::children(folder) {
        Ok(children) => children,
        Err(files::Error::NotFound) => return Ok(()),
        Err(err) => return Err(later(err)),
    };
    let found: Vec<NewJob> = children
        .iter()
        .filter(|child| !child.name.starts_with('.'))
        .filter_map(|child| {
            let kind = if child.is_dir {
                "scan"
            } else if is_media(child) {
                "media"
            } else {
                return None;
            };
            Some(NewJob { kind: kind.into(), key: child.id.clone(), data: "rescan".into() })
        })
        .collect();
    for chunk in found.chunks(1000) {
        queue::add(chunk).map_err(later)?;
    }
    Ok(())
}

/// Runs `prune` jobs: drops the files of folders the user no longer gives
/// the plugin, with their pictures.
pub fn run_prune(jobs: &[Job]) -> Vec<Outcome> {
    let result = prune();
    jobs.iter().map(|_| outcome(result.clone())).collect()
}

fn prune() -> Result<(), Problem> {
    let granted = files::granted().map_err(later)?;
    let roots: std::collections::HashSet<String> = granted.into_iter().filter(|n| n.is_dir).map(|n| n.id).collect();
    let mut shards: Shards<AppData> = Shards::new(AppData);
    let removed = shards.remove_where(|entry| !roots.contains(&entry.root)).map_err(later)?;
    shards.save().map_err(later)?;
    let ids: Vec<String> = removed.iter().map(|entry| entry.id.clone()).collect();
    faces::index_changed(&ids);
    crate::analysis::index_changed(&ids);
    for entry in removed {
        AppData.delete(&thumb_path(&entry.id)).map_err(later)?;
        AppData.delete(&preview_path(&entry.id)).map_err(later)?;
    }
    Ok(())
}

/// Starts walking the granted folders again (`scan` jobs).
pub fn rescan(folders: &[String]) -> Result<(), queue::Error> {
    let jobs: Vec<NewJob> =
        folders.iter().map(|id| NewJob { kind: "scan".into(), key: id.clone(), data: String::new() }).collect();
    jobs.chunks(1000).try_for_each(queue::add)
}

/// Marks an index whose old entries were sent to be dated again (0.8.1).
const DATES_MARK: &str = "dates.json";

/// Once per user: entries made before 0.8.1 date photos without a shot time
/// by their upload day. Their `media` jobs date them again — same content:
/// no ffmpeg, only the file's times. Entries made since are dated already,
/// so a fresh index sends none. Marked first, so pages opened together send
/// them once; if sending fails, "Rebuild" is there.
pub fn refresh_dates_once() {
    if !matches!(appdata::read(DATES_MARK), Err(appdata::Error::NotFound)) {
        return;
    }
    let mut old = Vec::new();
    let mut shards: Shards<AppData> = Shards::new(AppData);
    let read = shards.update_all(|entry| {
        if !entry.dated {
            old.push(NewJob { kind: "media".into(), key: entry.id.clone(), data: "dates".into() });
        }
        false
    });
    if read.is_err() || appdata::write(DATES_MARK, br#"{"version":1}"#).is_err() {
        return;
    }
    if let Err(err) = old.chunks(1000).try_for_each(queue::add) {
        crate::rustcloud::app::log::warn(&format!("dates of the index not refreshed: {err:?}"));
    }
}

/// A photo or video the gallery shows. SVG is a picture too, but ffmpeg
/// doesn't draw it.
pub fn is_media(node: &files::Node) -> bool {
    let mime = node.mime_type.as_deref().unwrap_or("");
    !node.is_dir
        && !node.name.starts_with('.')
        && (mime.starts_with("image/") || mime.starts_with("video/"))
        && mime != "image/svg+xml"
}

/// Pictures every browser shows as they are.
pub fn shown_by_browsers(mime: &str) -> bool {
    matches!(mime, "image/jpeg" | "image/png" | "image/webp" | "image/gif" | "image/avif" | "image/bmp")
}

/// The index entry of a file, from what ffmpeg says about it.
/// `date`: the file's own date, and whether it came from the client's
/// times ([`file_date`]).
pub fn entry(node: &files::Node, place: &Place, info: &Metadata, date: (String, bool)) -> Entry {
    let exposure = exif::exposure(&info.tags);
    let mime = node.mime_type.clone().filter(|m| !m.is_empty()).unwrap_or_else(|| info.mime_type.clone());
    let video = mime.starts_with("video/") || (!info.still && !info.video.is_empty());
    Entry {
        id: node.id.clone(),
        name: node.name.clone(),
        folder: place.path.clone(),
        root: place.root.clone(),
        etag: node.etag.clone().unwrap_or_default(),
        size: node.size,
        mime,
        video,
        taken: info.taken_at.clone(),
        modified: date.0,
        dated: date.1,
        width: info.width.unwrap_or(0),
        height: info.height.unwrap_or(0),
        camera: camera(info.camera_make.as_deref(), info.camera_model.as_deref()),
        lens: info.lens.clone().filter(|lens| !lens.is_empty()),
        iso: exposure.iso,
        aperture: exposure.aperture,
        exposure: exposure.exposure,
        focal: exposure.focal,
        gps: info.location.as_ref().map(|at| (at.latitude, at.longitude)),
        duration: if video { info.duration_ms } else { None },
        codec: if video { info.video.first().map(|v| v.codec.clone()) } else { None },
        container: info.container.clone(),
        thumb: false,
        preview: false,
        error: None,
    }
}

/// Metadata of a file ffmpeg couldn't read: only what the file node says.
pub fn no_metadata() -> Metadata {
    Metadata {
        container: String::new(),
        mime_type: String::new(),
        still: true,
        duration_ms: None,
        width: None,
        height: None,
        video: Vec::new(),
        audio: Vec::new(),
        subtitles: Vec::new(),
        taken_at: None,
        location: None,
        camera_make: None,
        camera_model: None,
        lens: None,
        tags: Vec::new(),
    }
}

/// `Apple iPhone 15 Pro`, but `Canon EOS R5` rather than `Canon Canon EOS R5`.
fn camera(make: Option<&str>, model: Option<&str>) -> Option<String> {
    let (make, model) = (make.unwrap_or("").trim(), model.unwrap_or("").trim());
    let name = match (make.is_empty(), model.is_empty()) {
        (_, true) => make.to_owned(),
        (true, false) => model.to_owned(),
        _ if model.to_lowercase().starts_with(&make.to_lowercase()) => model.to_owned(),
        _ => format!("{make} {model}"),
    };
    (!name.is_empty()).then_some(name)
}

/// The date of a file when its photo says none: the earlier of the times its
/// client gave (since API 0.2.21): when it was made on the device (`ctime`) or
/// last changed there (`mtime`). A copy made on Windows is born when copied
/// but keeps its mtime; a client that sent no `ctime` leaves the upload time
/// as the birth. `updated-at` is the server's last change — the upload day —
/// and stays only if the times can't be had (then `false`: dated again on
/// the next pass).
fn file_date(node: &files::Node) -> (String, bool) {
    match files::get_times(&node.id) {
        Ok(times) => (earliest(&times.modified_at, &times.born_at), true),
        Err(_) => (node.updated_at.clone(), false),
    }
}

/// The earlier of two RFC 3339 times from the server, both in UTC: compared
/// to the second, then as text (`…:00+00:00` before `…:00.5+00:00`).
fn earliest(a: &str, b: &str) -> String {
    let key = |t: &str| (t.get(..19).unwrap_or(t).to_owned(), t.to_owned());
    if key(b) < key(a) { b } else { a }.to_owned()
}

/// Where a file is among the granted folders.
#[derive(Clone, Debug, PartialEq)]
pub struct Place {
    /// The granted folder it is in.
    pub root: String,
    /// Folder names from the granted one down: `Photos/2024/Trip`.
    pub path: String,
}

/// A folder's name and parent, or none if it is gone.
type Folder = Option<(String, Option<String>)>;

fn lookup(id: &str) -> Result<Folder, Problem> {
    match files::get(id) {
        Ok(node) => Ok(Some((node.name, node.parent_id))),
        Err(files::Error::NotFound) => Ok(None),
        Err(err) => Err(later(err)),
    }
}

/// Finds the [`Place`] of folders, remembering them for the batch.
pub struct Places {
    roots: HashMap<String, String>,
    known: HashMap<String, Option<Place>>,
}

impl Places {
    /// `roots`: the granted folders, id → name.
    pub fn new(roots: impl IntoIterator<Item = (String, String)>) -> Self {
        Self { roots: roots.into_iter().collect(), known: HashMap::new() }
    }

    /// The place of a folder, or none if it is in no granted folder.
    pub fn of(
        &mut self,
        folder: Option<&str>,
        lookup: impl Fn(&str) -> Result<Folder, Problem>,
    ) -> Result<Option<Place>, Problem> {
        let Some(folder) = folder else { return Ok(None) };
        if let Some(place) = self.known.get(folder) {
            return Ok(place.clone());
        }
        let mut names = Vec::new();
        let mut current = folder.to_owned();
        let place = loop {
            if let Some(name) = self.roots.get(&current) {
                names.push(name.clone());
                names.reverse();
                break Some(Place { root: current, path: names.join("/") });
            }
            match lookup(&current)? {
                Some((name, Some(parent))) if names.len() < 256 => {
                    names.push(name);
                    current = parent;
                }
                _ => break None,
            }
        };
        self.known.insert(folder.to_owned(), place.clone());
        Ok(place)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rustcloud::app::use_ffmpeg::{Location, VideoStream};
    use std::cell::Cell;

    fn node(name: &str, mime: Option<&str>) -> files::Node {
        files::Node {
            id: "f1".into(),
            parent_id: Some("d1".into()),
            name: name.into(),
            is_dir: false,
            size: 1000,
            mime_type: mime.map(Into::into),
            etag: Some("e1".into()),
            updated_at: "2026-10-01T10:00:00Z".into(),
        }
    }

    #[test]
    fn what_counts_as_media() {
        assert!(is_media(&node("a.jpg", Some("image/jpeg"))));
        assert!(is_media(&node("a.heic", Some("image/heic"))));
        assert!(is_media(&node("a.cr2", Some("image/x-canon-cr2"))));
        assert!(is_media(&node("a.mov", Some("video/quicktime"))));
        assert!(!is_media(&node("a.svg", Some("image/svg+xml"))));
        assert!(!is_media(&node(".a.jpg", Some("image/jpeg"))));
        assert!(!is_media(&node("a.txt", Some("text/plain"))));
        assert!(!is_media(&node("a", None)));
        let mut dir = node("Photos", None);
        dir.is_dir = true;
        assert!(!is_media(&dir));
    }

    #[test]
    fn a_file_dates_from_the_earlier_client_time() {
        // A Windows copy: born when copied, changed when shot.
        assert_eq!(earliest("2024-03-01T09:00:00+00:00", "2026-11-25T06:00:00+00:00"), "2024-03-01T09:00:00+00:00");
        assert_eq!(earliest("2026-11-25T06:00:00+00:00", "2024-03-01T09:00:00+00:00"), "2024-03-01T09:00:00+00:00");
        // Within a second: the whole second first.
        assert_eq!(earliest("2024-03-01T09:00:00.500+00:00", "2024-03-01T09:00:00+00:00"), "2024-03-01T09:00:00+00:00");
        assert_eq!(earliest("2024-03-01T09:00:00Z", "2024-03-01T09:00:00Z"), "2024-03-01T09:00:00Z");
    }

    #[test]
    fn cameras_are_named_once() {
        assert_eq!(camera(Some("Apple"), Some("iPhone 15 Pro")).unwrap(), "Apple iPhone 15 Pro");
        assert_eq!(camera(Some("Canon"), Some("Canon EOS R5")).unwrap(), "Canon EOS R5");
        assert_eq!(camera(Some("SONY "), None).unwrap(), "SONY");
        assert_eq!(camera(None, Some("X100V")).unwrap(), "X100V");
        assert_eq!(camera(Some(" "), None), None);
    }

    #[test]
    fn an_entry_from_metadata() {
        let place = Place { root: "r".into(), path: "Photos/2024".into() };
        let info = Metadata {
            container: "heif".into(),
            mime_type: "image/heif".into(),
            still: true,
            width: Some(3024),
            height: Some(4032),
            taken_at: Some("2024-07-01T12:30:00+03:00".into()),
            location: Some(Location { latitude: 55.75, longitude: 37.61, altitude: None }),
            camera_make: Some("Apple".into()),
            camera_model: Some("iPhone 15 Pro".into()),
            tags: vec![("exif.FNumber".into(), "178/100 (1.7800)".into())],
            ..no_metadata()
        };
        let entry =
            entry(&node("IMG_1.HEIC", Some("image/heic")), &place, &info, ("2026-10-01T10:00:00+00:00".into(), true));
        assert_eq!(entry.mime, "image/heic");
        assert!(!entry.video);
        assert_eq!((entry.width, entry.height), (3024, 4032));
        assert_eq!(entry.folder, "Photos/2024");
        assert_eq!(entry.camera.as_deref(), Some("Apple iPhone 15 Pro"));
        assert_eq!(entry.aperture, Some(1.8));
        assert_eq!(entry.gps, Some((55.75, 37.61)));
        assert_eq!(entry.duration, None);
        assert!(!shown_by_browsers(&entry.mime));
    }

    #[test]
    fn a_video_entry() {
        let place = Place { root: "r".into(), path: "Photos".into() };
        let info = Metadata {
            container: "mov".into(),
            still: false,
            duration_ms: Some(12_500),
            video: vec![VideoStream {
                codec: "hevc".into(),
                width: 1920,
                height: 1080,
                fps: 30.0,
                rotation: 0,
                bit_depth: 8,
                hdr: false,
                bitrate: 0,
            }],
            ..no_metadata()
        };
        let entry = entry(
            &node("clip.mov", Some("video/quicktime")),
            &place,
            &info,
            ("2026-10-01T10:00:00+00:00".into(), true),
        );
        assert!(entry.video);
        assert_eq!(entry.duration, Some(12_500));
        assert_eq!(entry.codec.as_deref(), Some("hevc"));
    }

    #[test]
    fn places_walk_up_to_a_granted_folder() {
        // root ← 2024 ← Trip
        let tree = |id: &str| -> Result<Folder, Problem> {
            Ok(match id {
                "trip" => Some(("Trip".into(), Some("y2024".into()))),
                "y2024" => Some(("2024".into(), Some("root".into()))),
                "outside" => Some(("Other".into(), Some("home".into()))),
                "home" => Some(("Home".into(), None)),
                _ => None,
            })
        };
        let mut places = Places::new([("root".to_owned(), "Photos".to_owned())]);
        let trip = places.of(Some("trip"), tree).unwrap().unwrap();
        assert_eq!(trip, Place { root: "root".into(), path: "Photos/2024/Trip".into() });
        assert_eq!(places.of(Some("root"), tree).unwrap().unwrap().path, "Photos");
        assert_eq!(places.of(Some("outside"), tree).unwrap(), None);
        assert_eq!(places.of(None, tree).unwrap(), None);

        // Known folders are not looked up again.
        let calls = Cell::new(0);
        let counted = |id: &str| {
            calls.set(calls.get() + 1);
            tree(id)
        };
        places.of(Some("trip"), counted).unwrap();
        assert_eq!(calls.get(), 0);
    }
}
