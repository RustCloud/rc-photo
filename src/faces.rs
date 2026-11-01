//! Faces: the `faces` job finds the faces in each photo and makes a vector
//! of each one, through the server's `use-onnx`; [`crate::people`] groups
//! the vectors into people. Off until the user turns it on: faces are
//! biometric data, and they stay in the user's AppData.
//!
//! A photo costs the server one decoding: the picture comes as a tensor at
//! most [`WORK_SIDE`] px long, and the detector's input and every face are
//! made from it here ([`crate::vision`]).
//!
//! The faces of each photo are kept like the index, `faces/<shard>.json`,
//! written by jobs only (one batch at a time).

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::index::{Entry, Item, Shards, Store};
use crate::media::{later, outcome, AppData, Problem};
use crate::rustcloud::app::jobs::{self as queue, Job, NewJob, Outcome};
use crate::rustcloud::app::use_onnx::{
    self, Channels, ImageFit, ImageOptions, Layout, NamedTensor, Source, Tensor, TensorType,
};
use crate::rustcloud::app::{appdata, storage};
use crate::vision::{self, Picture, FACE_SIDE};
use crate::web::{error, ok, valid_id};
use crate::Response;

/// YOLO11n trained for faces, with 5 points per face.
pub const DETECTOR: &str = "models/face-yolo11n-5kp.onnx";
/// ArcFace w600k_r50 (insightface's buffalo_l): a 512-value vector per face.
pub const RECOGNIZER: &str = "models/arcface-w600k-r50.onnx";

/// The longest side of the picture faces are looked for in.
const WORK_SIDE: u32 = 1600;
const DETECTOR_SIDE: usize = 640;
const MIN_SCORE: f32 = 0.5;
/// Narrower faces (px of the work picture) are too blurred to tell people
/// apart. Small ones only join people ([`crate::people`]), never start one.
const MIN_FACE: f32 = 20.0;
const MAX_FACES: usize = 40;
/// Faces in one run of ArcFace.
const FACES_AT_ONCE: usize = 16;

const SETTINGS: &str = "faces.json";
/// Faces found since the people were last grouped.
const PENDING: &str = "faces-pending.txt";

/// Whether the user turned faces on; written by the page's requests.
#[derive(Default, Serialize, Deserialize)]
pub struct Settings {
    pub enabled: bool,
    /// Jobs were added for the whole library. Not yet while the models are
    /// downloading: the next look at the page adds them.
    #[serde(default)]
    pub started: bool,
}

impl Settings {
    pub fn read() -> Self {
        appdata::read(SETTINGS).ok().and_then(|data| serde_json::from_slice(&data).ok()).unwrap_or_default()
    }

    fn write(&self) -> Result<(), appdata::Error> {
        appdata::write(SETTINGS, &serde_json::to_vec(self).expect("settings serialize")).map(drop)
    }
}

/// The faces of one photo.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FaceFile {
    pub id: String,
    /// The file's etag when its faces were found.
    pub etag: String,
    pub faces: Vec<Face>,
}

impl Item for FaceFile {
    const DIR: &'static str = "faces";
    fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Face {
    /// Left, top, width, height: parts (0–1) of the picture as shown.
    pub b: [f32; 4],
    /// How sure the detector is.
    pub s: f32,
    /// Its width in pixels of the photo: how sharp it can be.
    pub w: u32,
    /// The person, once grouped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p: Option<String>,
    /// The vector, quantized ([`vision::quantize`]), base64.
    pub v: String,
}

impl Face {
    pub fn vector(&self) -> Vec<u8> {
        base64::engine::general_purpose::STANDARD.decode(&self.v).unwrap_or_default()
    }
}

/// Why faces can't be looked for now, if they can't.
pub fn readiness() -> Result<(), &'static str> {
    if !use_onnx::get_capabilities().available {
        return Err("no-engine");
    }
    for model in [DETECTOR, RECOGNIZER] {
        match storage::stat(model) {
            Ok(_) => {}
            Err(storage::Error::NotFound) => return Err("downloading"),
            Err(_) => return Err("failed"),
        }
    }
    match use_onnx::get_model_info(DETECTOR) {
        Ok(_) => Ok(()),
        Err(use_onnx::Error::NotAllowed) => Err("not-allowed"),
        Err(use_onnx::Error::Busy) => Ok(()),
        Err(_) => Err("failed"),
    }
}

// ── The job ────────────────────────────────────────────────────────────

/// Runs a batch of `faces` jobs: the keys are file ids.
pub fn run_faces(jobs: &[Job]) -> Vec<Outcome> {
    let done = || jobs.iter().map(|_| Outcome::Done).collect();
    let mut settings = Settings::read();
    if !settings.enabled {
        return done();
    }
    let models = match Models::new() {
        Ok(models) => models,
        Err(_) => {
            // Not allowed or not downloaded yet: the page starts them all again once it is.
            if settings.started {
                settings.started = false;
                let _ = settings.write();
            }
            return done();
        }
    };
    let mut index: Shards<AppData> = Shards::new(AppData);
    let mut faces: Shards<AppData, FaceFile> = Shards::new(AppData);
    let mut changed = 0;
    let results: Vec<_> = jobs
        .iter()
        .map(|job| {
            let result = look(&job.key, &models, &mut index, &mut faces);
            changed += matches!(result, Ok(true)) as u64;
            result.map(drop)
        })
        .collect();
    // Nothing counts as done until the faces are written.
    if let Err(err) = faces.save() {
        return jobs.iter().map(|_| Outcome::Retry(format!("faces: {err:?}"))).collect();
    }
    if changed > 0 {
        let _ = appdata::write(PENDING, (pending() + changed).to_string().as_bytes());
        let _ = queue::add(&[NewJob { kind: "people".into(), key: "all".into(), data: String::new() }]);
    }
    results.into_iter().map(outcome).collect()
}

/// Faces found since the people were last grouped.
pub fn pending() -> u64 {
    appdata::read(PENDING).ok().and_then(|data| String::from_utf8(data).ok()?.trim().parse().ok()).unwrap_or(0)
}

pub fn reset_pending() {
    let _ = AppData.delete(PENDING);
}

/// The models' input names.
struct Models {
    detector: String,
    recognizer: String,
}

impl Models {
    fn new() -> Result<Self, Problem> {
        readiness().map_err(|why| Problem::Later(why.into()))?;
        let input = |model: &str| -> Result<String, Problem> {
            let info = use_onnx::get_model_info(model).map_err(onnx_problem)?;
            info.inputs
                .into_iter()
                .next()
                .map(|input| input.name)
                .ok_or_else(|| Problem::Later(format!("{model} takes no input")))
        };
        Ok(Self { detector: input(DETECTOR)?, recognizer: input(RECOGNIZER)? })
    }
}

/// Brings one photo's faces up to date; whether they changed.
fn look(
    id: &str,
    models: &Models,
    index: &mut Shards<AppData>,
    faces: &mut Shards<AppData, FaceFile>,
) -> Result<bool, Problem> {
    let entry = index.get(id).map_err(later)?.cloned();
    // Gone, a video, or a picture ffmpeg can't read.
    let Some(entry) = entry.filter(|entry| !entry.video && entry.thumb) else {
        return Ok(faces.remove(id).map_err(later)?.is_some());
    };
    if faces.get(id).map_err(later)?.is_some_and(|known| known.etag == entry.etag) {
        return Ok(false);
    }
    let found = match detect(id, &entry, models) {
        Ok(found) => found,
        Err(Problem::Broken(why)) => {
            // Not tried again until the file changes.
            faces.put(FaceFile { id: id.into(), etag: entry.etag.clone(), faces: Vec::new() }).map_err(later)?;
            return Err(Problem::Broken(why));
        }
        Err(problem) => return Err(problem),
    };
    faces.put(FaceFile { id: id.into(), etag: entry.etag, faces: found }).map_err(later)?;
    Ok(true)
}

/// The faces of a photo, each with its vector.
fn detect(id: &str, entry: &Entry, models: &Models) -> Result<Vec<Face>, Problem> {
    let (picture, shown) = work_picture(id, entry)?;
    // Work picture pixels per pixel of the photo as shown.
    let scale = picture.width as f32 / shown.0 as f32;
    let (input, letterbox_scale) = vision::letterbox(&picture, DETECTOR_SIDE);
    let side = DETECTOR_SIDE as u32;
    let output = run(DETECTOR, &models.detector, tensor(vec![1, 3, side, side], &input))?;
    let mut found = vision::decode_yolo(&floats(&output.tensor), &output.tensor.shape, letterbox_scale, MIN_SCORE);
    found.retain(|face| face.width() >= MIN_FACE);
    found.truncate(MAX_FACES);

    let mut faces = Vec::with_capacity(found.len());
    for part in found.chunks(FACES_AT_ONCE) {
        let mut input = Vec::with_capacity(part.len() * 3 * FACE_SIDE * FACE_SIDE);
        for face in part {
            vision::aligned_face(&picture, &face.points, &mut input);
        }
        let side = FACE_SIDE as u32;
        let output = run(RECOGNIZER, &models.recognizer, tensor(vec![part.len() as u32, 3, side, side], &input))?;
        let values = floats(&output.tensor);
        let size = values.len() / part.len().max(1);
        if size == 0 {
            return Err(Problem::Broken("ArcFace gave no vector".into()));
        }
        for (face, vector) in part.iter().zip(values.chunks(size)) {
            let mut vector = vector.to_vec();
            vision::normalize(&mut vector);
            let [x1, y1, x2, y2] = face.bbox;
            let (w, h) = (picture.width as f32, picture.height as f32);
            let part_of = |v: f32, of: f32| ((v / of).clamp(0.0, 1.0) * 10_000.0).round() / 10_000.0;
            let (left, top) = (part_of(x1, w), part_of(y1, h));
            faces.push(Face {
                b: [left, top, part_of(x2, w) - left, part_of(y2, h) - top],
                s: (face.score * 1000.0).round() / 1000.0,
                w: (face.width() / scale).round() as u32,
                p: None,
                v: base64::engine::general_purpose::STANDARD.encode(vision::quantize(&vector)),
            });
        }
    }
    Ok(faces)
}

/// The photo as a picture at most [`WORK_SIDE`] long, and its size as shown.
/// Its size comes from the index; if the server shows it turned the other
/// way, it is asked for again at the right size.
fn work_picture(id: &str, entry: &Entry) -> Result<(Picture, (u32, u32)), Problem> {
    let mut known = (entry.width > 0 && entry.height > 0).then_some((entry.width, entry.height));
    for _ in 0..2 {
        let ((width, height), fit) = match known {
            Some((w, h)) => (vision::fit_inside(w, h, WORK_SIDE), ImageFit::Stretch),
            None => ((WORK_SIDE, WORK_SIDE), ImageFit::Inside),
        };
        let options = ImageOptions {
            width,
            height,
            fit,
            crop: None,
            channels: Channels::Rgb,
            layout: Layout::Nchw,
            mean: (0.0, 0.0, 0.0),
            std: (1.0, 1.0, 1.0),
        };
        let input = use_onnx::image_tensor(&Source::File(id.to_owned()), options).map_err(onnx_problem)?;
        let shown = (input.width.max(1), input.height.max(1));
        let picture = Picture::from_tensor(width as usize, height as usize, &input.tensor.data)
            .ok_or_else(|| Problem::Broken("the picture is not of the size asked for".into()))?;
        let fits = match fit {
            // Stretched: only right if the sides are in the same proportion.
            ImageFit::Stretch => {
                ((width as f32 / height as f32) - (shown.0 as f32 / shown.1 as f32)).abs()
                    < 0.02 * width as f32 / height as f32
            }
            _ => true,
        };
        if fits {
            return Ok((picture, shown));
        }
        known = Some(shown);
    }
    Err(Problem::Broken("the picture's size keeps changing".into()))
}

fn run(model: &str, input: &str, tensor: Tensor) -> Result<NamedTensor, Problem> {
    let outputs = use_onnx::run(model, &[NamedTensor { name: input.into(), tensor }]).map_err(onnx_problem)?;
    outputs.into_iter().next().ok_or_else(|| Problem::Broken(format!("{model} gave nothing")))
}

fn tensor(shape: Vec<u32>, values: &[f32]) -> Tensor {
    Tensor { dtype: TensorType::F32, shape, data: values.iter().flat_map(|v| v.to_le_bytes()).collect() }
}

fn floats(tensor: &Tensor) -> Vec<f32> {
    tensor.data.as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes(*b)).collect()
}

fn onnx_problem(err: use_onnx::Error) -> Problem {
    use use_onnx::Error::*;
    match err {
        // A file it can't read, a picture that breaks the helper.
        Unsupported(why) | InvalidInput(why) => Problem::Broken(why),
        Busy | Timeout | NoUser | NotAllowed | NotFound | Failed(_) => Problem::Later(format!("{err:?}")),
    }
}

// ── For the page ───────────────────────────────────────────────────────

/// What the page shows about faces.
pub fn status() -> serde_json::Value {
    let settings = Settings::read();
    if !settings.enabled {
        return json!({ "enabled": false });
    }
    match readiness() {
        Ok(()) => {
            if !settings.started {
                // Models downloaded, or allowed, since: now the whole library.
                let _ = start_all();
            }
            json!({ "enabled": true, "state": "ready" })
        }
        Err(why) => json!({ "enabled": true, "state": why }),
    }
}

/// Turns faces on (jobs for every photo) or off (nothing new is looked at;
/// what was found stays until [`forget_all`]).
pub fn set_enabled(on: bool) -> Response {
    let settings = Settings { enabled: on, started: false };
    if let Err(err) = settings.write() {
        return error(500, &format!("{err:?}"));
    }
    if on && readiness().is_ok() {
        if let Err(problem) = start_all() {
            return error(500, &format!("{problem:?}"));
        }
    }
    ok(status())
}

/// A `faces` job for every photo of the index.
fn start_all() -> Result<(), Problem> {
    let mut index: Shards<AppData> = Shards::new(AppData);
    let mut ids = Vec::new();
    index
        .update_all(|entry| {
            if !entry.video {
                ids.push(entry.id.clone());
            }
            false
        })
        .map_err(later)?;
    queue_for(&ids)?;
    Settings { enabled: true, started: true }.write().map_err(later)
}

/// `faces` jobs for these files, if faces are on: each brings one up to date.
pub fn queue_for(ids: &[String]) -> Result<(), Problem> {
    let jobs: Vec<NewJob> =
        ids.iter().map(|id| NewJob { kind: "faces".into(), key: id.clone(), data: String::new() }).collect();
    jobs.chunks(1000).try_for_each(queue::add).map_err(later)
}

/// After the index changed for these files.
pub fn index_changed(ids: &[String]) {
    let settings = Settings::read();
    if settings.enabled && settings.started && !ids.is_empty() {
        let _ = queue_for(ids);
    }
}

/// The faces of one photo, without their vectors: boxes and people.
pub fn of_file(id: &str) -> Response {
    if !valid_id(id) {
        return error(400, "not a file id");
    }
    let mut faces: Shards<AppData, FaceFile> = Shards::new(AppData);
    match faces.get(id) {
        Ok(Some(file)) => ok(json!({
            "faces": file.faces.iter().map(|face| json!({ "b": face.b, "w": face.w, "p": face.p })).collect::<Vec<_>>(),
        })),
        Ok(None) => ok(json!({ "faces": [] })),
        Err(err) => error(500, &format!("{err:?}")),
    }
}

/// Turns faces off and deletes all that was found: faces, people, names.
pub fn forget_all() -> Response {
    let result = Settings::default()
        .write()
        .and_then(|()| AppData.delete(FaceFile::DIR))
        .and_then(|()| AppData.delete(crate::people::PEOPLE))
        .and_then(|()| AppData.delete(crate::people::NAMES))
        .and_then(|()| AppData.delete(PENDING));
    match result {
        Ok(()) => ok(json!({ "enabled": false })),
        Err(err) => error(500, &format!("{err:?}")),
    }
}
