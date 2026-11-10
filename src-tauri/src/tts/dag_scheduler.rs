// AUTHR_OS — Topological Lookahead TTS DAG Scheduler (Sprint 10, §6.3).
//
// Speaker Dependency Graph: consecutive dialogue turns of the SAME speaker are
// serialized (turn k conditions on the 1.5 s acoustic latent of turn k−1);
// turns of DIFFERENT speakers are independent and synthesize in parallel.
//
//   Lane 1 (NARRATOR):    [101] ─────► [105]
//   Lane 2 (CHAR_KIRA):   [102] ─────► [104]      (lanes run in parallel)
//   Lane 3 (CHAR_JULIAN): [103] ─────► [106]
//
// Execution model: wave k = the k-th turn of every lane, dispatched across a
// bounded worker pool (max_workers, default 4). Deadlock-free degradation:
// if an upstream turn fails (engine error, or the 15 s latent timeout), its
// dependent immediately synthesizes with the BASE character voice instead of
// blocking or failing the pipeline. Cache-invalidation blast radius stays
// bounded to one downstream turn because the cache key fingerprints the
// speaker (not the per-turn latent content).

use crate::error::AppError;
use crate::ipc::{Ctx, args};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::time::Instant;

use super::latent::{self, Latent};
use super::engines::{Route, SynthRequest};
use super::{CoreOutcome, SegmentJob, cache, engines, synthesize_core};

pub const LATENT_WAIT_SECS: u64 = 15;

/// IPC #37 `tts_synthesize_chapter`.
pub fn synthesize_chapter(ctx: &mut Ctx, a: &Value) -> Result<Value, AppError> {
    let chapter_id = args::req_i64(a, "chapter_id")?;
    let max_workers = (args::get_i64(a, "max_workers").unwrap_or(4)).clamp(1, 16) as usize;
    // Cross-chapter continuity opt-in: wave-0 turns condition on the speaker's
    // persisted latent from earlier chapters (default off keeps re-renders
    // cache-stable; the key fingerprints the speaker, not the latent content).
    let continuity = a.get("continuity").and_then(Value::as_bool).unwrap_or(false);

    let segs = crate::storage::segments::list(&ctx.conn, chapter_id)?;
    let segs = segs["segments"].as_array().cloned().unwrap_or_default();
    if segs.is_empty() {
        return Err(AppError::not_found(format!("chapter {chapter_id} has no segments")));
    }

    // ── 1. Build speaker lanes in first-appearance order ──────────────────────
    let mut lane_order: Vec<String> = Vec::new();
    let mut lanes: HashMap<String, Vec<Value>> = HashMap::new();
    for seg in &segs {
        let speaker = seg["speaker_id"].as_str().unwrap_or("NARRATOR").to_string();
        if !lanes.contains_key(&speaker) {
            lane_order.push(speaker.clone());
        }
        lanes.entry(speaker).or_default().push(seg.clone());
    }
    let lane_count = lane_order.len();
    let waves = lanes.values().map(|v| v.len()).max().unwrap_or(0);

    // ── 2. Wave execution ──────────────────────────────────────────────────────
    let started = Instant::now();
    let mut jobs: Vec<Value> = Vec::new();
    // per-lane upstream state: latent + request id of the previous turn
    let mut lane_latent: HashMap<String, Option<Latent>> = lane_order.iter().map(|s| (s.clone(), None)).collect();
    let mut lane_prev_req: HashMap<String, Option<String>> =
        lane_order.iter().map(|s| (s.clone(), None)).collect();
    let mut lane_upstream_ok: HashMap<String, bool> = lane_order.iter().map(|s| (s.clone(), true)).collect();

    for wave in 0..waves {
        // ready set: the wave-th turn of every lane that has one
        let mut ready: Vec<(String, usize, SegmentJob)> = Vec::new();
        for speaker in &lane_order {
            let lane = &lanes[speaker];
            if wave < lane.len() {
                let seg = &lane[wave];
                let sid = seg["segment_id"].as_str().unwrap_or_default().to_string();
                let mut job = super::engine_for_segment(&ctx.conn, &sid)?;
                job.condition = wave > 0; // consecutive-turn conditioning
                if wave == 0 && continuity {
                    // seed the lane with the persisted cross-chapter latent so
                    // the job record reports what it actually consumed
                    if let Some(lat) = latent::load(&ctx.root, &speaker) {
                        job.condition = true;
                        lane_latent.insert(speaker.clone(), Some(lat));
                    }
                }
                ready.push((speaker.clone(), wave, job));
            }
        }

        // bounded parallel dispatch across the ready set
        let chunks: Vec<Vec<(String, usize, SegmentJob)>> =
            ready.chunks(max_workers).map(<[(String, usize, SegmentJob)]>::to_vec).collect();
        for chunk in chunks {
            let mut handles = Vec::with_capacity(chunk.len());
            for (speaker, idx, job) in chunk {
                let root = ctx.root.clone();
                let upstream = if idx > 0 && lane_upstream_ok[&speaker] {
                    lane_latent[&speaker].clone()
                } else if idx == 0 && continuity {
                    latent::load(&ctx.root, &speaker)
                } else {
                    None
                };
                let degraded_upstream = idx > 0 && !lane_upstream_ok[&speaker];
                let prev_req = lane_prev_req[&speaker].clone();
                let prev_upstream_req = if idx > 0 { prev_req.clone() } else { None };
                handles.push((
                    (speaker, prev_upstream_req),
                    std::thread::spawn(move || {
                        let outcome = if degraded_upstream {
                            // §6.3 deadlock-free degradation: upstream failure →
                            // base voice, no conditioning, no pipeline failure
                            match synthesize_core(&root, &job, None, prev_req.as_deref()) {
                                Ok(mut o) => {
                                    o.conditioned = false;
                                    o.status = "degraded".into();
                                    o.note = format!(
                                        "upstream turn failed/timeout — base voice fallback (§6.3, waited ≤{LATENT_WAIT_SECS}s)"
                                    );
                                    o.latent = latent::load(&root, &job.speaker);
                                    Ok(o)
                                }
                                Err(e) => Err(e),
                            }
                        } else {
                            synthesize_core(&root, &job, upstream.as_ref(), prev_req.as_deref())
                        };
                        (job, idx, outcome)
                    }),
                ));
            }

            for ((speaker, prev_upstream_req), handle) in handles {
                let (job, idx, outcome) = handle
                    .join()
                    .map_err(|_| AppError::storage("dag worker panicked"))?;
                let upstream = lane_latent[&speaker].clone();
                let depends_on: Option<String> = if idx > 0 {
                    lanes[&speaker][idx - 1]["segment_id"].as_str().map(str::to_string)
                } else {
                    None
                };
                match outcome {
                    Ok(out) => {
                        lane_upstream_ok.insert(speaker.clone(), true);
                        if let Some(l) = &out.latent {
                            lane_latent.insert(speaker.clone(), Some(l.clone()));
                        }
                        lane_prev_req.insert(
                            speaker.clone(),
                            Some(if out.request_id.is_empty() {
                                format!("cache_{}", &out.key[..16.min(out.key.len())])
                            } else {
                                out.request_id.clone()
                            }),
                        );
                        crate::tts::persist_job(&ctx.conn, &job, &out, prev_upstream_req.as_deref())?;
                        jobs.push(crate::tts::job_record(
                            &job, &out, upstream.as_ref(), depends_on.as_deref(), idx,
                        ));
                    }
                    Err(err) => {
                        // core failure (cache IO etc.) → mark lane degraded
                        lane_upstream_ok.insert(speaker.clone(), false);
                        jobs.push(json!({
                            "segment_id": job.segment_id,
                            "speaker_id": job.speaker,
                            "index_in_lane": idx,
                            "depends_on": depends_on,
                            "status": "failed",
                            "engine": job.voice_engine,
                            "error": err.message,
                        }));
                    }
                }
            }
        }
    }

    ctx.after_commit()?;
    let elapsed_ms = started.elapsed().as_millis() as u64;
    let synthesized = jobs.iter().filter(|j| j["status"] == "synthesized").count();
    let cache_hits = jobs.iter().filter(|j| j["status"] == "cache_hit").count();
    let degraded = jobs.iter().filter(|j| j["status"] == "degraded").count();
    let failed = jobs.iter().filter(|j| j["status"] == "failed").count();

    Ok(json!({
        "chapter_id": chapter_id,
        "jobs": jobs,
        "lanes": lane_count,
        "waves": waves,
        "max_workers": max_workers,
        "synthesized": synthesized,
        "cache_hits": cache_hits,
        "degraded": degraded,
        "failed": failed,
        "elapsed_ms": elapsed_ms,
        "continuity": {
            "latent_window_ms": latent::LATENT_WINDOW_MS,
            "latent_wait_secs": LATENT_WAIT_SECS,
            "speaker_fingerprint_key": true,
        },
    }))
}

///used by tests: build the lane graph without executing.
pub fn build_lanes(segments: &[Value]) -> Vec<(String, Vec<String>)> {
    let mut order: Vec<String> = Vec::new();
    let mut lanes: HashMap<String, Vec<String>> = HashMap::new();
    for seg in segments {
        let speaker = seg["speaker_id"].as_str().unwrap_or("NARRATOR").to_string();
        if !lanes.contains_key(&speaker) {
            order.push(speaker.clone());
        }
        lanes
            .entry(speaker)
            .or_default()
            .push(seg["segment_id"].as_str().unwrap_or_default().to_string());
    }
    order.into_iter().map(|s| { let v = lanes[&s].clone(); (s, v) }).collect()
}

#[allow(unused)]
fn _unused(_: Route, _: SynthRequest, _: CoreOutcome, _: engines::ResolvedEngine, _: &cache::ChunkKeyParts) {}
