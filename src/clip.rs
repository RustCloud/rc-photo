//! The `clip` job: a vector of each photo from SigLIP 2's picture half,
//! through the server's `use-onnx`. The server makes the model's input
//! itself (`image-tensor`: squashed to the model's square, values -1…1), so
//! the plugin only runs the model and keeps its output, quantized.

use crate::analysis::{self, ClipFile, Part};
use crate::index::{Entry, Shards};
use crate::media::{later, outcome, AppData, Problem};
use crate::models::{self, CLIP, CLIP_MEAN, CLIP_STD};
use crate::rustcloud::app::jobs::{Job, Outcome};
use crate::rustcloud::app::use_onnx::{self, Channels, ImageFit, ImageOptions, Layout, NamedTensor, Source};
use crate::vision;

/// SigLIP 2 B/16's side, if the model doesn't say.
const DEFAULT_SIDE: u32 = 224;

/// Runs a batch of `clip` jobs: the keys are file ids.
pub fn run_clip(jobs: &[Job]) -> Vec<Outcome> {
    let done = || jobs.iter().map(|_| Outcome::Done).collect();
    if !analysis::may_run(Part::Clip) {
        return done();
    }
    if let Some(why) = analysis::failed_lately(Part::Clip) {
        return jobs.iter().map(|_| Outcome::Failed(why.clone())).collect();
    }
    let model = match Model::new() {
        Ok(model) => model,
        Err(problem) => return jobs.iter().map(|_| outcome(Err(problem.clone()))).collect(),
    };
    let tag = models::clip_tag();
    let mut index: Shards<AppData> = Shards::new(AppData);
    let mut clips: Shards<AppData, ClipFile> = Shards::new(AppData);
    let mut results = Vec::with_capacity(jobs.len());
    for job in jobs {
        match look(&job.key, &model, &tag, &mut index, &mut clips) {
            Err(Look::ModelFailed(why)) => {
                // The model, not the photo: the part stops for a while.
                analysis::model_failed(Part::Clip, &why);
                results.extend(std::iter::repeat_n(Err(Problem::Broken(why)), jobs.len() - results.len()));
                break;
            }
            Err(Look::Photo(problem)) => results.push(Err(problem)),
            Ok(()) => results.push(Ok(())),
        }
    }
    // Nothing counts as done until the vectors are written.
    if let Err(err) = clips.save() {
        return jobs.iter().map(|_| Outcome::Retry(format!("clip: {err:?}"))).collect();
    }
    results.into_iter().map(outcome).collect()
}

/// The model's input name and side.
struct Model {
    input: String,
    side: u32,
}

impl Model {
    fn new() -> Result<Self, Problem> {
        let info = use_onnx::get_model_info(CLIP).map_err(|err| Problem::Later(format!("{err:?}")))?;
        let input = info.inputs.into_iter().next().ok_or_else(|| Problem::Broken(format!("{CLIP} takes no input")))?;
        // [1, 3, side, side]
        let side = input.shape.as_ref().and_then(|shape| shape.get(2).copied().flatten()).unwrap_or(DEFAULT_SIDE);
        Ok(Self { input: input.name, side })
    }
}

pub enum Look {
    /// This photo: kept as failed, or tried again.
    Photo(Problem),
    /// The model can't be loaded or run here.
    ModelFailed(String),
}

impl From<Problem> for Look {
    fn from(problem: Problem) -> Self {
        Look::Photo(problem)
    }
}

/// Brings one photo's vector up to date.
fn look(
    id: &str,
    model: &Model,
    tag: &str,
    index: &mut Shards<AppData>,
    clips: &mut Shards<AppData, ClipFile>,
) -> Result<(), Look> {
    let entry: Option<Entry> = index.get(id).map_err(later)?.cloned();
    let Some(entry) = entry.filter(|entry| !entry.video && entry.thumb) else {
        clips.remove(id).map_err(later)?;
        return Ok(());
    };
    if clips.get(id).map_err(later)?.is_some_and(|known| known.etag == entry.etag && known.m == tag) {
        return Ok(());
    }
    let record = |v: String| ClipFile { id: id.into(), etag: entry.etag.clone(), m: tag.into(), v };
    let options = ImageOptions {
        width: model.side,
        height: model.side,
        fit: ImageFit::Stretch,
        crop: None,
        channels: Channels::Rgb,
        layout: Layout::Nchw,
        mean: CLIP_MEAN,
        std: CLIP_STD,
    };
    let input = match use_onnx::image_tensor(&Source::File(id.to_owned()), options) {
        Ok(input) => input,
        Err(use_onnx::Error::Unsupported(why) | use_onnx::Error::InvalidInput(why)) => {
            // A picture that can't be read: kept, so it isn't tried again.
            clips.put(record(String::new())).map_err(later)?;
            return Err(Look::Photo(Problem::Broken(why)));
        }
        Err(err) => return Err(Look::Photo(Problem::Later(format!("{err:?}")))),
    };
    let outputs = match use_onnx::run(CLIP, &[NamedTensor { name: model.input.clone(), tensor: input.tensor }]) {
        Ok(outputs) => outputs,
        Err(err) => return Err(run_failed(err)),
    };
    let mut vector = outputs.into_iter().next().map(|output| floats(&output.tensor.data)).unwrap_or_default();
    if vector.is_empty() {
        return Err(Look::ModelFailed(format!("{CLIP} gave no vector")));
    }
    vision::normalize(&mut vector);
    clips.put(record(analysis::encode(&vision::quantize(&vector)))).map_err(later)?;
    Ok(())
}

/// A failed run: tried again if the server is busy, else the model failed.
pub fn run_failed(err: use_onnx::Error) -> Look {
    use use_onnx::Error::*;
    match err {
        Busy | Timeout | NoUser | NotAllowed | NotFound => Look::Photo(Problem::Later(format!("{err:?}"))),
        Failed(why) | Unsupported(why) | InvalidInput(why) => Look::ModelFailed(why),
    }
}

pub fn floats(bytes: &[u8]) -> Vec<f32> {
    bytes.as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes(*b)).collect()
}
