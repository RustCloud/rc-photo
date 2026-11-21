//! Search by description (since 0.8.0): "кот на диване", "snow", "торт"
//! find photos by what is in them. SigLIP 2's text half makes a vector of
//! the words, and it is compared with the vectors of photos made by its
//! picture half ([`crate::clip`]). Any language the model knows works:
//! its vocabulary is Gemma's, 256 000 pieces.
//!
//! The text model is large (1.1 GB, about 2.6 GB of memory while it
//! loads), more than the server's `[onnx]` limits allow by default:
//! `max_model_mib = 1200` and `memory_mib = 3072` are needed. Whether it
//! loads shows only when it is tried, so a `describe` job tries it once:
//! it also turns the model's `tokenizer.json` (34 MB) into a small sorted
//! vocabulary of the plugin's storage, which each search reads.
//!
//! Tokens are made as SentencePiece BPE does: the text's characters (bytes
//! `<0xNN>` for those the vocabulary lacks), then again and again the
//! neighbours whose join is a piece of the vocabulary, the piece of the
//! lowest number first. For this tokenizer that gives what its merges give
//! (checked on 6000 Russian and English texts against `tokenizers`).

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::json;

use crate::analysis::{self, ClipFile, Part};
use crate::clip::{floats, run_failed, Look};
use crate::index::SHARDS;
use crate::media::Problem;
use crate::models::{self, TEXT, TEXT_TOKENIZER};
use crate::rustcloud::app::jobs::{Job, Outcome};
use crate::rustcloud::app::use_onnx::{NamedTensor, Tensor, TensorType};
use crate::rustcloud::app::{storage, use_onnx};
use crate::web::{error, ok};
use crate::Response;

/// Least cosine of a photo found by description. Words and pictures are
/// never close in SigLIP: a cat and "кот" are about 0.10, unrelated
/// photos below 0.05.
const LEAST: f32 = 0.05;
/// ...and at least this share of the best photo's: a query that matches
/// one photo well doesn't bring the ones it barely matches.
const SHARE_OF_BEST: f32 = 0.6;
/// Most photos a search gives.
const MOST: usize = 500;
/// Tokens the model takes: the text's, then `<eos>`, then `<pad>`.
const DEFAULT_LENGTH: usize = 64;
/// Longer queries are cut: the model reads 63 tokens anyway.
const MOST_CHARS: usize = 300;
/// What a vocabulary file starts with.
const MAGIC: &[u8; 4] = b"RCV1";

/// The vocabulary file made from [`TEXT_TOKENIZER`].
fn vocabulary_path() -> String {
    format!("cache/text-vocabulary-{}.bin", models::text_tag())
}

// ── Vocabulary ─────────────────────────────────────────────────────────

/// The pieces of the vocabulary, sorted by their bytes, with their numbers.
///
/// File: `RCV1`, then u32 little-endian: count, `<eos>`, `<pad>`, `<unk>`,
/// then for each piece its offset in the text and its number, then the end
/// of the text, then the text: the pieces one after another.
pub struct Vocabulary {
    data: Vec<u8>,
    count: usize,
    pub eos: u32,
    pub pad: u32,
    unk: u32,
}

#[derive(Deserialize)]
struct TokenizerFile {
    model: TokenizerModel,
    #[serde(default)]
    added_tokens: Vec<AddedToken>,
}

#[derive(Deserialize)]
struct TokenizerModel {
    vocab: HashMap<String, u32>,
}

#[derive(Deserialize)]
struct AddedToken {
    content: String,
    #[serde(default)]
    special: bool,
}

impl Vocabulary {
    /// The vocabulary file from a `tokenizer.json`. Special tokens (`<eos>`,
    /// `<mask>`…) are left out: typed text never becomes one.
    pub fn build(tokenizer_json: &[u8]) -> Result<Vec<u8>, String> {
        let file: TokenizerFile = serde_json::from_slice(tokenizer_json).map_err(|err| format!("tokenizer: {err}"))?;
        let number = |piece: &str| file.model.vocab.get(piece).copied().ok_or(format!("tokenizer: no {piece}"));
        let (eos, pad, unk) = (number("<eos>")?, number("<pad>")?, number("<unk>")?);
        let mut pieces: Vec<(&str, u32)> = file
            .model
            .vocab
            .iter()
            .filter(|(piece, _)| !file.added_tokens.iter().any(|added| added.special && &added.content == *piece))
            .map(|(piece, &id)| (piece.as_str(), id))
            .collect();
        pieces.sort_unstable_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
        let mut out = Vec::with_capacity(20 + pieces.len() * 8 + 4 + pieces.iter().map(|p| p.0.len()).sum::<usize>());
        out.extend_from_slice(MAGIC);
        for value in [pieces.len() as u32, eos, pad, unk] {
            out.extend_from_slice(&value.to_le_bytes());
        }
        let mut offset = 0u32;
        for (piece, id) in &pieces {
            out.extend_from_slice(&offset.to_le_bytes());
            out.extend_from_slice(&id.to_le_bytes());
            offset += piece.len() as u32;
        }
        out.extend_from_slice(&offset.to_le_bytes());
        for (piece, _) in &pieces {
            out.extend_from_slice(piece.as_bytes());
        }
        Ok(out)
    }

    pub fn from_bytes(data: Vec<u8>) -> Option<Self> {
        if data.len() < 20 || &data[..4] != MAGIC {
            return None;
        }
        let word = |at: usize| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        let count = word(4) as usize;
        let (eos, pad, unk) = (word(8), word(12), word(16));
        let text = 20 + count * 8 + 4;
        if data.len() < text {
            return None;
        }
        let vocabulary = Self { data, count, eos, pad, unk };
        (vocabulary.offset(count) + text == vocabulary.data.len()).then_some(vocabulary)
    }

    fn word(&self, at: usize) -> u32 {
        u32::from_le_bytes(self.data[at..at + 4].try_into().unwrap())
    }

    /// Where piece `n` (in sorted order) starts in the text; `count` gives its end.
    fn offset(&self, n: usize) -> usize {
        self.word(20 + n * 8) as usize
    }

    fn piece(&self, n: usize) -> &[u8] {
        let text = 20 + self.count * 8 + 4;
        &self.data[text + self.offset(n)..text + self.offset(n + 1)]
    }

    /// The number of a piece, if the vocabulary has it.
    pub fn id(&self, piece: &str) -> Option<u32> {
        let (mut low, mut high) = (0, self.count);
        while low < high {
            let middle = (low + high) / 2;
            match self.piece(middle).cmp(piece.as_bytes()) {
                std::cmp::Ordering::Less => low = middle + 1,
                std::cmp::Ordering::Greater => high = middle,
                std::cmp::Ordering::Equal => return Some(self.word(20 + middle * 8 + 4)),
            }
        }
        None
    }

    /// The model's input for `text`: its tokens, `<eos>`, `<pad>` up to `length`.
    pub fn tokens(&self, text: &str, length: usize) -> Vec<u32> {
        let mut ids = self.pieces(&clean(text));
        ids.truncate(length.saturating_sub(1));
        ids.push(self.eos);
        ids.resize(length.max(ids.len()), self.pad);
        ids
    }

    /// SentencePiece BPE over the whole text, spaces as `▁`.
    fn pieces(&self, text: &str) -> Vec<u32> {
        // A symbol: its text, or a byte the vocabulary has no character for.
        enum Symbol {
            Text(String, u32),
            Byte(u32),
        }
        let mut symbols: Vec<Symbol> = Vec::new();
        for ch in text.replace(' ', "\u{2581}").chars() {
            let text = ch.to_string();
            match self.id(&text) {
                Some(id) => symbols.push(Symbol::Text(text, id)),
                None => {
                    for byte in text.bytes() {
                        symbols.push(Symbol::Byte(self.id(&format!("<0x{byte:02X}>")).unwrap_or(self.unk)));
                    }
                }
            }
        }
        loop {
            let mut best: Option<(u32, usize)> = None;
            for i in 0..symbols.len().saturating_sub(1) {
                if let (Symbol::Text(a, _), Symbol::Text(b, _)) = (&symbols[i], &symbols[i + 1]) {
                    if let Some(id) = self.id(&format!("{a}{b}")) {
                        if best.is_none_or(|(least, _)| id < least) {
                            best = Some((id, i));
                        }
                    }
                }
            }
            let Some((id, i)) = best else { break };
            let Symbol::Text(right, _) = symbols.remove(i + 1) else { unreachable!() };
            if let Symbol::Text(left, number) = &mut symbols[i] {
                left.push_str(&right);
                *number = id;
            }
        }
        symbols
            .into_iter()
            .map(|symbol| match symbol {
                Symbol::Text(_, id) | Symbol::Byte(id) => id,
            })
            .collect()
    }
}

/// The text as SigLIP's training saw it (open_clip's `canonicalize`):
/// spaces collapsed, ASCII punctuation dropped, lower case.
pub fn clean(text: &str) -> String {
    let text: String = text.chars().take(MOST_CHARS).filter(|c| !c.is_ascii_punctuation()).collect();
    text.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

// ── The `describe` job ─────────────────────────────────────────────────

/// Runs a `describe` job: the vocabulary made, if it isn't there, and the
/// text model tried once. Its key is the text models' tag.
pub fn run_describe(jobs: &[Job]) -> Vec<Outcome> {
    let outcome = prepare();
    jobs.iter().map(|_| outcome.clone()).collect()
}

fn prepare() -> Outcome {
    if !analysis::may_run(Part::Text) {
        return Outcome::Done;
    }
    let vocabulary = match read_vocabulary() {
        Ok(Some(vocabulary)) => vocabulary,
        Ok(None) => match make_vocabulary() {
            Ok(vocabulary) => vocabulary,
            Err(Problem::Broken(why)) => {
                analysis::model_failed(Part::Text, &why);
                return Outcome::Failed(why);
            }
            Err(problem) => return Outcome::Retry(format!("{problem:?}")),
        },
        Err(problem) => return Outcome::Retry(format!("{problem:?}")),
    };
    match embed(&vocabulary, "a photo") {
        Ok(_) => {
            analysis::text_ready();
            Outcome::Done
        }
        Err(Look::ModelFailed(why)) => {
            analysis::model_failed(Part::Text, &why);
            Outcome::Failed(why)
        }
        Err(Look::Photo(problem)) => Outcome::Retry(format!("{problem:?}")),
    }
}

fn read_vocabulary() -> Result<Option<Vocabulary>, Problem> {
    let path = vocabulary_path();
    let size = match storage::stat(&path) {
        Ok(file) => file.size,
        Err(storage::Error::NotFound) => return Ok(None),
        Err(err) => return Err(Problem::Later(format!("{err:?}"))),
    };
    let data = read_whole(&path, size)?;
    Ok(Vocabulary::from_bytes(data))
}

fn make_vocabulary() -> Result<Vocabulary, Problem> {
    let size = storage::stat(TEXT_TOKENIZER).map_err(|err| Problem::Later(format!("{err:?}")))?.size;
    let json = read_whole(TEXT_TOKENIZER, size)?;
    let data = Vocabulary::build(&json).map_err(Problem::Broken)?;
    drop(json);
    storage::write(&vocabulary_path(), &data).map_err(|err| Problem::Later(format!("{err:?}")))?;
    Vocabulary::from_bytes(data).ok_or_else(|| Problem::Broken("the vocabulary made can't be read".into()))
}

/// A file of the plugin's storage, read a part at a time (each read is
/// limited to the plugin's data limit).
fn read_whole(path: &str, size: u64) -> Result<Vec<u8>, Problem> {
    const PART: u64 = 8 << 20;
    let mut data = Vec::with_capacity(size as usize);
    while (data.len() as u64) < size {
        let part = storage::read(path, data.len() as u64, PART).map_err(|err| Problem::Later(format!("{err:?}")))?;
        if part.is_empty() {
            break;
        }
        data.extend_from_slice(&part);
    }
    Ok(data)
}

/// The unit vector of a text.
fn embed(vocabulary: &Vocabulary, text: &str) -> Result<Vec<f32>, Look> {
    let info = use_onnx::get_model_info(TEXT).map_err(run_failed)?;
    let input = info.inputs.into_iter().next().ok_or_else(|| Look::ModelFailed(format!("{TEXT} takes no input")))?;
    let length =
        input.shape.as_ref().and_then(|shape| shape.get(1).copied().flatten()).map_or(DEFAULT_LENGTH, |n| n as usize);
    let ids = vocabulary.tokens(text, length);
    let (dtype, data) = match input.dtype {
        Some(TensorType::I64) => (TensorType::I64, ids.iter().flat_map(|&id| i64::from(id).to_le_bytes()).collect()),
        _ => (TensorType::I32, ids.iter().flat_map(|&id| (id as i32).to_le_bytes()).collect()),
    };
    let tensor = Tensor { dtype, shape: vec![1, length as u32], data };
    let outputs = use_onnx::run(TEXT, &[NamedTensor { name: input.name, tensor }]).map_err(run_failed)?;
    let mut vector = outputs.into_iter().next().map(|output| floats(&output.tensor.data)).unwrap_or_default();
    let length = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
    if vector.is_empty() || length == 0.0 {
        return Err(Look::ModelFailed(format!("{TEXT} gave no vector")));
    }
    vector.iter_mut().for_each(|v| *v /= length);
    Ok(vector)
}

// ── Search ─────────────────────────────────────────────────────────────

/// `GET /api/describe?q=`: the photos that look like the words, best first.
pub fn search(query: &str) -> Response {
    if clean(query).is_empty() {
        return ok(json!({ "ready": true, "ids": [], "scores": [] }));
    }
    if let Err(why) = analysis::text_readiness() {
        return ok(json!({ "ready": false, "why": why }));
    }
    let vocabulary = match read_vocabulary() {
        Ok(Some(vocabulary)) => vocabulary,
        Ok(None) => return ok(json!({ "ready": false, "why": "preparing" })),
        Err(problem) => return error(503, &format!("{problem:?}")),
    };
    let asked = match embed(&vocabulary, query) {
        Ok(vector) => vector,
        Err(Look::ModelFailed(why)) => {
            analysis::model_failed(Part::Text, &why);
            return ok(json!({ "ready": false, "why": format!("failed: {why}") }));
        }
        Err(Look::Photo(problem)) => return error(503, &format!("{problem:?}")),
    };
    drop(vocabulary);
    let tag = models::clip_tag();
    let mut scored: Vec<(String, f32)> = Vec::new();
    for shard in 0..SHARDS {
        for file in analysis::read_shard::<ClipFile>(shard) {
            if file.m == tag && !file.v.is_empty() {
                if let Some(score) = analysis::cosine(&asked, &analysis::decode(&file.v)) {
                    scored.push((file.id, score));
                }
            }
        }
    }
    let top = scored.iter().map(|(_, score)| *score).fold(0.0f32, f32::max);
    let found = analysis::best(scored, LEAST.max(top * SHARE_OF_BEST), MOST);
    ok(json!({
        "ready": true,
        "ids": found.iter().map(|(id, _)| id).collect::<Vec<_>>(),
        "scores": found.iter().map(|(_, score)| (score * 1000.0).round() / 1000.0).collect::<Vec<_>>(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny tokenizer.json in the shape of Gemma's.
    fn vocabulary() -> Vocabulary {
        let pieces = [
            "<pad>", "<eos>", "<bos>", "<unk>", "<mask>", "▁", "к", "о", "т", "н", "а", "д", "и", "в", "е", "▁н", "ко",
            "▁на", "кот", "<0xF0>", "<0x9F>", "<0x99>", "<0x82>", "▁ди", "ва", "ване", "ди", "ван",
        ];
        let vocab: serde_json::Map<String, serde_json::Value> =
            pieces.iter().enumerate().map(|(i, piece)| (piece.to_string(), json!(i))).collect();
        let added: Vec<_> = pieces[..5].iter().map(|piece| json!({ "content": piece, "special": true })).collect();
        let file = json!({ "model": { "type": "BPE", "vocab": vocab, "merges": [] }, "added_tokens": added });
        Vocabulary::from_bytes(Vocabulary::build(&serde_json::to_vec(&file).unwrap()).unwrap()).unwrap()
    }

    #[test]
    fn the_vocabulary_round_trips() {
        let vocabulary = vocabulary();
        assert_eq!(vocabulary.id("кот"), Some(18));
        assert_eq!(vocabulary.id("▁"), Some(5));
        assert_eq!(vocabulary.id("<eos>"), None, "special tokens are left out");
        assert_eq!(vocabulary.id("собака"), None);
        assert_eq!((vocabulary.eos, vocabulary.pad), (1, 0));
        assert!(Vocabulary::from_bytes(b"RCV1short".to_vec()).is_none());
    }

    #[test]
    fn tokens_as_sentencepiece_makes_them() {
        let vocabulary = vocabulary();
        // The reference tokenizer gives ['кот', '▁на', '▁ди', 'ване', '<eos>'].
        assert_eq!(vocabulary.tokens("Кот на  диване!", 8), [18, 17, 23, 25, 1, 0, 0, 0]);
        // A character the vocabulary lacks: its bytes.
        assert_eq!(vocabulary.tokens("🙂", 6), [19, 20, 21, 22, 1, 0]);
        // Cut to the length, `<eos>` kept.
        assert_eq!(vocabulary.tokens("кот кот кот", 3), [18, 5, 1]);
    }

    #[test]
    fn text_is_cleaned_as_in_training() {
        assert_eq!(clean("  Кот,   на\tДИВАНЕ! "), "кот на диване");
        assert_eq!(clean("«ёлка» — 2023"), "«ёлка» — 2023", "only ASCII punctuation goes");
        assert_eq!(clean("?!"), "");
    }
}

/// Against the real tokenizer: `RC_TOKENIZER=tokenizer.json RC_TOKEN_CASES=cases.jsonl
/// cargo test real_tokenizer -- --ignored`, the cases made by `tokenizers`
/// (`{"text": …, "ids": […]}` a line, ids padded to 64).
#[cfg(test)]
mod real {
    use super::*;

    #[test]
    #[ignore]
    fn real_tokenizer() {
        let tokenizer = std::fs::read(std::env::var("RC_TOKENIZER").unwrap()).unwrap();
        let vocabulary = Vocabulary::from_bytes(Vocabulary::build(&tokenizer).unwrap()).unwrap();
        let cases = std::fs::read_to_string(std::env::var("RC_TOKEN_CASES").unwrap()).unwrap();
        let (mut all, mut wrong) = (0, 0);
        for line in cases.lines() {
            let case: serde_json::Value = serde_json::from_str(line).unwrap();
            let expected: Vec<u32> = serde_json::from_value(case["ids"].clone()).unwrap();
            let got = vocabulary.tokens(case["text"].as_str().unwrap(), 64);
            all += 1;
            if got != expected {
                wrong += 1;
                if wrong <= 5 {
                    eprintln!("{:?}: {got:?} != {expected:?}", case["text"]);
                }
            }
        }
        eprintln!("{all} cases, {wrong} wrong");
        assert_eq!(wrong, 0);
    }
}
