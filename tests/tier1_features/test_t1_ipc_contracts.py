"""Tier 1 — F15: IPC command surface & DTO schemas.

Test IDs T1.IP1–T1.IP6 (6 tests).
"""

import json
import subprocess

from helpers.assertions import assert_error, assert_ok, assert_single_line_json
from helpers import ipc_contract_harness as contract


def test_ip1_contract_lists_full_surface(authr_bin):
    """T1.IP1 — `authr --contract` enumerates all 34 frozen commands."""
    p = subprocess.run([authr_bin, "--contract"], capture_output=True, text=True, timeout=30)
    assert p.returncode == 0
    payload = assert_single_line_json(p.stdout)
    assert payload["ok"] is True
    cmds = payload["data"]["commands"]
    assert payload["data"]["count"] == contract.EXPECTED_COMMAND_COUNT
    assert set(cmds) == set(contract.CONTRACT_COMMANDS)


def test_ip2_success_envelope_shape(seeded_bundle, cli):
    """T1.IP2 — success envelope is exactly {ok:true, data} on exit 0."""
    r = cli("book_list", bundle=seeded_bundle)
    assert r.exit_code == 0
    contract.assert_envelope_ok(r.payload)


def test_ip3_error_envelope_shape_and_exit_code(cli, tmp_path):
    """T1.IP3 — failure envelope is {ok:false, error{code,message,category}} on exit 1."""
    r = cli("project_open", args={"path": str(tmp_path / "missing.authr")})
    assert r.exit_code == 1
    contract.assert_envelope_error(r.payload)


def test_ip4_entity_crud_roundtrip_dtos(seeded_bundle, cli):
    """T1.IP4 — book/chapter/character/lore/thread rows match contract §5 shapes."""
    b = seeded_bundle
    book = assert_ok(cli("book_get", bundle=b, args={"id": 1}), "book_get")
    contract.assert_row_shape(book, "book")
    chapter = assert_ok(cli("chapter_get", bundle=b, args={"id": 1}), "chapter_get")
    contract.assert_row_shape(chapter, "chapter")
    chars = assert_ok(cli("character_list", bundle=b))["characters"]
    assert chars, "no characters"
    contract.assert_row_shape(chars[0], "character")
    segs = assert_ok(cli("segment_list", bundle=b, args={"chapter_id": 1}))["segments"]
    assert segs
    contract.assert_row_shape(segs[0], "segment")
    lore = assert_ok(cli("lore_upsert", bundle=b, args={
        "key": "Ember Gate", "category": "LOCATION", "content": "A sealed rift.",
    }))
    contract.assert_row_shape(lore, "lore")
    thread = assert_ok(cli("thread_open", bundle=b, args={
        "thread_title": "The sealed rift",
        "introduced_in_chapter": 1,
        "payoff_requirement": "Rift must open in Act III",
        "thread_type": "CHEKHOV_GUN",
    }))
    contract.assert_row_shape(thread, "thread")


def test_ip5_unknown_command_rejected(cli, seeded_bundle):
    """T1.IP5 — an unknown command fails cleanly with an error envelope."""
    r = cli("definitely_not_a_command", bundle=seeded_bundle)
    assert_error(r, what="unknown command")
    contract.assert_envelope_error(r.payload)


def test_ip6_error_taxonomy_closed_set(cli, seeded_bundle, tmp_path):
    """T1.IP6 — sampled failure modes map into the 9-code closed taxonomy."""
    b = seeded_bundle
    samples = [
        cli("book_get", bundle=b, args={"id": 424242}),                       # ENTITY_NOT_FOUND
        cli("segment_append", bundle=b, args={                                 # FOREIGN_KEY_VIOLATION
            "chapter_id": 99, "segment_type": "NARRATION",
            "speaker_id": "NARRATOR", "text_content": "x",
        }),
        cli("project_open", args={"path": str(tmp_path / "nope.authr")}),      # BUNDLE_ERROR
        cli("sandbox_normalize", args={"path": "../escape"}),                  # PATH_TRAVERSAL_VIOLATION
    ]
    for r in samples:
        assert not r.ok
        contract.assert_envelope_error(r.payload)
    codes = {r.code for r in samples}
    assert codes <= contract.CANONICAL_CODES
