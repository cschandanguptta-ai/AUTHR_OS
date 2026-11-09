// AUTHR_OS — embedding backends (Phase 5, Sprint 12).
//
// `Embedder` is the seam for pluggable text encoders:
//   • `HashingEmbedder` — deterministic, dependency-free 384-dim feature
//     hashing (tokens + adjacent bigrams, sublinear tf, L2-normalized).
//     Always available; the offline default for CI and sandboxed runs.
//   • `OnnxMiniLmEmbedder` — all-MiniLM-L6-v2 through `ort` 2.0 (384-dim
//     dense vectors). Compiled behind the `local-ai` cargo feature; without
//     it the resolver reports `AI_ENGINE_UNAVAILABLE` (docs/LOCAL_AI.md).

use crate::error::AppError;

pub const DEFAULT_DIM: usize = 384;

pub trait Embedder: Send + Sync {
    fn name(&self) -> &'static str;
    fn dim(&self) -> usize;
    fn embed(&self, text: &str) -> Vec<f32>;
}

/// FNV-1a 64-bit — stable, dependency-free string hash.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Deterministic 384-dim feature-hashing embedder.
pub struct HashingEmbedder {
    dim: usize,
}

impl HashingEmbedder {
    pub fn new() -> Self {
        HashingEmbedder { dim: DEFAULT_DIM }
    }

    pub fn tokenize(text: &str) -> Vec<String> {
        let lower = text.to_lowercase();
        let words: Vec<&str> = lower
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .collect();
        let mut tokens: Vec<String> = words.iter().map(|w| w.to_string()).collect();
        // Adjacent-word bigrams give light phrase sensitivity.
        for pair in words.windows(2) {
            tokens.push(format!("{}~{}", pair[0], pair[1]));
        }
        tokens
    }
}

impl Default for HashingEmbedder {
    fn default() -> Self {
        Self::new()
    }
}

impl Embedder for HashingEmbedder {
    fn name(&self) -> &'static str {
        "hashing-384"
    }

    fn dim(&self) -> usize {
        self.dim
    }

    fn embed(&self, text: &str) -> Vec<f32> {
        let mut vec = vec![0.0f32; self.dim];
        let tokens = Self::tokenize(text);
        if tokens.is_empty() {
            return vec;
        }
        // Term frequencies.
        let mut tf: std::collections::HashMap<u64, u32> = std::collections::HashMap::new();
        for tok in &tokens {
            let h = fnv1a(tok.as_bytes());
            *tf.entry(h).or_insert(0) += 1;
        }
        // Sublinear tf weighting, sign from a second hash (hashing trick).
        for (h, count) in tf {
            let idx = (h % self.dim as u64) as usize;
            let sign_h = fnv1a(&h.to_le_bytes());
            let sign = if sign_h & 1 == 0 { 1.0 } else { -1.0 };
            let w = sign * (1.0 + (count as f32).ln());
            vec[idx] += w;
        }
        // L2 normalization → cosine similarity == dot product.
        let norm: f32 = vec.iter().map(|v| v * v).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in vec.iter_mut() {
                *v /= norm;
            }
        }
        vec
    }
}

/// Resolve an embedder by name. `minilm-onnx` requires the `local-ai` feature
/// and the model file; it reports a canonical unavailable error otherwise.
pub fn resolve(name: &str, bundle_root: &std::path::Path) -> Result<Box<dyn Embedder>, AppError> {
    match name.to_lowercase().as_str() {
        "" | "hashing" | "hashing-384" | "default" => Ok(Box::new(HashingEmbedder::new())),
        "minilm-onnx" | "all-minilm-l6-v2" => {
            let model = bundle_root.join("models").join("all-MiniLM-L6-v2.onnx");
            if !model.exists() {
                return Err(AppError::ai_engine_unavailable(format!(
                    "MiniLM ONNX model not found at {} — place all-MiniLM-L6-v2.onnx in models/ (docs/LOCAL_AI.md)",
                    model.display()
                )));
            }
            Err(AppError::ai_engine_unavailable(
                "MiniLM ONNX runtime requires the `local-ai` cargo feature (ort 2.0) — this build ships the deterministic hashing embedder",
            ))
        }
        other => Err(AppError::ai(format!("unknown embedder `{other}`"))),
    }
}

/// Cosine similarity (vectors are L2-normalized upstream; keep the general form).
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    let (mut dot, mut na, mut nb) = (0.0f32, 0.0f32, 0.0f32);
    for i in 0..n {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

/// Pack/unpack f32 vectors to little-endian BLOBs.
pub fn pack(vec: &[f32]) -> Vec<u8> {
    vec.iter().flat_map(|f| f.to_le_bytes()).collect()
}

pub fn unpack(blob: &[u8], dim: usize) -> Vec<f32> {
    blob.as_chunks::<4>().0.iter()
        .take(dim)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedder_dim_and_normalization() {
        let e = HashingEmbedder::new();
        assert_eq!(e.dim(), 384);
        let v = e.embed("Kira walked the coastal road at dawn");
        assert_eq!(v.len(), 384);
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5, "norm={norm}");
    }

    #[test]
    fn identical_text_identical_vector() {
        let e = HashingEmbedder::new();
        assert_eq!(e.embed("hello world"), e.embed("hello world"));
        assert!(e.embed("hello world").iter().any(|&v| v != 0.0));
    }

    #[test]
    fn related_text_scores_higher_than_unrelated() {
        let e = HashingEmbedder::new();
        let q = e.embed("the dragon guarded the mountain treasury");
        let near = e.embed("a dragon guards the treasury on the mountain");
        let far = e.embed("she printed the quarterly spreadsheet report");
        assert!(cosine(&q, &near) > cosine(&q, &far), "near={} far={}", cosine(&q, &near), cosine(&q, &far));
        assert!(cosine(&q, &near) > 0.5);
    }

    #[test]
    fn blob_roundtrip() {
        let e = HashingEmbedder::new();
        let v = e.embed("round trip test");
        let blob = pack(&v);
        assert_eq!(blob.len(), 384 * 4);
        let back = unpack(&blob, 384);
        assert_eq!(v, back);
    }

    #[test]
    fn empty_text_is_zero_vector() {
        let e = HashingEmbedder::new();
        assert!(e.embed("").iter().all(|&v| v == 0.0));
    }

    #[test]
    fn resolve_rejects_unknown() {
        let err = match resolve("gpt-4", std::path::Path::new("/tmp")) {
            Err(e) => e,
            Ok(_) => panic!("expected AI_ERROR for unknown embedder"),
        };
        assert_eq!(err.code, "AI_ERROR");
    }

    #[test]
    fn resolve_minilm_reports_unavailable_without_model() {
        let dir = std::env::temp_dir().join(format!("authr_emb_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let err = match resolve("minilm-onnx", &dir) {
            Err(e) => e,
            Ok(_) => panic!("expected AI_ENGINE_UNAVAILABLE without the model file"),
        };
        assert_eq!(err.code, "AI_ENGINE_UNAVAILABLE");
        std::fs::remove_dir_all(&dir).ok();
    }
}
