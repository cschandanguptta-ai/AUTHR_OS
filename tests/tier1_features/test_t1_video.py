"""Tier 1 — Video novel studio (contract v1.5): draft-time media derivation,
SCRIPTED frame engine, asset registry, ffmpeg assembly, validation gate,
fail-closed engine seam. The audio pipeline's architecture, one medium over.

Test IDs T1.VD1–T1.VD10. Requires ffmpeg on PATH for the assembly tests
(same prerequisite as publish_m4b); they SKIP cleanly without it.
"""

import shutil
import subprocess

from helpers.ipc_contract_harness import assert_envelope_error, assert_envelope_ok

SEG1 = "The harbor lights blinked twice before Kira stepped onto the wet pier."
SEG2 = "Kira said the tide table was wrong and the ferry would wait for no one."


def _make_chapter(bundle, cli, chapter_number=1, title="The Harbor"):
    r = cli("chapter_create", bundle=bundle,
            args={"book_id": 1, "title": title, "chapter_number": chapter_number})
    assert_envelope_ok(r.payload)
    return r.payload["data"]["id"]


def _append(bundle, cli, chapter_id, speaker, text, seg_type="NARRATION"):
    # Speakers are FK-bound to the cast (chapter_segments.speaker_id →
    # character_profiles); NARRATOR is seeded by project_init, others must
    # exist first.
    if speaker != "NARRATOR":
        r = cli("character_upsert", bundle=bundle, args={
            "character_key": speaker, "display_name": speaker.title(),
            "role": "PROTAGONIST", "bio": "gray coat, sea-glass eyes"})
        assert_envelope_ok(r.payload)
    r = cli("segment_append", bundle=bundle, args={
        "chapter_id": chapter_id, "segment_type": seg_type,
        "speaker_id": speaker, "text_content": text,
    })
    assert_envelope_ok(r.payload)
    return r.payload["data"]


def test_vd1_draft_time_derivation_serves_plan_without_rederive(bundle, cli):
    """T1.VD1 — segments appended → video_plan is served from the draft-time
    derivation (`refreshed:false` — the hook already computed it) with
    canonical estimator durations and the ≤3-ref character ladder."""
    b = bundle()
    cid = _make_chapter(b, cli)
    _append(b, cli, cid, "NARRATOR", SEG1)
    _append(b, cli, cid, "KIRA", SEG2, seg_type="DIALOGUE")
    r = cli("video_plan", bundle=b, args={"chapter_id": cid})
    assert_envelope_ok(r.payload)
    d = r.payload["data"]
    assert d["scene_count"] == 2
    assert d["derived"]["refreshed"] is False, "draft hook must have derived already"
    assert d["derived"]["source"].startswith("draft-time derivation")
    assert all(s["duration_ms"] > 0 for s in d["scenes"])
    assert d["scenes"][0]["ladder"] in ("TEXT_ONLY", "PARTIAL")
    assert d["narration_ready"] is False
    assert d["resolution"] == "1280x720" and d["fps"] == 30
    # Plan totals: scenes + the 2 s title card.
    assert d["total_ms"] == sum(s["duration_ms"] for s in d["scenes"])


def test_vd2_mutation_catchup_rederives_once(bundle, cli):
    """T1.VD2 — an AST mutation (text changed) re-derives at DRAFT TIME (the
    hook inside the mutation path updates the persisted rows before any stage
    runs), and the following plan is served without re-deriving."""
    b = bundle()
    cid = _make_chapter(b, cli)
    _append(b, cli, cid, "NARRATOR", SEG1)
    p1 = cli("video_plan", bundle=b, args={"chapter_id": cid}).payload["data"]
    assert p1["derived"]["refreshed"] is False
    segs = cli("segment_list", bundle=b, args={"chapter_id": cid}).payload["data"]["segments"]
    r = cli("sync_lexical_ast_mutation", bundle=b, args={
        "chapter_id": cid,
        "upserts": [dict(segs[0],
                         text_content=SEG1 + " The fog rolled in heavier now.")],
        "deletes": [],
    })
    assert_envelope_ok(r.payload)
    # The draft hook inside the mutation path must have already re-derived:
    # the persisted scene duration is updated BEFORE any stage runs.
    rows = b.query("SELECT duration_ms FROM video_scenes ORDER BY scene_index")
    assert rows[0]["duration_ms"] > p1["scenes"][0]["duration_ms"], \
        "mutation hook must re-derive at draft time"
    p2 = cli("video_plan", bundle=b, args={"chapter_id": cid}).payload["data"]
    assert p2["derived"]["refreshed"] is False, "plan must be served, not re-derived"
    assert p2["scenes"][0]["duration_ms"] == rows[0]["duration_ms"]


def test_vd3_plan_fails_closed_on_empty_and_missing_chapters(bundle, cli):
    """T1.VD3 — no segments / no chapter ⇒ canonical ENTITY_NOT_FOUND."""
    b = bundle()
    cid = _make_chapter(b, cli)
    r = cli("video_plan", bundle=b, args={"chapter_id": cid})
    assert_envelope_error(r.payload)
    assert r.payload["error"]["code"] == "ENTITY_NOT_FOUND"
    r = cli("video_plan", bundle=b, args={"chapter_id": 9999})
    assert_envelope_error(r.payload)
    assert r.payload["error"]["code"] == "ENTITY_NOT_FOUND"


def test_vd4_scripted_render_is_complete_and_idempotent(bundle, cli):
    """T1.VD4 — render paints sheet+title+scenes, writes legal BMPs and
    re-renders byte-identically (deterministic engine)."""
    b = bundle()
    cid = _make_chapter(b, cli)
    cli("character_upsert", bundle=b, args={
        "character_key": "KIRA", "display_name": "Kira",
        "role": "PROTAGONIST", "bio": "gray coat, sea-glass eyes"})
    _append(b, cli, cid, "NARRATOR", SEG1)
    _append(b, cli, cid, "KIRA", SEG2, seg_type="DIALOGUE")
    r = cli("video_render", bundle=b, args={"chapter_id": cid})
    assert_envelope_ok(r.payload)
    d = r.payload["data"]
    kinds = [x["kind"] for x in d["rendered"]]
    assert kinds.count("SCENE") == 2 and kinds.count("TITLE") == 1 and kinds.count("SHEET") == 1
    for x in d["rendered"]:
        p = b.path / x["path"]
        assert p.is_file() and p.stat().st_size > 0
        head = p.read_bytes()[:2]
        assert head == b"BM", f"{x['path']} is not a BMP"
        assert (x["width"], x["height"]) in ((1280, 720), (1024, 1024))
    # Idempotence: same engine, same seeds → same bytes, all assets stable.
    r2 = cli("video_render", bundle=b, args={"chapter_id": cid})
    d2 = r2.payload["data"]
    shas1 = sorted(x["sha256"] for x in d["rendered"])
    shas2 = sorted(x["sha256"] for x in d2["rendered"])
    assert shas1 == shas2, "SCRIPTED engine must be byte-deterministic"


def test_vd5_reference_ladder_flips_after_sheets_only(bundle, cli):
    """T1.VD5 — plan flags the missing sheet; `sheets_only` render closes the
    gap; the ladder flips to REFS_READY for scenes naming the character."""
    b = bundle()
    cid = _make_chapter(b, cli)
    cli("character_upsert", bundle=b, args={
        "character_key": "KIRA", "display_name": "Kira",
        "role": "PROTAGONIST", "bio": "gray coat, sea-glass eyes"})
    _append(b, cli, cid, "NARRATOR", "Kira watched the pier from the upper deck.")
    plan = cli("video_plan", bundle=b, args={"chapter_id": cid}).payload["data"]
    assert "KIRA" in plan["missing_sheets"]
    assert plan["scenes"][0]["ladder"] == "PARTIAL"
    r = cli("video_render", bundle=b, args={"chapter_id": cid, "sheets_only": True})
    assert_envelope_ok(r.payload)
    sheets = [x for x in r.payload["data"]["rendered"] if x["kind"] == "SHEET"]
    assert len(sheets) == 1 and sheets[0]["width"] == 1024
    plan2 = cli("video_plan", bundle=b, args={"chapter_id": cid}).payload["data"]
    assert plan2["missing_sheets"] == []
    assert plan2["scenes"][0]["ladder"] == "REFS_READY"
    assert plan2["ladder"]["refs_ready"] == 1


def test_vd6_focal_persists_into_scene_rows(bundle, cli):
    """T1.VD6 — the VideoStudio focal selector persists per scene and flows
    back through the plan (Ken Burns anchor state)."""
    b = bundle()
    cid = _make_chapter(b, cli)
    _append(b, cli, cid, "NARRATOR", SEG1)
    _append(b, cli, cid, "KIRA", SEG2, seg_type="DIALOGUE")
    r = cli("video_render", bundle=b, args={
        "chapter_id": cid, "focal": {"0": "left", "1": "bottom"}})
    assert_envelope_ok(r.payload)
    plan = cli("video_plan", bundle=b, args={"chapter_id": cid}).payload["data"]
    assert plan["scenes"][0]["focal"] == "left"
    assert plan["scenes"][1]["focal"] == "bottom"
    # Invalid focal values are ignored (defaults stay).
    r2 = cli("video_render", bundle=b, args={"chapter_id": cid, "focal": {"0": "diagonal"}})
    assert_envelope_ok(r2.payload)
    plan2 = cli("video_plan", bundle=b, args={"chapter_id": cid}).payload["data"]
    assert plan2["scenes"][0]["focal"] == "left"


def test_vd7_assemble_silent_track(bundle, cli):
    """T1.VD7 — no narration ⇒ assembler auto-renders missing assets and
    encodes a silent-track MP4 sized to plan + title card."""
    if shutil.which("ffmpeg") is None:
        import pytest
        pytest.skip("ffmpeg not available")
    b = bundle()
    cid = _make_chapter(b, cli)
    _append(b, cli, cid, "NARRATOR", SEG1)
    r = cli("video_assemble", bundle=b, args={"chapter_id": cid})
    assert_envelope_ok(r.payload)
    d = r.payload["data"]
    assert d["narration"] == ""
    assert d["clips"] == d["scenes"] + 1  # + title card
    assert d["fps"] == 30 and d["resolution"] == "1280x720"
    mp4 = b.path / d["out"]
    assert mp4.is_file() and mp4.stat().st_size == d["bytes"] > 0
    # MP4 magic: ftyp box at byte 4.
    assert mp4.read_bytes()[4:8] == b"ftyp"


def test_vd8_narration_auto_reuse_from_audio_pipeline(bundle, cli):
    """T1.VD8 — after audio_synthesize, the assembler reuses the per-scene
    pipeline audio (`auto:pipeline-audio`) — the audio work is never redone."""
    if shutil.which("ffmpeg") is None:
        import pytest
        pytest.skip("ffmpeg not available")
    b = bundle()
    cid = _make_chapter(b, cli)
    _append(b, cli, cid, "NARRATOR", SEG1)
    _append(b, cli, cid, "KIRA", SEG2, seg_type="DIALOGUE")
    s = cli("audio_synthesize", bundle=b, args={"chapter_id": cid, "engine": "SCRIPTED"})
    assert_envelope_ok(s.payload)
    plan = cli("video_plan", bundle=b, args={"chapter_id": cid}).payload["data"]
    assert plan["narration_ready"] is True
    assert all(scn["narration_path"] for scn in plan["scenes"])
    r = cli("video_assemble", bundle=b, args={"chapter_id": cid})
    d = r.payload["data"]
    assert d["narration"] == "auto:pipeline-audio"
    v = cli("video_validate", bundle=b, args={"chapter_id": cid})
    assert_envelope_ok(v.payload)
    checks = {c["name"]: c for c in v.payload["data"]["checks"]}
    assert checks["mp4_present"]["status"] == "PASS"
    assert checks["duration_vs_plan"]["status"] == "PASS", checks["duration_vs_plan"]["detail"]
    assert v.payload["data"]["ok"] is True


def test_vd9_validate_reports_the_whole_chain(bundle, cli):
    """T1.VD9 — validate covers plan → registry → file with kdp_check shape."""
    b = bundle()
    cid = _make_chapter(b, cli)
    _append(b, cli, cid, "NARRATOR", SEG1)
    cli("video_render", bundle=b, args={"chapter_id": cid})
    v = cli("video_validate", bundle=b, args={"chapter_id": cid})
    d = v.payload["data"]
    names = [c["name"] for c in d["checks"]]
    assert "scene_plan_derived" in names and "scenes_rendered" in names
    assert "title_card" in names and "mp4_present" in names
    assert d["plan"]["total_ms"] > 0 and d["plan"]["resolution"] == "1280x720"
    # No MP4 yet → WARN, not FAIL (the gate still passes).
    mp4_check = next(c for c in d["checks"] if c["name"] == "mp4_present")
    assert mp4_check["status"] == "WARN"


def test_vd10_engine_seam_fails_closed(bundle, cli):
    """T1.VD10 — R2I_ENDPOINT without AUTHR_IMG_ENDPOINT ⇒ canonical
    AI_ENGINE_UNAVAILABLE (a sheet target must exist, so the endpoint tier is
    actually reached); unknown engine ⇒ AUDIO_ERROR (taxonomy intact)."""
    b = bundle()
    cid = _make_chapter(b, cli)
    _append(b, cli, cid, "KIRA", SEG2, seg_type="DIALOGUE")
    r = cli("video_render", bundle=b, args={"chapter_id": cid, "engine": "R2I_ENDPOINT"})
    assert_envelope_error(r.payload)
    assert r.payload["error"]["code"] == "AI_ENGINE_UNAVAILABLE"
    r2 = cli("video_render", bundle=b, args={"chapter_id": cid, "engine": "WARP_DRIVE"})
    assert_envelope_error(r2.payload)
    assert r2.payload["error"]["code"] == "AUDIO_ERROR"
