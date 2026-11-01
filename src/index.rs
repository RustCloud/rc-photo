//! The library index: one [`Entry`] per photo or video, kept in the user's
//! AppData as `index/<shard>.json`, a JSON array per shard.
//!
//! A file's shard comes from a hash of its id, so a batch of jobs rewrites a
//! few small files rather than one large one, and the page fetches the shards
//! by links, past the plugin. Only batches write the index, one at a time
//! (`workers = 1` in the manifest), so there are no concurrent rewrites.
//! The faces of each photo ([`crate::faces`]) are kept the same way.

use std::collections::{BTreeMap, BTreeSet};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// How many shards the index is split into.
pub const SHARDS: u32 = 64;
/// The AppData folder of the shards.
pub const DIR: &str = "index";

/// What the gallery knows about one photo or video.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// The file's id.
    pub id: String,
    pub name: String,
    /// Folders from the granted one down to the file's: `Photos/2024/Trip`.
    pub folder: String,
    /// The granted folder it is in.
    pub root: String,
    /// The file's etag when it was described: a new one means describe again.
    pub etag: String,
    pub size: u64,
    pub mime: String,
    pub video: bool,
    /// When it was shot (RFC 3339, perhaps without an offset).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub taken: Option<String>,
    /// The file's last change, RFC 3339: the date when `taken` is missing.
    pub modified: String,
    /// As shown, rotation applied; 0 when unknown.
    pub width: u32,
    pub height: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lens: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iso: Option<u32>,
    /// f-number: 1.8.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aperture: Option<f32>,
    /// Shutter speed as people write it: `1/250`, `2s`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exposure: Option<String>,
    /// Focal length, mm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focal: Option<f32>,
    /// Latitude and longitude.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gps: Option<(f64, f64)>,
    /// Videos: length in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<u64>,
    /// Videos: the codec of the picture (`h264`, `hevc`…).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codec: Option<String>,
    /// `jpeg`, `heif`, `cr2`, `mp4`, `mov`…
    pub container: String,
    /// `thumbs/<id>.webp` exists.
    pub thumb: bool,
    /// `previews/<id>.jpg` exists: a large picture for a format the browser
    /// doesn't show (HEIC, RAW, TIFF).
    pub preview: bool,
    /// Why the file couldn't be described in full.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// What a shard holds: one item per file.
pub trait Item: Serialize + DeserializeOwned + PartialEq {
    /// The AppData folder of the shards.
    const DIR: &'static str;
    /// The file's id.
    fn id(&self) -> &str;
}

impl Item for Entry {
    const DIR: &'static str = DIR;
    fn id(&self) -> &str {
        &self.id
    }
}

/// Where the shards are read from and written to: AppData in the plugin,
/// memory in tests.
pub trait Store {
    type Error;
    /// The file's content, or none if there is no such file.
    fn read(&self, path: &str) -> Result<Option<Vec<u8>>, Self::Error>;
    fn write(&self, path: &str, data: &[u8]) -> Result<(), Self::Error>;
    /// Deletes the file; no error if it is missing.
    fn delete(&self, path: &str) -> Result<(), Self::Error>;
}

/// The shard a file's entry is in.
pub fn shard_of(id: &str) -> u32 {
    fnv1a(id.as_bytes()) % SHARDS
}

/// The AppData path of a shard in `dir`.
pub fn shard_path(dir: &str, shard: u32) -> String {
    format!("{dir}/{shard:02x}.json")
}

/// FNV-1a, 32 bits: a small, stable hash.
fn fnv1a(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811c_9dc5, |hash, &byte| (hash ^ byte as u32).wrapping_mul(0x0100_0193))
}

/// The shards a batch reads and changes; [`Shards::save`] writes back the
/// changed ones.
pub struct Shards<S: Store, T: Item = Entry> {
    store: S,
    loaded: BTreeMap<u32, Vec<T>>,
    changed: BTreeSet<u32>,
}

impl<S: Store, T: Item> Shards<S, T> {
    pub fn new(store: S) -> Self {
        Self { store, loaded: BTreeMap::new(), changed: BTreeSet::new() }
    }

    /// The entry of a file, if the index has one.
    pub fn get(&mut self, id: &str) -> Result<Option<&T>, S::Error> {
        Ok(self.shard(shard_of(id))?.iter().find(|entry| entry.id() == id))
    }

    /// Adds the entry, or replaces the one of the same file.
    pub fn put(&mut self, entry: T) -> Result<(), S::Error> {
        let shard = shard_of(entry.id());
        let entries = self.shard(shard)?;
        match entries.iter_mut().find(|old| old.id() == entry.id()) {
            Some(old) if *old == entry => return Ok(()),
            Some(old) => *old = entry,
            None => entries.push(entry),
        }
        self.changed.insert(shard);
        Ok(())
    }

    /// Removes a file's entry; returns it, if there was one.
    pub fn remove(&mut self, id: &str) -> Result<Option<T>, S::Error> {
        let shard = shard_of(id);
        let entries = self.shard(shard)?;
        let Some(at) = entries.iter().position(|entry| entry.id() == id) else {
            return Ok(None);
        };
        let entry = entries.remove(at);
        self.changed.insert(shard);
        Ok(Some(entry))
    }

    /// Removes every entry `drop` picks, in all shards; returns them.
    pub fn remove_where(&mut self, drop: impl Fn(&T) -> bool) -> Result<Vec<T>, S::Error> {
        let mut removed = Vec::new();
        for shard in 0..SHARDS {
            let entries = self.shard(shard)?;
            let before = entries.len();
            let (gone, kept): (Vec<T>, Vec<T>) = std::mem::take(entries).into_iter().partition(&drop);
            *entries = kept;
            if entries.len() != before {
                self.changed.insert(shard);
            }
            removed.extend(gone);
        }
        Ok(removed)
    }

    /// Changes every entry with `change`, in all shards; it says whether it
    /// changed one, so only those shards are written.
    pub fn update_all(&mut self, mut change: impl FnMut(&mut T) -> bool) -> Result<(), S::Error> {
        for shard in 0..SHARDS {
            let mut changed = false;
            for entry in self.shard(shard)? {
                changed |= change(entry);
            }
            if changed {
                self.changed.insert(shard);
            }
        }
        Ok(())
    }

    /// Writes the changed shards; an empty one is deleted.
    pub fn save(mut self) -> Result<(), S::Error> {
        for shard in std::mem::take(&mut self.changed) {
            let entries = &self.loaded[&shard];
            let path = shard_path(T::DIR, shard);
            if entries.is_empty() {
                self.store.delete(&path)?;
            } else {
                let json = serde_json::to_vec(entries).expect("entries serialize");
                self.store.write(&path, &json)?;
            }
        }
        Ok(())
    }

    fn shard(&mut self, shard: u32) -> Result<&mut Vec<T>, S::Error> {
        if !self.loaded.contains_key(&shard) {
            let entries = match self.store.read(&shard_path(T::DIR, shard))? {
                // A shard that can't be read is rebuilt from scratch rather
                // than stopping every batch that touches it.
                Some(json) => serde_json::from_slice(&json).unwrap_or_default(),
                None => Vec::new(),
            };
            self.loaded.insert(shard, entries);
        }
        Ok(self.loaded.get_mut(&shard).expect("just loaded"))
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;

    /// Files in memory, shared between the store and the test.
    #[derive(Clone, Default)]
    pub struct Memory(pub Rc<RefCell<HashMap<String, Vec<u8>>>>);

    impl Store for Memory {
        type Error = ();
        fn read(&self, path: &str) -> Result<Option<Vec<u8>>, ()> {
            Ok(self.0.borrow().get(path).cloned())
        }
        fn write(&self, path: &str, data: &[u8]) -> Result<(), ()> {
            self.0.borrow_mut().insert(path.to_owned(), data.to_vec());
            Ok(())
        }
        fn delete(&self, path: &str) -> Result<(), ()> {
            self.0.borrow_mut().remove(path);
            Ok(())
        }
    }

    fn entry(id: &str, root: &str) -> Entry {
        Entry { id: id.into(), root: root.into(), name: format!("{id}.jpg"), ..Entry::default() }
    }

    #[test]
    fn shards_are_stable_and_in_range() {
        assert_eq!(shard_of("0a1b"), shard_of("0a1b"));
        let ids = (0..1000).map(|n| format!("{n:08x}-0000-4000-8000-000000000000"));
        let used: BTreeSet<u32> = ids.map(|id| shard_of(&id)).collect();
        assert!(used.iter().all(|&s| s < SHARDS));
        // Ids spread over (nearly) all shards.
        assert!(used.len() > 60, "{} shards used", used.len());
        assert_eq!(shard_path(DIR, 10), "index/0a.json");
    }

    #[test]
    fn entries_survive_a_save() {
        let memory = Memory::default();
        let mut shards: Shards<_> = Shards::new(memory.clone());
        shards.put(entry("a", "r")).unwrap();
        shards.put(entry("b", "r")).unwrap();
        shards.save().unwrap();

        let mut shards: Shards<_> = Shards::new(memory.clone());
        assert_eq!(shards.get("a").unwrap().unwrap().name, "a.jpg");
        let mut changed = entry("a", "r");
        changed.name = "renamed.jpg".into();
        shards.put(changed).unwrap();
        assert_eq!(shards.get("a").unwrap().unwrap().name, "renamed.jpg");
        assert!(shards.get("c").unwrap().is_none());
    }

    #[test]
    fn only_changed_shards_are_written() {
        let memory = Memory::default();
        let mut shards: Shards<_> = Shards::new(memory.clone());
        shards.put(entry("a", "r")).unwrap();
        shards.save().unwrap();
        let path = shard_path(DIR, shard_of("a"));
        memory.0.borrow_mut().insert(path.clone(), b"[]".to_vec()); // to see a rewrite

        let mut shards: Shards<_> = Shards::new(memory.clone());
        assert!(shards.get("a").unwrap().is_none());
        shards.put(entry("a", "r")).unwrap();
        shards.save().unwrap();
        assert_ne!(memory.0.borrow()[&path], b"[]".to_vec());

        // The same entry again changes nothing.
        memory.0.borrow_mut().insert(path.clone(), serde_json::to_vec(&[entry("a", "r")]).unwrap());
        let mut shards: Shards<_> = Shards::new(memory.clone());
        shards.put(entry("a", "r")).unwrap();
        assert!(shards.changed.is_empty());
    }

    #[test]
    fn empty_shards_are_deleted() {
        let memory = Memory::default();
        let mut shards: Shards<_> = Shards::new(memory.clone());
        shards.put(entry("a", "r")).unwrap();
        shards.save().unwrap();
        let mut shards: Shards<_> = Shards::new(memory.clone());
        assert_eq!(shards.remove("a").unwrap().unwrap().id, "a");
        assert!(shards.remove("a").unwrap().is_none());
        shards.save().unwrap();
        assert!(memory.0.borrow().is_empty());
    }

    #[test]
    fn remove_where_goes_through_all_shards() {
        let memory = Memory::default();
        let mut shards: Shards<_> = Shards::new(memory.clone());
        for n in 0..200 {
            shards.put(entry(&format!("f{n}"), if n % 2 == 0 { "kept" } else { "gone" })).unwrap();
        }
        shards.save().unwrap();
        let mut shards: Shards<_> = Shards::new(memory.clone());
        let removed = shards.remove_where(|e| e.root == "gone").unwrap();
        assert_eq!(removed.len(), 100);
        shards.save().unwrap();
        let mut shards: Shards<_> = Shards::new(memory);
        assert!(shards.get("f0").unwrap().is_some());
        assert!(shards.get("f1").unwrap().is_none());
    }

    #[test]
    fn a_broken_shard_starts_empty() {
        let memory = Memory::default();
        memory.0.borrow_mut().insert(shard_path(DIR, shard_of("a")), b"not json".to_vec());
        let mut shards: Shards<_> = Shards::new(memory);
        assert!(shards.get("a").unwrap().is_none());
    }
}
