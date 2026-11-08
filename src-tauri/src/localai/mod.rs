// AUTHR_OS — local-first AI runtime (Phase 5, Sprint 13).
//
//   llm.rs — LLM backends: `llama-server` HTTP client (llama.cpp server,
//            prompt-cache pinned with `cache_prompt`, `keep_alive` semantics)
//            and the deterministic `OFFLINE_GRAMMAR` engine used headless.
//   tts.rs — Kokoro-82M ONNX availability negotiation + offline engine route.
//
// The 100%-offline deliverable: `local_llm_generate` works with zero network
// via OFFLINE_GRAMMAR; pointing `server_url` at a running llama.cpp server
// switches to real inference with the same contract (docs/LOCAL_AI.md).

pub mod llm;
pub mod tts;
