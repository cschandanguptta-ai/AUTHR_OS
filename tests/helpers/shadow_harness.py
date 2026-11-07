"""AUTHR_OS — shadow markdown & AST JSON serializer/validator."""

import json
import re
from pathlib import Path


def _p(bundle_path) -> Path:
    return Path(getattr(bundle_path, "path", bundle_path))


def read_md(bundle_path, nn: int) -> str:
    return (_p(bundle_path) / "manuscript" / f"ch{nn:02d}.md").read_text()


def read_ast(bundle_path, nn: int):
    return json.loads((_p(bundle_path) / "manuscript" / f"ch{nn:02d}.ast.json").read_text())


def parse_frontmatter(md: str) -> dict:
    """Parse the YAML frontmatter block into a dict (flat string values)."""
    m = re.match(r"^---\n(.*?)\n---\n", md, re.DOTALL)
    if not m:
        return {}
    out = {}
    for line in m.group(1).splitlines():
        if ":" in line:
            key, _, val = line.partition(":")
            out[key.strip()] = val.strip().strip('"')
    return out


def chapter_heading(md: str):
    m = re.search(r"^# Chapter (\d+): (.+)$", md, re.MULTILINE)
    return (int(m.group(1)), m.group(2)) if m else None


def dialogue_lines(md: str):
    return re.findall(r"\*\*(.+?)\*\*: \"(.+?)\"", md)


def monologue_lines(md: str):
    return re.findall(r"^\*(.+)\*$", md, re.MULTILINE)


def narration_paragraphs(md: str):
    """Plain paragraphs that are neither dialogue nor monologue."""
    body = re.sub(r"^---\n.*?\n---\n", "", md, flags=re.DOTALL)
    body = re.sub(r"^# .*$", "", body, flags=re.MULTILINE)
    paras = [p.strip() for p in body.split("\n\n") if p.strip()]
    return [p for p in paras
            if not (p.startswith("**") and re.match(r"^\*\*.+\*\*: \"", p))
            and not (p.startswith("*") and p.endswith("*"))]


def assert_ast_shape(segment_obj: dict):
    for field in [
        "segment_id", "chapter_id", "sequence_order", "segment_type", "speaker_id",
        "emotion", "prosody_rate", "prosody_pitch", "text_content", "ambient_cue",
        "sfx_cue", "prev_request_id", "audio_hash", "audio_start_ms", "audio_duration_ms",
    ]:
        assert field in segment_obj, f"ast.json segment missing `{field}`"
