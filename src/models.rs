//! The models of photo analysis (`clip`, `ocr`) and the administrator's
//! switches for it.
//!
//! The models are files of the plugin's storage, downloaded by the server
//! from the manifest's `[[downloads]]` — one copy for the whole server. To
//! use another model, change its `[[downloads]]` block (url, sha256, size)
//! and build again: the path stays, the tag below changes with the sha256,
//! and every photo is analysed again. A model that isn't SigLIP may need
//! another [`CLIP_MEAN`] and [`CLIP_STD`].

use crate::rustcloud::app::settings;

/// SigLIP 2 B/16, its picture half: a vector per photo.
pub const CLIP: &str = "models/clip-visual.onnx";
/// PP-OCRv5 mobile: where text is (a map of the picture).
pub const OCR_DETECTOR: &str = "models/ocr-det.onnx";
/// PP-OCRv5 mobile, Cyrillic and Latin: the text of a line.
pub const OCR_RECOGNIZER: &str = "models/ocr-rec.onnx";
/// The recognizer's characters (`PostProcess.character_dict`).
pub const OCR_DICTIONARY: &str = "models/ocr-rec.yml";

/// What [`CLIP`] takes: values 0–255 become `(v - mean) / std`. SigLIP's
/// are 127.5 and 127.5 (-1…1); OpenAI CLIP's would be 122.77, 116.75,
/// 104.09 and 68.50, 66.63, 70.32.
pub const CLIP_MEAN: (f32, f32, f32) = (127.5, 127.5, 127.5);
pub const CLIP_STD: (f32, f32, f32) = (127.5, 127.5, 127.5);

/// Raised when what is made of a model's output changes (preparing the
/// picture, reading the output): every photo is analysed again.
const CLIP_VERSION: u32 = 1;
const OCR_VERSION: u32 = 1;

const MANIFEST: &str = include_str!("../manifest.toml");

/// What the vectors of photos were made by: the model's file and our
/// handling of it. Vectors of different tags can't be compared.
pub fn clip_tag() -> String {
    tag(&[CLIP], CLIP_VERSION)
}

pub fn ocr_tag() -> String {
    tag(&[OCR_DETECTOR, OCR_RECOGNIZER, OCR_DICTIONARY], OCR_VERSION)
}

fn tag(paths: &[&str], version: u32) -> String {
    let hashes: Vec<String> =
        paths.iter().map(|path| download_sha256(MANIFEST, path).unwrap_or("none").chars().take(12).collect()).collect();
    format!("{}.{version}", hashes.join("."))
}

/// The sha256 of the `[[downloads]]` block with this path.
fn download_sha256<'a>(manifest: &'a str, path: &str) -> Option<&'a str> {
    manifest.split("[[downloads]]").skip(1).find_map(|block| {
        let block = block.split("\n[").next().unwrap_or(block);
        let value = |key: &str| {
            block.lines().find_map(|line| {
                let (name, value) = line.split_once('=')?;
                (name.trim() == key).then(|| value.trim().trim_matches('"'))
            })
        };
        (value("path") == Some(path)).then(|| value("sha256")).flatten()
    })
}

/// The administrator's switches (`[[server_settings]]`, on by default).
pub struct Switches {
    /// Photo analysis: similar photos, text in photos.
    pub vision: bool,
    pub stories: bool,
}

impl Switches {
    pub fn read() -> Self {
        Self::from_json(&settings::values())
    }

    fn from_json(values: &str) -> Self {
        let values: serde_json::Value = serde_json::from_str(values).unwrap_or_default();
        // The manifest's defaults are `true`; an older server sends nothing.
        let on = |key: &str| values.get(key).and_then(|value| value.as_bool()).unwrap_or(true);
        Self { vision: on("vision"), stories: on("stories") }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_come_from_the_manifest() {
        let manifest = "id = \"x\"\n\n[[downloads]]\npath = \"models/a.onnx\"\nurl = \"https://h/a\"\n\
                        sha256 = \"0123456789abcdef\"\n\n[[downloads]]\npath = \"models/b.onnx\"\n\
                        sha256 = \"fedcba9876543210\"\n\n[limits]\nsha256 = \"no\"\n";
        assert_eq!(download_sha256(manifest, "models/a.onnx"), Some("0123456789abcdef"));
        assert_eq!(download_sha256(manifest, "models/b.onnx"), Some("fedcba9876543210"));
        assert_eq!(download_sha256(manifest, "models/c.onnx"), None);
    }

    #[test]
    fn the_real_manifest_has_every_model() {
        for path in [CLIP, OCR_DETECTOR, OCR_RECOGNIZER, OCR_DICTIONARY] {
            let sha = download_sha256(MANIFEST, path).unwrap_or_else(|| panic!("{path} is not downloaded"));
            assert_eq!(sha.len(), 64, "{path}");
        }
        assert!(!clip_tag().contains("none") && !ocr_tag().contains("none"));
        assert_ne!(clip_tag(), ocr_tag());
    }

    #[test]
    fn switches_are_on_unless_turned_off() {
        let on = Switches::from_json("{}");
        assert!(on.vision && on.stories);
        let off = Switches::from_json(r#"{"vision": false, "stories": true}"#);
        assert!(!off.vision && off.stories);
        assert!(Switches::from_json("not json").vision);
    }
}
