// AUTHR_OS — SHA-256 content-addressable audio chunk cache
// (Phase 4, Sprint 9). Chunks live in `<bundle>/audio/segments/`:
//   <sha256>.wav   — 16-bit PCM mono synthesis result
//   <sha256>.json  — metadata (engine, voice, sample_rate, duration, text)
//   stats.json     — cumulative hit/miss counters (sub-second cache hits)

use crate::error::AppError;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub struct AudioCache {
    root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct CachedChunk {
    pub hash: String,
    pub wav_path: PathBuf,
    pub meta: Value,
}

#[derive(Debug, Clone, Copy)]
pub struct CacheStats {
    pub chunks: u64,
    pub bytes: u64,
    pub hits: u64,
    pub misses: u64,
}

impl AudioCache {
    pub fn new(bundle_root: &Path) -> Self {
        AudioCache { root: bundle_root.join("audio").join("segments") }
    }

    /// Deterministic content key: sha256(engine|voice|rate|ipa|text).
    pub fn key(engine: &str, voice: &str, sample_rate: u32, ipa: &str, text: &str) -> String {
        let mut h = Sha256::new();
        h.update(engine.as_bytes());
        h.update(b"|");
        h.update(voice.as_bytes());
        h.update(b"|");
        h.update(sample_rate.to_string().as_bytes());
        h.update(b"|");
        h.update(ipa.as_bytes());
        h.update(b"|");
        h.update(text.as_bytes());
        let digest = h.finalize();
        digest.iter().map(|b| format!("{b:02x}")).collect()
    }

    fn stats_path(&self) -> PathBuf {
        self.root.join("stats.json")
    }

    fn read_stats(&self) -> (u64, u64) {
        match std::fs::read_to_string(self.stats_path()) {
            Ok(raw) => {
                let v: Value = serde_json::from_str(&raw).unwrap_or_else(|_| json!({}));
                (v["hits"].as_u64().unwrap_or(0), v["misses"].as_u64().unwrap_or(0))
            }
            Err(_) => (0, 0),
        }
    }

    fn write_stats(&self, hits: u64, misses: u64) -> Result<(), AppError> {
        std::fs::create_dir_all(&self.root)
            .map_err(|e| AppError::audio(format!("create cache dir: {e}")))?;
        let tmp = self.root.join("stats.json.tmp");
        std::fs::write(&tmp, serde_json::to_string(&json!({ "hits": hits, "misses": misses })).unwrap())
            .map_err(|e| AppError::audio(format!("write stats: {e}")))?;
        std::fs::rename(&tmp, self.stats_path())
            .map_err(|e| AppError::audio(format!("rename stats: {e}")))?;
        Ok(())
    }

    /// Fetch a cached chunk by key. A hit bumps the hit counter.
    pub fn lookup(&self, hash: &str) -> Result<Option<CachedChunk>, AppError> {
        let wav = self.root.join(format!("{hash}.wav"));
        let meta_path = self.root.join(format!("{hash}.json"));
        if !wav.exists() || !meta_path.exists() {
            let (hits, misses) = self.read_stats();
            self.write_stats(hits, misses + 1)?;
            return Ok(None);
        }
        let meta: Value = serde_json::from_str(
            &std::fs::read_to_string(&meta_path)
                .map_err(|e| AppError::audio(format!("read meta: {e}")))?,
        )
        .map_err(|e| AppError::audio(format!("parse meta: {e}")))?;
        let (hits, misses) = self.read_stats();
        self.write_stats(hits + 1, misses)?;
        Ok(Some(CachedChunk { hash: hash.to_string(), wav_path: wav, meta }))
    }

    /// Store synthesized samples; overwrites atomically. Bumps the miss counter.
    pub fn store(
        &self,
        hash: &str,
        samples: &[f32],
        sample_rate: u32,
        meta: Value,
    ) -> Result<CachedChunk, AppError> {
        std::fs::create_dir_all(&self.root)
            .map_err(|e| AppError::audio(format!("create cache dir: {e}")))?;
        let wav = self.root.join(format!("{hash}.wav"));
        crate::audio::wav::write_wav(&wav, samples, sample_rate)?;
        let mut meta = meta;
        meta["hash"] = json!(hash);
        meta["sample_rate"] = json!(sample_rate);
        meta["duration_ms"] = json!((samples.len() as f64 * 1000.0 / sample_rate as f64).round() as u64);
        meta["samples"] = json!(samples.len());
        let meta_path = self.root.join(format!("{hash}.json"));
        let tmp = self.root.join(format!("{hash}.json.tmp"));
        std::fs::write(&tmp, serde_json::to_string_pretty(&meta).unwrap())
            .map_err(|e| AppError::audio(format!("write meta: {e}")))?;
        std::fs::rename(&tmp, &meta_path).map_err(|e| AppError::audio(format!("rename meta: {e}")))?;
        let (hits, misses) = self.read_stats();
        self.write_stats(hits, misses + 1)?;
        Ok(CachedChunk { hash: hash.to_string(), wav_path: wav, meta })
    }

    /// Aggregate cache statistics (chunk count, total bytes, hit/miss).
    pub fn stats(&self) -> Result<CacheStats, AppError> {
        let (hits, misses) = self.read_stats();
        let mut chunks = 0u64;
        let mut bytes = 0u64;
        if let Ok(rd) = std::fs::read_dir(&self.root) {
            for entry in rd.flatten() {
                let p = entry.path();
                if p.extension().and_then(|e| e.to_str()) == Some("wav") {
                    chunks += 1;
                    if let Ok(md) = std::fs::metadata(&p) {
                        bytes += md.len();
                    }
                }
            }
        }
        Ok(CacheStats { chunks, bytes, hits, misses })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_is_stable_and_discriminates() {
        let a = AudioCache::key("SCRIPTED", "af_bella", 24000, "", "hello world");
        let b = AudioCache::key("SCRIPTED", "af_bella", 24000, "", "hello world");
        let c = AudioCache::key("SCRIPTED", "af_bella", 24000, "", "hello worlds");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.len(), 64);
    }
}
