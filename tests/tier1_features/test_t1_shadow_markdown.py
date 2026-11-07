"""Tier 1 — F14: Shadow Markdown & companion AST JSON rendering.

Test IDs T1.SM1–T1.SM6 (6 tests).
"""

from helpers import shadow_harness as sh


def test_sm1_frontmatter_rendered(seeded_bundle):
    """T1.SM1 — ch01.md has YAML frontmatter with identity fields."""
    md = sh.read_md(seeded_bundle, 1)
    fm = sh.parse_frontmatter(md)
    assert fm["chapter_id"] == "1"
    assert fm["chapter_number"] == "1"
    assert fm["title"] == "Cold Orbit"
    assert fm["act_number"] == "1"
    assert fm["book_id"] == "1"
    assert sh.chapter_heading(md) == (1, "Cold Orbit")


def test_sm2_narration_verbatim(seeded_bundle):
    """T1.SM2 — narration paragraphs are preserved verbatim."""
    paras = sh.narration_paragraphs(sh.read_md(seeded_bundle, 1))
    assert any(p == "The station hummed with dormant power." for p in paras)


def test_sm3_dialogue_quoted_with_speaker(seeded_bundle):
    """T1.SM3 — dialogue renders as `**Name**: "Text"` with display name."""
    dlg = sh.dialogue_lines(sh.read_md(seeded_bundle, 1))
    assert ("Kira Voss", "We have maybe three minutes before it wakes up.") in dlg


def test_sm4_monologue_italicized(seeded_bundle):
    """T1.SM4 — internal monologue renders italicized."""
    mono = sh.monologue_lines(sh.read_md(seeded_bundle, 1))
    assert "Three minutes was never going to be enough." in mono


def test_sm5_ast_json_companion(seeded_bundle):
    """T1.SM5 — ch01.ast.json is a JSON array of full NarrativeSegmentAst objects."""
    ast = sh.read_ast(seeded_bundle, 1)
    assert isinstance(ast, list) and len(ast) == 3
    seqs = [s["sequence_order"] for s in ast]
    assert seqs == sorted(seqs)
    for seg in ast:
        sh.assert_ast_shape(seg)
    types = {s["segment_type"] for s in ast}
    assert types == {"NARRATION", "DIALOGUE", "INTERNAL_MONOLOGUE"}


def test_sm6_atomic_writes_no_tmp_leftovers(seeded_bundle):
    """T1.SM6 — shadow files are staged via .tmp and renamed; no leftovers."""
    assert seeded_bundle.shadow_tmp_leftovers() == []
    files = [f for f in seeded_bundle.list_files() if f.startswith("manuscript/")]
    assert "manuscript/ch01.md" in files
    assert "manuscript/ch01.ast.json" in files
