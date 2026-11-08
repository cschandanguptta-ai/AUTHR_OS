// AUTHR_OS — embedded vector RAG (Phase 5, Sprint 12).
//
// Layout:
//   embed.rs — Embedder trait + deterministic hashing embedder (384-dim)
//   store.rs — rag_chunks table + rag_chunks_fts (FTS5/BM25) persistence
//   index.rs — lorebook / characters / chapters indexing pipelines
//   search.rs — dense cosine, FTS5 BM25, and hybrid RRF fusion retrieval
//
// The default build ships a pure-Rust cosine engine over packed f32 BLOBs
// (sqlite-vec drop-in schema shape); the ONNX MiniLM embedder hooks through
// the same Embedder trait (see docs/LOCAL_AI.md).

pub mod embed;
pub mod index;
pub mod search;
pub mod store;
