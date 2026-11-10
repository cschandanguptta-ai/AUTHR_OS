// AUTHR_OS — llama.cpp C FFI (Phase 5 / Sprint 13, feature `local-llm`).
//
// Raw bindings to the llama.cpp public C API subset used by
// [`LlamaCppEngine`], plus the engine itself:
//   * GPU acceleration hook — `n_gpu_layers` from `AUTHR_LLAMA_GPU_LAYERS`
//     (offload depth; Metal / DirectML / CUDA backends come from how
//     llama.cpp itself was built).
//   * `keep_alive = -1` semantics — model + context are held resident for
//     the process lifetime, and the tokenized STATIC PREFIX is decoded once
//     and pinned in the KV cache (`llama_kv_self_seq_cp`); every later
//     generation decodes only the suffix from the pinned position.
//   * Greedy argmax sampling (temperature 0) — deterministic output.
//
// Compile:  cargo check --features local-llm
// Link:     provide a llama.cpp static lib via `LLAMA_LIB_DIR` +
//           `LLAMA_LIB_NAME` (see docs/PHASE5_NOTES.md — pinned tag listed).
#![allow(non_camel_case_types, non_snake_case, dead_code)]

use crate::ai::llm::{Beat, GenContext, LlmEngine};
use crate::error::AppError;
use serde_json::Value;
use std::ffi::{c_char, c_float, c_int, c_void, CString};
use std::sync::{Mutex, OnceLock};

pub type llama_token = i32;
pub type llama_pos = i32;
pub type llama_seq_id = i32;

#[repr(C)] pub struct llama_model { _opaque: [u8; 0] }
#[repr(C)] pub struct llama_context { _opaque: [u8; 0] }
#[repr(C)] pub struct llama_vocab { _opaque: [u8; 0] }
#[repr(C)] pub struct llama_batch { _opaque: [u8; 0] }

/// llama_model_params — layout mirrors llama.cpp (pinned tag, see
/// docs/PHASE5_NOTES.md). Field order matters for ABI.
#[repr(C)]
pub struct llama_model_params {
    pub n_gpu_layers: c_int,
    pub split_mode: c_int,
    pub main_gpu: c_int,
    pub tensor_split: *const c_float,
    pub progress_callback: Option<unsafe extern "C" fn(*mut c_void) -> bool>,
    pub progress_callback_user_data: *mut c_void,
    pub kv_overrides: *mut c_void,
    pub use_mmap: bool,
    pub use_mlock: bool,
    pub check_tensors: bool,
}

/// llama_context_params — same ABI caveat as above.
#[repr(C)]
pub struct llama_context_params {
    pub n_ctx: u32,
    pub n_batch: u32,
    pub n_ubatch: u32,
    pub n_seq_max: u32,
    pub n_threads: c_int,
    pub n_threads_batch: c_int,
    pub rope_freq_base: c_float,
    pub rope_freq_scale: c_float,
    pub yarn_logit_scale: c_float,
    pub embeddings: bool,
    pub offload_kqv: bool,
    pub flash_attn: bool,
    pub no_perf: bool,
    pub op_offload: bool,
    pub swa_full: bool,
    pub abort_callback: Option<unsafe extern "C" fn(*mut c_void) -> bool>,
    pub abort_callback_user_data: *mut c_void,
}

extern "C" {
    pub fn llama_model_default_params() -> llama_model_params;
    pub fn llama_model_load_from_file(
        path_model: *const c_char,
        params: llama_model_params,
    ) -> *mut llama_model;
    pub fn llama_model_free(model: *mut llama_model);
    pub fn llama_context_default_params() -> llama_context_params;
    pub fn llama_init_from_model(
        model: *mut llama_model,
        params: llama_context_params,
    ) -> *mut llama_context;
    pub fn llama_free(ctx: *mut llama_context);
    pub fn llama_get_vocab(model: *const llama_model) -> *const llama_vocab;
    pub fn llama_vocab_n_tokens(vocab: *const llama_vocab) -> c_int;
    pub fn llama_vocab_is_eog(vocab: *const llama_vocab, token: llama_token) -> bool;
    pub fn llama_tokenize(
        model: *const llama_model,
        text: *const c_char,
        text_len: c_int,
        tokens: *mut llama_token,
        n_tokens_max: c_int,
        add_special: bool,
        parse_special: bool,
    ) -> c_int;
    pub fn llama_token_to_piece(
        model: *const llama_model,
        token: llama_token,
        buf: *mut c_char,
        length: c_int,
        lstrip: c_int,
        special: bool,
    ) -> c_int;
    pub fn llama_batch_get_one(tokens: *const llama_token, n_tokens: c_int) -> llama_batch;
    pub fn llama_decode(ctx: *mut llama_context, batch: llama_batch) -> c_int;
    pub fn llama_get_logits_ith(ctx: *mut llama_context, i: c_int) -> *const c_float;
    pub fn llama_n_ctx(ctx: *const llama_context) -> u32;
    // KV-cache pinning (prefix reuse — the raw-FFI analog of keep_alive = -1).
    pub fn llama_kv_self_clear(ctx: *mut llama_context);
    pub fn llama_kv_self_seq_cp(
        ctx: *mut llama_context,
        seq_id_src: llama_seq_id,
        seq_id_dst: llama_seq_id,
        p0: llama_pos,
        p1: llama_pos,
    ) -> c_int;
    pub fn llama_kv_self_seq_rm(
        ctx: *mut llama_context,
        seq_id: llama_seq_id,
        p0: llama_pos,
        p1: llama_pos,
    ) -> c_int;
}

// ── resident engine (keep_alive = -1) ────────────────────────────────────────

struct Resident {
    model: *mut llama_model,
    ctx: *mut llama_context,
    vocab: *const llama_vocab,
    /// Tokenized static prefix + its length pinned in KV (seq 0 original,
    /// generation copies start from seq 1).
    prefix_tokens: Vec<llama_token>,
    prefix_text_hash: u64,
}

unsafe impl Send for Resident {}

static RESIDENT: OnceLock<Mutex<Option<Resident>>> = OnceLock::new();

fn resident_slot() -> &'static Mutex<Option<Resident>> {
    RESIDENT.get_or_init(|| Mutex::new(None))
}

/// Hash of the static prompt prefix (never changes per model+persona).
fn prefix_hash(model_desc: &str) -> u64 {
    crate::ai::embedder::fnv1a64(model_desc.as_bytes())
}

const STATIC_PREFIX: &str = "You are the AUTHR_OS local novel engine. Emit STRICT JSON: \
an array of objects with keys segment_type, speaker_id, emotion, prosody_rate, \
prosody_pitch, text_content, ambient_cue, sfx_cue. No prose outside JSON.\n";

pub struct LlamaCppEngine {
    model_path: String,
    n_gpu_layers: i32,
    n_ctx: u32,
}

impl LlamaCppEngine {
    /// Build from environment:
    /// `AUTHR_LLAMA_MODEL` (required GGUF path), `AUTHR_LLAMA_GPU_LAYERS`
    /// (offload depth, default 99 = all), `AUTHR_LLAMA_CTX` (default 8192).
    pub fn from_env() -> Result<Self, AppError> {
        let model_path = std::env::var("AUTHR_LLAMA_MODEL").map_err(|_| {
            AppError::storage("AUTHR_LLAMA_MODEL is not set — no local GGUF model to load")
        })?;
        if !std::path::Path::new(&model_path).is_file() {
            return Err(AppError::not_found(format!(
                "llama model file `{model_path}` not found"
            )));
        }
        let n_gpu_layers = std::env::var("AUTHR_LLAMA_GPU_LAYERS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(99);
        let n_ctx: u32 = std::env::var("AUTHR_LLAMA_CTX")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(8192);
        Ok(Self { model_path, n_gpu_layers, n_ctx })
    }

    fn name(&self) -> &'static str {
        "llama-cpp"
    }
}

impl Drop for Resident {
    fn drop(&mut self) {
        unsafe {
            llama_free(self.ctx);
            llama_model_free(self.model);
        }
    }
}

impl LlmEngine for LlamaCppEngine {
    fn name(&self) -> &'static str {
        "llama-cpp"
    }

    fn model(&self) -> String {
        self.model_path.clone()
    }

    fn offline(&self) -> bool {
        true
    }

    /// Generate beats via the resident engine. The STATIC PREFIX is decoded
    /// once and pinned; the dynamic suffix (context pack + beat instruction)
    /// is decoded per request at the pinned position.
    fn generate(&self, gen: &GenContext) -> Result<Vec<Beat>, AppError> {
        let slot = resident_slot();
        let mut guard = slot
            .lock()
            .map_err(|_| AppError::storage("llama resident mutex poisoned"))?;

        unsafe {
            // ── load model once (process lifetime) ─────────────────────────
            if guard.is_none() {
                let path = CString::new(self.model_path.clone())
                    .map_err(|_| AppError::storage("model path contains NUL"))?;
                let mparams = llama_model_default_params();
                let mut mparams = mparams;
                mparams.n_gpu_layers = self.n_gpu_layers; // GPU acceleration hook
                mparams.use_mmap = true;
                let model = llama_model_load_from_file(path.as_ptr(), mparams);
                if model.is_null() {
                    return Err(AppError::storage("llama_model_load_from_file failed"));
                }
                let mut cparams = llama_context_default_params();
                cparams.n_ctx = self.n_ctx;
                let ctx = llama_init_from_model(model, cparams);
                if ctx.is_null() {
                    llama_model_free(model);
                    return Err(AppError::storage("llama_init_from_model failed"));
                }
                let vocab = llama_get_vocab(model);
                // Decode + pin the static prefix (seq 0 = golden prefix).
                let want = STATIC_PREFIX;
                let mut tokens = vec![0i32; want.len() + 8];
                let n = llama_tokenize(
                    model,
                    want.as_ptr() as *const c_char,
                    want.len() as c_int,
                    tokens.as_mut_ptr(),
                    tokens.len() as c_int,
                    true,
                    true,
                );
                if n < 0 {
                    llama_free(ctx);
                    llama_model_free(model);
                    return Err(AppError::storage("llama_tokenize(prefix) failed"));
                }
                tokens.truncate(n as usize);
                llama_kv_self_clear(ctx);
                let batch = llama_batch_get_one(tokens.as_ptr(), n);
                if llama_decode(ctx, batch) != 0 {
                    llama_free(ctx);
                    llama_model_free(model);
                    return Err(AppError::storage("llama_decode(prefix) failed"));
                }
                *guard = Some(Resident {
                    model,
                    ctx,
                    vocab,
                    prefix_tokens: tokens,
                    prefix_text_hash: prefix_hash(&self.model_path),
                });
            }
            let resident = guard.as_mut().unwrap();

            // ── dynamic suffix (per request) ───────────────────────────────
            let suffix = build_suffix(gen);
            let mut tokens = vec![0i32; suffix.len() + 8];
            let n_suffix = llama_tokenize(
                resident.model,
                suffix.as_ptr() as *const c_char,
                suffix.len() as c_int,
                tokens.as_mut_ptr(),
                tokens.len() as c_int,
                false,
                true,
            );
            if n_suffix < 0 {
                return Err(AppError::storage("llama_tokenize(suffix) failed"));
            }
            tokens.truncate(n_suffix as usize);

            // Prefix KV was left in seq 0; work on seq 1, trimming any prior
            // generation from this session first.
            llama_kv_self_seq_rm(resident.ctx, 1, -1, -1);
            llama_kv_self_seq_cp(resident.ctx, 0, 1, -1, -1);

            let vocab = resident.vocab;
            let n_vocab = llama_vocab_n_tokens(vocab);
            let budget = 1024usize;
            let mut out = String::new();
            let mut toks = tokens;

            for _ in 0..budget {
                let batch = llama_batch_get_one(toks.as_ptr(), toks.len() as c_int);
                if llama_decode(resident.ctx, batch) != 0 {
                    return Err(AppError::storage("llama_decode(suffix) exhausted context"));
                }
                // Greedy argmax over logits of the last position.
                let logits = llama_get_logits_ith(resident.ctx, 0);
                let mut best = 0usize;
                let mut best_v = f32::NEG_INFINITY;
                for t in 0..n_vocab {
                    let v = *logits.add(t as usize);
                    if v > best_v {
                        best_v = v;
                        best = t as usize;
                    }
                }
                let token = best as llama_token;
                if llama_vocab_is_eog(vocab, token) {
                    break;
                }
                // Detokenize incrementally.
                let mut buf = [0u8; 64];
                let m = llama_token_to_piece(
                    resident.model,
                    token,
                    buf.as_mut_ptr() as *mut c_char,
                    buf.len() as c_int,
                    0,
                    true,
                );
                if m > 0 {
                    out.push_str(
                        &String::from_utf8_lossy(&buf[..m as usize]).to_string(),
                    );
                }
                if out.contains(']') && parse_beats(&out).is_ok() {
                    break; // complete JSON array seen
                }
                toks = vec![token];
            }

            // Trim seq 1 back to the prefix copy for the next call.
            llama_kv_self_seq_rm(resident.ctx, 1, resident.prefix_tokens.len() as llama_pos, -1);

            parse_beats(&out)
        }
    }
}

fn build_suffix(gen: &GenContext) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "PREMISE: {}\nTHEME: {}\nGENRE: {}\nCHAPTER: {}\nACT: {}\nTASK: {} — emit exactly {} beat(s).\n",
        gen.premise, gen.theme, gen.genre, gen.chapter_title, gen.act_number,
        if gen.prompt.trim().is_empty() { "continue the scene" } else { &gen.prompt },
        gen.beats_requested
    ));
    if !gen.rag_hits.is_empty() {
        s.push_str("CONTEXT:\n");
        for h in gen.rag_hits.iter().take(6) {
            s.push_str(&format!("- [{}] {}: {}\n", h.source, h.title, h.snippet));
        }
    }
    if !gen.open_threads.is_empty() {
        s.push_str("OPEN THREADS:\n");
        for t in gen.open_threads.iter().take(4) {
            s.push_str(&format!("- {}: {}\n", t.thread_title, t.payoff_requirement));
        }
    }
    if !gen.cast.is_empty() {
        s.push_str("CAST:\n");
        for (k, d) in gen.cast.iter().take(8) {
            s.push_str(&format!("- {k} ({d})\n"));
        }
    }
    s.push_str("JSON:\n");
    s
}

/// Parse the model's JSON output into beats — zero-tolerance for malformed
/// JSON (the GBNF grammar path is the production guard; greedy decoding plus
/// strict parsing here keeps the same guarantee honest).
fn parse_beats(raw: &str) -> Result<Vec<Beat>, AppError> {
    let start = raw.find('[').ok_or_else(|| {
        AppError::storage(format!("llama output missing JSON array: {raw:.120}"))
    })?;
    let end = raw.rfind(']').ok_or_else(|| {
        AppError::storage(format!("llama output JSON array unterminated: {raw:.120}"))
    })?;
    let slice = &raw[start..=end];
    let v: Value = serde_json::from_str(slice)
        .map_err(|e| AppError::storage(format!("llama output JSON invalid: {e}")))?;
    let arr = v
        .as_array()
        .ok_or_else(|| AppError::storage("llama output JSON is not an array"))?;
    let mut beats = Vec::new();
    for b in arr {
        beats.push(Beat {
            segment_type: b.get("segment_type").and_then(|x| x.as_str()).unwrap_or("NARRATION").into(),
            speaker_id: b.get("speaker_id").and_then(|x| x.as_str()).unwrap_or("NARRATOR").into(),
            emotion: b.get("emotion").and_then(|x| x.as_str()).unwrap_or("neutral").into(),
            prosody_rate: b.get("prosody_rate").and_then(|x| x.as_f64()).unwrap_or(1.0),
            prosody_pitch: b.get("prosody_pitch").and_then(|x| x.as_f64()).unwrap_or(1.0),
            text_content: b.get("text_content").and_then(|x| x.as_str()).unwrap_or("").into(),
            ambient_cue: b.get("ambient_cue").and_then(|x| x.as_str()).map(String::from),
            sfx_cue: b.get("sfx_cue").and_then(|x| x.as_str()).map(String::from),
        });
    }
    if beats.is_empty() {
        return Err(AppError::storage("llama output contained zero beats"));
    }
    Ok(beats)
}
