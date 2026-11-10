// AUTHR_OS — Local embedding providers (Phase 5 / Sprint 12).
//
// Engine matrix (selection via `AUTHR_EMBEDDER`, default `auto`):
//   hash   — deterministic feature-hashing embedder, pure Rust, always
//            available offline (unigram + bigram + char 4-gram features,
//            signed 384-bucket projection, sublinear TF, L2-normalized).
//   minilm — `all-MiniLM-L6-v2` ONNX via ort 2.0 (384-dim). Adapter contract
//            in docs/PHASE5_NOTES.md; attaches to this same trait on desktop
//            builds that vendor the model + runtime.

use crate::error::AppError;
use serde_json::Value;

pub trait Embedder: Send + Sync {
    fn name(&self) -> &'static str;
    fn dim(&self) -> usize;
    /// Deterministic embedding of `text` — same bytes ⇒ same vector.
    fn embed(&self, text: &str) -> Vec<f32>;
}

#[derive(Default)]
pub struct LocalHashEmbedder;

const DIM: usize = 384;

impl Embedder for LocalHashEmbedder {
    fn name(&self) -> &'static str {
        "hash-v1"
    }

    fn dim(&self) -> usize {
        DIM
    }

    fn embed(&self, text: &str) -> Vec<f32> {
        let mut counts = vec![0f32; DIM];
        let lower = text.to_lowercase();

        // Tokenize on non-alphanumeric boundaries (unicode aware).
        let tokens: Vec<&str> = lower
            .split(|c: char| !c.is_alphanumeric())
            .filter(|t| !t.is_empty())
            .collect();

        let mut bump = |feature: &str, weight: f32| {
            let h = fnv1a64(feature.as_bytes());
            let bucket = (h % DIM as u64) as usize;
            // Signed hashing trick halves bucket bias.
            let sign = if h & 1 == 1 { 1.0f32 } else { -1.0f32 };
            counts[bucket] += sign * weight;
        };

        // Unigrams + sublinear TF.
        for (i, tok) in tokens.iter().enumerate() {
            let tf_weight = 1.0 + (tok.len() as f32).ln().max(0.0);
            bump(tok, tf_weight);
            // Bigrams preserve local word order.
            if i > 0 {
                let bigram = format!("{}/{}", tokens[i - 1], tok);
                bump(&bigram, 1.4);
            }
            // Character 4-grams with boundary markers (morphology robustness).
            let padded = format!("^{}$", tok);
            if padded.chars().count() >= 4 {
                let chars: Vec<char> = padded.chars().collect();
                for w in chars.windows(4) {
                    let gram: String = w.iter().collect();
                    bump(&gram, 0.6);
                }
            }
        }

        // L2 normalize.
        let norm = counts.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > f32::EPSILON {
            for x in counts.iter_mut() {
                *x /= norm;
            }
        }
        counts
    }
}

/// FNV-1a 64-bit — stable across platforms and processes (no SipHash
/// randomness, required for cross-run determinism of the whole RAG stack).
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Resolve the active embedder from `AUTHR_EMBEDDER` (`auto` | `hash`).
/// `auto` selects the strongest available engine; without the ONNX adapter
/// that is always the hash embedder.
pub fn default_embedder() -> Result<std::sync::Arc<dyn Embedder>, AppError> {
    let choice = std::env::var("AUTHR_EMBEDDER").unwrap_or_else(|_| "auto".into());
    match choice.as_str() {
        "auto" | "hash" | "hash-v1" => Ok(std::sync::Arc::new(LocalHashEmbedder)),
        "minilm" => Err(AppError::storage(
            "AUTHR_EMBEDDER=minilm requires an ONNX adapter build — see docs/PHASE5_NOTES.md",
        )),
        other => Err(AppError::storage(format!(
            "unknown embedder `{other}` — available: hash"
        ))),
    }
}

/// Registry entries for `ai_status`.
pub fn registry() -> Vec<Value> {
    vec![serde_json::json!({ "id": "hash", "name": "hash-v1", "dim": DIM })]
}
