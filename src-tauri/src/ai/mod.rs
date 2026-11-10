// AUTHR_OS — Local-First AI runtime (Phase 5, build plan §8 Sprints 12–13).
//
// Module map:
//   embedder   — 384-dim local embeddings (hash engine; ONNX adapter contract)
//   rag        — (storage/rag.rs) sqlite-vec + BM25 hybrid retrieval
//   llm        — offline scaffold generator; llama.cpp engine behind feature
//   llama_ffi  — raw llama.cpp C FFI + resident KV-pinned engine
//   g2p        — English G2P tokenizer + IPA proper-noun substitution
//   tts        — offline formant synthesizer; Kokoro ONNX adapter contract
//   audio_cache— SHA-256 content-addressable WAV chunks (audio/segments/)

pub mod audio_cache;
pub mod embedder;
pub mod g2p;
pub mod llama_ffi;
pub mod llm;
pub mod tts;

use crate::error::AppError;
use crate::ipc::{args, Ctx};
use llm::{GenContext, RagRef, ThreadRef};
use serde_json::{json, Value};

// ── ai_generate ──────────────────────────────────────────────────────────────

/// `ai_generate {book_id?, chapter_id?, prompt?, beats?, use_rag?, seed?}` —
/// produce `NarrativeSegmentAst`-shaped beats (contract §4.8 shape, no
/// `segment_id` — storage mints UUIDs on append). Fully offline.
pub fn generate(ctx: &mut Ctx, a: &Value) -> Result<Value, AppError> {
    let t0 = std::time::Instant::now();
    let prompt = args::get_str(a, "prompt").unwrap_or_default();
    let beats_requested = args::get_i64(a, "beats").unwrap_or(4).clamp(1, 16) as usize;
    let use_rag = a.get("use_rag").and_then(|v| v.as_bool()).unwrap_or(true);
    let seed = a.get("seed").and_then(|v| v.as_u64()).unwrap_or_else(|| {
        // Deterministic default: hash of prompt + chapter/book resolution.
        embedder::fnv1a64(format!("{prompt}|{}", args::get_i64(a, "chapter_id").unwrap_or(0)).as_bytes())
    });

    // Resolve the scene context (chapter → book, or book, or first book).
    let (book, chapter) = resolve_scene(&ctx.conn, a)?;
    let book_id = book
        .get("id")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| AppError::not_found("no book in project — create one first"))?;

    // Hybrid RAG context pack (best-effort: an empty index degrades to none).
    let mut rag_hits: Vec<RagRef> = Vec::new();
    let mut rag_used = false;
    if use_rag {
        let query = if prompt.trim().is_empty() {
            chapter
                .as_ref()
                .map(|c| {
                    format!(
                        "{} {}",
                        c.get("title").and_then(|v| v.as_str()).unwrap_or(""),
                        c.get("synopsis").and_then(|v| v.as_str()).unwrap_or("")
                    )
                })
                .unwrap_or_else(|| {
                    book.get("premise").and_then(|v| v.as_str()).unwrap_or("").to_string()
                })
        } else {
            prompt.clone()
        };
        if !query.trim().is_empty() {
            if let Ok(res) = crate::storage::rag::search(&ctx.conn, &json!({
                "query": query, "limit": 6,
            })) {
                if let Some(hits) = res.get("hits").and_then(|v| v.as_array()) {
                    rag_used = !hits.is_empty();
                    for h in hits {
                        rag_hits.push(RagRef {
                            source: h.get("source").and_then(|v| v.as_str()).unwrap_or("").into(),
                            ref_key: h.get("ref_key").and_then(|v| v.as_str()).unwrap_or("").into(),
                            title: h.get("title").and_then(|v| v.as_str()).unwrap_or("").into(),
                            snippet: h.get("content").and_then(|v| v.as_str()).unwrap_or("").into(),
                        });
                    }
                }
            }
        }
    }

    // Chekhov ledger: open threads (Act III injects the payoff requirement).
    let mut stmt = ctx
        .conn
        .prepare("SELECT thread_title, payoff_requirement FROM unresolved_threads WHERE status != 'RESOLVED' ORDER BY id")
        .map_err(|e| AppError::storage(e.to_string()))?;
    let open_threads = stmt
        .query_map([], |r| {
            Ok(ThreadRef {
                thread_title: r.get::<_, String>(0)?,
                payoff_requirement: r.get::<_, String>(1)?,
            })
        })
        .map_err(|e| AppError::storage(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);

    // Voice cast (speakers available to the generator).
    let mut stmt = ctx
        .conn
        .prepare("SELECT character_key, display_name FROM character_profiles ORDER BY character_key")
        .map_err(|e| AppError::storage(e.to_string()))?;
    let cast = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(|e| AppError::storage(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);

    let engine = llm::default_engine()?;
    let act_number = chapter
        .as_ref()
        .and_then(|c| c.get("act_number"))
        .and_then(|v| v.as_i64())
        .unwrap_or(1);
    let gen = GenContext {
        prompt,
        beats_requested,
        premise: book.get("premise").and_then(|v| v.as_str()).unwrap_or("").into(),
        theme: book.get("theme").and_then(|v| v.as_str()).unwrap_or("").into(),
        genre: book.get("genre").and_then(|v| v.as_str()).unwrap_or("").into(),
        chapter_title: chapter
            .as_ref()
            .and_then(|c| c.get("title"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .into(),
        act_number,
        rag_hits: rag_hits.clone(),
        open_threads,
        cast,
        seed,
    };
    let beats = engine.generate(&gen)?;
    let payoff_injected = act_number >= 3 && !beats.is_empty();
    let beat_json: Vec<Value> = beats
        .iter()
        .enumerate()
        .map(|(i, b)| {
            json!({
                "sequence_order": i,
                "segment_type": b.segment_type,
                "speaker_id": b.speaker_id,
                "emotion": b.emotion,
                "prosody_rate": b.prosody_rate,
                "prosody_pitch": b.prosody_pitch,
                "text_content": b.text_content,
                "ambient_cue": b.ambient_cue,
                "sfx_cue": b.sfx_cue,
            })
        })
        .collect();

    Ok(json!({
        "engine": engine.name(),
        "model": engine.model(),
        "offline": engine.offline(),
        "beats": beat_json,
        "beats_count": beat_json.len(),
        "rag_used": rag_used,
        "rag_context": rag_hits
            .iter()
            .map(|h| json!({"source": h.source, "ref_key": h.ref_key, "title": h.title}))
            .collect::<Vec<_>>(),
        "act_number": act_number,
        "payoff_injected": payoff_injected,
        "book_id": book_id,
        "chapter_id": chapter.as_ref().and_then(|c| c.get("id").and_then(|v| v.as_i64())),
        "took_ms": t0.elapsed().as_millis() as i64,
    }))
}

/// Resolve (book, chapter) for generation. Missing explicit ids are
/// ENTITY_NOT_FOUND; no ids at all → first book.
fn resolve_scene(
    conn: &rusqlite::Connection,
    a: &Value,
) -> Result<(Value, Option<Value>), AppError> {
    let book_col = crate::storage::db::BOOK_COLS.join(", ");
    let chapter = if let Some(cid) = args::get_i64(a, "chapter_id") {
        Some(crate::storage::chapters::get(conn, cid)?)
    } else {
        None
    };
    let book = if let Some(bid) = args::get_i64(a, "book_id") {
        crate::storage::books::get(conn, bid)?
    } else if let Some(ch) = &chapter {
        let id = ch.get("book_id").and_then(|v| v.as_i64()).unwrap_or(1);
        crate::storage::books::get(conn, id)?
    } else {
        // Deterministic default: first book by id.
        let sql = format!("SELECT {book_col} FROM books ORDER BY id LIMIT 1");
        conn.query_row(&sql, [], |r| crate::storage::db::row_json(r, crate::storage::db::BOOK_COLS))
            .map_err(|_| AppError::not_found("no book in project — create one first"))?
    };
    Ok((book, chapter))
}

// ── ai_status ────────────────────────────────────────────────────────────────

/// `ai_status {}` — engine matrix + offline report (bundle-free).
pub fn status() -> Result<Value, AppError> {
    // Probe sqlite-vec from an in-memory connection (module registered above).
    crate::storage::rag::register_vec_module();
    let vec_version: String = rusqlite::Connection::open_in_memory()
        .and_then(|c| c.query_row("SELECT vec_version()", [], |r| r.get::<_, String>(0)))
        .unwrap_or_else(|_| "unavailable".into());

    let tts = tts::default_tts()?;
    let llm_engine = llm::default_engine()?;
    let embedder = embedder::default_embedder()?;

    Ok(json!({
        "offline": true,
        "engines": {
            "llm": {
                "default": llm_engine.name(),
                "model": llm_engine.model(),
                "available": llm::registry(),
            },
            "embedder": {
                "default": embedder.name(),
                "dim": embedder.dim(),
                "available": embedder::registry(),
            },
            "tts": {
                "default": tts.name(),
                "available": [
                    {"id": "offline-formant", "name": "offline-formant", "available": true},
                    {"id": "kokoro", "name": "kokoro-82m-onnx", "available": false},
                ],
            },
        },
        "rag": {
            "dense_backend": format!("sqlite-vec {vec_version}"),
            "sparse_backend": "fts5 porter unicode61",
            "dim": crate::storage::rag::RAG_DIM,
            "fusion": "rrf-k60",
        },
        "audio_cache_dir": audio_cache::CACHE_DIR,
        "features": {
            "gui": cfg!(feature = "gui"),
            "os_keychain": cfg!(feature = "os-keychain"),
            "local_llm": cfg!(feature = "local-llm"),
        },
    }))
}

// ── ai_phonemize ─────────────────────────────────────────────────────────────

/// `ai_phonemize {text, ipa_overrides?}` — G2P + proper-noun IPA
/// substitution (bundle-free; deterministic).
pub fn phonemize(a: &Value) -> Result<Value, AppError> {
    let text = args::req_str(a, "text")?;
    if text.trim().is_empty() {
        return Err(AppError::storage("text must not be empty"));
    }
    let mut overrides: Vec<(String, String)> = Vec::new();
    let mut applied: Vec<Value> = Vec::new();
    if let Some(Value::Array(items)) = a.get("ipa_overrides") {
        for it in items {
            let term = it
                .get("term")
                .and_then(|v| v.as_str())
                .ok_or_else(|| AppError::storage("ipa_overrides entries require `term`"))?;
            let ipa = it
                .get("ipa")
                .and_then(|v| v.as_str())
                .ok_or_else(|| AppError::storage("ipa_overrides entries require `ipa`"))?;
            overrides.push((term.to_string(), ipa.to_string()));
            applied.push(json!({"term": term, "ipa": ipa}));
        }
    }
    let phonemes = g2p::phonemize(&text, &overrides);
    let speakable: Vec<&String> = phonemes.iter().filter(|p| p.as_str() != g2p::PAUSE).collect();
    if speakable.is_empty() {
        return Err(AppError::storage("text produced no speakable phonemes"));
    }
    Ok(json!({
        "text": text,
        "phonemes": phonemes,
        "ipa": g2p::phonemes_to_ipa(&phonemes),
        "vowel_count": g2p::vowel_count(&phonemes),
        "ipa_overrides": applied,
        "offline": true,
    }))
}
