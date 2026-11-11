# Phase 5 — Local-First AI, Vector RAG & Offline Runtime (Engineering Notes)

**Scope**: build plan §8, Sprints 12–13. Contract impact: `docs/CLI_CONTRACT.md`
bumped **v1 → v2** (43 commands; §4.11 Vector RAG, §4.12 Offline AI; the 9-code
error taxonomy stays a closed set).

---

## 1. Sprint 12 — Embedded Vector RAG

### Architecture

```
rag_reindex ──▶ docs collected from ──▶ LocalHashEmbedder.embed(text)
                lorebook / character_profiles /        │  L2-normalize
                chapter_segments                       ▼
                                    ┌─ rag_chunks (chunk_id, source, ref_key,
                                    │   chapter_id, title, content, dim, embedding BLOB)
                                    ├─ rag_vec   (vec0: embedding float[384], L2 KNN)
                                    └─ rag_fts   (fts5 porter unicode61: title+content)

rag_search  ──▶ query embed → dense KNN (vec0 MATCH … AND k) ─┐
              → sparse BM25 (rag_fts MATCH, OR-quoted terms) ─┤ RRF fusion (k=60)
                                                              ▼
                                              fused hits (score, dense_rank,
                                              sparse_rank, distance, provenance)
```

Design decisions:

* **Sidecar tables live inside `project.db`** (not a separate file): they are
  transactional with content changes, ride along in `project_export`, and are
  lazy — a fresh bundle contains no RAG artifacts until the first RAG command
  runs. For SQLite clients without the vec module these tables are inert rows
  in `sqlite_master`.
* **Unit-normalized embeddings** make vec0's L2 distance ordering identical to
  cosine ordering — no dependency on optional distance-metric features.
* **Embeddings also persist as plain BLOBs in `rag_chunks`**, so the index can
  always be rebuilt (`rag_reindex`) even if the vec module were unavailable.
* **Eager invalidation**: `lore_delete` / `character_delete` /
  `chapter_delete` / `clean_sweep` prune matching chunks from all three
  sidecar tables in one `BEGIN IMMEDIATE` transaction — no stale-hit windows.
* **Dimension guard**: `rag_search` fails loudly with `STORAGE_ERROR`
  ("dimension mismatch — rerun rag_reindex") when the stored dim ≠ 384; a
  reindex heals the index (verified by T2.RB5).

### Embedder engine matrix

Selection: `AUTHR_EMBEDDER` (`auto` | `hash`), default `auto`.

| Engine | id | Availability | Description |
|--------|----|--------------|-------------|
| Feature-hashing embedder | `hash-v1` | **always** (pure Rust) | deterministic 384-dim: unigram + bigram + char-4-gram features, FNV-1a signed 384-bucket projection, sublinear TF, L2 norm. Cross-process identical — the whole RAG stack is reproducible. |
| `all-MiniLM-L6-v2` ONNX | `minilm` | adapter (see §4 below) | ort 2.0, 384-dim, same trait (`ai::embedder::Embedder`). |

## 2. Sprint 13 — Local LLM & Local Neural TTS

### LLM engine matrix

Selection: `AUTHR_LLM_ENGINE` (`auto` | `offline-template` | `llama-cpp`).

| Engine | Availability | Description |
|--------|--------------|-------------|
| `offline-template` (`heuristic-scaffold-v1`) | **always** | deterministic scaffold generator: 4-beat scene cycle (NARRATION → DIALOGUE → NARRATION → INTERNAL_MONOLOGUE), grounded in the RAG context pack (lore titles woven into the beats) + Chekhov ledger (Act III injects the payoff requirement into beat 0). xorshift-seeded — same inputs ⇒ identical beats. |
| `llama-cpp` | `--features local-llm` + `AUTHR_LLAMA_MODEL=<gguf>` | raw C FFI (`ai/llama_ffi.rs`): resident model+context (`keep_alive = -1` analog — the static prompt prefix is tokenized once, decoded once, and pinned in the KV cache via `llama_kv_self_seq_cp`; each generation decodes only the suffix from the pinned position), GPU offload depth via `AUTHR_LLAMA_GPU_LAYERS` (Metal/DirectML/CUDA come from the llama.cpp build), greedy argmax sampling, strict JSON beat parsing. |

Env knobs: `AUTHR_LLAMA_MODEL`, `AUTHR_LLAMA_GPU_LAYERS` (default 99),
`AUTHR_LLAMA_CTX` (default 8192).

### G2P tokenizer (`ai/g2p.rs`)

* Rule-based English G2P: digraphs (sh/ch/th/ph/ck/ng/qu…), vowel teams,
  magic-e (final silent ⟨e⟩ consumed with the whole V-C-e triple), soft c/g,
  function-word `<th>` voicing (DH), word-boundary pauses.
* **Proper-noun IPA substitution**: whole-word overrides win first —
  `character_profiles.ipa_phonetic` and `lorebook.ipa_phonetic` flow in via
  `tts_synthesize`; `ai_phonemize` accepts explicit `ipa_overrides`.
  A best-effort IPA→ARPAbet bridge (incl. stress marks) maps them into the
  phoneme stream.
* Output: ARPAbet-style tokens with stress digits (`K IH1 R AH0`) + an IPA
  rendering (stress digits are not re-marked in the display string).
* 4 Rust unit tests guard digraphs / magic-e / soft c-g / override round-trip.

### TTS engine matrix

Selection: `AUTHR_TTS_ENGINE` (`auto` | `offline-formant`).

| Engine | Availability | Description |
|--------|--------------|-------------|
| `offline-formant` | **always** | source-filter synthesizer over the G2P stream: formant pair per phoneme class, glottal excitation at the voice profile's f0 (voice_id-hash-derived 100–190 Hz × `base_pitch`), declination + vibrato + stressed-vowel lift, deterministic noise bursts for fricatives/plosives, 24 kHz 16-bit mono RIFF WAV (hand-rolled 44-byte header writer), fade-in/out. |
| `kokoro-82m` | adapter (see §4 below) | ort 2.0, fed by the same G2P tokenizer, 24 kHz output — same trait (`ai::tts::LocalTts`). |

### Audio chunk cache (`ai/audio_cache.rs`)

* Content-addressable: `sha256(engine|voice_id|voice_engine|rate|pitch|ipa_overrides|text)`
  → `audio/segments/<hash>.wav` — the exact Sprint-9 layout the Phase 4
  lookahead scheduler consumes.
* Atomic writes (`.tmp` → rename); cache hit only when the segment row points
  at the exact hash **and** the file still exists — a deleted/corrupt chunk
  file re-synthesizes (T2.RB6).
* `tts_synthesize` stamps `audio_hash` / `audio_cache_path` /
  `audio_duration_ms` on the segment row (schema §6.1 columns, previously
  reserved for Phase 4).

## 3. Verification (this sandbox)

* `cargo build --release` — default feature set, clean (0 warnings).
* `cargo check --features local-llm` — llama FFI bindings compile-checked;
  linking requires a llama.cpp static lib (§4).
* `cargo test --release --lib` — 7 unit tests (3 sandbox + 4 G2P).
* `python tests/run_e2e.py` — **128/128 passing** (101 pre-existing + 27 new
  across `test_t1_rag_hybrid.py` (10), `test_t1_offline_ai.py` (9),
  `test_t2_rag_boundaries.py` (8)).
* sqlite-vec pinned: v0.1.9 (bundled C sources via the `sqlite-vec` crate,
  registered as a rusqlite auto-extension before any connection opens).

## 4. Adapter contracts for model-backed engines

The sandbox build is fully offline **without** model files. Desktop builds can
attach neural engines without touching the dispatcher:

1. **MiniLM embeddings** — implement `ai::embedder::Embedder` (384-dim) with
   `ort 2.0` + `tokenizers`, register as `AUTHR_EMBEDDER=minilm`, vendor
   `all-MiniLM-L6-v2.onnx`, then run `rag_reindex` (a full rebuild keeps the
   index homogeneous under the new embedder; `rag_search` enforces dim=384).
2. **Kokoro-82M TTS** — implement `ai::tts::LocalTts`, register as
   `AUTHR_TTS_ENGINE=kokoro`; feed `g2p::phonemize` output (IPA rendering
   available via `phonemes_to_ipa`) into the ONNX graph; write chunks through
   `audio_cache::store` (the hash already includes the engine id).
3. **llama.cpp** — build llama.cpp (pinned tag compatible with the
   `llama_model_params` / `llama_context_params` layouts and the
   `llama_kv_self_*` API family declared in `ai/llama_ffi.rs`; llama.cpp
   b5000-class), then:
   ```bash
   export LLAMA_LIB_DIR=/path/to/llama.cpp/build/src
   export LLAMA_LIB_NAME=libllama.a
   cargo build --release --features local-llm
   AUTHR_LLAMA_MODEL=/models/novel-engine.gguf AUTHR_LLM_ENGINE=auto authr ai_generate ...
   ```
   A `build.rs` addition reading `LLAMA_LIB_DIR`/`LLAMA_LIB_NAME` emits
   `cargo:rustc-link-search` / `cargo:rustc-link-lib=static=llama` — kept out
   of the default build so the portable repo never requires the C library.

## 5. Phase-ordering note

The build plan's Phase 4 (Lookahead TTS connectors, DAG scheduler, 3-layer DSP
rack) and its Phase 2 (hybrid LLM router, GBNF compiler, anti-cliché scrubber)
are **not yet implemented** — Phase 5 was built so those phases plug in cleanly:

* the audio chunk cache + segment audio columns (Phase 4 Sprint 9 deliverable)
  are already wired and consumed by `tts_synthesize`;
* `LlmEngine::generate` is the exact seam where the GBNF scene expander
  (plan-Phase 2) and the lookahead scheduler (Phase 4 Sprint 10) attach;
* the cloud TTS connectors (Edge/ElevenLabs/Cartesia) extend the same
  `LocalTts`-style trait boundary used by the offline engine.
