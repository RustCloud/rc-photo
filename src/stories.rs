//! Stories (since 0.7.0): a story a day from the user's photos, like a
//! memory — this day years ago, a trip, an event, a person through the
//! years. A story is a script for the page to play (photos, video clips, how
//! long each shows, where to zoom to), not a video file.
//!
//! - The page's look at the state ([`status`]) adds the day's `story` job if
//!   the administrator left stories on and the day has none yet; users who
//!   don't open the gallery cost nothing. The job's key is the day, so a day
//!   gets one job however often the page looks.
//! - The job picks the story ([`choose`]) and its photos ([`pick`]) and
//!   writes `stories/<day>.json`. A story shows for [`LIFETIME_DAYS`] days.
//! - A photo isn't in another story for [`PHOTO_BREAK_DAYS`] days, a theme
//!   (this day of a year, a trip, a person) isn't told again for
//!   [`THEME_BREAK_DAYS`] days (`stories-made.json`, written by the job). The
//!   user hiding a story hides its theme for good (`stories-seen.json`,
//!   written by the page's requests, with the stories seen).

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::analysis::ClipFile;
use crate::faces::FaceFile;
use crate::index::{shard_path, Entry, Shards, Store, SHARDS};
use crate::media::{later, outcome, AppData, Problem};
use crate::models::Switches;
use crate::people::{Names, People, PEOPLE};
use crate::rustcloud::app::appdata;
use crate::rustcloud::app::jobs::{self as queue, Job, NewJob, Outcome};
use crate::web::{error, ok};
use crate::Response;

pub const DIR: &str = "stories";
const MADE: &str = "stories-made.json";
const SEEN: &str = "stories-seen.json";

pub const LIFETIME_DAYS: i64 = 3;
pub const PHOTO_BREAK_DAYS: i64 = 28;
pub const THEME_BREAK_DAYS: i64 = 21;
/// What the job remembers of photos and themes, at most this long.
const REMEMBER_DAYS: i64 = 60;
/// The fewest photos a story is made of, and the most.
const MIN_PHOTOS: usize = 5;
const MOST_PHOTOS: usize = 20;
const MOST_VIDEOS: usize = 3;
const PHOTO_MS: u64 = 4500;
const MOST_CLIP_MS: u64 = 5000;
/// Photos this close in time are one moment: a burst, a few tries.
const BURST_SECONDS: i64 = 3;
/// Photos this alike (cosine of their vectors) are one shot.
const SAME_SHOT: f32 = 0.95;
/// This day years ago: the days around it.
const AROUND_DAYS: i64 = 3;
/// A trip: photos this far from home.
const TRIP_KM: f64 = 50.0;
/// An event: a day with this many photos at least, and this many times the
/// usual day.
const EVENT_PHOTOS: usize = 12;
const EVENT_TIMES: usize = 3;
/// Trips and events of the last days aren't memories yet.
const SETTLED_DAYS: i64 = 7;
/// A person without a name is told about with this many photos.
const UNNAMED_PERSON_PHOTOS: usize = 15;

// ── Dates ──────────────────────────────────────────────────────────────

/// Days since 1970-01-01 of a `YYYY-MM-DD` date.
pub fn day_number(date: &str) -> Option<i64> {
    let (y, rest) = date.get(..10)?.split_once('-')?;
    let (m, d) = rest.split_once('-')?;
    let (y, m, d): (i64, i64, i64) = (y.parse().ok()?, m.parse().ok()?, d.parse().ok()?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    // Howard Hinnant's days_from_civil.
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

/// The `YYYY-MM-DD` date of a day number.
pub fn date_of(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}")
}

pub fn valid_date(date: &str) -> bool {
    date.len() == 10 && day_number(date).is_some_and(|days| date_of(days) == date)
}

/// The same day `years` years earlier (29 February → 28 February).
fn years_before(date: &str, years: i64) -> Option<String> {
    let year: i64 = date.get(..4)?.parse().ok()?;
    let mut earlier = format!("{:04}{}", year - years, date.get(4..10)?);
    if !valid_date(&earlier) {
        earlier = format!("{:04}{}", year - years, date.get(4..8)?.to_owned() + "28");
    }
    valid_date(&earlier).then_some(earlier)
}

// ── What a story is made from ─────────────────────────────────────────

/// A photo or video of the index, as stories need it.
#[derive(Clone, Debug, PartialEq)]
pub struct Shot {
    pub id: String,
    /// Days since 1970 and seconds since 1970 (the camera's clock).
    pub day: i64,
    pub seconds: i64,
    pub gps: Option<(f64, f64)>,
    pub video: bool,
    /// A video's length.
    pub duration_ms: Option<u64>,
    pub pixels: u64,
    pub favorite: bool,
}

impl Shot {
    fn from_entry(entry: &Entry, favorites: &HashSet<String>) -> Option<Self> {
        let when = entry.taken.as_deref().filter(|t| t.len() >= 10 && valid_date(&t[..10])).unwrap_or(&entry.modified);
        let day = day_number(when)?;
        let time = when.get(11..19).unwrap_or("00:00:00");
        let parts: Vec<i64> = time.split(':').filter_map(|p| p.parse().ok()).collect();
        let seconds = day * 86_400
            + parts.first().unwrap_or(&0) * 3600
            + parts.get(1).unwrap_or(&0) * 60
            + parts.get(2).unwrap_or(&0);
        // Not described yet, or broken: nothing to show.
        if !entry.thumb || entry.error.is_some() {
            return None;
        }
        Some(Self {
            id: entry.id.clone(),
            day,
            seconds,
            gps: entry.gps,
            video: entry.video,
            duration_ms: entry.duration,
            pixels: u64::from(entry.width) * u64::from(entry.height),
            favorite: favorites.contains(&entry.id),
        })
    }
}

/// A story that could be told: its theme and its photos (indexes of shots).
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    pub kind: &'static str,
    /// The theme: not told again for a while, hidden for good by the user.
    pub key: String,
    pub params: serde_json::Value,
    pub shots: Vec<usize>,
}

/// This day (and the days around it) in each earlier year with enough
/// photos, the year with the most first.
pub fn on_this_day(shots: &[Shot], today: &str) -> Vec<Candidate> {
    let mut found = Vec::new();
    for years in 1..=50 {
        let Some(then) = years_before(today, years) else { break };
        let Some(center) = day_number(&then) else { continue };
        let picked: Vec<usize> = (0..shots.len()).filter(|&i| (shots[i].day - center).abs() <= AROUND_DAYS).collect();
        if picked.len() >= MIN_PHOTOS {
            found.push(Candidate {
                kind: "on-this-day",
                key: format!("on-this-day:{then}"),
                params: json!({ "years": years, "date": then }),
                shots: picked,
            });
        }
    }
    found.sort_by_key(|c| std::cmp::Reverse(c.shots.len()));
    found
}

/// Days with many more photos than usual, settled ones, the busiest first.
pub fn events(shots: &[Shot], today: i64) -> Vec<Candidate> {
    let mut by_day: BTreeMap<i64, Vec<usize>> = BTreeMap::new();
    for (i, shot) in shots.iter().enumerate() {
        by_day.entry(shot.day).or_default().push(i);
    }
    let mut counts: Vec<usize> = by_day.values().map(Vec::len).collect();
    counts.sort_unstable();
    let usual = counts.get(counts.len() / 2).copied().unwrap_or(0);
    let mut found: Vec<Candidate> = by_day
        .into_iter()
        .filter(|(day, picked)| {
            today - day >= SETTLED_DAYS && picked.len() >= EVENT_PHOTOS && picked.len() >= EVENT_TIMES * usual
        })
        .map(|(day, picked)| {
            let date = date_of(day);
            Candidate { kind: "event", key: format!("event:{date}"), params: json!({ "date": date }), shots: picked }
        })
        .collect();
    found.sort_by_key(|c| std::cmp::Reverse(c.shots.len()));
    found
}

fn km(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (la1, lo1, la2, lo2) = (a.0.to_radians(), a.1.to_radians(), b.0.to_radians(), b.1.to_radians());
    let h = ((la2 - la1) / 2.0).sin().powi(2) + la1.cos() * la2.cos() * ((lo2 - lo1) / 2.0).sin().powi(2);
    6371.0 * 2.0 * h.sqrt().asin()
}

/// Where most photos with a place were taken: the middle of the busiest
/// cell of about 50 km.
pub fn home(shots: &[Shot]) -> Option<(f64, f64)> {
    let mut cells: HashMap<(i64, i64), (usize, f64, f64)> = HashMap::new();
    for (lat, lon) in shots.iter().filter_map(|s| s.gps) {
        let cell = cells.entry(((lat * 2.0).floor() as i64, (lon * 2.0).floor() as i64)).or_default();
        *cell = (cell.0 + 1, cell.1 + lat, cell.2 + lon);
    }
    cells.into_values().max_by_key(|c| c.0).map(|(n, lat, lon)| (lat / n as f64, lon / n as f64))
}

/// Days away from home in a row (a day between them doesn't break a trip),
/// with all the photos of those days, settled ones, the longest first.
pub fn trips(shots: &[Shot], today: i64) -> Vec<Candidate> {
    let Some(home) = home(shots) else { return Vec::new() };
    let away: BTreeSet<i64> =
        shots.iter().filter(|s| s.gps.is_some_and(|place| km(place, home) > TRIP_KM)).map(|s| s.day).collect();
    let mut spans: Vec<(i64, i64)> = Vec::new();
    for day in away {
        match spans.last_mut() {
            Some((_, end)) if day - *end <= 2 => *end = day,
            _ => spans.push((day, day)),
        }
    }
    let mut found: Vec<Candidate> = spans
        .into_iter()
        .filter(|&(_, end)| today - end >= SETTLED_DAYS)
        .filter_map(|(start, end)| {
            let picked: Vec<usize> = (0..shots.len()).filter(|&i| (start..=end).contains(&shots[i].day)).collect();
            (picked.len() >= MIN_PHOTOS).then(|| Candidate {
                kind: "trip",
                key: format!("trip:{}", date_of(start)),
                params: json!({ "from": date_of(start), "to": date_of(end) }),
                shots: picked,
            })
        })
        .collect();
    found.sort_by_key(|c| std::cmp::Reverse(c.shots.len()));
    found
}

/// A person through the years: people with a name first, then others with
/// many photos; hidden ones never.
pub fn persons(shots: &[Shot], people: &People, names: &Names) -> Vec<Candidate> {
    let index: HashMap<&str, usize> = shots.iter().enumerate().map(|(i, s)| (s.id.as_str(), i)).collect();
    // A person merged into another tells with them.
    let target = |id: &str| -> String {
        let mut id = id.to_owned();
        for _ in 0..16 {
            match names.merged.get(&id) {
                Some(next) => id = next.clone(),
                None => break,
            }
        }
        id
    };
    let mut files: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
    for person in &people.people {
        let into = files.entry(target(&person.id)).or_default();
        into.extend(person.files.iter().filter_map(|f| index.get(f.as_str()).copied()));
    }
    let mut found: Vec<(bool, Candidate)> = files
        .into_iter()
        .filter(|(id, _)| !names.hidden.contains(id))
        .filter_map(|(id, picked)| {
            let named = names.names.contains_key(&id);
            let enough = if named { MIN_PHOTOS * 2 } else { UNNAMED_PERSON_PHOTOS };
            (picked.len() >= enough).then(|| {
                let candidate = Candidate {
                    kind: "person",
                    key: format!("person:{id}"),
                    params: json!({ "person": id }),
                    shots: picked.into_iter().collect(),
                };
                (named, candidate)
            })
        })
        .collect();
    found.sort_by_key(|(named, c)| (std::cmp::Reverse(*named), std::cmp::Reverse(c.shots.len())));
    found.into_iter().map(|(_, c)| c).collect()
}

// ── Choosing ───────────────────────────────────────────────────────────

/// What the job remembers: the last story, photos and themes used.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Made {
    /// The last day a story was looked for (made or not).
    #[serde(default)]
    pub last: Option<String>,
    #[serde(default)]
    pub last_kind: Option<String>,
    /// Photo → the day of its last story.
    #[serde(default)]
    pub photos: BTreeMap<String, String>,
    /// Theme → the day it was last told.
    #[serde(default)]
    pub themes: BTreeMap<String, String>,
}

/// What the user did: stories seen, themes hidden for good.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Seen {
    #[serde(default)]
    pub seen: BTreeSet<String>,
    #[serde(default)]
    pub hidden: BTreeSet<String>,
}

fn days_since(day: &str, today: i64) -> i64 {
    day_number(day).map_or(i64::MAX, |then| today - then)
}

/// The day's story from the candidates: themes on a break or hidden are
/// left out, and photos on a break; a candidate with too few photos left
/// is too. The kind of yesterday's story goes last, so kinds take turns.
/// The order otherwise: this day years ago, trips, events, people.
pub fn choose(
    mut candidates: Vec<Candidate>,
    shots: &[Shot],
    made: &Made,
    seen: &Seen,
    today: i64,
) -> Option<Candidate> {
    let rank = |kind: &str| match kind {
        "on-this-day" => 0,
        "trip" => 1,
        "event" => 2,
        _ => 3,
    };
    candidates.sort_by_key(|c| (made.last_kind.as_deref() == Some(c.kind), rank(c.kind)));
    candidates.into_iter().find_map(|mut candidate| {
        if seen.hidden.contains(&candidate.key)
            || made.themes.get(&candidate.key).is_some_and(|day| days_since(day, today) < THEME_BREAK_DAYS)
        {
            return None;
        }
        candidate
            .shots
            .retain(|&i| made.photos.get(&shots[i].id).is_none_or(|day| days_since(day, today) >= PHOTO_BREAK_DAYS));
        (candidate.shots.len() >= MIN_PHOTOS).then_some(candidate)
    })
}

/// The photos of a story, in time order: one of each burst or same shot,
/// spread over its time (the best of each stretch), at most
/// [`MOST_PHOTOS`] and [`MOST_VIDEOS`] videos.
pub fn pick(
    chosen: &[usize],
    shots: &[Shot],
    vectors: &HashMap<String, Vec<i8>>,
    faces: &HashSet<String>,
) -> Vec<usize> {
    let mut order: Vec<usize> = chosen.to_vec();
    order.sort_by_key(|&i| (shots[i].seconds, shots[i].id.clone()));
    let score = |i: usize| -> i64 {
        let shot = &shots[i];
        (shot.favorite as i64) * 100
            + (faces.contains(&shot.id) as i64) * 30
            + (shot.video as i64) * 10
            + (shot.pixels / 1_000_000).min(9) as i64
    };
    // One of each burst: the best.
    let mut moments: Vec<usize> = Vec::new();
    for i in order {
        match moments.last_mut() {
            Some(last) if shots[i].seconds - shots[*last].seconds <= BURST_SECONDS => {
                if score(i) > score(*last) {
                    *last = i;
                }
            }
            _ => moments.push(i),
        }
    }
    // One of each shot: drop the ones too like one kept before.
    let mut kept: Vec<usize> = Vec::new();
    for i in moments {
        let alike = vectors
            .get(&shots[i].id)
            .is_some_and(|v| kept.iter().any(|&k| vectors.get(&shots[k].id).is_some_and(|w| cosine(v, w) > SAME_SHOT)));
        if !alike {
            kept.push(i);
        }
    }
    // Spread: equal stretches of the photos, the best of each.
    let mut picked: Vec<usize> = if kept.len() <= MOST_PHOTOS {
        kept
    } else {
        (0..MOST_PHOTOS)
            .filter_map(|n| {
                let stretch = &kept[n * kept.len() / MOST_PHOTOS..(n + 1) * kept.len() / MOST_PHOTOS];
                stretch.iter().copied().max_by_key(|&i| (score(i), std::cmp::Reverse(i)))
            })
            .collect()
    };
    let mut videos = 0;
    picked.retain(|&i| {
        if shots[i].video {
            videos += 1;
            return videos <= MOST_VIDEOS;
        }
        true
    });
    picked
}

fn cosine(a: &[i8], b: &[i8]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    let (mut dot, mut aa, mut bb) = (0f32, 0f32, 0f32);
    for (&x, &y) in a.iter().zip(b) {
        let (x, y) = (f32::from(x), f32::from(y));
        dot += x * y;
        aa += x * x;
        bb += y * y;
    }
    if aa == 0.0 || bb == 0.0 {
        0.0
    } else {
        dot / (aa.sqrt() * bb.sqrt())
    }
}

// ── The story file ─────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Story {
    /// The day it was made for; it shows to `expires` included.
    pub date: String,
    pub expires: String,
    pub kind: String,
    pub key: String,
    /// What the page makes the title from (`years`, `date`, `from`, `to`, `person`).
    pub params: serde_json::Value,
    pub cover: String,
    pub items: Vec<StoryItem>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StoryItem {
    pub id: String,
    /// How long it shows.
    pub ms: u64,
    /// A video: where its clip starts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<u64>,
    /// Where to zoom to (parts of the picture, 0–1): a face, else the middle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus: Option<[f32; 2]>,
}

fn item(shot: &Shot, focus: Option<[f32; 2]>) -> StoryItem {
    match (shot.video, shot.duration_ms) {
        (true, Some(length)) if length > 0 => {
            let ms = length.min(MOST_CLIP_MS);
            StoryItem { id: shot.id.clone(), ms, from: Some((length / 2).saturating_sub(ms / 2)), focus: None }
        }
        _ => StoryItem { id: shot.id.clone(), ms: PHOTO_MS, from: None, focus },
    }
}

// ── The job ────────────────────────────────────────────────────────────

fn read<T: Default + for<'de> Deserialize<'de>>(path: &str) -> T {
    appdata::read(path).ok().and_then(|data| serde_json::from_slice(&data).ok()).unwrap_or_default()
}

fn write(path: &str, value: &impl Serialize) -> Result<(), appdata::Error> {
    appdata::write(path, &serde_json::to_vec(value).expect("stories serialize")).map(drop)
}

fn story_path(date: &str) -> String {
    format!("{DIR}/{date}.json")
}

/// Runs `story` jobs: the key is the user's day, `YYYY-MM-DD`.
pub fn run_story(jobs: &[Job]) -> Vec<Outcome> {
    jobs.iter().map(|job| outcome(make(&job.key))).collect()
}

fn make(date: &str) -> Result<(), Problem> {
    if !valid_date(date) || !Switches::read().stories {
        return Ok(());
    }
    let mut made: Made = read(MADE);
    if made.last.as_deref() == Some(date) {
        return Ok(());
    }
    let today = day_number(date).expect("a valid date");
    let seen: Seen = read(SEEN);
    let favorites: HashSet<String> = read::<Vec<String>>(crate::api::FAVORITES).into_iter().collect();
    let shots = shots(&favorites)?;
    let people: People = read(PEOPLE);
    let names = Names::read();
    let mut candidates = on_this_day(&shots, date);
    candidates.extend(trips(&shots, today));
    candidates.extend(events(&shots, today));
    candidates.extend(persons(&shots, &people, &names));
    let story = choose(candidates, &shots, &made, &seen, today);
    if let Some(candidate) = story {
        let ids: Vec<&str> = candidate.shots.iter().map(|&i| shots[i].id.as_str()).collect();
        let vectors = vectors(&ids);
        let faces = faces(&ids);
        let picked = pick(&candidate.shots, &shots, &vectors, &faces.keys().cloned().collect());
        let items: Vec<StoryItem> = picked.iter().map(|&i| item(&shots[i], faces.get(&shots[i].id).copied())).collect();
        let cover = picked
            .iter()
            .find(|&&i| !shots[i].video && (shots[i].favorite || faces.contains_key(&shots[i].id)))
            .or_else(|| picked.iter().find(|&&i| !shots[i].video))
            .or(picked.first())
            .map(|&i| shots[i].id.clone())
            .unwrap_or_default();
        let story = Story {
            date: date.to_owned(),
            expires: date_of(today + LIFETIME_DAYS - 1),
            kind: candidate.kind.to_owned(),
            key: candidate.key.clone(),
            params: candidate.params,
            cover,
            items,
        };
        write(&story_path(date), &story).map_err(later)?;
        for item in &story.items {
            made.photos.insert(item.id.clone(), date.to_owned());
        }
        made.themes.insert(candidate.key, date.to_owned());
        made.last_kind = Some(candidate.kind.to_owned());
    }
    made.last = Some(date.to_owned());
    made.photos.retain(|_, day| days_since(day, today) < REMEMBER_DAYS);
    made.themes.retain(|_, day| days_since(day, today) < REMEMBER_DAYS);
    write(MADE, &made).map_err(later)?;
    forget_old(today);
    Ok(())
}

/// The shots of the index, one shard at a time.
fn shots(favorites: &HashSet<String>) -> Result<Vec<Shot>, Problem> {
    let mut shots = Vec::new();
    for shard in 0..SHARDS {
        let json = AppData.read(&shard_path(crate::index::DIR, shard)).map_err(later)?;
        let entries: Vec<Entry> = json.and_then(|json| serde_json::from_slice(&json).ok()).unwrap_or_default();
        shots.extend(entries.iter().filter_map(|entry| Shot::from_entry(entry, favorites)));
    }
    Ok(shots)
}

/// The vectors of these photos, where analysis made them.
fn vectors(ids: &[&str]) -> HashMap<String, Vec<i8>> {
    let tag = crate::models::clip_tag();
    let mut clips: Shards<AppData, ClipFile> = Shards::new(AppData);
    let mut found = HashMap::new();
    for id in ids {
        if let Ok(Some(file)) = clips.get(id) {
            if file.m == tag && !file.v.is_empty() {
                use base64::Engine;
                let bytes = base64::engine::general_purpose::STANDARD.decode(&file.v).unwrap_or_default();
                found.insert((*id).to_owned(), bytes.into_iter().map(|b| b as i8).collect());
            }
        }
    }
    found
}

/// The middle of the largest face of these photos, where faces were found.
fn faces(ids: &[&str]) -> HashMap<String, [f32; 2]> {
    let mut faces: Shards<AppData, FaceFile> = Shards::new(AppData);
    let mut found = HashMap::new();
    for id in ids {
        if let Ok(Some(file)) = faces.get(id) {
            let largest = file.faces.iter().max_by(|a, b| (a.b[2] * a.b[3]).total_cmp(&(b.b[2] * b.b[3])));
            if let Some(face) = largest {
                found.insert((*id).to_owned(), [face.b[0] + face.b[2] / 2.0, face.b[1] + face.b[3] / 2.0]);
            }
        }
    }
    found
}

/// Deletes the stories that no longer show.
fn forget_old(today: i64) {
    let Ok(files) = appdata::list(DIR) else { return };
    for file in files {
        let date = file.name.trim_end_matches(".json");
        if day_number(date).is_some_and(|day| today - day >= LIFETIME_DAYS) {
            let _ = AppData.delete(&story_path(date));
        }
    }
}

// ── The page ───────────────────────────────────────────────────────────

/// For the page's state: the stories showing on the user's day (`today`,
/// from the page; else the server's), and the day's job added if due.
pub fn status(today: Option<&str>) -> serde_json::Value {
    if !Switches::read().stories {
        return json!({ "enabled": false });
    }
    let date = today.filter(|d| valid_date(d)).map(str::to_owned).unwrap_or_else(server_today);
    let Some(day) = day_number(&date) else { return json!({ "enabled": false }) };
    let seen: Seen = read(SEEN);
    let mut list: Vec<Story> = appdata::list(DIR)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|file| {
            let shown = file
                .name
                .strip_suffix(".json")
                .and_then(day_number)
                .is_some_and(|d| (0..LIFETIME_DAYS).contains(&(day - d)));
            if !shown {
                return None;
            }
            appdata::read(&format!("{DIR}/{}", file.name))
                .ok()
                .and_then(|data| serde_json::from_slice::<Story>(&data).ok())
        })
        .filter(|story| !seen.hidden.contains(&story.key))
        .collect();
    list.sort_by(|a, b| b.date.cmp(&a.date));
    let made: Made = read(MADE);
    if made.last.as_deref() != Some(date.as_str()) && !list.iter().any(|s| s.date == date) {
        let _ = queue::add(&[NewJob { kind: "story".into(), key: date.clone(), data: String::new() }]);
    }
    let seen_now: Vec<&String> = list.iter().map(|s| &s.date).filter(|d| seen.seen.contains(*d)).collect();
    json!({ "enabled": true, "list": list, "seen": seen_now })
}

fn server_today() -> String {
    let seconds = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    date_of((seconds / 86_400) as i64)
}

/// The user watched a story.
pub fn mark_seen(date: &str) -> Response {
    change_seen(date, |seen, _| {
        seen.seen.insert(date.to_owned());
    })
}

/// The user doesn't want this story, nor its theme again.
pub fn hide(date: &str) -> Response {
    change_seen(date, |seen, story| {
        if let Some(story) = story {
            seen.hidden.insert(story.key.clone());
        }
    })
}

fn change_seen(date: &str, change: impl FnOnce(&mut Seen, Option<&Story>)) -> Response {
    if !valid_date(date) {
        return error(400, "not a date");
    }
    let story: Option<Story> = appdata::read(&story_path(date)).ok().and_then(|d| serde_json::from_slice(&d).ok());
    let mut seen: Seen = read(SEEN);
    change(&mut seen, story.as_ref());
    // Only the stories still showing are worth remembering as seen.
    if let Some(today) = day_number(date) {
        seen.seen.retain(|d| day_number(d).is_some_and(|day| (today - day).abs() < LIFETIME_DAYS + 1));
    }
    match write(SEEN, &seen) {
        Ok(()) => ok(json!({ "ok": true })),
        Err(err) => error(500, &format!("{err:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shot(id: &str, date: &str, time: &str) -> Shot {
        let day = day_number(date).unwrap();
        let t: Vec<i64> = time.split(':').map(|p| p.parse().unwrap()).collect();
        Shot {
            id: id.into(),
            day,
            seconds: day * 86_400 + t[0] * 3600 + t[1] * 60 + t[2],
            gps: None,
            video: false,
            duration_ms: None,
            pixels: 12_000_000,
            favorite: false,
        }
    }

    fn day_of(date: &str, count: usize, prefix: &str) -> Vec<Shot> {
        (0..count).map(|i| shot(&format!("{prefix}{i}"), date, &format!("12:{:02}:00", i % 60))).collect()
    }

    #[test]
    fn dates_go_both_ways() {
        for date in ["1970-01-01", "2000-02-29", "2024-12-31", "2026-11-21", "1969-07-20"] {
            assert_eq!(date_of(day_number(date).unwrap()), date);
        }
        assert_eq!(day_number("1970-01-02"), Some(1));
        assert!(!valid_date("2023-02-29") && !valid_date("2026-13-01") && !valid_date("x"));
        assert_eq!(years_before("2024-02-29", 1).as_deref(), Some("2023-02-28"));
        assert_eq!(years_before("2026-11-21", 3).as_deref(), Some("2023-11-21"));
    }

    #[test]
    fn this_day_years_ago() {
        let mut shots = day_of("2023-11-20", 6, "a");
        shots.extend(day_of("2025-11-23", 8, "b"));
        shots.extend(day_of("2024-11-10", 9, "c")); // too far from the day
        let found = on_this_day(&shots, "2026-11-21");
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].key, "on-this-day:2025-11-21");
        assert_eq!(found[0].params["years"], 1);
        assert_eq!(found[1].key, "on-this-day:2023-11-21");
        assert_eq!(found[1].shots.len(), 6);
    }

    #[test]
    fn a_busy_day_is_an_event() {
        let mut shots = Vec::new();
        for d in 1..=10 {
            shots.extend(day_of(&format!("2026-03-{d:02}"), 2, &format!("d{d}-")));
        }
        shots.extend(day_of("2026-04-01", 20, "party"));
        shots.extend(day_of("2026-11-19", 30, "recent")); // not settled yet
        let found = events(&shots, day_number("2026-11-21").unwrap());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].key, "event:2026-04-01");
    }

    #[test]
    fn days_away_from_home_are_a_trip() {
        let at = |mut s: Shot, place: (f64, f64)| {
            s.gps = Some(place);
            s
        };
        let moscow = (55.75, 37.62);
        let sochi = (43.59, 39.72);
        let mut shots: Vec<Shot> =
            (0..20).map(|i| at(shot(&format!("h{i}"), "2026-01-10", "10:00:00"), moscow)).collect();
        shots.extend((0..4).map(|i| at(shot(&format!("s{i}"), "2026-07-01", "10:00:00"), sochi)));
        shots.extend((0..3).map(|i| shot(&format!("n{i}"), "2026-07-02", "11:00:00"))); // no place, same trip
        shots.extend((0..4).map(|i| at(shot(&format!("t{i}"), "2026-07-03", "10:00:00"), sochi)));
        let found = trips(&shots, day_number("2026-11-21").unwrap());
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].params, json!({ "from": "2026-07-01", "to": "2026-07-03" }));
        assert_eq!(found[0].shots.len(), 11);
    }

    #[test]
    fn named_people_come_first_and_hidden_never() {
        let shots: Vec<Shot> = day_of("2026-01-01", 30, "f");
        let person = |id: &str, n: std::ops::Range<usize>| crate::people::Person {
            id: id.into(),
            count: n.len(),
            files: n.map(|i| format!("f{i}")).collect(),
            cover: crate::people::Cover { file: "f0".into(), b: [0.0; 4] },
        };
        let people = People {
            next: 4,
            people: vec![person("p1", 0..20), person("p2", 0..10), person("p3", 0..12), person("p4", 10..16)],
        };
        let mut names = Names::default();
        names.names.insert("p2".into(), "Аня".into());
        names.hidden.insert("p3".into());
        names.merged.insert("p4".into(), "p2".into());
        let found = persons(&shots, &people, &names);
        let keys: Vec<&str> = found.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(keys, ["person:p2", "person:p1"]);
        assert_eq!(found[0].shots.len(), 16, "p4's photos joined p2");
    }

    #[test]
    fn breaks_and_hidden_themes_are_kept() {
        let shots = day_of("2025-11-21", 8, "a");
        let candidate = |key: &str, kind: &'static str| Candidate {
            kind,
            key: key.into(),
            params: json!({}),
            shots: (0..8).collect(),
        };
        let today = day_number("2026-11-21").unwrap();
        let mut made = Made::default();
        let seen = Seen::default();
        let chosen = choose(
            vec![candidate("event:x", "event"), candidate("on-this-day:y", "on-this-day")],
            &shots,
            &made,
            &seen,
            today,
        );
        assert_eq!(chosen.unwrap().key, "on-this-day:y", "this day years ago first");
        // Yesterday's kind goes last.
        made.last_kind = Some("on-this-day".into());
        let chosen = choose(
            vec![candidate("event:x", "event"), candidate("on-this-day:y", "on-this-day")],
            &shots,
            &made,
            &seen,
            today,
        );
        assert_eq!(chosen.unwrap().key, "event:x");
        // A theme told 20 days ago waits, 21 days ago it may come back.
        made.themes.insert("event:x".into(), date_of(today - 20));
        assert_eq!(choose(vec![candidate("event:x", "event")], &shots, &made, &seen, today), None);
        made.themes.insert("event:x".into(), date_of(today - 21));
        assert!(choose(vec![candidate("event:x", "event")], &shots, &made, &seen, today).is_some());
        // Photos in a story 27 days ago are left out: too few remain.
        for i in 0..4 {
            made.photos.insert(format!("a{i}"), date_of(today - 27));
        }
        assert_eq!(choose(vec![candidate("event:x", "event")], &shots, &made, &seen, today), None);
        made.photos.clear();
        // Hidden for good.
        let mut seen = Seen::default();
        seen.hidden.insert("event:x".into());
        assert_eq!(choose(vec![candidate("event:x", "event")], &shots, &made, &seen, today), None);
    }

    #[test]
    fn picking_drops_bursts_and_same_shots_and_spreads() {
        let mut shots = vec![
            shot("a", "2026-01-01", "10:00:00"),
            shot("a2", "2026-01-01", "10:00:02"), // a burst with a
            shot("b", "2026-01-01", "11:00:00"),
            shot("c", "2026-01-01", "12:00:00"), // the same shot as b
        ];
        shots[1].favorite = true;
        let mut vectors = HashMap::new();
        vectors.insert("b".to_owned(), vec![10i8, 0, 0]);
        vectors.insert("c".to_owned(), vec![10i8, 1, 0]);
        let picked: Vec<&str> =
            pick(&[0, 1, 2, 3], &shots, &vectors, &HashSet::new()).iter().map(|&i| shots[i].id.as_str()).collect();
        assert_eq!(picked, ["a2", "b"]);
        // Many photos: at most MOST_PHOTOS, over the whole time.
        let many: Vec<Shot> = (0..100)
            .map(|i| shot(&format!("m{i}"), "2026-01-01", &format!("{:02}:{:02}:00", i / 60, i % 60)))
            .collect();
        let picked = pick(&(0..100).collect::<Vec<_>>(), &many, &HashMap::new(), &HashSet::new());
        assert_eq!(picked.len(), MOST_PHOTOS);
        assert!(picked[0] < 5 && *picked.last().unwrap() >= 95);
    }

    #[test]
    fn a_video_shows_a_clip_from_its_middle() {
        let mut video = shot("v", "2026-01-01", "10:00:00");
        video.video = true;
        video.duration_ms = Some(60_000);
        let clip = item(&video, None);
        assert_eq!((clip.ms, clip.from), (5000, Some(27_500)));
        video.duration_ms = Some(3000);
        assert_eq!((item(&video, None).ms, item(&video, None).from), (3000, Some(0)));
        let photo = item(&shot("p", "2026-01-01", "10:00:00"), Some([0.3, 0.4]));
        assert_eq!((photo.ms, photo.from, photo.focus), (PHOTO_MS, None, Some([0.3, 0.4])));
    }
}
