"""Tier 4 — Real-world author workflow scenarios (5 tests).

Complete journeys: project init, multi-chapter speech casting, worldbuilding
ledger, FTS revision, archive export/import roundtrip.
"""

import json

from helpers.assertions import assert_ok
from helpers import git_harness as gh
from helpers import shadow_harness as sh


def test_s1_new_novel_init(cli, tmp_path):
    """T4.S1 — init a project: manifest, seeded book, narrator, git scaffold."""
    path = tmp_path / "Ember Gate.authr"
    r = cli("project_init", args={
        "path": str(path), "title": "The Ember Gate", "author": "K. Voss",
    })
    data = assert_ok(r, "project_init")
    assert data["book_id"] == 1
    m = data["manifest"]
    assert m["title"] == "The Ember Gate" and m["schema_version"] == 1

    books = assert_ok(cli("book_list", bundle=path))["books"]
    assert len(books) == 1 and books[0]["title"] == "The Ember Gate"
    chars = assert_ok(cli("character_list", bundle=path))["characters"]
    assert any(c["character_key"] == "NARRATOR" for c in chars)
    assert gh.commit_messages(path)[0] == gh.SCAFFOLD_MESSAGE


def test_s2_multi_chapter_speech_casting(cli, bundle):
    """T4.S2 — cast three voices, draft two chapters with attributed dialogue."""
    b = bundle(title="Signal Bells")
    for key, name, role, engine, vid in [
        ("CHAR_KIRA", "Kira Voss", "PROTAGONIST", "EDGE_TTS", "en-US-JennyNeural"),
        ("CHAR_JULIAN", "Julian Marsh", "ANTAGONIST", "ELEVEN_LABS", "pNInz6obpageDIXIuce65"),
        ("CHAR_WREN", "Wren Okafor", "SUPPORTING", "KOKORO_LOCAL", "af_bella"),
    ]:
        assert_ok(cli("character_upsert", bundle=b, args={
            "character_key": key, "display_name": name, "role": role,
            "voice_engine": engine, "voice_id": vid,
            "ipa_phonetic": " IPA pending ", "base_rate": 1.0, "base_pitch": 1.0,
        }))

    ch1 = assert_ok(cli("chapter_create", bundle=b, args={
        "book_id": 1, "chapter_number": 1, "title": "First Bell", "act_number": 1,
    }))["id"]
    ch2 = assert_ok(cli("chapter_create", bundle=b, args={
        "book_id": 1, "chapter_number": 2, "title": "Second Bell", "act_number": 1,
    }))["id"]

    script = [
        (ch1, "NARRATION", "NARRATOR", "neutral", "The bells rang wrong that morning."),
        (ch1, "DIALOGUE", "CHAR_KIRA", "tense", "That is not the hour bell."),
        (ch1, "DIALOGUE", "CHAR_JULIAN", "cold_anger", "Then someone retuned it."),
        (ch2, "NARRATION", "NARRATOR", "neutral", "Kira climbed the bell tower alone."),
        (ch2, "INTERNAL_MONOLOGUE", "NARRATOR", "whisper", "Who touches the bells touches the city."),
        (ch2, "DIALOGUE", "CHAR_WREN", "joyful", "Up here! I found the rope!"),
    ]
    for ch, stype, speaker, emotion, text in script:
        assert_ok(cli("segment_append", bundle=b, args={
            "chapter_id": ch, "segment_type": stype, "speaker_id": speaker,
            "emotion": emotion, "text_content": text,
        }))

    segs1 = assert_ok(cli("segment_list", bundle=b, args={"chapter_id": ch1}))["segments"]
    segs2 = assert_ok(cli("segment_list", bundle=b, args={"chapter_id": ch2}))["segments"]
    assert [s["sequence_order"] for s in segs1] == [0, 1, 2]
    assert [s["sequence_order"] for s in segs2] == [0, 1, 2]
    assert segs1[1]["speaker_id"] == "CHAR_KIRA" and segs1[1]["emotion"] == "tense"

    md = sh.read_md(b, 1)
    assert ('Kira Voss', 'That is not the hour bell.') in sh.dialogue_lines(md)
    ast = sh.read_ast(b, 2)
    assert ast[1]["segment_type"] == "INTERNAL_MONOLOGUE"

    # Word counts maintained.
    ch_rows = assert_ok(cli("chapter_list", bundle=b))["chapters"]
    by_id = {c["id"]: c for c in ch_rows}
    assert by_id[ch1]["actual_word_count"] > 0


def test_s3_worldbuilding_ledger(cli, bundle):
    """T4.S3 — lorebook + Chekhov ledger with payoff resolution."""
    b = bundle(title="Ledger of Ash")
    ch1 = assert_ok(cli("chapter_create", bundle=b, args={
        "book_id": 1, "chapter_number": 1, "title": "Ledgers",
    }))["id"]
    ch3 = assert_ok(cli("chapter_create", bundle=b, args={
        "book_id": 1, "chapter_number": 2, "title": "Payoff",
    }))["id"]

    for key, cat, content in [
        ("Ember Gate", "LOCATION", "A sealed rift between sectors."),
        ("Ashwright Guild", "FACTION", "Keepsers of the buried ledgers."),
        ("Cold Fire", "MAGIC_TECH", "Burns only at absolute zero."),
    ]:
        assert_ok(cli("lore_upsert", bundle=b, args={
            "key": key, "category": cat, "content": content,
        }))
    lore = assert_ok(cli("lore_list", bundle=b))["lore"]
    assert {row["category"] for row in lore} == {"LOCATION", "FACTION", "MAGIC_TECH"}

    gun = assert_ok(cli("thread_open", bundle=b, args={
        "thread_title": "The burned ledger page",
        "introduced_in_chapter": ch1,
        "payoff_requirement": "The burned page resurfaces in Act III",
        "thread_type": "CHEKHOV_GUN",
    }))
    mystery = assert_ok(cli("thread_open", bundle=b, args={
        "thread_title": "Who signed the ledger?",
        "introduced_in_chapter": ch1,
        "payoff_requirement": "Signer revealed",
        "thread_type": "MYSTERY",
    }))
    open_threads = assert_ok(cli("thread_list", bundle=b, args={"status": "INTRODUCED"}))["threads"]
    assert len(open_threads) == 2

    resolved = assert_ok(cli("thread_resolve", bundle=b, args={
        "id": gun["id"], "resolved_in_chapter": ch3,
    }))
    assert resolved["status"] == "RESOLVED" and resolved["resolved_in_chapter"] == ch3
    still_open = assert_ok(cli("thread_list", bundle=b, args={"status": "INTRODUCED"}))["threads"]
    assert [t["id"] for t in still_open] == [mystery["id"]]


def test_s4_fts_revision_cycle(cli, bundle):
    """T4.S4 — draft, search, revise, re-search, remove: full FTS lifecycle."""
    b = bundle(title="Revision Cycle")
    ch = assert_ok(cli("chapter_create", bundle=b, args={
        "book_id": 1, "chapter_number": 1, "title": "Drafts",
    }))["id"]
    assert_ok(cli("segment_append", bundle=b, args={
        "chapter_id": ch, "segment_type": "NARRATION", "speaker_id": "NARRATOR",
        "text_content": "The running water carried emberlight downstream.",
    }))
    hits = assert_ok(cli("segment_search", bundle=b, args={"query": "emberlight"}))["hits"]
    assert len(hits) == 1
    sid = hits[0]["segment_id"]

    segs = assert_ok(cli("segment_list", bundle=b, args={"chapter_id": ch}))["segments"]
    assert_ok(cli("sync_lexical_ast_mutation", bundle=b, args={
        "chapter_id": ch,
        "upserts": [dict(segs[0],
                         text_content="The still water carried lantern flames downstream.")],
        "deletes": [],
    }))
    assert assert_ok(cli("segment_search", bundle=b, args={"query": "emberlight"}))["hits"] == []
    hits2 = assert_ok(cli("segment_search", bundle=b, args={"query": "lantern"}))["hits"]
    assert any(h["segment_id"] == sid for h in hits2)

    assert_ok(cli("sync_lexical_ast_mutation", bundle=b, args={
        "chapter_id": ch, "upserts": [], "deletes": [sid],
    }))
    assert assert_ok(cli("segment_search", bundle=b, args={"query": "lantern"}))["hits"] == []


def test_s5_archive_export_import_roundtrip(cli, seeded_bundle, tmp_path):
    """T4.S5 — export → import → identical content; archive carries no volatile state."""
    b = seeded_bundle
    original_books = assert_ok(cli("book_list", bundle=b))["books"]
    original_segments = assert_ok(cli("segment_list", bundle=b, args={"chapter_id": 1}))["segments"]

    out = tmp_path / "Novel.authr.zip"
    assert_ok(cli("project_export", bundle=b, args={"out": str(out)}))
    dest = tmp_path / "Restored.authr"
    assert_ok(cli("project_import", args={"zip": str(out), "dest": str(dest)}))

    restored_books = assert_ok(cli("book_list", bundle=dest))["books"]
    assert [(x["id"], x["title"]) for x in restored_books] == \
           [(x["id"], x["title"]) for x in original_books]
    restored_segments = assert_ok(cli("segment_list", bundle=dest, args={"chapter_id": 1}))["segments"]
    assert [(s["segment_id"], s["text_content"]) for s in restored_segments] == \
           [(s["segment_id"], s["text_content"]) for s in original_segments]
    assert dest.joinpath(".authr_lock").exists() is False
    wal = dest / "project.db-wal"
    assert not wal.exists() or wal.stat().st_size == 0
