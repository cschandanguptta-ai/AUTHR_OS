// AUTHR_OS — release manifest signing & verification (Phase 6, Sprint 15).
//
// The Tauri v2 updater consumes a `latest.json` manifest whose `signature`
// field is an ed25519 signature over the artifact bytes. This module generates
// the manifest + signature for a release artifact (installer, .zip, .AppImage)
// and verifies it independently. Keys:
//   1. explicit `key_hex` argument (64 hex chars = 32-byte seed)
//   2. `AUTHR_RELEASE_KEY_HEX` environment variable
//   3. ephemeral generated key (returned as `generated_key_hex` — CI only;
//      production keys belong in the OS keychain, `keychain_store`)
//
// These commands are bundle-free (like keychain_*), operating on filesystem
// paths so release automation can run outside any project bundle.

use crate::error::AppError;
use ed25519_dalek::{Signer, Verifier};
use rand_core::RngCore;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str, expect: usize) -> Result<Vec<u8>, AppError> {
    let s = s.trim();
    if s.len() != expect * 2 || !s.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(AppError::bundle(format!(
            "expected {expect}-byte hex string ({} chars), got {} chars",
            expect * 2,
            s.len()
        )));
    }
    (0..expect)
        .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).map_err(|e| AppError::bundle(format!("hex: {e}"))))
        .collect()
}

fn seed32(s: &str) -> Result<[u8; 32], AppError> {
    let v = unhex(s, 32)?;
    v.try_into().map_err(|_| AppError::bundle("seed length mismatch"))
}

/// #52 release_manifest — sign an artifact, emit the updater manifest.
pub fn release_manifest(a: &Value) -> crate::error::IpcResult {
    let artifact = a.get("artifact").and_then(|v| v.as_str()).ok_or_else(|| {
        AppError::bundle("missing required arg `artifact` (path to the release file)")
    })?;
    let artifact_path = std::path::Path::new(artifact);
    let bytes = std::fs::read(artifact_path)
        .map_err(|e| AppError::bundle(format!("read artifact `{artifact}`: {e}")))?;

    let (seed, generated) = match a.get("key_hex").and_then(|v| v.as_str()).map(str::to_string)
    {
        Some(k) => (seed32(&k)?, None),
        None => match std::env::var("AUTHR_RELEASE_KEY_HEX").ok() {
            Some(k) => (seed32(&k)?, None),
            None => {
                let mut seed = [0u8; 32];
                rand_core::OsRng.fill_bytes(&mut seed);
                (seed, Some(hex(&seed)))
            }
        },
    };
    let signing = ed25519_dalek::SigningKey::from_bytes(&seed);
    let public = signing.verifying_key();

    let signature = signing.sign(&bytes);
    let sha = Sha256::digest(&bytes);
    let version = a
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or(env!("CARGO_PKG_VERSION"))
        .to_string();
    let manifest = json!({
        "authr_release": "1",
        "version": version,
        "pub_date": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "artifact": artifact_path.file_name().map(|f| f.to_string_lossy().into_owned()),
        "artifact_sha256": hex(&sha),
        "artifact_bytes": bytes.len(),
        "platform": { "os": std::env::consts::OS, "arch": std::env::consts::ARCH },
        "url": a.get("url").and_then(|v| v.as_str()),
        "public_key": hex(&public.to_bytes()),
        "signature": hex(&signature.to_bytes()),
    });

    // Manifest location: explicit `out` or alongside the artifact as latest.json.
    let out = a
        .get("out")
        .and_then(|v| v.as_str())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| artifact_path.with_file_name("latest.json"));
    std::fs::write(&out, serde_json::to_string_pretty(&manifest).unwrap())
        .map_err(|e| AppError::bundle(format!("write manifest: {e}")))?;

    Ok(json!({
        "manifest": out.display().to_string(),
        "manifest_data": manifest,
        "artifact_bytes": bytes.len(),
        "artifact_sha256": hex(&sha),
        "generated_key_hex": generated,
    }))
}

/// #53 release_verify — verify an artifact against its signed manifest.
pub fn release_verify(a: &Value) -> crate::error::IpcResult {
    let manifest_path = a
        .get("manifest")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::bundle("missing required arg `manifest`"))?;
    let raw = std::fs::read_to_string(manifest_path)
        .map_err(|e| AppError::bundle(format!("read manifest: {e}")))?;
    let manifest: Value = serde_json::from_str(&raw)
        .map_err(|e| AppError::bundle(format!("parse manifest: {e}")))?;

    let artifact = a
        .get("artifact")
        .and_then(|v| v.as_str())
        .map(std::path::PathBuf::from)
        .or_else(|| {
            manifest["artifact"]
                .as_str()
                .map(|name| std::path::Path::new(manifest_path).with_file_name(name))
        })
        .ok_or_else(|| AppError::bundle("manifest carries no artifact name"))?;
    let bytes = std::fs::read(&artifact)
        .map_err(|e| AppError::bundle(format!("read artifact: {e}")))?;

    let sha = Sha256::digest(&bytes);
    let sha_ok = manifest["artifact_sha256"].as_str() == Some(&hex(&sha));
    let sig_ok = (|| -> Result<bool, AppError> {
        let pk_bytes: [u8; 32] = unhex(manifest["public_key"].as_str().unwrap_or(""), 32)?
            .try_into()
            .unwrap();
        let sig_bytes: [u8; 64] = unhex(manifest["signature"].as_str().unwrap_or(""), 64)?
            .try_into()
            .unwrap();
        let pk = ed25519_dalek::VerifyingKey::from_bytes(&pk_bytes)
            .map_err(|e| AppError::bundle(format!("public key: {e}")))?;
        let sig = ed25519_dalek::Signature::from_bytes(&sig_bytes);
        Ok(pk.verify(&bytes, &sig).is_ok())
    })()
    .unwrap_or(false);

    Ok(json!({
        "valid": sha_ok && sig_ok,
        "checks": { "sha256": sha_ok, "signature": sig_ok },
        "artifact": artifact.display().to_string(),
        "version": manifest["version"].clone(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrip() {
        let b = vec![0u8, 1, 0xfe, 0xff];
        assert_eq!(unhex(&hex(&b), 4).unwrap(), b);
        assert!(unhex("zz", 2).is_err());
        assert!(unhex("aabb", 3).is_err());
    }
}
