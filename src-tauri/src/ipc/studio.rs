// AUTHR_OS — Phase 4/5 studio command handlers (contract §4.11–4.12).
// Every handler is bundle-scoped and goes through the shared Ctx (lease,
// manifest, sandbox). File arguments are bundle-relative and resolved with
// `resolve_in_root`, so traversal attempts surface PATH_TRAVERSAL_VIOLATION.

use crate::error::{AppError, IpcResult};
use crate::ipc::args;
use crate::ipc::Ctx;
use serde_json::{json, Value};
use std::path::PathBuf;

// ─────────────────────────────────────────────────────────────────────────────
// Phase 4 — audio studio
// ─────────────────────────────────────────────────────────────────────────────

/// #36 audio_cache_stats
pub fn audio_cache_stats(ctx: &Ctx, _args: &Value) -> IpcResult {
    let cache = crate::audio::cache::AudioCache::new(&ctx.root);
    let s = cache.stats()?;
    Ok(json!({
        "root": "audio/segments",
        "chunks": s.chunks,
        "bytes": s.bytes,
        "hits": s.hits,
        "misses": s.misses,
        "hit_rate": if s.hits + s.misses > 0 {
            (s.hits as f64 / (s.hits + s.misses) as f64 * 1000.0).round() / 1000.0
        } else { 0.0 },
    }))
}

/// #37 audio_pronounce
pub fn audio_pronounce(_ctx: &Ctx, a: &Value) -> IpcResult {
    let names = a
        .get("names")
        .and_then(|v| v.as_array())
        .ok_or_else(|| AppError::audio("missing required array arg `names`"))?;
    if names.len() > 256 {
        return Err(AppError::audio("names batch limited to 256 entries"));
    }
    let list: Vec<String> = names
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_string)
                .ok_or_else(|| AppError::audio("names entries must be strings"))
        })
        .collect::<Result<_, _>>()?;
    Ok(crate::audio::g2p::pronounce(&list))
}

/// #38 audio_plan — topological lookahead schedule for a chapter's turns.
pub fn audio_plan(ctx: &Ctx, a: &Value) -> IpcResult {
    let chapter_id = args::req_i64(a, "chapter_id")?;
    let rows = chapter_turn_rows(&ctx.conn, chapter_id)?;
    if rows.is_empty() {
        return Err(AppError::not_found(format!("chapter {chapter_id} has no segments")));
    }
    let pairs: Vec<(String, u64)> = rows
        .iter()
        .map(|(speaker, text, dur)| {
            let est = if *dur > 0 { *dur } else { ((text.chars().count() as f64) * 55.0 + 400.0) as u64 };
            (speaker.clone(), est)
        })
        .collect();
    let mut plan = crate::audio::scheduler::plan_json(pairs)?;
    // Attach the per-turn segment view for the synthesizer pipeline.
    plan["segment_turns"] = json!(rows.iter().enumerate().map(|(i, (sp, text, _))| json!({
        "idx": i, "speaker": sp, "text_chars": text.chars().count(),
    })).collect::<Vec<_>>());
    Ok(plan)
}

/// Fetch (speaker, text, audio_duration_ms) in sequence order.
fn chapter_turn_rows(
    conn: &rusqlite::Connection,
    chapter_id: i64,
) -> Result<Vec<(String, String, u64)>, AppError> {
    // Chapter must exist (cheap existence check → ENTITY_NOT_FOUND otherwise).
    let exists: i64 = conn
        .query_row("SELECT COUNT(*) FROM chapters WHERE id = ?1", [chapter_id], |r| r.get(0))
        .map_err(AppError::from)?;
    if exists == 0 {
        return Err(AppError::not_found(format!("chapter {chapter_id} does not exist")));
    }
    let mut stmt = conn
        .prepare(
            "SELECT speaker_id, text_content, audio_duration_ms
             FROM chapter_segments WHERE chapter_id = ?1 ORDER BY sequence_order",
        )
        .map_err(AppError::from)?;
    let rows = stmt
        .query_map([chapter_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)? as u64))
        })
        .map_err(AppError::from)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(AppError::from)?);
    }
    Ok(out)
}

/// #39 audio_synthesize — single text or whole-chapter batch (continuity-chained).
pub fn audio_synthesize(ctx: &mut Ctx, a: &Value) -> IpcResult {
    let engine = crate::audio::tts::Engine::parse(
        args::get_str(a, "engine").as_deref().unwrap_or("SCRIPTED"),
    )?;
    let sample_rate = args::get_i64(a, "sample_rate").unwrap_or(24000) as u32;
    if !(8000..=96000).contains(&sample_rate) {
        return Err(AppError::audio(format!("sample_rate {sample_rate} outside 8000–96000")));
    }
    let ipa_names: Vec<String> = a
        .get("ipa_names")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default();

    if let Some(chapter_id) = args::get_i64(a, "chapter_id") {
        synthesize_chapter(ctx, a, chapter_id, engine, sample_rate, &ipa_names)
    } else {
        let text = args::req_str(a, "text")?;
        if text.trim().is_empty() {
            return Err(AppError::audio("text must not be empty"));
        }
        let voice = args::get_str(a, "voice").unwrap_or_else(|| "af_bella".into());
        let req = crate::audio::tts::TtsRequest {
            text: text.clone(),
            voice: voice.clone(),
            rate: args::get_f64(a, "rate").unwrap_or(1.0) as f32,
            pitch: args::get_f64(a, "pitch").unwrap_or(1.0) as f32,
            sample_rate,
            previous_request_id: None,
            pronunciations: Vec::new(),
        };
        let cache = crate::audio::cache::AudioCache::new(&ctx.root);
        let key = crate::audio::cache::AudioCache::key(engine.as_str(), &voice, sample_rate, "", &text);
        if let Some(hit) = cache.lookup(&key)? {
            return Ok(json!({ "cached": true, "hash": key, "engine": engine.as_str(),
                "path": rel_path(&ctx.root, &hit.wav_path), "meta": hit.meta }));
        }
        let audio = crate::audio::tts::synthesize_with(engine, &req)?;
        let stored = cache.store(&key, &audio.samples, audio.sample_rate, json!({
            "engine": engine.as_str(), "voice": voice, "text": text,
        }))?;
        Ok(json!({
            "cached": false,
            "hash": key,
            "engine": engine.as_str(),
            "path": rel_path(&ctx.root, &stored.wav_path),
            "duration_ms": stored.meta["duration_ms"],
            "request_id": audio.request_id,
        }))
    }
}

#[allow(clippy::too_many_arguments)]
fn synthesize_chapter(
    ctx: &mut Ctx,
    a: &Value,
    chapter_id: i64,
    engine: crate::audio::tts::Engine,
    sample_rate: u32,
    ipa_names: &[String],
) -> IpcResult {
    // Per-speaker voice cast.
    let mut voices: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    {
        let mut stmt = ctx
            .conn
            .prepare("SELECT character_key, voice_id FROM character_profiles")
            .map_err(AppError::from)?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(AppError::from)?;
        for r in rows {
            let (k, v) = r.map_err(AppError::from)?;
            voices.insert(k, v);
        }
    }
    let rows = chapter_turn_rows(&ctx.conn, chapter_id)?;
    if rows.is_empty() {
        return Err(AppError::not_found(format!("chapter {chapter_id} has no segments")));
    }
    // Segment ids in sequence order (for DB updates).
    let mut stmt = ctx
        .conn
        .prepare("SELECT segment_id FROM chapter_segments WHERE chapter_id = ?1 ORDER BY sequence_order")
        .map_err(AppError::from)?;
    let seg_ids: Vec<String> = stmt
        .query_map([chapter_id], |r| r.get(0))
        .map_err(AppError::from)?
        .collect::<Result<_, _>>()
        .map_err(AppError::from)?;

    let turns: Vec<crate::audio::scheduler::Turn> = rows
        .iter()
        .enumerate()
        .map(|(i, (speaker, text, dur))| {
            let est = if *dur > 0 { *dur } else { ((text.chars().count() as f64) * 55.0 + 400.0) as u64 };
            crate::audio::scheduler::Turn { idx: i, speaker: speaker.clone(), est_ms: est }
        })
        .collect();
    let plan = crate::audio::scheduler::plan(&turns)?;

    let cache = crate::audio::cache::AudioCache::new(&ctx.root);
    let dry_run = a.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(false);
    let mut results: Vec<Value> = Vec::with_capacity(rows.len());
    let mut last_request_by_speaker: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut cache_hits = 0usize;

    for &turn_idx in &plan.order {
        let (speaker, text, _) = &rows[turn_idx];
        let seg_id = &seg_ids[turn_idx];
        let voice = voices
            .get(speaker)
            .cloned()
            .unwrap_or_else(|| "af_bella".to_string());
        let prev = plan.prev_request_of.get(&turn_idx).and_then(|p| {
            last_request_by_speaker.get(&rows[*p].0).cloned()
        });
        let req = crate::audio::tts::TtsRequest {
            text: text.clone(),
            voice: voice.clone(),
            rate: 1.0,
            pitch: 1.0,
            sample_rate,
            previous_request_id: prev.clone(),
            pronunciations: ipa_names
                .iter()
                .filter(|n| text.contains(n.as_str()))
                .map(|n| {
                    let (ipa, _) = crate::audio::g2p::to_ipa(n);
                    (n.clone(), ipa)
                })
                .collect(),
        };
        let key = crate::audio::cache::AudioCache::key(engine.as_str(), &voice, sample_rate, "", text);
        let (hit, duration_ms, request_id, rel) = if let Some(hit) = cache.lookup(&key)? {
            cache_hits += 1;
            let d = hit.meta["duration_ms"].as_u64().unwrap_or(0);
            let rid = hit.meta["request_id"].as_str().map(str::to_string);
            let rel = rel_path(&ctx.root, &hit.wav_path);
            (true, d, rid, rel)
        } else {
            if dry_run {
                results.push(json!({ "idx": turn_idx, "segment_id": seg_id, "speaker": speaker, "dry_run": true }));
                continue;
            }
            let audio = crate::audio::tts::synthesize_with(engine, &req)?;
            let stored = cache.store(&key, &audio.samples, audio.sample_rate, json!({
                "engine": engine.as_str(), "voice": voice, "text": text,
                "request_id": audio.request_id, "previous_request_id": prev,
            }))?;
            let d = stored.meta["duration_ms"].as_u64().unwrap_or(0);
            let rel = rel_path(&ctx.root, &stored.wav_path);
            (false, d, audio.request_id.clone(), rel)
        };
        if let Some(rid) = &request_id {
            last_request_by_speaker.insert(speaker.clone(), rid.clone());
        }
        if !hit && !dry_run {
            // Update the segment row: cache pointer + duration + continuity id
            // (prev = the speaker's previous turn request id, None for firsts).
            ctx.conn
                .execute(
                    "UPDATE chapter_segments SET audio_hash = ?1, audio_cache_path = ?2,
                     audio_duration_ms = ?3, prev_request_id = ?4, last_modified_at = strftime('%s','now')
                     WHERE segment_id = ?5",
                    rusqlite::params![key, rel, duration_ms as i64, prev, seg_id],
                )
                .map_err(AppError::from)?;
        }
        results.push(json!({
            "idx": turn_idx,
            "segment_id": seg_id,
            "speaker": speaker,
            "hash": key,
            "path": rel,
            "cached": hit,
            "duration_ms": duration_ms,
            "request_id": request_id,
            "previous_request_id": prev,
        }));
    }

    // Restore the manuscript order in the payload.
    results.sort_by_key(|r| r["idx"].as_u64().unwrap_or(0));
    ctx.after_commit()?;
    Ok(json!({
        "chapter_id": chapter_id,
        "engine": engine.as_str(),
        "turns": results.len(),
        "cache_hits": cache_hits,
        "waves": plan.waves.len(),
        "results": results,
    }))
}

/// #40 audio_master — 3-layer mix → two-pass normalize → limiter → WAV.
pub fn audio_master(ctx: &mut Ctx, a: &Value) -> IpcResult {
    let layers_v = a
        .get("layers")
        .and_then(|v| v.as_array())
        .ok_or_else(|| AppError::audio("missing required array arg `layers`"))?;
    if layers_v.is_empty() || layers_v.len() > 16 {
        return Err(AppError::audio("layers must contain 1–16 entries"));
    }
    let mut layers = Vec::new();
    for (i, lv) in layers_v.iter().enumerate() {
        let spec = crate::audio::mixer::parse_layer_args_bundle(&ctx.root, lv)
            .map_err(|e| {
                // Preserve the canonical code for security violations; only
                // shape errors get the extra layer context.
                if e.category == crate::error::Category::Audio {
                    AppError::audio(format!("layer {i}: {}", e.message))
                } else {
                    e
                }
            })?;
        layers.push(spec);
    }
    let out_rate = args::get_i64(a, "sample_rate").unwrap_or(24000) as u32;
    if !(8000..=96000).contains(&out_rate) {
        return Err(AppError::audio(format!("sample_rate {out_rate} outside 8000–96000")));
    }
    let duck = if a.get("duck").and_then(|v| v.as_bool()).unwrap_or(true) {
        let d = a.get("duck_params");
        Some(crate::audio::mixer::DuckParams {
            depth_db: d.and_then(|x| x.get("depth_db")).and_then(|v| v.as_f64()).unwrap_or(18.0) as f32,
            attack_ms: d.and_then(|x| x.get("attack_ms")).and_then(|v| v.as_f64()).unwrap_or(120.0) as f32,
            release_ms: d.and_then(|x| x.get("release_ms")).and_then(|v| v.as_f64()).unwrap_or(750.0) as f32,
        })
    } else {
        None
    };
    let target_rms = args::get_f64(a, "target_rms_db").unwrap_or(crate::audio::master::DEFAULT_TARGET_RMS_DB as f64) as f32;
    let ceiling = args::get_f64(a, "ceiling_db").unwrap_or(crate::audio::master::DEFAULT_CEILING_DB as f64) as f32;

    let mixed = crate::audio::mixer::mix(&layers, out_rate, duck.as_ref())?;
    if mixed.samples.is_empty() {
        return Err(AppError::audio("mix produced no samples"));
    }
    let (mastered, metrics) = crate::audio::master::master(&mixed.samples, out_rate, target_rms, ceiling);
    let report = crate::audio::master::validate_acx(&mastered, out_rate);

    // Output is always written inside <bundle>/exports/ (sandbox-resolved).
    let out_name = args::get_str(a, "out").unwrap_or_else(|| "mastered.wav".into());
    let out_path = out_path_in_exports(&ctx.root, &out_name)?;
    crate::audio::wav::write_wav(&out_path, &mastered, out_rate)?;

    ctx.after_commit()?;
    Ok(json!({
        "out": rel_path(&ctx.root, &out_path),
        "sample_rate": out_rate,
        "layers": mixed.layers,
        "duck": crate::audio::mixer::duck_summary(mixed.ducked_samples, out_rate),
        "metrics": metrics.to_json(),
        "acx": report.to_json(),
    }))
}

/// #41 audio_validate — ACX compliance report for a bundled WAV.
pub fn audio_validate(ctx: &Ctx, a: &Value) -> IpcResult {
    let rel = args::req_str(a, "wav")?;
    let path = crate::security::sandbox::resolve_in_root(&ctx.root, std::path::Path::new(&rel))?;
    let data = crate::audio::wav::read_wav(&path)?;
    if data.samples.is_empty() {
        return Err(AppError::audio("wav contains no samples"));
    }
    Ok(crate::audio::master::validate_acx(&data.samples, data.sample_rate).to_json())
}

// ─────────────────────────────────────────────────────────────────────────────
// Phase 5 — vector RAG + local AI
// ─────────────────────────────────────────────────────────────────────────────

/// #42 rag_index
pub fn rag_index(ctx: &mut Ctx, a: &Value) -> IpcResult {
    crate::rag::store::ensure_rag_schema(&ctx.conn)?;
    let embedder_name = args::get_str(a, "embedder").unwrap_or_else(|| "hashing".into());
    let emb = crate::rag::embed::resolve(&embedder_name, &ctx.root)?;
    let sources: Vec<String> = match a.get("sources") {
        Some(Value::Array(arr)) if !arr.is_empty() => arr
            .iter()
            .map(|v| {
                v.as_str().map(str::to_string)
                    .ok_or_else(|| AppError::ai("sources entries must be strings"))
            })
            .collect::<Result<_, _>>()?,
        _ => crate::rag::index::SOURCES.iter().map(|s| s.to_string()).collect(),
    };
    let book_id = args::get_i64(a, "book_id");
    let mut out = crate::rag::index::index(&ctx.conn, &sources, book_id, emb.as_ref())?;
    ctx.after_commit()?;
    out["sources_requested"] = json!(sources);
    Ok(out)
}

/// #43 rag_search
pub fn rag_search(ctx: &Ctx, a: &Value) -> IpcResult {
    crate::rag::store::ensure_rag_schema(&ctx.conn)?;
    let query = args::req_str(a, "query")?;
    let k = args::get_i64(a, "k").unwrap_or(5).clamp(1, 50) as usize;
    let mode = args::get_str(a, "mode").unwrap_or_else(|| "hybrid".into());
    let embedder_name = args::get_str(a, "embedder").unwrap_or_else(|| "hashing".into());
    let emb = crate::rag::embed::resolve(&embedder_name, &ctx.root)?;
    crate::rag::search::search(&ctx.conn, emb.as_ref(), &query, k, &mode)
}

/// #44 rag_status
pub fn rag_status(ctx: &Ctx, _a: &Value) -> IpcResult {
    crate::rag::store::ensure_rag_schema(&ctx.conn)?;
    Ok(json!({
        "chunks": crate::rag::store::count(&ctx.conn)?,
        "by_source": crate::rag::store::count_by_source(&ctx.conn)?,
        "embedder": "hashing-384",
        "dim": crate::rag::embed::DEFAULT_DIM,
        "modes": ["dense", "fts", "hybrid"],
    }))
}

/// #45 local_llm_generate
pub fn local_llm_generate(_ctx: &Ctx, a: &Value) -> IpcResult {
    let prompt = args::req_str(a, "prompt")?;
    if prompt.trim().is_empty() {
        return Err(AppError::ai("prompt must not be empty"));
    }
    let req = crate::localai::llm::GenerateRequest {
        prompt,
        max_tokens: args::get_i64(a, "max_tokens").unwrap_or(64).clamp(1, 4096) as usize,
        temperature: args::get_f64(a, "temperature").unwrap_or(0.7),
        seed: args::get_i64(a, "seed").unwrap_or(42) as u64,
        grammar: args::get_str(a, "grammar"),
        server_url: args::get_str(a, "server_url"),
    };
    crate::localai::llm::generate(&req)
}

/// #46 local_llm_status
pub fn local_llm_status(_ctx: &Ctx, _a: &Value) -> IpcResult {
    Ok(crate::localai::llm::status())
}

/// #47 local_tts_generate
pub fn local_tts_generate(ctx: &mut Ctx, a: &Value) -> IpcResult {
    let text = args::req_str(a, "text")?;
    let voice = args::get_str(a, "voice").unwrap_or_else(|| "af_bella".into());
    crate::localai::tts::generate(&ctx.root, &text, &voice)
}

/// #48 local_ai_status
pub fn local_ai_status(ctx: &Ctx, _a: &Value) -> IpcResult {
    let mut out = crate::localai::tts::status(&ctx.root);
    if let Ok(status) = crate::audio::cache::AudioCache::new(&ctx.root).stats() {
        out["audio_cache"] = json!({ "chunks": status.chunks, "bytes": status.bytes });
    }
    Ok(out)
}

// ─────────────────────────────────────────────────────────────────────────────
// helpers
// ─────────────────────────────────────────────────────────────────────────────

fn rel_path(root: &std::path::Path, p: &std::path::Path) -> String {
    p.strip_prefix(root)
        .map(|r| r.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| p.to_string_lossy().into_owned())
}

/// Resolve an output file name inside `<bundle>/exports/`. Bare file names
/// only — separators, `..` and drive prefixes are rejected outright.
fn out_path_in_exports(root: &std::path::Path, name: &str) -> Result<PathBuf, AppError> {
    if name.contains('/') || name.contains('\\') || name.contains("..") || name.contains(':') {
        return Err(AppError::traversal(format!(
            "output name must be a bare file name inside exports/: `{name}`"
        )));
    }
    let clean = std::path::Path::new(name)
        .file_name()
        .ok_or_else(|| AppError::audio(format!("invalid output name `{name}`")))?
        .to_string_lossy()
        .into_owned();
    if !clean.ends_with(".wav") {
        return Err(AppError::audio(format!("output name must end with .wav, got `{clean}`")));
    }
    let exports = root.join("exports");
    std::fs::create_dir_all(&exports).map_err(|e| AppError::audio(format!("create exports dir: {e}")))?;
    Ok(exports.join(clean))
}
