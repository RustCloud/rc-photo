//! Photo analysis (since 0.6.0): a vector of each photo ([`crate::clip`])
//! and the text in it ([`crate::ocr`]), for similar photos and, later,
//! stories. Two kinds of small jobs, a couple of photos a batch, so the
//! server's other work gets its turn soon.
//!
//! The administrator turns it on or off for the whole server
//! ([`Switches`]). When the user looks at the page and a part's models are
//! there but its jobs haven't been added for the current models (a first
//! look, an update of the plugin, another model, turned on again), they are
//! added for every photo of the index; a photo whose record is of the same
//! file and models is skipped at once. The `media` jobs add them for new and
//! changed files ([`index_changed`]).
//!
//! What a part found is kept like the index: `clip/<shard>.json` and
//! `ocr/<shard>.json`, each written by its own jobs only.

use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::index::{shard_path, Entry, Item, Shards, Store, SHARDS};
use crate::media::{later, AppData, Problem};
use crate::models::{self, Switches};
use crate::rustcloud::app::jobs::{self as queue, NewJob};
use crate::rustcloud::app::{appdata, storage, use_onnx};
use crate::web::{error, ok, valid_id};
use crate::Response;

const STATE: &str = "analysis.json";
/// A part whose model failed is tried again after this long: the server may
/// have been updated or given it more memory.
const RETRY_FAILED_SECONDS: u64 = 24 * 3600;
/// Most photos [`similar`] gives.
const MOST_SIMILAR: usize = 200;
/// Least similarity (cosine of the vectors, plus the bonus for shared
/// words) of a photo [`similar`] gives.
const LEAST_SIMILAR: f32 = 0.62;
/// What each word a photo shares with the asked one adds, and at most.
const WORD_BONUS: f32 = 0.04;
const MOST_WORD_BONUS: f32 = 0.12;

/// The vector of one photo.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClipFile {
    pub id: String,
    /// The file's etag when it was looked at.
    pub etag: String,
    /// The models' tag ([`models::clip_tag`]).
    pub m: String,
    /// The vector, quantized, base64; empty if the picture couldn't be read.
    pub v: String,
}

impl Item for ClipFile {
    const DIR: &'static str = "clip";
    fn id(&self) -> &str {
        &self.id
    }
}

/// The text of one photo, line by line.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OcrFile {
    pub id: String,
    pub etag: String,
    /// The models' tag ([`models::ocr_tag`]).
    pub m: String,
    /// The lines read, top to bottom; none for a photo without text.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub t: Vec<String>,
}

impl Item for OcrFile {
    const DIR: &'static str = "ocr";
    fn id(&self) -> &str {
        &self.id
    }
}

/// The two parts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Part {
    Clip,
    Ocr,
}

impl Part {
    const ALL: [Part; 2] = [Part::Clip, Part::Ocr];

    pub fn kind(self) -> &'static str {
        match self {
            Part::Clip => "clip",
            Part::Ocr => "ocr",
        }
    }

    pub fn tag(self) -> String {
        match self {
            Part::Clip => models::clip_tag(),
            Part::Ocr => models::ocr_tag(),
        }
    }

    fn files(self) -> &'static [&'static str] {
        match self {
            Part::Clip => &[models::CLIP],
            Part::Ocr => &[models::OCR_DETECTOR, models::OCR_RECOGNIZER, models::OCR_DICTIONARY],
        }
    }

    /// A small model of the part, to see whether the plugin may use models.
    fn probe(self) -> &'static str {
        match self {
            // SigLIP is large: OCR's detector answers for both.
            Part::Clip | Part::Ocr => models::OCR_DETECTOR,
        }
    }
}

/// How a part stands for the user, kept in AppData.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PartState {
    /// The tag jobs were added for, for every photo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started: Option<String>,
    /// The model failed (it can't be loaded or run here): when, and why.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failed: Option<Failure>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Failure {
    /// Seconds since 1970.
    pub at: u64,
    pub why: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    #[serde(default)]
    pub clip: PartState,
    #[serde(default)]
    pub ocr: PartState,
}

impl State {
    pub fn read() -> Self {
        appdata::read(STATE).ok().and_then(|data| serde_json::from_slice(&data).ok()).unwrap_or_default()
    }

    fn write(&self) -> Result<(), appdata::Error> {
        appdata::write(STATE, &serde_json::to_vec(self).expect("state serializes")).map(drop)
    }

    pub fn part(&self, part: Part) -> &PartState {
        match part {
            Part::Clip => &self.clip,
            Part::Ocr => &self.ocr,
        }
    }

    fn part_mut(&mut self, part: Part) -> &mut PartState {
        match part {
            Part::Clip => &mut self.clip,
            Part::Ocr => &mut self.ocr,
        }
    }
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Why a part can't run now, if it can't.
fn readiness(part: Part) -> Result<(), &'static str> {
    if !use_onnx::get_capabilities().available {
        return Err("no-engine");
    }
    for file in part.files() {
        match storage::stat(file) {
            Ok(_) => {}
            Err(storage::Error::NotFound) => return Err("downloading"),
            Err(_) => return Err("failed"),
        }
    }
    match use_onnx::get_model_info(part.probe()) {
        Ok(_) | Err(use_onnx::Error::Busy) => Ok(()),
        Err(use_onnx::Error::NotAllowed) => Err("not-allowed"),
        Err(_) => Ok(()), // The part's own jobs tell what is wrong with its models.
    }
}

/// What a batch of a part's jobs does first: whether to work at all. When
/// the administrator turned analysis off or the models aren't usable, the
/// jobs end without work and the next look at the page adds them again.
pub fn may_run(part: Part) -> bool {
    if Switches::read().vision && readiness(part).is_ok() {
        return true;
    }
    let mut state = State::read();
    if state.part(part).started.is_some() {
        state.part_mut(part).started = None;
        let _ = state.write();
    }
    false
}

/// The part's model failed: its jobs stop until [`RETRY_FAILED_SECONDS`].
pub fn model_failed(part: Part, why: &str) {
    let mut state = State::read();
    state.part_mut(part).failed = Some(Failure { at: now(), why: why.chars().take(300).collect() });
    let _ = state.write();
}

/// Whether the part's model failed lately: its jobs end at once.
pub fn failed_lately(part: Part) -> Option<String> {
    let state = State::read();
    let failure = state.part(part).failed.as_ref()?;
    (now().saturating_sub(failure.at) < RETRY_FAILED_SECONDS).then(|| failure.why.clone())
}

/// For the page's state: how analysis stands, and its jobs added where due.
pub fn status() -> serde_json::Value {
    if !Switches::read().vision {
        let state = State::read();
        if state != State::default() {
            // Turned on again later, every photo gets jobs again (done ones are skipped).
            let cleared = State {
                clip: PartState { started: None, ..state.clip },
                ocr: PartState { started: None, ..state.ocr },
            };
            let _ = cleared.write();
        }
        return json!({ "enabled": false });
    }
    let mut state = State::read();
    let mut parts = serde_json::Map::new();
    for part in Part::ALL {
        let tag = part.tag();
        let shown = match readiness(part) {
            Err(why) => why.to_owned(),
            Ok(()) => {
                let current = state.part_mut(part);
                let retry = current.failed.as_ref().is_some_and(|f| now().saturating_sub(f.at) >= RETRY_FAILED_SECONDS);
                if retry {
                    *current = PartState::default();
                }
                if current.failed.is_none() && current.started.as_deref() != Some(tag.as_str()) {
                    match start_all(part) {
                        Ok(()) => {
                            current.started = Some(tag);
                            let _ = state.write();
                        }
                        Err(problem) => return json!({ "enabled": true, "error": format!("{problem:?}") }),
                    }
                }
                match &state.part(part).failed {
                    Some(failure) => format!("failed: {}", failure.why),
                    None => "ready".to_owned(),
                }
            }
        };
        parts.insert(part.kind().to_owned(), json!(shown));
    }
    json!({ "enabled": true, "parts": parts })
}

/// Jobs of the part for every photo of the index.
fn start_all(part: Part) -> Result<(), Problem> {
    let ids = photo_ids()?;
    queue_for(part, &ids)
}

fn photo_ids() -> Result<Vec<String>, Problem> {
    let mut ids = Vec::new();
    for shard in 0..SHARDS {
        // One shard at a time: a large library's index is never held whole.
        let json = AppData.read(&shard_path(crate::index::DIR, shard)).map_err(later)?;
        let entries: Vec<Entry> = json.and_then(|json| serde_json::from_slice(&json).ok()).unwrap_or_default();
        ids.extend(entries.into_iter().filter(|entry| !entry.video).map(|entry| entry.id));
    }
    Ok(ids)
}

fn queue_for(part: Part, ids: &[String]) -> Result<(), Problem> {
    let jobs: Vec<NewJob> =
        ids.iter().map(|id| NewJob { kind: part.kind().into(), key: id.clone(), data: String::new() }).collect();
    jobs.chunks(1000).try_for_each(queue::add).map_err(later)
}

/// After the index changed for these files: their analysis follows (done
/// again, or forgotten), for the parts started.
pub fn index_changed(ids: &[String]) {
    if ids.is_empty() || !Switches::read().vision {
        return;
    }
    let state = State::read();
    for part in Part::ALL {
        if state.part(part).started.is_some() {
            let _ = queue_for(part, ids);
        }
    }
}

// ── Similar photos ─────────────────────────────────────────────────────

/// Photos like this one, most similar first: its vector compared with every
/// other, and a little more for each word of text they share.
pub fn similar(id: &str) -> Response {
    if !valid_id(id) {
        return error(400, "not a file id");
    }
    let tag = models::clip_tag();
    let mut clips: Shards<AppData, ClipFile> = Shards::new(AppData);
    let asked = match clips.get(id) {
        Ok(Some(file)) if file.m == tag && !file.v.is_empty() => decode(&file.v),
        Ok(_) => return ok(json!({ "ready": false, "ids": [] })),
        Err(err) => return error(500, &format!("{err:?}")),
    };
    let Some(asked) = unit(&asked) else {
        return ok(json!({ "ready": false, "ids": [] }));
    };
    let mut ocrs: Shards<AppData, OcrFile> = Shards::new(AppData);
    let asked_words = match ocrs.get(id) {
        Ok(Some(file)) => words(&file.t),
        _ => Vec::new(),
    };
    let mut scored: Vec<(String, f32)> = Vec::new();
    for shard in 0..SHARDS {
        let files: Vec<ClipFile> = read_shard(shard);
        for file in files.iter().filter(|file| file.id != id && file.m == tag && !file.v.is_empty()) {
            if let Some(score) = cosine(&asked, &decode(&file.v)) {
                scored.push((file.id.clone(), score));
            }
        }
    }
    if !asked_words.is_empty() {
        let mut shared = std::collections::HashMap::new();
        for shard in 0..SHARDS {
            for file in read_shard::<OcrFile>(shard) {
                let count = words(&file.t).iter().filter(|word| asked_words.contains(word)).count();
                if count > 0 {
                    shared.insert(file.id, count);
                }
            }
        }
        for (id, score) in &mut scored {
            if let Some(&count) = shared.get(id.as_str()) {
                *score += (count as f32 * WORD_BONUS).min(MOST_WORD_BONUS);
            }
        }
    }
    let best = best(scored, LEAST_SIMILAR, MOST_SIMILAR);
    ok(json!({
        "ready": true,
        "ids": best.iter().map(|(id, _)| id).collect::<Vec<_>>(),
        "scores": best.iter().map(|(_, score)| (score * 1000.0).round() / 1000.0).collect::<Vec<_>>(),
    }))
}

/// The text of a photo, if it was read.
pub fn text_of(id: &str) -> Response {
    if !valid_id(id) {
        return error(400, "not a file id");
    }
    let mut ocrs: Shards<AppData, OcrFile> = Shards::new(AppData);
    match ocrs.get(id) {
        Ok(Some(file)) if file.m == models::ocr_tag() => ok(json!({ "read": true, "lines": file.t })),
        Ok(_) => ok(json!({ "read": false, "lines": [] })),
        Err(err) => error(500, &format!("{err:?}")),
    }
}

fn read_shard<T: Item>(shard: u32) -> Vec<T> {
    match AppData.read(&shard_path(T::DIR, shard)) {
        Ok(Some(json)) => serde_json::from_slice(&json).unwrap_or_default(),
        _ => Vec::new(),
    }
}

pub fn encode(vector: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(vector)
}

fn decode(vector: &str) -> Vec<u8> {
    base64::engine::general_purpose::STANDARD.decode(vector).unwrap_or_default()
}

/// A quantized vector as unit floats; none if it is empty or all zero.
fn unit(vector: &[u8]) -> Option<Vec<f32>> {
    let values: Vec<f32> = vector.iter().map(|&b| b as i8 as f32).collect();
    let length = values.iter().map(|v| v * v).sum::<f32>().sqrt();
    (length > 0.0).then(|| values.iter().map(|v| v / length).collect())
}

/// Cosine of a unit vector and a quantized one of the same length.
fn cosine(unit_vector: &[f32], other: &[u8]) -> Option<f32> {
    if other.len() != unit_vector.len() {
        return None;
    }
    let (mut dot, mut length) = (0.0f32, 0.0f32);
    for (a, &b) in unit_vector.iter().zip(other) {
        let b = b as i8 as f32;
        dot += a * b;
        length += b * b;
    }
    (length > 0.0).then(|| dot / length.sqrt())
}

/// The words of text worth matching: lower case, letters and digits, three
/// or more, each once.
pub fn words(lines: &[String]) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for line in lines {
        for word in line.split(|c: char| !c.is_alphanumeric()) {
            let word = word.to_lowercase();
            if word.chars().count() >= 3 && !found.contains(&word) {
                found.push(word);
            }
        }
    }
    found
}

/// The best scores of at least `least`, most first, at most `most`.
fn best(mut scored: Vec<(String, f32)>, least: f32, most: usize) -> Vec<(String, f32)> {
    scored.retain(|(_, score)| *score >= least);
    scored.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    scored.truncate(most);
    scored
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vision::quantize;

    #[test]
    fn cosine_of_quantized_vectors() {
        let a = [0.6, 0.8, 0.0];
        let same = quantize(&a);
        let other = quantize(&[0.0, 0.0, 1.0]);
        let asked = unit(&same).unwrap();
        assert!((cosine(&asked, &same).unwrap() - 1.0).abs() < 1e-3);
        assert!(cosine(&asked, &other).unwrap().abs() < 1e-3);
        assert_eq!(cosine(&asked, &[1, 2]), None, "another length");
        assert_eq!(unit(&[0, 0, 0]), None);
    }

    #[test]
    fn words_worth_matching() {
        let lines = vec!["УЛИЦА Самоилова, 12".to_owned(), "улица — ok".to_owned()];
        assert_eq!(words(&lines), ["улица", "самоилова"]);
    }

    #[test]
    fn best_keeps_the_most_similar() {
        let scored = vec![("a".into(), 0.7), ("b".into(), 0.9), ("c".into(), 0.5), ("d".into(), 0.9)];
        let best = best(scored, 0.6, 2);
        assert_eq!(best, [("b".to_owned(), 0.9), ("d".to_owned(), 0.9)]);
    }

    #[test]
    fn state_round_trips_and_tolerates_old_files() {
        let state = State {
            clip: PartState { started: Some("t.1".into()), failed: None },
            ocr: PartState { started: None, failed: Some(Failure { at: 5, why: "no".into() }) },
        };
        let json = serde_json::to_vec(&state).unwrap();
        assert_eq!(serde_json::from_slice::<State>(&json).unwrap(), state);
        assert_eq!(serde_json::from_str::<State>("{}").unwrap(), State::default());
    }
}
