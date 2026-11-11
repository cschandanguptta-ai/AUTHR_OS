# AUTHR_OS — Local-First AI: Architecture & Extension Points (Phase 5)

The local-AI layer is built around three narrow traits so the deterministic
offline engines that power headless CI can be swapped for real model runtimes
without touching any contract surface:

| Seam | Trait / module | Offline default | Production backend |
|------|----------------|-----------------|--------------------|
| Text embedding | `rag::embed::Embedder` | `HashingEmbedder` (deterministic 384-dim feature hashing, L2-normalized) | `all-MiniLM-L6-v2` ONNX via `ort` 2.0 |
| Text generation | `localai::llm::GenerateRequest` routing | `OFFLINE_GRAMMAR` (seeded SplitMix64 composer) | llama.cpp `llama-server` HTTP (`/completion`) |
| Neural TTS | `localai::tts::generate` availability negotiation | `audio::tts::ScriptedEngine` | `Kokoro-82M v1.0` ONNX via `ort` 2.0 + `audio::g2p` tokenizer |

## 1. Vector RAG store

`rag_chunks` (+ `rag_chunks_fts`, FTS5 `porter unicode61`) is created by an
idempotent migration on every connection — the frozen §6.1 core DDL is never
edited. Embeddings are little-endian f32 BLOBs:

```
rag_chunks(chunk_id PK, book_id, source, ref_id, content, dim, embedding BLOB, created_at)
  chunk_id = "<source>:<ref_id>"     e.g. "lorebook:coldfire", "chapters:<segment-uuid>"
```

Retrieval modes (`rag_search`):
- **dense** — cosine scan in pack order (deterministic ranks);
- **fts** — FTS5 `bm25()` (query terms are sanitized into quoted OR prefixes,
  so hostile input can never inject MATCH syntax);
- **hybrid** — Reciprocal Rank Fusion (`1/(60 + rank)` over both lists).

The store's schema shape mirrors a `sqlite-vec` `vec0` virtual table
(rowid + float32 BLOB), so swapping the pure-Rust cosine engine for the
`sqlite-vec` extension is a drop-in change localized to `rag/search.rs`.

## 2. Enabling the ONNX runtimes (ort 2.0)

The default build is hermetic (no model downloads, no ONNX runtime). To compile
the neural backends:

1. `cargo add ort@2 --features download-binaries` (or link a system onnxruntime).
2. Implement `Embedder` for the MiniLM graph (mean-pool the 38 token-dim outputs,
   L2-normalize) behind `resolve("minilm-onnx")` in `rag/embed.rs`.
3. Wire the Kokoro v1.0 graph (phonemes from `audio::g2p::phonemize` → style
   vector → 24 kHz waveform) into `localai/tts.rs::generate`.

Model placement inside a bundle:

```
<bundle>/models/all-MiniLM-L6-v2.onnx   # 384-dim sentence embeddings (~90 MB)
<bundle>/models/kokoro-v1.0.onnx        # Kokoro-82M TTS (~300 MB)
```

Until both steps are done the commands keep answering with the canonical
`AI_ENGINE_UNAVAILABLE` envelope — the frontend can negotiate features off
`local_ai_status` (`minilm_model_present`, `kokoro.model_present`) without
special-casing errors.

## 3. llama.cpp server (GPU inference)

`local_llm_generate` speaks the llama.cpp `llama-server` protocol directly
(no FFI linking in the core binary):

```bash
llama-server -m model.gguf --port 8080          # start the server
authr local_llm_generate --bundle . --args '{
  "prompt": "Chapter 3 opening:",
  "max_tokens": 256,
  "server_url": "http://127.0.0.1:8080"
}'
```

The client pins the static prompt prefix in the KV-cache
(`"cache_prompt": true`) and never evicts it (`"keep_alive": -1`), matching the
build plan's VRAM-residency requirement; an optional `grammar` string is passed
through for GBNF-constrained decoding. `AUTHR_LLAMA_SERVER` sets a default
endpoint for the session. Without a server the deterministic `OFFLINE_GRAMMAR`
engine answers, so authoring pipelines stay testable offline.
