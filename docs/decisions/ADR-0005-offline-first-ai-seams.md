# ADR-0005 — Deterministic offline-first AI with swappable seams

**Status**: Accepted (Phase 5; `test_t1_rag.py`, `test_t1_local_ai.py`,
extension guide `docs/LOCAL_AI.md`)

**Context**: CI must be hermetic (no model downloads, no GPUs, no network),
while production wants real neural backends — without two code paths.

**Decision**: Three narrow seams, each with a deterministic default:
- Embedding: trait `rag::embed::Embedder`; default `HashingEmbedder`
  (FNV-1a feature hashing, 384-dim, L2-normalized, `embed.rs:22-102`);
  `resolve("minilm-onnx")` is the documented ONNX seam (`embed.rs:104`,
  `LOCAL_AI.md §2`).
- Generation: `localai/llm.rs::generate:168` routes to llama-server HTTP when
  `server_url`/`AUTHR_LLAMA_SERVER` is set, else seeded-SplitMix64
  `OFFLINE_GRAMMAR` (`llm.rs:26-103`). llama client pins
  `cache_prompt:true`, `keep_alive:-1` (`llm.rs:179-215`).
- Neural TTS: `localai/tts.rs::generate:46` negotiates
  `models/kokoro-v1.0.onnx` presence; absent ⇒ canonical
  `AI_ENGINE_UNAVAILABLE` with placement guidance; `SCRIPTED` engine covers
  pipeline testing (`audio/tts.rs:108`).

**Alternatives**: bundling models/ORT in the default build — rejected by the
hermetic-CI and 25 MB binary budget evidence (`docs/BENCHMARKS.md`);
SQLite-vec extension — documented as a drop-in future swap localized to
`rag/search.rs` (`LOCAL_AI.md §1`), schema already mirrors `vec0` shape.

**Consequences**: all AI commands are unit/E2E-testable offline; embeddings
are engine-specific — switching embedder requires re-index (`rag_index`) and
invalidates stored vectors; errors are negotiated via `local_ai_status`
(`minilm_model_present`, `kokoro.model_present`) so the frontend never
special-cases availability.

**Do not change unless**: the offline defaults remain byte-deterministic
(CI asserts determinism: `tts.rs:712`, `llm.rs:278`, `embed.rs:165`) and the
`AI_ENGINE_UNAVAILABLE` contract stays.

**Evidence**: `src-tauri/src/rag/*`, `localai/*`, `docs/LOCAL_AI.md`,
`docs/CLI_CONTRACT.md §4.12`.
