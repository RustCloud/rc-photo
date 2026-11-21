//! The `ocr` job: the text in each photo, through the server's `use-onnx`
//! and PaddleOCR's PP-OCRv5 mobile models.
//!
//! 1. The server gives the photo at most [`WORK_SIDE`] px long, sides
//!    rounded to 32 (what the detector takes), values 0–255, BGR.
//! 2. The detector gives a map: how likely each pixel is text. Pixels above
//!    [`MAP_THRESHOLD`] that touch make a line ([`text_boxes`]), grown a
//!    little as the detector was trained ("unclip").
//! 3. Each line, cut out and made [`LINE_HEIGHT`] px high, goes to the
//!    recognizer, a few at once; its output is read the CTC way with the
//!    model's own characters ([`decode`]).
//!
//! Only lines across the picture are read for now (not upright ones).

use crate::analysis::{self, OcrFile, Part};
use crate::clip::{floats, run_failed, Look};
use crate::index::{Entry, Shards};
use crate::media::{later, outcome, AppData, Problem};
use crate::models::{self, OCR_DETECTOR, OCR_DICTIONARY, OCR_RECOGNIZER};
use crate::rustcloud::app::jobs::{Job, Outcome};
use crate::rustcloud::app::storage;
use crate::rustcloud::app::use_onnx::{
    self, Channels, ImageFit, ImageOptions, Layout, NamedTensor, Source, Tensor, TensorType,
};
use crate::vision::{self, Picture};

const WORK_SIDE: u32 = 960;
const MAP_THRESHOLD: f32 = 0.3;
/// Least average likelihood of a line's pixels.
const BOX_THRESHOLD: f32 = 0.6;
/// How much a line grows: its area × this / its perimeter, on each side.
const UNCLIP: f32 = 1.5;
const MIN_BOX_SIDE: f32 = 3.0;
const MOST_LINES: usize = 40;
const LINE_HEIGHT: usize = 48;
const MOST_LINE_WIDTH: usize = 1600;
const LINES_AT_ONCE: usize = 6;
/// Least average likelihood of a line's characters.
const MIN_LINE_SCORE: f32 = 0.6;
const MOST_LINE_CHARS: usize = 200;
/// The detector's normalization (ImageNet), in the order of the picture's
/// planes: PaddleOCR applies it to BGR pictures as they are.
const DET_MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const DET_STD: [f32; 3] = [0.229, 0.224, 0.225];

/// Runs a batch of `ocr` jobs: the keys are file ids.
pub fn run_ocr(jobs: &[Job]) -> Vec<Outcome> {
    let done = || jobs.iter().map(|_| Outcome::Done).collect();
    if !analysis::may_run(Part::Ocr) {
        return done();
    }
    if let Some(why) = analysis::failed_lately(Part::Ocr) {
        return jobs.iter().map(|_| Outcome::Failed(why.clone())).collect();
    }
    let models = match Models::new() {
        Ok(models) => models,
        Err(Look::ModelFailed(why)) => {
            analysis::model_failed(Part::Ocr, &why);
            return jobs.iter().map(|_| Outcome::Failed(why.clone())).collect();
        }
        Err(Look::Photo(problem)) => return jobs.iter().map(|_| outcome(Err(problem.clone()))).collect(),
    };
    let tag = models::ocr_tag();
    let mut index: Shards<AppData> = Shards::new(AppData);
    let mut texts: Shards<AppData, OcrFile> = Shards::new(AppData);
    let mut results = Vec::with_capacity(jobs.len());
    for job in jobs {
        match look(&job.key, &models, &tag, &mut index, &mut texts) {
            Err(Look::ModelFailed(why)) => {
                analysis::model_failed(Part::Ocr, &why);
                results.extend(std::iter::repeat_n(Err(Problem::Broken(why)), jobs.len() - results.len()));
                break;
            }
            Err(Look::Photo(problem)) => results.push(Err(problem)),
            Ok(()) => results.push(Ok(())),
        }
    }
    if let Err(err) = texts.save() {
        return jobs.iter().map(|_| Outcome::Retry(format!("ocr: {err:?}"))).collect();
    }
    results.into_iter().map(outcome).collect()
}

struct Models {
    detector: String,
    recognizer: String,
    characters: Vec<String>,
}

impl Models {
    fn new() -> Result<Self, Look> {
        let input = |model: &str| -> Result<String, Look> {
            let info = use_onnx::get_model_info(model).map_err(run_failed)?;
            info.inputs
                .into_iter()
                .next()
                .map(|input| input.name)
                .ok_or_else(|| Look::ModelFailed(format!("{model} takes no input")))
        };
        let file = storage::stat(OCR_DICTIONARY).map_err(|err| Look::Photo(Problem::Later(format!("{err:?}"))))?;
        let yaml = storage::read(OCR_DICTIONARY, 0, file.size)
            .map_err(|err| Look::Photo(Problem::Later(format!("{err:?}"))))?;
        let characters = dictionary(&String::from_utf8_lossy(&yaml));
        if characters.is_empty() {
            return Err(Look::ModelFailed(format!("{OCR_DICTIONARY} has no character_dict")));
        }
        Ok(Self { detector: input(OCR_DETECTOR)?, recognizer: input(OCR_RECOGNIZER)?, characters })
    }
}

fn look(
    id: &str,
    models: &Models,
    tag: &str,
    index: &mut Shards<AppData>,
    texts: &mut Shards<AppData, OcrFile>,
) -> Result<(), Look> {
    let entry: Option<Entry> = index.get(id).map_err(later)?.cloned();
    let Some(entry) = entry.filter(|entry| !entry.video && entry.thumb && entry.width > 0 && entry.height > 0) else {
        texts.remove(id).map_err(later)?;
        return Ok(());
    };
    if texts.get(id).map_err(later)?.is_some_and(|known| known.etag == entry.etag && known.m == tag) {
        return Ok(());
    }
    let record = |t: Vec<String>| OcrFile { id: id.into(), etag: entry.etag.clone(), m: tag.into(), t };
    let (width, height) = detector_size(entry.width, entry.height);
    let options = ImageOptions {
        width,
        height,
        fit: ImageFit::Stretch,
        crop: None,
        channels: Channels::Bgr,
        layout: Layout::Nchw,
        mean: (0.0, 0.0, 0.0),
        std: (1.0, 1.0, 1.0),
    };
    let input = match use_onnx::image_tensor(&Source::File(id.to_owned()), options) {
        Ok(input) => input,
        Err(use_onnx::Error::Unsupported(why) | use_onnx::Error::InvalidInput(why)) => {
            texts.put(record(Vec::new())).map_err(later)?;
            return Err(Look::Photo(Problem::Broken(why)));
        }
        Err(err) => return Err(Look::Photo(Problem::Later(format!("{err:?}")))),
    };
    let picture = Picture::from_tensor(width as usize, height as usize, &input.tensor.data)
        .ok_or_else(|| Look::Photo(Problem::Broken("the picture is not of the size asked for".into())))?;
    let lines = read(&picture, models)?;
    texts.put(record(lines)).map_err(later)?;
    Ok(())
}

/// The detector's input size: within [`WORK_SIDE`], sides of 32s.
fn detector_size(width: u32, height: u32) -> (u32, u32) {
    let (w, h) = vision::fit_inside(width, height, WORK_SIDE);
    let round = |v: u32| (((v + 16) / 32) * 32).max(32);
    (round(w), round(h))
}

/// The lines of text in a picture, top to bottom.
fn read(picture: &Picture, models: &Models) -> Result<Vec<String>, Look> {
    let (w, h) = (picture.width, picture.height);
    let plane = w * h;
    let mut input = Vec::with_capacity(3 * plane);
    for channel in 0..3 {
        input.extend(
            picture.data[channel * plane..(channel + 1) * plane]
                .iter()
                .map(|v| (v / 255.0 - DET_MEAN[channel]) / DET_STD[channel]),
        );
    }
    let map = run(OCR_DETECTOR, &models.detector, vec![1, 3, h as u32, w as u32], &input)?;
    if map.len() != plane {
        return Err(Look::ModelFailed(format!("{OCR_DETECTOR} gave {} values for {w}×{h}", map.len())));
    }
    let boxes = text_boxes(&map, w, h);
    let mut lines = Vec::new();
    for group in boxes.chunks(LINES_AT_ONCE) {
        let crops: Vec<Line> = group.iter().map(|b| line_crop(picture, b)).collect();
        let widest = crops.iter().map(|c| c.width).max().unwrap_or(0);
        if widest == 0 {
            continue;
        }
        // Lines of a run share its width: the narrower ones end in zeros (grey).
        let mut input = vec![0.0; crops.len() * 3 * LINE_HEIGHT * widest];
        for (n, crop) in crops.iter().enumerate() {
            for channel in 0..3 {
                for y in 0..LINE_HEIGHT {
                    let to = ((n * 3 + channel) * LINE_HEIGHT + y) * widest;
                    let from = (channel * LINE_HEIGHT + y) * crop.width;
                    input[to..to + crop.width].copy_from_slice(&crop.data[from..from + crop.width]);
                }
            }
        }
        let shape = vec![crops.len() as u32, 3, LINE_HEIGHT as u32, widest as u32];
        let output = run_shaped(OCR_RECOGNIZER, &models.recognizer, shape, &input)?;
        let [count, steps, classes] = output.shape[..] else {
            return Err(Look::ModelFailed(format!("{OCR_RECOGNIZER} gave the shape {:?}", output.shape)));
        };
        let values = floats(&output.tensor.data);
        for n in 0..(count as usize).min(crops.len()) {
            let at = n * (steps * classes) as usize;
            let rows = &values[at..at + (steps * classes) as usize];
            let (text, score) = decode(rows, classes as usize, &models.characters);
            let text = text.trim().chars().take(MOST_LINE_CHARS).collect::<String>();
            if score >= MIN_LINE_SCORE && text.chars().filter(|c| c.is_alphanumeric()).count() >= 2 {
                lines.push(text);
            }
        }
    }
    Ok(lines)
}

struct Shaped {
    shape: Vec<u32>,
    tensor: Tensor,
}

fn run(model: &str, input: &str, shape: Vec<u32>, values: &[f32]) -> Result<Vec<f32>, Look> {
    run_shaped(model, input, shape, values).map(|output| floats(&output.tensor.data))
}

fn run_shaped(model: &str, input: &str, shape: Vec<u32>, values: &[f32]) -> Result<Shaped, Look> {
    let tensor = Tensor { dtype: TensorType::F32, shape, data: values.iter().flat_map(|v| v.to_le_bytes()).collect() };
    let outputs = use_onnx::run(model, &[NamedTensor { name: input.into(), tensor }]).map_err(run_failed)?;
    let output = outputs.into_iter().next().ok_or_else(|| Look::ModelFailed(format!("{model} gave nothing")))?;
    Ok(Shaped { shape: output.tensor.shape.clone(), tensor: output.tensor })
}

/// A line of text in the detector's picture: left, top, right, bottom
/// (pixels, right and bottom past the end).
#[derive(Clone, Debug, PartialEq)]
pub struct TextBox {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub score: f32,
}

/// The lines in the detector's map, top to bottom, left to right.
pub fn text_boxes(map: &[f32], width: usize, height: usize) -> Vec<TextBox> {
    let mut seen = vec![false; map.len()];
    let mut boxes = Vec::new();
    let mut stack = Vec::new();
    for start in 0..map.len() {
        if seen[start] || map[start] <= MAP_THRESHOLD {
            continue;
        }
        // The pixels above the threshold that touch this one.
        let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
        let (mut count, mut sum) = (0usize, 0.0f32);
        seen[start] = true;
        stack.push(start);
        while let Some(at) = stack.pop() {
            let (x, y) = (at % width, at / width);
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
            count += 1;
            sum += map[at];
            let mut visit = |next: usize| {
                if !seen[next] && map[next] > MAP_THRESHOLD {
                    seen[next] = true;
                    stack.push(next);
                }
            };
            if x > 0 {
                visit(at - 1);
            }
            if x + 1 < width {
                visit(at + 1);
            }
            if y > 0 {
                visit(at - width);
            }
            if y + 1 < height {
                visit(at + width);
            }
        }
        let score = sum / count as f32;
        let (w, h) = ((x1 - x0 + 1) as f32, (y1 - y0 + 1) as f32);
        if score < BOX_THRESHOLD || w.min(h) < MIN_BOX_SIDE {
            continue;
        }
        // Upright text isn't read yet.
        if h > 1.5 * w {
            continue;
        }
        let grow = w * h * UNCLIP / (2.0 * (w + h));
        boxes.push(TextBox {
            left: (x0 as f32 - grow).max(0.0),
            top: (y0 as f32 - grow).max(0.0),
            right: (x1 as f32 + 1.0 + grow).min(width as f32),
            bottom: (y1 as f32 + 1.0 + grow).min(height as f32),
            score,
        });
    }
    // The largest lines, if there are too many; read top to bottom.
    boxes.sort_by(|a, b| area(b).total_cmp(&area(a)));
    boxes.truncate(MOST_LINES);
    boxes.sort_by(|a, b| (a.top, a.left).partial_cmp(&(b.top, b.left)).unwrap_or(std::cmp::Ordering::Equal));
    boxes
}

fn area(b: &TextBox) -> f32 {
    (b.right - b.left) * (b.bottom - b.top)
}

/// A line made the recognizer's input: [`LINE_HEIGHT`] high, values -1…1.
struct Line {
    width: usize,
    /// Three planes, row by row.
    data: Vec<f32>,
}

fn line_crop(picture: &Picture, b: &TextBox) -> Line {
    let (w, h) = (b.right - b.left, b.bottom - b.top);
    let width = ((w * LINE_HEIGHT as f32 / h).ceil() as usize).clamp(8, MOST_LINE_WIDTH);
    let mut data = vec![0.0; 3 * LINE_HEIGHT * width];
    let (step_x, step_y) = (w / width as f32, h / LINE_HEIGHT as f32);
    for channel in 0..3 {
        for y in 0..LINE_HEIGHT {
            for x in 0..width {
                let sx = b.left + (x as f32 + 0.5) * step_x - 0.5;
                let sy = b.top + (y as f32 + 0.5) * step_y - 0.5;
                let clamp = |v: f32, most: usize| v.clamp(0.0, (most - 1) as f32);
                let value = picture.sample(channel, clamp(sx, picture.width), clamp(sy, picture.height)).unwrap_or(0.0);
                data[(channel * LINE_HEIGHT + y) * width + x] = value / 127.5 - 1.0;
            }
        }
    }
    Line { width, data }
}

/// The text of one line from the recognizer's output (`steps × classes`):
/// the likeliest class at each step; repeats and blanks (class 0) dropped;
/// class `k` is character `k - 1`, past the last one a space. And how
/// likely the characters were, on average.
pub fn decode(rows: &[f32], classes: usize, characters: &[String]) -> (String, f32) {
    let mut text = String::new();
    let mut scores = Vec::new();
    let mut last = 0;
    for row in rows.chunks(classes) {
        let probabilities = softmax_if_needed(row);
        let (best, score) =
            probabilities
                .iter()
                .enumerate()
                .fold((0, f32::MIN), |found, (k, &p)| if p > found.1 { (k, p) } else { found });
        if best != 0 && best != last {
            text.push_str(characters.get(best - 1).map_or(" ", String::as_str));
            scores.push(score);
        }
        last = best;
    }
    let score = if scores.is_empty() { 0.0 } else { scores.iter().sum::<f32>() / scores.len() as f32 };
    (text, score)
}

/// The recognizer gives likelihoods; should another give raw scores, they
/// are made likelihoods here.
fn softmax_if_needed(row: &[f32]) -> Vec<f32> {
    let sum: f32 = row.iter().sum();
    if row.iter().all(|&v| (0.0..=1.0).contains(&v)) && (sum - 1.0).abs() < 0.05 {
        return row.to_vec();
    }
    let most = row.iter().fold(f32::MIN, |m, &v| m.max(v));
    let exps: Vec<f32> = row.iter().map(|v| (v - most).exp()).collect();
    let total: f32 = exps.iter().sum();
    exps.iter().map(|v| v / total).collect()
}

/// The characters of `PostProcess.character_dict` in PaddleOCR's
/// `inference.yml`: a YAML list, one item a line, plain or quoted.
pub fn dictionary(yaml: &str) -> Vec<String> {
    let mut lines = yaml.lines().skip_while(|line| line.trim() != "character_dict:").skip(1);
    let mut characters = Vec::new();
    for line in lines.by_ref() {
        let Some(item) = line.trim_start().strip_prefix("- ") else { break };
        let item = item.trim_end();
        let value = if let Some(quoted) = item.strip_prefix('\'').and_then(|i| i.strip_suffix('\'')) {
            quoted.replace("''", "'")
        } else if item.starts_with('"') {
            serde_json::from_str::<String>(item).unwrap_or_else(|_| item.trim_matches('"').to_owned())
        } else {
            item.to_owned()
        };
        characters.push(value);
    }
    characters
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dictionary_of_paddleocr() {
        let yaml = "PostProcess:\n  name: CTCLabelDecode\n  character_dict:\n  - '!'\n  - ''''\n  - \"\\\\\"\n  - а\n  - Б\n  - '1'\nOther: x\n";
        assert_eq!(dictionary(yaml), ["!", "'", "\\", "а", "Б", "1"]);
        assert!(dictionary("no list here").is_empty());
    }

    #[test]
    fn ctc_drops_repeats_and_blanks() {
        let characters: Vec<String> = ["к", "о", "т"].iter().map(|c| c.to_string()).collect();
        // classes: blank, к, о, т, space
        let step = |k: usize| {
            let mut row = vec![0.0; 5];
            row[k] = 1.0;
            row
        };
        let rows: Vec<f32> = [1, 1, 0, 2, 3, 3, 4, 1].into_iter().flat_map(step).collect();
        let (text, score) = decode(&rows, 5, &characters);
        assert_eq!(text, "кот к");
        assert!((score - 1.0).abs() < 1e-6);
        // Raw scores are made likelihoods.
        let raw: Vec<f32> = [0.0, 5.0, 0.0, 0.0, 0.0].to_vec();
        let (text, score) = decode(&raw, 5, &characters);
        assert_eq!(text, "к");
        assert!(score > 0.9 && score < 1.0);
    }

    #[test]
    fn boxes_around_lines() {
        let (w, h) = (100, 40);
        let mut map = vec![0.0; w * h];
        // A line: x 10–49, y 5–14; a dot too small; an upright one.
        for y in 5..15 {
            for x in 10..50 {
                map[y * w + x] = 0.9;
            }
        }
        map[30 * w + 80] = 0.9;
        for y in 20..40 {
            for x in 90..94 {
                map[y * w + x] = 0.9;
            }
        }
        let boxes = text_boxes(&map, w, h);
        assert_eq!(boxes.len(), 1, "{boxes:?}");
        let b = &boxes[0];
        // 40 × 10 grows by 40·10·1.5 / (2·(40 + 10)) = 6 on each side, within the map.
        assert_eq!((b.left, b.top, b.right, b.bottom), (4.0, 0.0, 56.0, 21.0));
        assert!((b.score - 0.9).abs() < 1e-4);
    }

    #[test]
    fn weak_lines_are_dropped() {
        let (w, h) = (20, 10);
        let mut map = vec![0.0; w * h];
        for y in 2..8 {
            for x in 2..18 {
                map[y * w + x] = 0.4;
            }
        }
        assert!(text_boxes(&map, w, h).is_empty());
    }

    #[test]
    fn the_detector_takes_sides_of_32() {
        assert_eq!(detector_size(4000, 3000), (960, 736));
        assert_eq!(detector_size(100, 50), (96, 64));
        assert_eq!(detector_size(10, 10), (32, 32));
    }

    #[test]
    fn a_line_is_48_high() {
        let picture = Picture { width: 4, height: 2, data: vec![255.0; 24] };
        let line = line_crop(&picture, &TextBox { left: 0.0, top: 0.0, right: 4.0, bottom: 2.0, score: 1.0 });
        assert_eq!(line.width, 96);
        assert_eq!(line.data.len(), 3 * 48 * 96);
        assert!(line.data.iter().all(|&v| (v - 1.0).abs() < 1e-6));
    }
}
