"""Tier 1 — embedded vector RAG (Phase 5, Sprint 12): index, hybrid search, status."""

from helpers.audio_fixtures import seed_audio_chapter


def test_r1_rag_status_empty(bundle, cli):
    r = cli("rag_status", bundle=bundle())
    assert r.ok, r.payload
    d = r.payload["data"]
    assert d["chunks"] == 0
    assert d["embedder"] == "hashing-384"
    assert d["dim"] == 384
    assert set(d["modes"]) == {"dense", "fts", "hybrid"}


def test_r2_index_all_sources_is_idempotent(bundle, cli):
    b = bundle()
    cli("lore_upsert", bundle=b, args={
        "key": "merrow_light", "category": "LOCATION",
        "content": "The Port Merrow lighthouse burns coldfire; its keeper sees the drowned year.",
    })
    seed_audio_chapter(b, cli)
    r1 = cli("rag_index", bundle=b, args={})
    assert r1.ok, r1.payload
    d1 = r1.payload["data"]
    assert d1["embedder"] == "hashing-384"
    assert d1["dim"] == 384
    assert d1["indexed"]["lorebook"] == 1
    assert d1["indexed"]["characters"] == 3  # NARRATOR seed + KIRA + ORIN
    assert d1["indexed"]["chapters"] == 5
    assert d1["total_chunks"] == 9
    r2 = cli("rag_index", bundle=b, args={"sources": ["lorebook"]})
    assert r2.ok
    assert r2.payload["data"]["indexed"]["lorebook"] == 1
    assert cli("rag_status", bundle=b).payload["data"]["chunks"] == 9


def test_r3_hybrid_search_fuses_dense_and_bm25(bundle, cli):
    b = bundle()
    cli("lore_upsert", bundle=b, args={
        "key": "coldfire", "content": "Coldfire lighthouses repel the deep fog entities.",
    })
    cli("lore_upsert", bundle=b, args={
        "key": "bakery", "content": "The colony bakery queues start before dawn.",
    })
    cli("rag_index", bundle=b, args={"sources": ["lorebook"]})
    r = cli("rag_search", bundle=b, args={"query": "coldfire fog lighthouse", "k": 3, "mode": "hybrid"})
    assert r.ok, r.payload
    d = r.payload["data"]
    assert d["mode"] == "hybrid"
    assert d["returned"] == 2
    top = d["hits"][0]
    assert top["ref_id"] == "coldfire"
    assert top["ranks"]["dense"] is not None and top["ranks"]["fts"] is not None
    assert 0 < d["latency_ms"] < 1000


def test_r4_chapter_segments_are_searchable(bundle, cli):
    b = bundle()
    _, _, seg_ids = seed_audio_chapter(b, cli)
    cli("rag_index", bundle=b, args={"sources": ["chapters"]})
    r = cli("rag_search", bundle=b, args={"query": "static curtains ridge", "k": 2, "mode": "hybrid"})
    assert r.ok, r.payload
    top = r.payload["data"]["hits"][0]
    assert top["source"] == "chapters"
    assert top["ref_id"] == seg_ids[2]


def test_r5_dense_mode_ranks_semantic_neighbors_first(bundle, cli):
    b = bundle()
    cli("lore_upsert", bundle=b, args={"key": "dragons", "content": "Dragons hoard gold in mountain caves and fear cold iron."})
    cli("lore_upsert", bundle=b, args={"key": "finance", "content": "Quarterly revenue spreadsheets drive the colony budget."})
    cli("rag_index", bundle=b, args={})
    r = cli("rag_search", bundle=b, args={"query": "dragon hoard mountain gold", "k": 2, "mode": "dense"})
    assert r.ok
    assert r.payload["data"]["hits"][0]["ref_id"] == "dragons"


def test_r6_fts_mode_returns_bm25_ranks(bundle, cli):
    b = bundle()
    cli("lore_upsert", bundle=b, args={"key": "a", "content": "lighthouse lighthouse beams"})
    cli("lore_upsert", bundle=b, args={"key": "b", "content": "quiet tide pools"})
    cli("rag_index", bundle=b, args={})
    r = cli("rag_search", bundle=b, args={"query": "lighthouse", "k": 2, "mode": "fts"})
    assert r.ok
    d = r.payload["data"]
    assert d["returned"] == 1
    assert d["hits"][0]["ranks"]["fts"] == 1
    assert d["hits"][0]["ranks"]["dense"] is None


def test_r7_reindex_reflects_updates(bundle, cli):
    b = bundle()
    cli("lore_upsert", bundle=b, args={"key": "k1", "content": "old weather text about storms"})
    cli("rag_index", bundle=b, args={"sources": ["lorebook"]})
    cli("lore_upsert", bundle=b, args={"key": "k1", "content": "new volcano observatory notes"})
    cli("rag_index", bundle=b, args={"sources": ["lorebook"]})
    r = cli("rag_search", bundle=b, args={"query": "volcano observatory", "k": 3, "mode": "fts"})
    assert r.ok
    hits = r.payload["data"]["hits"]
    assert len(hits) == 1
    assert "volcano" in hits[0]["snippet"]
    assert cli("rag_status", bundle=b).payload["data"]["chunks"] == 1
