"""WCAG 2.2 contrast CI check for the shipped theme tokens (UX re-assessment
D6 Phase 3, "sepia accent + contrast CI check").

Parses src/index.css (the single source of truth: per-theme ``--X-rgb``
channel triples) and computes WCAG 2.x relative-luminance contrast ratios
for every pair that carries readable text in the UI. Gates:

  4.5:1  — all static text pairings (the app renders 11px labels, so there
           is no "large text" 3:1 tier in practice)

Covered per theme: ink and muted on app/panel/panel2; accent, accent-2,
ok, warn, bad on panel/panel2 (chips and status text); on-accent on accent
(primary buttons). Divider/line tokens are decorative and intentionally
not gated.

The sepia theme is the regression this test exists for: the audited build
shipped accent #B45309 (4.46:1 on app bg — under AA for 11px text) and ok
green #0F8A5F (4.07:1 on panel).
"""

import re
from pathlib import Path

import pytest

CSS_PATH = Path(__file__).resolve().parents[2] / "src" / "index.css"

# Theme block selector → the triples we gate.
THEMES = {
    "dark": [":root", ".theme-dark"],
    "midnight": [".theme-midnight"],
    "sepia": [".theme-sepia"],
}


def _parse_blocks(css: str) -> dict:
    """Return {selector: {token: (r, g, b)}} for every theme block."""
    blocks = {}
    # Split on top-level rule boundaries of the shapes we care about.
    pattern = re.compile(
        r"(?P<sel>(?::root|\.theme-[a-z]+)(?:\s*,\s*(?::root|\.theme-[a-z]+))*)\s*\{(?P<body>[^}]*)\}",
        re.S,
    )
    for m in pattern.finditer(css):
        tokens = {}
        for t in re.finditer(
            r"--([a-z0-9-]+)-rgb:\s*(\d+)\s+(\d+)\s+(\d+)\s*;", m.group("body")
        ):
            tokens[t.group(1)] = (int(t.group(2)), int(t.group(3)), int(t.group(4)))
        if tokens:
            for sel in m.group("sel").split(","):
                blocks[sel.strip()] = tokens
    return blocks


def _lum(rgb):
    def chan(c):
        c = c / 255
        return c / 12.92 if c <= 0.03928 else ((c + 0.055) / 1.055) ** 2.4

    r, g, b = rgb
    return 0.2126 * chan(r) + 0.7152 * chan(g) + 0.0722 * chan(b)


def _ratio(fg, bg):
    a, b = _lum(fg), _lum(bg)
    hi, lo = max(a, b), min(a, b)
    return (hi + 0.05) / (lo + 0.05)


@pytest.fixture(scope="module")
def blocks():
    css = CSS_PATH.read_text(encoding="utf-8")
    parsed = _parse_blocks(css)
    missing = [sel for sels in THEMES.values() for sel in sels if sel not in parsed]
    assert not missing, f"theme selectors missing from index.css: {missing}"
    return parsed


def _assert_pair(blocks, theme_sels, fg_token, bg_token, minimum=4.5):
    ratios = []
    for sel in theme_sels:
        tokens = blocks[sel]
        assert fg_token in tokens, f"{sel}: --{fg_token}-rgb missing"
        assert bg_token in tokens, f"{sel}: --{bg_token}-rgb missing"
        r = _ratio(tokens[fg_token], tokens[bg_token])
        ratios.append((sel, r))
        assert r >= minimum, (
            f"{sel}: {fg_token} on {bg_token} is {r:.2f}:1 — below WCAG AA "
            f"({minimum}:1). Darken the token; do not lower this gate."
        )
    return ratios


def test_body_and_label_text_pass_aa(blocks):
    """ink + muted on the three static surface tiers, all themes."""
    for theme, sels in THEMES.items():
        for bg in ("app", "panel", "panel-2"):
            _assert_pair(blocks, sels, "ink", bg)
            _assert_pair(blocks, sels, "muted", bg)


def test_accent_text_passes_aa(blocks):
    """Accent text (links, chips, labels) on the surfaces it renders on."""
    for theme, sels in THEMES.items():
        for bg in ("app", "panel", "panel-2"):
            _assert_pair(blocks, sels, "accent", bg)


def test_status_chip_text_passes_aa(blocks):
    """ok / warn / bad / accent-2 chip text on panel tiers, all themes."""
    for theme, sels in THEMES.items():
        for bg in ("panel", "panel-2"):
            for fg in ("ok", "warn", "bad", "accent-2"):
                _assert_pair(blocks, sels, fg, bg)


def test_on_accent_button_text_passes_aa(blocks):
    """Primary button: on-accent text on the accent fill, all themes."""
    for theme, sels in THEMES.items():
        _assert_pair(blocks, sels, "on-accent", "accent")


def test_sepia_accent_regression(blocks):
    """The audited regression, pinned numerically (RA-06 / D6 Phase 3).

    #B45309 scored 4.46:1 on the sepia app background; the shipped value
    #A34A08 must clear 4.5 with headroom and stay in the amber-brown family.
    """
    tokens = blocks[".theme-sepia"]
    r, g, b = tokens["accent"]
    assert (r, g, b) == (163, 74, 8), (
        f"sepia accent drifted to #{r:02X}{g:02X}{b:02X} — update this pin "
        "together with a fresh AA computation"
    )
    assert _ratio(tokens["accent"], tokens["app"]) >= 5.0
