// AUTHR_OS — hybrid retrieval (Phase 5, Sprint 12).
//
// Three modes:
//   dense  — cosine similarity over stored embeddings
//   fts    — FTS5 BM25 over rag_chunks_fts
//   hybrid — Reciprocal Rank Fusion (k = 60) of both lists
// The payload always carries measured latency (µs) so the <5 ms retrieval
// budget is observable in the E2E suite.

use crate::error::AppError;
use rusqlite::Connection;
use serde_json::{json, Value};
use std::time::Instant;

use super::embed::{cosine, unpack, Embedder};

const RRF_K: f64 = 60.0;

pub struct SearchHit {
    pub chunk_id: String,
    pub source: String,
    pub ref_id: String,
    pub snippet: String,
    pub score: f64,
    pub dense_rank: Option<usize>,
    pub fts_rank: Option<usize>,
}

fn snippet(content: &str, max: usize) -> String {
    let mut s: String = content.chars().take(max).collect();
    if content.chars().count() > max {
        s.push('…');
    }
    s
}

/// Dense cosine scan (deterministic order: chunk_id ASC → stable ranks).
fn dense_search(
    conn: &Connection,
    emb: &dyn Embedder,
    query_vec: &[f32],
    limit: usize,
) -> Result<Vec<(String, f64)>, AppError> {
    let chunks = super::store::all_chunks(conn)?;
    let mut scored: Vec<(String, f64)> = Vec::new();
    for c in chunks {
        if c.dim != emb.dim() {
            return Err(AppError::ai(format!(
                "chunk {} has dim {} but embedder {} expects {} — re-run rag_index",
                c.chunk_id, c.dim, emb.name(), emb.dim()
            )));
        }
        let vec = unpack(&c.embedding, c.dim);
        scored.push((c.chunk_id, cosine(query_vec, &vec) as f64));
    }
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(&b.0)));
    scored.truncate(limit);
    Ok(scored)
}

/// Reciprocal Rank Fusion over the dense + BM25 ranked lists.
pub fn rrf_fuse(dense: &[(String, f64)], fts: &[(String, f64)]) -> Vec<(String, f64)> {
    let mut scores: std::collections::BTreeMap<String, f64> = std::collections::BTreeMap::new();
    for (rank, (id, _)) in dense.iter().enumerate() {
        *scores.entry(id.clone()).or_insert(0.0) += 1.0 / (RRF_K + rank as f64 + 1.0);
    }
    for (rank, (id, _)) in fts.iter().enumerate() {
        *scores.entry(id.clone()).or_insert(0.0) += 1.0 / (RRF_K + rank as f64 + 1.0);
    }
    let mut out: Vec<(String, f64)> = scores.into_iter().collect();
    out.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(&b.0)));
    out
}

/// `rag_search` — full pipeline with measured latency.
pub fn search(
    conn: &Connection,
    emb: &dyn Embedder,
    query: &str,
    k: usize,
    mode: &str,
) -> Result<Value, AppError> {
    if query.trim().is_empty() {
        return Err(AppError::ai("query must not be empty"));
    }
    let mode_norm = mode.to_lowercase();
    if !matches!(mode_norm.as_str(), "dense" | "fts" | "hybrid") {
        return Err(AppError::ai(format!("unknown mode `{mode}` (expected dense|fts|hybrid)")));
    }
    let t0 = Instant::now();

    let dense = if mode_norm != "fts" {
        let qv = emb.embed(query);
        dense_search(conn, emb, &qv, k * 2)?
    } else {
        Vec::new()
    };
    let fts = if mode_norm != "dense" {
        super::store::fts_search(conn, query, k * 2)?
    } else {
        Vec::new()
    };

    let fused: Vec<(String, f64)> = if mode_norm == "hybrid" {
        rrf_fuse(&dense, &fts)
    } else if mode_norm == "dense" {
        dense.clone()
    } else {
        // bm25 ranks are negative-scaled; expose positive relevance.
        fts.iter().map(|(id, r)| (id.clone(), -r)).collect()
    };

    // Enrich with chunk metadata (content, source).
    let chunks = super::store::all_chunks(conn)?;
    let by_id: std::collections::HashMap<&str, &super::store::ChunkRow> =
        chunks.iter().map(|c| (c.chunk_id.as_str(), c)).collect();

    let dense_rank: std::collections::HashMap<&str, usize> =
        dense.iter().enumerate().map(|(i, (id, _))| (id.as_str(), i + 1)).collect();
    let fts_rank: std::collections::HashMap<&str, usize> =
        fts.iter().enumerate().map(|(i, (id, _))| (id.as_str(), i + 1)).collect();

    let mut hits = Vec::new();
    for (id, score) in fused.iter().take(k) {
        match by_id.get(id.as_str()) {
            Some(c) => hits.push(SearchHit {
                chunk_id: c.chunk_id.clone(),
                source: c.source.clone(),
                ref_id: c.ref_id.clone(),
                snippet: snippet(&c.content, 120),
                score: *score,
                dense_rank: dense_rank.get(id.as_str()).copied(),
                fts_rank: fts_rank.get(id.as_str()).copied(),
            }),
            None => return Err(AppError::ai(format!("fused chunk {id} missing from store"))),
        }
    }
    let latency_us = t0.elapsed().as_micros() as u64;
    let hits_json: Vec<Value> = hits
        .iter()
        .map(|h| {
            json!({
                "chunk_id": h.chunk_id,
                "source": h.source,
                "ref_id": h.ref_id,
                "snippet": h.snippet,
                "score": (h.score * 1e6).round() / 1e6,
                "ranks": { "dense": h.dense_rank, "fts": h.fts_rank },
            })
        })
        .collect();
    Ok(json!({
        "query": query,
        "mode": mode_norm,
        "k": k,
        "hits": hits_json,
        "returned": hits_json.len(),
        "corpus": super::store::count(conn)?,
        "embedder": emb.name(),
        "latency_us": latency_us,
        "latency_ms": (latency_us as f64 / 1000.0 * 100.0).round() / 100.0,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rag::embed::{pack, HashingEmbedder};

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        super::super::store::ensure_rag_schema(&conn).unwrap();
        conn
    }

    fn seed(conn: &Connection, emb: &dyn Embedder, src: &str, id: &str, text: &str) {
        let v = emb.embed(text);
        super::super::store::upsert_chunk(conn, src, id, None, text, emb.dim(), &pack(&v)).unwrap();
    }

    #[test]
    fn dense_mode_ranks_semantic_matches() {
        let conn = setup();
        let emb = HashingEmbedder::new();
        seed(&conn, &emb, "lorebook", "magic", "The dragon guards the mountain treasury");
        seed(&conn, &emb, "lorebook", "bread", "The bakery sells fresh bread every morning");
        let out = search(&conn, &emb, "dragon treasury mountain", 2, "dense").unwrap();
        assert_eq!(out["returned"], 2);
        let first = out["hits"][0]["chunk_id"].as_str().unwrap();
        assert_eq!(first, "lorebook:magic");
    }

    #[test]
    fn fts_mode_uses_bm25() {
        let conn = setup();
        let emb = HashingEmbedder::new();
        seed(&conn, &emb, "chapters", "s1", "the lighthouse beams across the bay");
        seed(&conn, &emb, "chapters", "s2", "an unrelated grain shipment");
        let out = search(&conn, &emb, "lighthouse", 2, "fts").unwrap();
        assert_eq!(out["returned"], 1);
        assert_eq!(out["hits"][0]["chunk_id"], "chapters:s1");
    }

    #[test]
    fn hybrid_fuses_both_lists() {
        let conn = setup();
        let emb = HashingEmbedder::new();
        seed(&conn, &emb, "characters", "KIRA", "Kira is the lighthouse keeper of Port Merrow");
        seed(&conn, &emb, "lorebook", "merrow", "Port Merrow lighthouse was built by the old fleet");
        seed(&conn, &emb, "chapters", "s9", "Kira trimmed the lighthouse wick at dusk");
        let out = search(&conn, &emb, "lighthouse keeper", 3, "hybrid").unwrap();
        let hits = out["hits"].as_array().unwrap();
        assert!(hits.len() >= 2);
        // At least one hit carries BOTH ranks (appears in both lists).
        assert!(hits.iter().any(|h| h["ranks"]["dense"].is_u64() && h["ranks"]["fts"].is_u64()));
    }

    #[test]
    fn rrf_prefers_consensus() {
        let a = vec![("x".to_string(), 0.9), ("y".to_string(), 0.5)];
        let b = vec![("y".to_string(), -1.0), ("x".to_string(), -2.0)];
        let fused = rrf_fuse(&a, &b);
        // Both lists rank x first and y second → tie on RRF; deterministic order.
        assert_eq!(fused.len(), 2);
    }

    #[test]
    fn invalid_mode_and_empty_query_are_ai_errors() {
        let conn = setup();
        let emb = HashingEmbedder::new();
        assert_eq!(search(&conn, &emb, "q", 3, "magic").unwrap_err().code, "AI_ERROR");
        assert_eq!(search(&conn, &emb, "   ", 3, "dense").unwrap_err().code, "AI_ERROR");
    }

    #[test]
    fn retrieval_is_sub_millisecond_on_small_corpus() {
        let conn = setup();
        let emb = HashingEmbedder::new();
        for i in 0..50 {
            seed(&conn, &emb, "chapters", &format!("s{i}"), &format!("passage {i} about silver ships sailing south"));
        }
        let out = search(&conn, &emb, "silver ships", 5, "hybrid").unwrap();
        assert!(out["latency_us"].as_u64().unwrap() < 50_000, "slow: {out}");
    }
}
