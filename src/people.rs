//! People: the `people` job groups the faces' vectors ([`crate::faces`]).
//!
//! A face without a person joins the person whose average face is closest,
//! if close enough; faces left over that are close to each other make new
//! people, at least [`MIN_FACES`] of them. People the user merged are put
//! together; names are the user's, kept apart from what the job writes:
//!
//! - `people.json` — the people, their photos and a face to show: written by
//!   the job only;
//! - `people-names.json` — names, hidden people, merges, faces put with a
//!   person by hand, chosen faces to show: written by the page's requests
//!   only, read by the job. Faces put by hand are never moved by the job.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::faces::{self, FaceFile};
use crate::index::Shards;
use crate::media::{later, outcome, AppData, Problem};
use crate::rustcloud::app::appdata;
use crate::rustcloud::app::jobs::{self as queue, Job, NewJob, Outcome};
use crate::vision;
use crate::web::{error, ok, valid_id};
use crate::Response;

pub const PEOPLE: &str = "people.json";
pub const NAMES: &str = "people-names.json";

/// A face joins a person when this close to their average face.
const JOIN: f32 = 0.45;
/// Faces this close are linked when new people are looked for.
const LINK: f32 = 0.5;
/// The fewest faces a new person is made of.
const MIN_FACES: usize = 3;
/// Narrower faces (px of the photo) only join people, never start one.
const SEED_WIDTH: u32 = 48;
/// The most faces without a person compared with each other in one run,
/// the sharpest first.
const POOL: usize = 2000;
/// While `faces` jobs wait, the people are grouped again only after this
/// many photos changed: each grouping reads all the faces.
const EVERY: u64 = 300;

#[derive(Default, Serialize, Deserialize)]
pub struct People {
    /// The number of the next new person.
    #[serde(default)]
    pub next: u64,
    pub people: Vec<Person>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Person {
    pub id: String,
    /// Faces.
    pub count: usize,
    /// The photos they are in.
    pub files: Vec<String>,
    /// The face to show: the sharpest.
    pub cover: Cover,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cover {
    pub file: String,
    pub b: [f32; 4],
}

/// What the user said about people.
#[derive(Default, Serialize, Deserialize)]
pub struct Names {
    #[serde(default)]
    pub names: BTreeMap<String, String>,
    #[serde(default)]
    pub hidden: BTreeSet<String>,
    /// A person merged into another: its faces go there.
    #[serde(default)]
    pub merged: BTreeMap<String, String>,
    /// Faces put with a person, or with nobody, by hand: `<file>/<index>`.
    #[serde(default)]
    pub faces: BTreeMap<String, Assigned>,
    /// The face each person is shown by, if the user chose one.
    #[serde(default)]
    pub covers: BTreeMap<String, Cover>,
    /// The number of the next person made by hand (`m<n>`).
    #[serde(default)]
    pub next: u64,
}

/// A face's person as the user set it, while the file is as it was then.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Assigned {
    pub etag: String,
    /// None: nobody ("not this person").
    #[serde(default)]
    pub p: Option<String>,
}

/// The key of a face in [`Names::faces`].
pub fn face_key(file: &str, index: usize) -> String {
    format!("{file}/{index}")
}

fn read<T: Default + for<'de> Deserialize<'de>>(path: &str) -> T {
    appdata::read(path).ok().and_then(|data| serde_json::from_slice(&data).ok()).unwrap_or_default()
}

fn write(path: &str, value: &impl Serialize) -> Result<(), appdata::Error> {
    appdata::write(path, &serde_json::to_vec(value).expect("people serialize")).map(drop)
}

impl Names {
    pub fn read() -> Self {
        read(NAMES)
    }
}

// ── Grouping ───────────────────────────────────────────────────────────

/// One face as grouping sees it.
pub struct Point {
    pub person: Option<String>,
    /// Quantized, as kept.
    vector: Vec<i8>,
    length: f32,
    /// Its width in pixels of the photo.
    width: u32,
    /// Put with its person (or nobody) by hand: grouping leaves it.
    fixed: bool,
}

impl Point {
    pub fn new(person: Option<String>, vector: &[u8], width: u32) -> Self {
        let vector: Vec<i8> = vector.iter().map(|&b| b as i8).collect();
        let length = vector.iter().map(|&v| (v as f32) * (v as f32)).sum::<f32>().sqrt().max(1.0);
        Self { person, vector, length, width, fixed: false }
    }

    /// The same, put by hand.
    pub fn fixed(mut self) -> Self {
        self.fixed = true;
        self
    }

    fn free(&self) -> bool {
        self.person.is_none() && !self.fixed
    }

    fn similar(&self, other: &Point) -> f32 {
        let dot: i32 = self.vector.iter().zip(&other.vector).map(|(&a, &b)| a as i32 * b as i32).sum();
        dot as f32 / (self.length * other.length)
    }

    fn similar_to(&self, average: &[f32]) -> f32 {
        self.vector.iter().zip(average).map(|(&v, a)| v as f32 * a).sum::<f32>() / self.length
    }
}

/// Where a merged person's faces go now (merges may chain).
fn resolve<'a>(merged: &'a BTreeMap<String, String>, mut id: &'a str) -> &'a str {
    for _ in 0..16 {
        match merged.get(id) {
            Some(into) if into != id => id = into,
            _ => break,
        }
    }
    id
}

/// Gives the faces without a person one, where it can; new people get ids
/// from `next`.
pub fn group(points: &mut [Point], merged: &BTreeMap<String, String>, next: &mut u64) {
    let first = *next;
    for point in points.iter_mut() {
        if let Some(person) = &point.person {
            let into = resolve(merged, person);
            if into != person {
                point.person = Some(into.to_owned());
            }
        }
    }

    join(points);

    // New people from the rest: groups of linked faces (DBSCAN). A face with
    // enough close ones is a core; a group is what its cores reach.
    let mut pool: Vec<usize> =
        (0..points.len()).filter(|&i| points[i].free() && points[i].width >= SEED_WIDTH).collect();
    pool.sort_by(|&a, &b| points[b].width.cmp(&points[a].width));
    pool.truncate(POOL);
    let mut links = vec![Vec::new(); pool.len()];
    for a in 0..pool.len() {
        for b in a + 1..pool.len() {
            if points[pool[a]].similar(&points[pool[b]]) >= LINK {
                links[a].push(b);
                links[b].push(a);
            }
        }
    }
    let core = |a: usize| links[a].len() + 1 >= MIN_FACES;
    let mut taken = vec![false; pool.len()];
    for start in 0..pool.len() {
        if taken[start] || !core(start) {
            continue;
        }
        let mut members = vec![start];
        taken[start] = true;
        let mut at = 0;
        while at < members.len() {
            let a = members[at];
            at += 1;
            if !core(a) {
                continue;
            }
            for &b in &links[a] {
                if !taken[b] {
                    taken[b] = true;
                    members.push(b);
                }
            }
        }
        // Always true for a core and its links; kept for clarity.
        if members.len() >= MIN_FACES {
            let id = format!("p{next}");
            *next += 1;
            for a in members {
                points[pool[a]].person = Some(id.clone());
            }
        }
    }
    // Faces too small to start a person may join the new ones.
    if *next != first {
        join(points);
    }
}

/// Gives each face without a person the person whose average face is
/// closest, if close enough.
fn join(points: &mut [Point]) {
    let mut sums: BTreeMap<&str, Vec<f32>> = BTreeMap::new();
    for point in points.iter() {
        if let Some(person) = &point.person {
            let sum = sums.entry(person.as_str()).or_insert_with(|| vec![0.0; point.vector.len()]);
            for (s, &v) in sum.iter_mut().zip(&point.vector) {
                *s += v as f32 / point.length;
            }
        }
    }
    let averages: Vec<(String, Vec<f32>)> = sums
        .into_iter()
        .map(|(person, mut sum)| {
            vision::normalize(&mut sum);
            (person.to_owned(), sum)
        })
        .collect();
    for point in points.iter_mut().filter(|point| point.free()) {
        let best = averages
            .iter()
            .map(|(person, average)| (person, point.similar_to(average)))
            .max_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((person, similarity)) = best {
            if similarity >= JOIN {
                point.person = Some(person.clone());
            }
        }
    }
}

/// A face as the summary of people sees it.
struct Seen {
    person: Option<String>,
    b: [f32; 4],
    /// Its width times the detector's score: the sharpest becomes the cover.
    sharpness: f32,
}

/// What is counted for one person.
struct Tally<'a> {
    faces: usize,
    files: BTreeSet<&'a str>,
    sharpness: f32,
    cover: Cover,
}

/// The people the faces now have, the most photos first.
fn summarize(files: &[(String, Vec<Seen>)]) -> Vec<Person> {
    let mut people: BTreeMap<&str, Tally> = BTreeMap::new();
    for (file, faces) in files {
        for face in faces {
            let Some(person) = &face.person else { continue };
            let tally = people.entry(person.as_str()).or_insert_with(|| Tally {
                faces: 0,
                files: BTreeSet::new(),
                sharpness: -1.0,
                cover: Cover { file: file.clone(), b: face.b },
            });
            tally.faces += 1;
            tally.files.insert(file.as_str());
            if face.sharpness > tally.sharpness {
                tally.sharpness = face.sharpness;
                tally.cover = Cover { file: file.clone(), b: face.b };
            }
        }
    }
    let mut list: Vec<Person> = people
        .into_iter()
        .map(|(id, tally)| Person {
            id: id.to_owned(),
            count: tally.faces,
            files: tally.files.into_iter().map(str::to_owned).collect(),
            cover: tally.cover,
        })
        .collect();
    list.sort_by(|a, b| b.files.len().cmp(&a.files.len()).then_with(|| a.id.cmp(&b.id)));
    list
}

// ── The job ────────────────────────────────────────────────────────────

/// Runs `people` jobs (one, key `all`): groups all the faces again.
/// `data` is `edit` when the user changed something: then at once.
pub fn run_people(jobs: &[Job]) -> Vec<Outcome> {
    let result = regroup(jobs.iter().any(|job| job.data == "edit"));
    jobs.iter().map(|_| outcome(result.clone())).collect()
}

fn regroup(edited: bool) -> Result<(), Problem> {
    let faces_waiting = queue::progress()
        .ok()
        .and_then(|kinds| kinds.into_iter().find(|kind| kind.kind == "faces"))
        .map_or(0, |kind| kind.waiting);
    // The last `faces` batch adds this job again.
    if !edited && faces_waiting > 0 && faces::pending() < EVERY {
        return Ok(());
    }
    let names = Names::read();
    let mut people: People = read(PEOPLE);
    let mut store: Shards<AppData, FaceFile> = Shards::new(AppData);
    let mut points = Vec::new();
    store
        .update_all(|file| {
            for (index, face) in file.faces.iter().enumerate() {
                let by_hand = names.faces.get(&face_key(&file.id, index)).filter(|set| set.etag == file.etag);
                points.push(match by_hand {
                    Some(set) => Point::new(set.p.clone(), &face.vector(), face.w).fixed(),
                    None => Point::new(face.p.clone(), &face.vector(), face.w),
                });
            }
            false
        })
        .map_err(later)?;
    group(&mut points, &names.merged, &mut people.next);

    // The same order again: shard by shard, file by file.
    let mut at = 0;
    let mut summary = Vec::new();
    store
        .update_all(|file| {
            let mut changed = false;
            let mut faces = Vec::with_capacity(file.faces.len());
            for face in &mut file.faces {
                let person = points[at].person.take();
                at += 1;
                if face.p != person {
                    face.p = person;
                    changed = true;
                }
                faces.push(Seen { person: face.p.clone(), b: face.b, sharpness: face.w as f32 * face.s });
            }
            summary.push((file.id.clone(), faces));
            changed
        })
        .map_err(later)?;
    store.save().map_err(later)?;
    people.people = summarize(&summary);
    write(PEOPLE, &people).map_err(later)?;
    faces::reset_pending();
    Ok(())
}

// ── For the page ───────────────────────────────────────────────────────

/// Names, hides or shows a person.
pub fn change(id: &str, name: Option<&str>, hidden: Option<bool>, cover: Option<Cover>) -> Response {
    if !valid_person(id) {
        return error(400, "not a person id");
    }
    let mut names = Names::read();
    if let Some(cover) = cover {
        if !valid_id(&cover.file) {
            return error(400, "not a file id");
        }
        names.covers.insert(id.to_owned(), cover);
    }
    if let Some(name) = name {
        let name = name.trim();
        if name.is_empty() {
            names.names.remove(id);
        } else {
            names.names.insert(id.to_owned(), name.chars().take(100).collect());
        }
    }
    match hidden {
        Some(true) => {
            names.hidden.insert(id.to_owned());
        }
        Some(false) => {
            names.hidden.remove(id);
        }
        None => {}
    }
    match write(NAMES, &names) {
        Ok(()) => ok(json!({ "names": names })),
        Err(err) => error(500, &format!("{err:?}")),
    }
}

/// Merges people into one; the job moves their faces.
pub fn merge(from: &[String], into: &str) -> Response {
    if !valid_person(into) || from.iter().any(|id| !valid_person(id) || id == into) || from.is_empty() {
        return error(400, "not person ids");
    }
    let mut names = Names::read();
    // Into whoever they were merged into: merges never go round.
    let into = resolve(&names.merged, into).to_owned();
    if from.contains(&into) {
        return error(400, "a person can't be merged into themself");
    }
    for id in from {
        names.merged.insert(id.clone(), into.clone());
        // The name and the face shown stay with the one merged into, unless it had none.
        if let Some(name) = names.names.remove(id) {
            names.names.entry(into.clone()).or_insert(name);
        }
        if let Some(cover) = names.covers.remove(id) {
            names.covers.entry(into.clone()).or_insert(cover);
        }
        names.hidden.remove(id);
    }
    if let Err(err) = write(NAMES, &names) {
        return error(500, &format!("{err:?}"));
    }
    regroup_soon();
    ok(json!({ "names": names }))
}

/// Puts one face with a person (`Some`), with nobody (`None`), or with a
/// new person of this name.
pub fn assign(file: &str, index: usize, person: Option<&str>, new_name: Option<&str>) -> Response {
    if !valid_id(file) {
        return error(400, "not a file id");
    }
    let mut store: Shards<AppData, FaceFile> = Shards::new(AppData);
    let etag = match store.get(file) {
        Ok(Some(found)) if index < found.faces.len() => found.etag.clone(),
        Ok(_) => return error(404, "no such face"),
        Err(err) => return error(500, &format!("{err:?}")),
    };
    let mut names = Names::read();
    let person = match (person, new_name.map(str::trim)) {
        (_, Some(name)) if !name.is_empty() => {
            let id = format!("m{}", names.next);
            names.next += 1;
            names.names.insert(id.clone(), name.chars().take(100).collect());
            Some(id)
        }
        (_, Some(_)) => return error(400, "a new person needs a name"),
        (Some(id), None) if valid_person(id) => Some(id.to_owned()),
        (Some(_), None) => return error(400, "not a person id"),
        (None, None) => None,
    };
    names.faces.insert(face_key(file, index), Assigned { etag, p: person.clone() });
    if let Err(err) = write(NAMES, &names) {
        return error(500, &format!("{err:?}"));
    }
    regroup_soon();
    ok(json!({ "names": names, "person": person }))
}

/// A person's faces, the sharpest first: to choose the one they are shown by.
pub fn faces_of(id: &str) -> Response {
    if !valid_person(id) {
        return error(400, "not a person id");
    }
    let names = Names::read();
    let mut store: Shards<AppData, FaceFile> = Shards::new(AppData);
    let mut found = Vec::new();
    let result = store.update_all(|file| {
        for (index, face) in file.faces.iter().enumerate() {
            let by_hand = names.faces.get(&face_key(&file.id, index)).filter(|set| set.etag == file.etag);
            let person = by_hand.map_or(face.p.as_deref(), |set| set.p.as_deref());
            if person.is_some_and(|person| resolve(&names.merged, person) == id) {
                found.push((face.w as f32 * face.s, json!({ "file": file.id, "i": index, "b": face.b, "w": face.w })));
            }
        }
        false
    });
    if let Err(err) = result {
        return error(500, &format!("{err:?}"));
    }
    found.sort_by(|a, b| b.0.total_cmp(&a.0));
    found.truncate(60);
    ok(json!({ "faces": found.into_iter().map(|(_, face)| face).collect::<Vec<_>>() }))
}

/// The people are grouped again soon: the user changed something.
fn regroup_soon() {
    let _ = queue::add(&[NewJob { kind: "people".into(), key: "all".into(), data: "edit".into() }]);
}

/// `p<n>`: found by the job; `m<n>`: made by hand.
fn valid_person(id: &str) -> bool {
    id.len() > 1
        && id.len() < 24
        && (id.starts_with('p') || id.starts_with('m'))
        && id[1..].bytes().all(|b| b.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A unit vector pointing mostly along axis `axis`, a little off by `seed`.
    fn face(axis: usize, seed: u32) -> Vec<u8> {
        let mut v: Vec<f32> =
            (0..64).map(|i| (((i as u32 * 7919 + seed * 104_729) % 97) as f32 / 97.0 - 0.5) * 0.6).collect();
        v[axis] = 3.0;
        vision::normalize(&mut v);
        vision::quantize(&v)
    }

    fn point(person: Option<&str>, axis: usize, seed: u32) -> Point {
        Point::new(person.map(str::to_owned), &face(axis, seed), 100)
    }

    fn people(points: &[Point]) -> Vec<Option<&str>> {
        points.iter().map(|p| p.person.as_deref()).collect()
    }

    #[test]
    fn close_faces_make_new_people_and_lone_ones_stay_unknown() {
        let mut points: Vec<Point> = vec![
            point(None, 0, 1),
            point(None, 0, 2),
            point(None, 0, 3),
            point(None, 5, 4),
            point(None, 5, 5),
            point(None, 5, 6),
            point(None, 5, 7),
            point(None, 9, 8),
        ];
        let mut next = 3;
        group(&mut points, &BTreeMap::new(), &mut next);
        let got = people(&points);
        assert_eq!(&got[..3], &[Some("p3"); 3]);
        assert_eq!(&got[3..7], &[Some("p4"); 4]);
        assert_eq!(got[7], None);
        assert_eq!(next, 5);
    }

    #[test]
    fn two_faces_are_not_enough_but_a_known_person_is_joined() {
        let mut points = vec![point(None, 0, 1), point(None, 0, 2)];
        group(&mut points, &BTreeMap::new(), &mut 0);
        assert_eq!(people(&points), vec![None, None]);
        let mut points = vec![point(Some("p7"), 0, 1), point(None, 0, 2), point(None, 3, 3)];
        group(&mut points, &BTreeMap::new(), &mut 0);
        assert_eq!(people(&points), vec![Some("p7"), Some("p7"), None]);
    }

    #[test]
    fn small_faces_join_but_never_start_a_person() {
        let mut points: Vec<Point> = (1..=4).map(|seed| Point::new(None, &face(0, seed), 20)).collect();
        group(&mut points, &BTreeMap::new(), &mut 0);
        assert!(people(&points).iter().all(Option::is_none));
        // A person found in the same run takes them.
        points.extend((5..=7).map(|seed| point(None, 0, seed)));
        group(&mut points, &BTreeMap::new(), &mut 0);
        assert!(people(&points).iter().all(|p| *p == Some("p0")), "{:?}", people(&points));
    }

    #[test]
    fn merged_people_go_where_they_were_merged() {
        let mut merged = BTreeMap::new();
        merged.insert("p1".to_owned(), "p2".to_owned());
        merged.insert("p2".to_owned(), "p3".to_owned());
        let mut points = vec![point(Some("p1"), 0, 1), point(Some("p2"), 4, 2)];
        group(&mut points, &merged, &mut 0);
        assert_eq!(people(&points), vec![Some("p3"), Some("p3")]);
    }

    #[test]
    fn faces_put_by_hand_stay_and_draw_others() {
        // One face put with m0 by hand; a close one joins m0. A face set to
        // nobody stays nobody's, however close.
        let mut points = vec![
            point(Some("m0"), 0, 1).fixed(),
            point(None, 0, 2),
            point(None, 0, 3).fixed(),
            point(Some("m0"), 6, 4).fixed(),
        ];
        group(&mut points, &BTreeMap::new(), &mut 0);
        assert_eq!(people(&points), vec![Some("m0"), Some("m0"), None, Some("m0")]);
    }

    #[test]
    fn people_are_summed_up_with_their_sharpest_face() {
        let seen = |person: Option<&str>, b: f32, sharpness: f32| Seen {
            person: person.map(str::to_owned),
            b: [b; 4],
            sharpness,
        };
        let files = vec![
            ("a".to_owned(), vec![seen(Some("p1"), 0.1, 50.0), seen(None, 0.2, 90.0)]),
            ("b".to_owned(), vec![seen(Some("p1"), 0.3, 80.0), seen(Some("p2"), 0.4, 10.0)]),
        ];
        let list = summarize(&files);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, "p1");
        assert_eq!(list[0].files, vec!["a", "b"]);
        assert_eq!(list[0].cover, Cover { file: "b".into(), b: [0.3; 4] });
        assert_eq!(list[1].count, 1);
    }

    #[test]
    fn person_ids_are_checked() {
        assert!(valid_person("p12"));
        assert!(!valid_person("p"));
        assert!(valid_person("m3"));
        assert!(!valid_person("x1"));
        assert!(!valid_person("p1/../x"));
    }
}
