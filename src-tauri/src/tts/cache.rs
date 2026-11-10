// AUTHR_OS — Content-Addressable Audio Chunk Cache (build plan §6.2 / Sprint 9).
// Key = SHA-256 over the canonical synthesis request. Chunks live in
// `audio/segments/<sha256>.wav` (24 kHz mono PCM16). Editing one sentence
// re-synthesizes only that single sentence; identical requests are cache hits.

use crate::audio::wav::{read_wav, write_wav};
use crate::error::AppError;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub struct ChunkKeyParts<'a> {
    pub engine: &'a str,
    pub voice_id: &'a str,
    pub speaker_fp: &'a str,
    pub text: &'a str,
    pub rate: f64,
    pub pitch: f64,
    pub emotion: &'a str,
    pub conditioned: bool,
}

pub fn chunk_dir(root: &Path) -> PathBuf {
    root.join("audio").join("segments")
}

fn latent_dir(root: &Path) -> PathBuf {
    chunk_dir(root).join(".latents")
}

/// Stable per-speaker acoustic fingerprint (NOT per-turn content — this is
/// what bounds the §6.3 cache-invalidation blast radius to one turn).
pub fn speaker_fingerprint(speaker: &str, engine: &str, voice_id: &str) -> String {
    let h = crate::audio::dsp::fnv64(format!("{speaker}|{engine}|{voice_id}").as_bytes());
    format!("{h:016x}")
}

/// SHA-256 cache key over the canonical request string.
pub fn chunk_key(p: &ChunkKeyParts) -> String {
    let canon = format!(
        "authr.audio.v1|engine={}|voice={}|speaker_fp={}|rate={:.3}|pitch={:.3}|emotion={}|cond={}|text={}",
        p.engine, p.voice_id, p.speaker_fp, p.rate, p.pitch, p.emotion, p.conditioned as u8, p.text
    );
    let digest = Sha256::digest(canon.as_bytes());
    hex::encode(digest)
}

pub fn chunk_path(root: &Path, key: &str) -> PathBuf {
    chunk_dir(root).join(format!("{key}.wav"))
}

pub fn chunk_exists(root: &Path, key: &str) -> bool {
    chunk_path(root, key).is_file()
}

pub fn read_chunk(root: &Path, key: &str) -> Result<Option<Vec<f32>>, AppError> {
    let p = chunk_path(root, key);
    if !p.is_file() {
        return Ok(None);
    }
    let (samples, _fs) = read_wav(&p)?;
    Ok(Some(samples))
}

pub fn write_chunk(root: &Path, key: &str, samples: &[f32], fs: u32) -> Result<(), AppError> {
    write_wav(&chunk_path(root, key), samples, fs)
}

pub fn delete_chunk(root: &Path, key: &str) {
    let _ = std::fs::remove_file(chunk_path(root, key));
}

pub struct CacheStats {
    pub chunks: u64,
    pub bytes: u64,
}

pub fn status(root: &Path) -> Result<CacheStats, AppError> {
    let mut chunks = 0u64;
    let mut bytes = 0u64;
    let dir = chunk_dir(root);
    if dir.is_dir() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let p = entry.path();
            if p.is_file() && p.extension().and_then(|e| e.to_str()) == Some("wav") {
                chunks += 1;
                bytes += entry.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
    }
    Ok(CacheStats { chunks, bytes })
}

// ── speaker acoustic latents (1.5 s tail) ────────────────────────────────────

fn sanitize_speaker(speaker: &str) -> String {
    speaker
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect()
}

pub fn latent_path(root: &Path, speaker: &str) -> PathBuf {
    latent_dir(root).join(format!("{}.json", sanitize_speaker(speaker)))
}

pub fn speakers_cached(root: &Path) -> u64 {
    let dir = latent_dir(root);
    match std::fs::read_dir(&dir) {
        Ok(rd) => rd.filter_map(|e| e.ok()).filter(|e| {
            e.path().extension().and_then(|x| x.to_str()) == Some("json")
        }).count() as u64,
        Err(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts(text: &str, cond: bool) -> ChunkKeyParts {
        ChunkKeyParts {
            engine: "EDGE_TTS",
            voice_id: "en-US-JennyNeural",
            speaker_fp: "abc123",
            text,
            rate: 1.0,
            pitch: 1.0,
            emotion: "tense",
            conditioned: cond,
        }
    }

    #[test]
    fn key_is_sha256_and_deterministic() {
        let a = chunk_key(&parts("Hello world", false));
        let b = chunk_key(&parts("Hello world", false));
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
        // different text → different key (per-sentence invalidation)
        assert_ne!(a, chunk_key(&parts("Hello worlds", false)));
        // different conditioning → different key
        assert_ne!(a, chunk_key(&parts("Hello world", true)));
        // float formatting is stable at 3 decimals
        let mut p = parts("x", false);
        p.rate = 1.234567;
        let mut q = parts("x", false);
        q.rate = 1.2346; // both format to "1.235"
        assert_eq!(chunk_key(&p), chunk_key(&q));
        let mut r = parts("x", false);
        r.rate = 1.25;   // formats to "1.250"
        assert_ne!(chunk_key(&p), chunk_key(&r));
    }

    #[test]
    fn fingerprint_stable_per_speaker() {
        let a = speaker_fingerprint("CHAR_KIRA", "EDGE_TTS", "v1");
        let b = speaker_fingerprint("CHAR_KIRA", "EDGE_TTS", "v1");
        let c = speaker_fingerprint("CHAR_JULIAN", "EDGE_TTS", "v1");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    use crate::audio::dsp::CHUNK_FS;

    #[test]
    fn chunk_roundtrip() {
        let root = std::env::temp_dir().join(format!("authr_cache_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let key = chunk_key(&parts("roundtrip", false));
        assert!(!chunk_exists(&root, &key));
        write_chunk(&root, &key, &[0.1f32, -0.2, 0.3], CHUNK_FS).unwrap();
        assert!(chunk_exists(&root, &key));
        let samples = read_chunk(&root, &key).unwrap().unwrap();
        assert_eq!(samples.len(), 3);
        let st = status(&root).unwrap();
        assert_eq!(st.chunks, 1);
        assert!(st.bytes > 0);
        delete_chunk(&root, &key);
        assert!(!chunk_exists(&root, &key));
        let _ = std::fs::remove_dir_all(&root);
    }
}

