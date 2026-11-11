"""Tier 1 — Retention & serialization kit (contract v1.4): SFB-2.0 blueprint,
chapter QC scoring, ending rotation, micro-arc phases, cadence arithmetic,
drafting worksheet.

Test IDs T1.RT1–T1.RT9.
"""

import json
import sqlite3

from helpers.ipc_contract_harness import assert_envelope_error, assert_envelope_ok


def test_rt1_retention_catalog_is_bundlefree_and_complete(cli):
    """T1.RT1 — craft_catalog kind=retention serves the SFB-2.0 blueprint
    without a bundle; counts are frozen (7 beats / 6 endings / 4 phases /
    7 QC criteria / 5 worksheet sections)."""
    r = cli("craft_catalog", args={"kind": "retention"})
    assert_envelope_ok(r.payload)
    d = r.payload["data"]
    bp = d["blueprint"]
    assert bp["blueprint"] == "SFB-2.0"
    assert d["count"] == 1 and len(d["entries"]) == 1
    assert bp["counts"] == {
        "chapter_beats": 7,
        "ending_types": 6,
        "micro_arc_phases": 4,
        "qc_criteria": 7,
        "worksheet_sections": 5,
    }
    assert [b["id"] for b in bp["chapter_beats"]] == [
        "opening_continuity", "objective", "resistance", "escalation",
        "turning_point", "payoff", "forward_pull",
    ]
    assert [e["id"] for e in bp["ending_types"]] == [
        "danger", "emotional_revelation", "mystery_discovery",
        "irreversible_choice", "reversal", "anticipation",
    ]
    assert [p["id"] for p in bp["micro_arc"]] == [
        "setup", "escalation", "reversal_pivot", "climax_resolution",
    ]
    assert bp["qc_criteria"][1] == "narrative_objective"
    assert bp["zero_flag_criteria"] == ["narrative_objective", "chapter_payoff"]
    assert all(e["example"] and e["when"] for e in bp["ending_types"])
    assert bp["cadence"]["chapters_per_week"] == [2, 3]
    assert bp["cadence"]["buffer_chapters"] == [4, 8]
    assert bp["opening"]["benchmark_words"] == [200, 400]
    # The `all` slice now embeds the blueprint too.
    r_all = cli("craft_catalog", args={"kind": "all"})
    assert r_all.payload["data"]["retention"]["blueprint"] == "SFB-2.0"
    # Unknown kind still a CRAFT_ERROR.
    r_bad = cli("craft_catalog", args={"kind": "astrology"})
    assert_envelope_error(r_bad.payload)
    assert r_bad.payload["error"]["code"] == "CRAFT_ERROR"


def test_rt2_worksheet_plans_unchaptered_next_chapter(bundle, cli):
    """T1.RT2 — worksheet works for chapters that do not exist yet and fuses
    phase, rotation and section context."""
    b = bundle()
    r = cli("craft_retention_worksheet", bundle=b, args={"book_id": 1, "chapter_number": 1})
    assert_envelope_ok(r.payload)
    d = r.payload["data"]
    assert d["chapter_exists"] is False
    assert d["blueprint"] == "SFB-2.0"
    assert d["phase"]["phase_id"] == "setup"
    assert d["phase"]["cycle"] == 1 and d["phase"]["step_in_phase"] == 1
    assert len(d["ending_rotation"]["suggested"]) == 6  # no history yet
    letters = [s["letter"] for s in d["sections"]]
    assert letters == ["A", "B", "C", "D", "E"]
    for s in d["sections"]:
        assert s["prompts"] and s["context"]
    assert d["qc_criteria"][0] == "opening_attention"
    # Craft defaults ride along (plot arc + persona from craft_state defaults).
    assert d["arc"]["id"] == "SAVE_THE_CAT"
    assert d["beat_stamp"] is None
    assert d["previous_qc"] is None


def test_rt3_check_scores_persists_and_steers(bundle, cli):
    """T1.RT3 — a perfect check persists (14/STRONG), suggests next-chapter
    endings without the one just used, and the report sees it."""
    b = bundle()
    assert cli("craft_scaffold", bundle=b, args={"book_id": 1, "chapter_count": 3}).ok
    r = cli("craft_retention_check", bundle=b, args={
        "book_id": 1, "chapter_number": 1,
        "scores": [2, 2, 2, 2, 2, 2, 2], "ending_type": "danger",
        "notes": "clean open, strong pull",
    })
    assert_envelope_ok(r.payload)
    d = r.payload["data"]
    assert d["qc"]["total"] == 14
    assert d["qc"]["band"] == "STRONG"
    assert d["qc"]["revision_required"] is False
    assert d["chapter_id"] >= 1
    nxt = d["next_chapter"]
    assert nxt["number"] == 2
    assert nxt["phase"]["phase_id"] == "setup"  # ch2 still in Phase 1
    sugg = [e["id"] for e in nxt["suggested_endings"]]
    assert len(sugg) == 5 and "danger" not in sugg

    # Persisted: report shows 1 scored chapter, STRONG band, danger ending.
    rep = cli("craft_retention_report", bundle=b, args={"book_id": 1}).payload["data"]
    assert rep["summary"]["qc_scored"] == 1
    assert rep["summary"]["bands"]["STRONG"] == 1
    assert rep["summary"]["revision_required"] == 0
    assert rep["summary"]["endings_used"] == {"danger": 1}
    ch1 = next(c for c in rep["chapters"] if c["chapter_number"] == 1)
    assert ch1["qc"]["total"] == 14
    assert ch1["qc"]["ending_type"] == "danger"
    assert ch1["qc"]["notes"] == "clean open, strong pull"


def test_rt4_zero_flag_forces_revision_despite_strong_total(bundle, cli):
    """T1.RT4 — clarity zero (blueprint Part 9) forces revision even at 12/14."""
    b = bundle()
    assert cli("craft_scaffold", bundle=b, args={"book_id": 1, "chapter_count": 3}).ok
    r = cli("craft_retention_check", bundle=b, args={
        "book_id": 1, "chapter_number": 2, "scores": [2, 0, 2, 2, 2, 2, 2],
    })
    assert_envelope_ok(r.payload)
    d = r.payload["data"]
    assert d["qc"]["total"] == 12
    assert d["qc"]["band"] == "STRONG"
    assert d["qc"]["revision_required"] is True
    assert d["qc"]["zero_flags"] == ["narrative_objective"]
    rep = cli("craft_retention_report", bundle=b, args={"book_id": 1}).payload["data"]
    assert rep["summary"]["revision_required"] == 1
    ch2 = next(c for c in rep["chapters"] if c["chapter_number"] == 2)
    assert ch2["qc"]["revision_required"] is True
    # Worksheet for the same chapter surfaces the open revision.
    ws = cli("craft_retention_worksheet", bundle=b, args={
        "book_id": 1, "chapter_number": 2,
    }).payload["data"]
    assert ws["previous_qc"]["total"] == 12
    assert ws["previous_qc"]["revision_required"] is True


def test_rt5_check_validates_inputs_canonically(bundle, cli):
    """T1.RT5 — malformed scores / unknown ids surface canonical codes."""
    b = bundle()
    assert cli("craft_scaffold", bundle=b, args={"book_id": 1, "chapter_count": 2}).ok
    bad_args = [
        {"book_id": 1, "chapter_number": 1, "scores": [2, 2, 2]},            # wrong length
        {"book_id": 1, "chapter_number": 1, "scores": [2, 2, 2, 2, 2, 2, 3]},  # out of range
        {"book_id": 1, "chapter_number": 1, "scores": [2, 2, 2, 2, 2, 2, -1]},
        {"book_id": 1, "chapter_number": 1, "scores": [2, 2, 2, 2, 2, 2, 2],
         "ending_type": "plot_armor"},                                        # unknown ending
        {"book_id": 1, "chapter_number": 0, "scores": [2, 2, 2, 2, 2, 2, 2]},  # bad number
    ]
    for args in bad_args:
        r = cli("craft_retention_check", bundle=b, args=args)
        assert_envelope_error(r.payload)
        assert r.payload["error"]["code"] == "CRAFT_ERROR", f"{args}: {r.payload}"
    # Missing chapter / book are entity misses, not craft-arg misses.
    for args in (
        {"book_id": 1, "chapter_number": 99, "scores": [2, 2, 2, 2, 2, 2, 2]},
        {"book_id": 999, "chapter_number": 1, "scores": [2, 2, 2, 2, 2, 2, 2]},
    ):
        r = cli("craft_retention_check", bundle=b, args=args)
        assert_envelope_error(r.payload)
        assert r.payload["error"]["code"] == "ENTITY_NOT_FOUND", f"{args}: {r.payload}"
    # Report/worksheet on a missing book are ENTITY_NOT_FOUND too.
    for cmd in ("craft_retention_report", "craft_retention_worksheet"):
        r = cli(cmd, bundle=b, args={"book_id": 424242, "chapter_number": 1})
        assert_envelope_error(r.payload)
        assert r.payload["error"]["code"] == "ENTITY_NOT_FOUND"


def test_rt6_ending_rotation_blocks_repeats_in_window(bundle, cli):
    """T1.RT6 — the worksheet excludes endings used in the last 3 chapters
    and the report flags repetition."""
    b = bundle()
    assert cli("craft_scaffold", bundle=b, args={"book_id": 1, "chapter_count": 4}).ok
    plan = [(1, "danger"), (2, "reversal"), (3, "danger")]
    for num, ending in plan:
        r = cli("craft_retention_check", bundle=b, args={
            "book_id": 1, "chapter_number": num,
            "scores": [2, 2, 2, 2, 2, 2, 2], "ending_type": ending,
        })
        assert r.ok, f"check ch{num}: {r.payload}"
    ws = cli("craft_retention_worksheet", bundle=b, args={
        "book_id": 1, "chapter_number": 4,
    }).payload["data"]
    recent = ws["ending_rotation"]["recent"]
    assert recent == ["danger", "reversal", "danger"]
    sugg = [e["id"] for e in ws["ending_rotation"]["suggested"]]
    assert len(sugg) == 4
    assert "danger" not in sugg and "reversal" not in sugg
    assert "irreversible_choice" in sugg and "anticipation" in sugg

    rep = cli("craft_retention_report", bundle=b, args={"book_id": 1}).payload["data"]
    assert rep["summary"]["recent_endings"] == ["danger", "reversal", "danger"]
    assert rep["summary"]["ending_repetition_in_window"] == ["danger"]
    assert rep["summary"]["endings_used"] == {"danger": 2, "reversal": 1}


def test_rt7_report_cadence_buffer_and_phase_map(bundle, cli):
    """T1.RT7 — cadence arithmetic uses the chapter pipeline; the phase map
    covers every chapter plus the next one."""
    b = bundle()
    assert cli("craft_scaffold", bundle=b, args={"book_id": 1, "chapter_count": 6}).ok
    # Pipeline: 2 POLISHED + 1 DRAFTING reserve, 1 PUBLISHED, 2 PLANNED.
    for cid, status in ((2, "POLISHED"), (3, "POLISHED"), (4, "DRAFTING"), (5, "PUBLISHED")):
        assert cli("chapter_update", bundle=b, args={"id": cid, "status": status}).ok
    rep = cli("craft_retention_report", bundle=b,
              args={"book_id": 1, "chapters_per_week": 2}).payload["data"]
    cad = rep["cadence"]
    assert cad["reserve_chapters"] == 3
    assert cad["published_chapters"] == 1
    assert cad["planned_chapters"] == 2
    assert cad["buffer_weeks"] == 1.5
    assert cad["buffer_healthy"] is False  # 3 < the 4-chapter minimum
    # Chapter 7 sits in the escalation phase (4–7) of the first micro-arc.
    assert rep["summary"]["next_phase"]["chapter_number"] == 7
    assert rep["summary"]["next_phase"]["phase_id"] == "escalation"
    phases = {c["chapter_number"]: c["phase"]["phase_id"] for c in rep["chapters"]}
    assert phases[1] == "setup" and phases[5] == "escalation"  # 1–3 setup · 4–7 escalation
    assert rep["chapters_per_week"] == 2.0

    # Word-length conformance flags (blueprint range 1000–2000).
    conn = sqlite3.connect(b.path / "project.db")
    try:
        conn.execute("UPDATE chapters SET actual_word_count = 1500 WHERE id = 1")
        conn.execute("UPDATE chapters SET actual_word_count = 400 WHERE id = 2")
        conn.execute("UPDATE chapters SET actual_word_count = 2600 WHERE id = 3")
        conn.commit()
    finally:
        conn.close()
    rep2 = cli("craft_retention_report", bundle=b, args={"book_id": 1}).payload["data"]
    lens = {c["chapter_number"]: c["word_length_guidance"] for c in rep2["chapters"]}
    assert lens[1] == "in_range"
    assert lens[2] == "short"
    assert lens[3] == "long"
    assert lens[4] == "unwritten"  # 0 words


def test_rt8_check_is_a_lease_enforcing_writer(bundle, cli, tmp_path):
    """T1.RT8 — craft_retention_check enforces the foreign lease (writer),
    while report/worksheet stay readable under it."""
    b = bundle()
    assert cli("craft_scaffold", bundle=b, args={"book_id": 1, "chapter_count": 1}).ok
    lock = b.path / ".authr_lock"
    lock.write_text(json.dumps({
        "pid": 999999, "hostname": "other-host", "user": "someone",
        "locked_at": "2026-11-09T00:00:00Z", "lease_expires_at": "2099-01-01T00:00:00Z",
        "session_id": "foreign",
    }))
    r = cli("craft_retention_check", bundle=b, args={
        "book_id": 1, "chapter_number": 1, "scores": [2, 2, 2, 2, 2, 2, 2],
    })
    assert not r.ok
    assert r.payload["error"]["code"] == "READ_ONLY_PROJECT", r.payload
    for cmd in ("craft_retention_report", "craft_retention_worksheet"):
        rr = cli(cmd, bundle=b, args={"book_id": 1, "chapter_number": 1})
        assert rr.ok, f"{cmd} should stay readable: {rr.payload}"


def test_rt9_beat_stamp_and_persona_flow_into_worksheet(bundle, cli):
    """T1.RT9 — worksheet context carries the arc beat stamp, persona
    directive and phase goals from the v1.3 craft layer."""
    b = bundle()
    assert cli("craft_apply", bundle=b, args={
        "book_id": 1, "plot_arc_id": "THREE_ACT",
        "style_persona_id": "EDGE_THRILLER",
    }).ok
    assert cli("craft_scaffold", bundle=b, args={"book_id": 1, "chapter_count": 6}).ok
    ws = cli("craft_retention_worksheet", bundle=b, args={
        "book_id": 1, "chapter_number": 1,
    }).payload["data"]
    assert ws["beat_stamp"] == {"beat_id": "inciting_incident", "beat_label": "Inciting Incident"}
    assert ws["arc"]["id"] == "THREE_ACT"
    assert "EDGE_THRILLER" in ws["sections"][1]["context"] or \
        "edge" in ws["sections"][1]["context"].lower()
    assert "Phase goals" in ws["sections"][2]["context"]
    # Chapter 12 lands in the climax phase of the micro-arc cycle.
    ws12 = cli("craft_retention_worksheet", bundle=b, args={
        "book_id": 1, "chapter_number": 12,
    }).payload["data"]
    assert ws12["phase"]["phase_id"] == "climax_resolution"
    assert ws12["chapter_exists"] is False  # scaffold made 6; planning ahead works
