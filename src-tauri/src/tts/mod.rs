// AUTHR_OS — Lookahead TTS: content-addressable synthesis, multi-engine
// connectors, topological DAG scheduling with acoustic continuity (Sprints 9–10).

pub mod cache;
pub mod dag_scheduler;
pub mod edge_tts;
pub mod engines;
pub mod elevenlabs;
pub mod cartesia;
pub mod hf_mms;
pub mod latent;
pub mod openai;
pub mod phonemize;

use crate::error::AppError;
use crate::ipc::{Ctx, args};
use serde_json::{Value, json};

use cache::ChunkKeyParts;
use engines::{Route, SynthRequest};

/// IPC #36 `tts_synthesize_segment` — single-segment synthesis with cache.
pub fn synthesize_segment(ctx: &mut Ctx, a: &Value) -> Result<Value, AppError> {
    let segment_id = args::req_str(a, "segment_id")?;
    let mut job = engine_for_segment(&ctx.conn, &segment_id)?;
    // Single-segment rendering runs on the base voice unless conditioning is
    // explicitly requested — keeps identical re-renders cache-stable.
    // Use `tts_synthesize_chapter` for continuity-conditioned lane rendering.
    let want_condition = a.get("condition").and_then(Value::as_bool).unwrap_or(false);
    let latent = latent::load(&ctx.root, &job.speaker);
    job.condition = want_condition && latent.is_some();
    let prev = ctx
        .conn
        .query_row(
            "SELECT prev_request_id FROM chapter_segments WHERE segment_id = ?1",
            rusqlite::params![&segment_id],
            |r| r.get::<_, Option<String>>(0),
        )
        .ok()
        .flatten();
    let result = run_job(ctx, &job, latent.as_ref(), prev.as_deref())?;
    Ok(result)
}

/// IPC #38 `audio_cache_status` — chunk inventory + per-speaker latents.
pub fn cache_status(ctx: &Ctx) -> Result<Value, AppError> {
    let st = cache::status(&ctx.root)?;
    let rendered: i64 = ctx
        .conn
        .query_row(
            "SELECT COUNT(*) FROM chapter_segments WHERE audio_hash IS NOT NULL AND audio_cache_path IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .map_err(|e| AppError::storage(e.to_string()))?;
    let invalidated: i64 = ctx
        .conn
        .query_row(
            "SELECT COUNT(*) FROM chapter_segments WHERE audio_hash IS NULL",
            [],
            |r| r.get(0),
        )
        .map_err(|e| AppError::storage(e.to_string()))?;
    let speakers = latent::speakers_cached(&ctx.root);
    Ok(json!({
        "chunks": st.chunks,
        "bytes": st.bytes,
        "segments_rendered": rendered,
        "segments_invalidated": invalidated,
        "speakers_with_latent": speakers,
        "chunk_sample_rate_hz": crate::audio::dsp::CHUNK_FS,
    }))
}

/// IPC #39 `audio_cache_invalidate` — drop audio_hash for one segment (with
/// the §6.3 blast-radius rule: propagate to the immediate next turn of the
/// same speaker within the chapter, never further downstream).
pub fn invalidate(ctx: &mut Ctx, a: &Value) -> Result<Value, AppError> {
    let segment_id = args::req_str(a, "segment_id")?;
    let downstream = a.get("downstream").and_then(Value::as_bool).unwrap_or(false);
    let row: Option<(i64, String)> = ctx
        .conn
        .query_row(
            "SELECT chapter_id, speaker_id FROM chapter_segments WHERE segment_id = ?1",
            rusqlite::params![&segment_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    let (chapter_id, speaker) = match row {
        Some(v) => v,
        None => return Err(AppError::not_found(format!("segment `{segment_id}` not found"))),
    };
    let mut invalidated = vec![segment_id.clone()];
    ctx.conn
        .execute(
            "UPDATE chapter_segments SET audio_hash = NULL, audio_cache_path = NULL,
                    audio_duration_ms = 0 WHERE segment_id = ?1",
            rusqlite::params![&segment_id],
        )
        .map_err(|e| AppError::storage(e.to_string()))?;
    if downstream {
        let next: Option<String> = ctx
            .conn
            .query_row(
                "SELECT segment_id FROM chapter_segments
                 WHERE chapter_id = ?1 AND speaker_id = ?2 AND sequence_order >
                       (SELECT sequence_order FROM chapter_segments WHERE segment_id = ?3)
                 ORDER BY sequence_order LIMIT 1",
                rusqlite::params![chapter_id, speaker, &segment_id],
                |r| r.get(0),
            )
            .ok();
        if let Some(next_id) = next {
            ctx.conn
                .execute(
                    "UPDATE chapter_segments SET audio_hash = NULL, audio_cache_path = NULL,
                            audio_duration_ms = 0 WHERE segment_id = ?1",
                    rusqlite::params![&next_id],
                )
                .map_err(|e| AppError::storage(e.to_string()))?;
            invalidated.push(next_id);
        }
    }
    ctx.after_commit()?;
    Ok(json!({ "invalidated": invalidated, "downstream_applied": downstream }))
}

// ── shared job plumbing ──────────────────────────────────────────────────────

#[derive(Clone)]
pub struct SegmentJob {
    pub segment_id: String,
    pub speaker: String,
    pub voice_engine: String,
    pub voice_id: String,
    pub base_rate: f64,
    pub base_pitch: f64,
    pub text: String,
    pub emotion: String,
    pub prosody_rate: f64,
    pub prosody_pitch: f64,
    pub condition: bool, // lane turn > 0 ⇒ conditioned on upstream latent
}

/// Resolve a segment row into engine parameters (speaker profile lookup,
/// NARRATOR default = KOKORO_LOCAL local stack).
pub fn engine_for_segment(conn: &rusqlite::Connection, segment_id: &str) -> Result<SegmentJob, AppError> {
    let seg = crate::storage::segments::get(conn, segment_id)?;
    let speaker = seg["speaker_id"].as_str().unwrap_or("NARRATOR").to_string();
    let profile = conn
        .query_row(
            "SELECT voice_engine, voice_id, base_rate, base_pitch FROM character_profiles
             WHERE character_key = ?1",
            rusqlite::params![&speaker],
            |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, f64>(2)?,
                    r.get::<_, f64>(3)?,
                ))
            },
        )
        .ok();
    let (engine, voice_id, base_rate, base_pitch) = match profile {
        Some((e, v, br, bp)) => (
            e.unwrap_or_else(|| "KOKORO_LOCAL".into()),
            v.unwrap_or_else(|| "local_default".into()),
            br,
            bp,
        ),
        None => ("KOKORO_LOCAL".to_string(), "narrator_default".to_string(), 1.0, 1.0),
    };
    Ok(SegmentJob {
        segment_id: segment_id.to_string(),
        speaker,
        voice_engine: engine,
        voice_id,
        base_rate,
        base_pitch,
        text: seg["text_content"].as_str().unwrap_or_default().to_string(),
        emotion: seg["emotion"].as_str().unwrap_or("neutral").to_string(),
        prosody_rate: seg["prosody_rate"].as_f64().unwrap_or(1.0),
        prosody_pitch: seg["prosody_pitch"].as_f64().unwrap_or(1.0),
        condition: false,
    })
}

/// Thread-safe synthesis core (no DB access): cache lookup → engine route →
/// chunk write → latent update. Used by the single-segment IPC command and by
/// the DAG scheduler's parallel workers. Engine failures degrade to the base
/// voice per §6.3 instead of failing the pipeline.
pub struct CoreOutcome {
    pub key: String,
    pub cache_hit: bool,
    pub conditioned: bool,
    pub status: String, // synthesized | cache_hit | degraded
    pub engine: String,
    pub request_id: String,
    pub duration_ms: i64,
    pub fs: u32,
    pub latent: Option<latent::Latent>,
    pub note: String,
}

pub fn synthesize_core(
    root: &std::path::Path,
    job: &SegmentJob,
    upstream_latent: Option<&latent::Latent>,
    prev_request_id: Option<&str>,
) -> Result<CoreOutcome, AppError> {
    let conditioned = job.condition && upstream_latent.is_some();
    let engine = engines::resolve(
        &job.voice_engine,
        &job.voice_id,
        job.base_rate * job.prosody_rate,
        job.base_pitch * job.prosody_pitch,
    );
    let rate = job.base_rate * job.prosody_rate;
    let pitch = job.base_pitch * job.prosody_pitch;
    let key_parts = ChunkKeyParts {
        engine: &engine.name,
        voice_id: &job.voice_id,
        speaker_fp: &cache::speaker_fingerprint(&job.speaker, &engine.name, &job.voice_id),
        text: &job.text,
        rate,
        pitch,
        emotion: &job.emotion,
        conditioned,
    };
    let key = cache::chunk_key(&key_parts);

    // ── cache hit (sub-second path) ─────────────────────────────────────────
    if let Some(samples) = cache::read_chunk(root, &key)? {
        let _ = samples.len();
        let request_id = format!("cache_{}", &key[..16.min(key.len())]);
        return Ok(CoreOutcome {
            key,
            cache_hit: true,
            conditioned,
            status: "cache_hit".into(),
            engine: engine.name,
            request_id,
            duration_ms: 0, // caller re-reads from DB row or chunk if needed
            fs: crate::audio::dsp::CHUNK_FS,
            latent: latent::load(root, &job.speaker),
            note: String::new(),
        });
    }

    // ── synthesis route ──────────────────────────────────────────────────────
    let req = SynthRequest {
        text: job.text.clone(),
        speaker: job.speaker.clone(),
        voice_id: job.voice_id.clone(),
        rate,
        pitch,
        emotion: job.emotion.clone(),
        condition: upstream_latent,
        prev_request_id: prev_request_id.map(str::to_string),
    };
    let (out, status, note) = match &engine.route {
        Route::Local(e) | Route::Cloud(e) => match e.synthesize(&req) {
            Ok(o) => (o, "synthesized", String::new()),
            Err(err) => {
                // §6.3 resilient fallback: never fail the pipeline
                let mut o = engines::base_voice_synthesis(&req);
                o.degraded = true;
                o.note = format!("engine {} failed → base voice: {}", engine.name, err.message);
                let note = o.note.clone();
                (o, "degraded", note)
            }
        },
        Route::OfflineFallback(e) => {
            let mut o = e.synthesize(&req)?;
            o.degraded = true;
            o.note = "offline_mode: AUTHR_TTS_LIVE not set; cloud connector bypassed".to_string();
            let note = o.note.clone();
            (o, "degraded", note)
        }
    };

    cache::write_chunk(root, &key, &out.samples, out.fs)?;
    // speaker latent: 1.5 s tail of the fresh synthesis
    let lat = latent::extract(&job.speaker, &out.samples, out.fs);
    latent::save(root, &lat)?;

    Ok(CoreOutcome {
        duration_ms: (out.samples.len() as f64 / out.fs as f64 * 1000.0).round() as i64,
        key,
        cache_hit: false,
        conditioned,
        status: status.into(),
        engine: engine.name,
        request_id: out.request_id,
        fs: out.fs,
        latent: Some(lat),
        note,
    })
}

/// IPC #36 single-segment path: core synthesis + DB persistence.
pub fn run_job(
    ctx: &mut Ctx,
    job: &SegmentJob,
    upstream_latent: Option<&latent::Latent>,
    prev_request_id: Option<&str>,
) -> Result<Value, AppError> {
    let out = synthesize_core(&ctx.root, job, upstream_latent, prev_request_id)?;
    persist_job(&ctx.conn, job, &out, prev_request_id)?;
    ctx.after_commit()?;
    Ok(job_record(job, &out, upstream_latent, None, 0))
}

/// DB persistence for one core outcome (shared by single + DAG paths).
/// `prev_request_id` column = pointer to the UPSTREAM turn's request id
/// (build plan §5.3: "Pointer to speaker's prior segment for acoustic latents").
pub fn persist_job(
    conn: &rusqlite::Connection,
    job: &SegmentJob,
    out: &CoreOutcome,
    upstream_request_id: Option<&str>,
) -> Result<(), AppError> {
    let rel_path = format!("audio/segments/{}.wav", out.key);
    conn.execute(
        "UPDATE chapter_segments SET audio_hash = ?1, audio_cache_path = ?2,
                audio_duration_ms = ?3, prev_request_id = ?4 WHERE segment_id = ?5",
        rusqlite::params![
            out.key,
            rel_path,
            out.duration_ms,
            upstream_request_id,
            job.segment_id
        ],
    )
    .map_err(|e| AppError::storage(e.to_string()))?;
    Ok(())
}

pub fn job_record(
    job: &SegmentJob,
    out: &CoreOutcome,
    upstream_latent: Option<&latent::Latent>,
    depends_on: Option<&str>,
    index_in_lane: usize,
) -> Value {
    let rel_path = format!("audio/segments/{}.wav", out.key);
    json!({
        "segment_id": job.segment_id,
        "speaker_id": job.speaker,
        "index_in_lane": index_in_lane,
        "depends_on": depends_on,
        "engine": out.engine,
        "status": out.status,
        "cache_hit": out.cache_hit,
        "conditioned": out.conditioned,
        "audio_hash": out.key,
        "audio_cache_path": rel_path,
        "audio_duration_ms": out.duration_ms,
        "request_id": out.request_id,
        "note": out.note,
        "upstream_latent": upstream_latent.map(|l| json!({
            "speaker": l.speaker, "pitch_hz": (l.pitch_hz * 10.0).round() / 10.0,
            "rms_dbfs": (l.rms_dbfs * 10.0).round() / 10.0,
        })),
    })
}
