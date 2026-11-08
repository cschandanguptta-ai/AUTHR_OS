# ADR-0004 — Content-addressable audio cache + DAG lookahead scheduling

**Status**: Accepted (Phase 4; `test_t1_audio_surface.py`,
`test_t1_tts_engines.py`, `test_t4_audio_rag_scenario.py`)

**Context**: Audiobook synthesis re-generates identical lines constantly
(retries, re-renders, unchanged chapters), and engines charge per request;
acoustic continuity needs the *same speaker's previous line's* engine request
id, which serializes same-speaker turns.

**Decision**:
- Cache key = `sha256(engine|voice|sample_rate|ipa|text)` — pure, no
  timestamps (`audio/cache.rs:37-50`); chunks stored as
  `audio/segments/<hash>.wav` + `.json` meta + cumulative `stats.json`.
- Synthesis order comes from a topological DAG (`audio/scheduler.rs::plan:54`):
  edges chain consecutive turns of the same speaker (continuity), independent
  speakers land in the same wave; Kahn layering with longest-first in-wave
  ordering (`scheduler.rs:79`).
- `audio_synthesize` walks `plan.order`, threads `previous_request_id` through
  `last_request_by_speaker`, and **stamps every segment row — including cache
  hits** (`ipc/studio.rs:212-289`; regression for bug B-1).
- The continuity *preview* in `audio_plan` derives ids from the cache-key
  function, not a lookup (`scheduler.rs:130-137`).

**Alternatives**: per-chapter files (no dedup) — not implemented; eager
network parallelism ignoring speaker chains — would break
`previous_request_ids` continuity (ElevenLabs), the documented Phase 4 goal.

**Consequences**: identical (engine,voice,rate,ipa,text) is synthesized once
per bundle ever; changing voice/rate/text produces a new hash; cache hits
cannot reproduce engine-side continuity for re-synthesized audio (stored
request ids are reused — see `invariants-and-gotchas.md §3`).

**Do not change unless**: you accept invalidating every existing cache chunk
and re-validating `audio_surface` + `tts_engines` + tier-4 audio scenario, and
you keep hit-branch stamping intact.

**Evidence**: `src-tauri/src/audio/cache.rs`, `scheduler.rs`,
`ipc/studio.rs:212-289`; bug B-1 fix in git `2ab4eb8`.
