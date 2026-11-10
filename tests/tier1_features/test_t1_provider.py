"""Tier 1 — Provider settings ladder & media access (contract v1.6, ADR-0012).

The resolution ladder for every live AI endpoint the runtime talks to:
per-call args > project settings (`provider_settings` table) > env
(`AUTHR_IMG_ENDPOINT` / `AUTHR_LLAMA_SERVER`) > fail-closed. The API key
never lives in the settings table — keychain (`authr_img`) or env only — and
`provider_get` redacts it to a `key_source` badge. `provider_test` runs one
real generation probe and fails CLOSED offline. `video_style` is the
novelcast-parity art-style catalog + per-book selection; `media_read` is the
bundle-scoped read-only accessor the Scene Studio storyboard paints from.

Test IDs T1.PV1–T1.PV9. Fully offline — no network probes with a configured
endpoint; the fail-closed path needs none.
"""

import sqlite3

from helpers.ipc_contract_harness import assert_envelope_error, assert_envelope_ok

SECRET = "sk-super-secret-provider-key"
GOOD_URL = "https://api.example.com/v1/images/generations"


def test_pv1_provider_get_redacts_the_key_and_reports_ladder(bundle, cli, env):
    """T1.PV1 — provider_get carries settings + resolution badges but NEVER the
    key itself; a keychain-stored secret surfaces only as key_source=keychain."""
    b = bundle()
    # Store the key through the keychain (mock backend, same code path as the
    # desktop OS keychain).
    r = cli("keychain_store", bundle=b, env=env, args={
        "service": "authr_img", "key": "api_key", "value": SECRET})
    assert_envelope_ok(r.payload)
    r = cli("provider_get", bundle=b, env=env)
    assert_envelope_ok(r.payload)
    d = r.payload["data"]
    text = repr(d)
    assert SECRET not in text, "keychain secret leaked via provider_get"
    assert d["image"]["resolved"]["key_configured"] is True
    assert d["image"]["resolved"]["key_source"] == "keychain"
    assert d["image"]["resolved"]["endpoint_source"] == "none"
    assert d["ladder"].startswith("args > settings > env")
    assert d["llm"]["resolved"]["source"] == "offline"


def test_pv2_provider_set_validates_shape(bundle, cli, env):
    """T1.PV2 — invalid URLs, out-of-bounds timeout/retries and empty args all
    fail loudly; nothing is persisted."""
    b = bundle()
    for bad in ("notaurl", "ftp://x", "https://user:pass@host", "https://a b.com"):
        r = cli("provider_set", bundle=b, env=env, args={"endpoint_url": bad})
        assert_envelope_error(r.payload)
        assert r.payload["error"]["code"] == "AI_ERROR"
    for args in ({"timeout_s": 1}, {"timeout_s": 999}, {"retries": 9}, {}):
        r = cli("provider_set", bundle=b, env=env, args=args)
        assert_envelope_error(r.payload)
        assert r.payload["error"]["code"] == "AI_ERROR"
    # Nothing leaked into the table.
    r = cli("provider_get", bundle=b, env=env)
    d = r.payload["data"]
    assert d["image"]["settings"]["endpoint_url"] is None
    assert d["image"]["resolved"]["endpoint_source"] == "none"


def test_pv3_provider_set_then_clear_roundtrip(bundle, cli, env):
    """T1.PV3 — a valid set flips the resolved source to `settings`; nulls
    clear rows and the resolution falls back to fail-closed `none`."""
    b = bundle()
    r = cli("provider_set", bundle=b, env=env, args={
        "endpoint_url": GOOD_URL, "model": "flux-pro", "timeout_s": 45, "retries": 2})
    assert_envelope_ok(r.payload)
    d = r.payload["data"]
    assert set(d["applied"]) == {"image.endpoint_url", "image.model", "image.timeout_s", "image.retries"}
    assert d["image"]["resolved"]["endpoint_source"] == "settings"
    assert d["image"]["resolved"]["endpoint_url"] == GOOD_URL
    assert d["image"]["resolved"]["model_source"] == "settings"
    assert d["image"]["resolved"]["retries"] == 2
    # Clear everything (nulls delete).
    r = cli("provider_set", bundle=b, env=env, args={
        "endpoint_url": None, "model": None, "timeout_s": None, "retries": None})
    assert_envelope_ok(r.payload)
    d = r.payload["data"]
    assert d["image"]["settings"]["endpoint_url"] is None
    assert d["image"]["resolved"]["endpoint_source"] == "none"


def test_pv4_ladder_settings_outrank_env(bundle, cli, env):
    """T1.PV4 — the env tier answers only when no setting exists; a stored
    setting always wins; removing it re-exposes env. LLM ladder mirrors it."""
    b = bundle()
    env_with = dict(env)
    env_with["AUTHR_IMG_ENDPOINT"] = "http://env-host/v1/images/generations"
    env_with["AUTHR_LLAMA_SERVER"] = "http://env-llama:8080"
    env_llm = dict(env)
    env_llm["AUTHR_LLAMA_SERVER"] = "http://env-llama:8080"

    d = cli("provider_get", bundle=b, env=env_with).payload["data"]
    assert d["image"]["resolved"]["endpoint_source"] == "env"
    assert d["image"]["resolved"]["endpoint_url"] == "http://env-host/v1/images/generations"
    assert d["llm"]["resolved"]["source"] == "env"

    cli("provider_set", bundle=b, env=env_with, args={"endpoint_url": GOOD_URL})
    d = cli("provider_get", bundle=b, env=env_with).payload["data"]
    assert d["image"]["resolved"]["endpoint_source"] == "settings"
    assert d["image"]["resolved"]["endpoint_url"] == GOOD_URL

    # LLM: settings beat env too.
    cli("provider_set", bundle=b, env=env_llm, args={"server_url": "http://settings-llama:8080"})
    d = cli("provider_get", bundle=b, env=env_llm).payload["data"]
    assert d["llm"]["resolved"]["source"] == "settings"
    assert d["llm"]["resolved"]["server_url"] == "http://settings-llama:8080"


def test_pv5_provider_test_fails_closed_offline(bundle, cli, env):
    """T1.PV5 — with no endpoint configured (settings/env/args all empty) the
    probe fails CLOSED with AI_ENGINE_UNAVAILABLE and writes nothing."""
    b = bundle()
    r = cli("provider_test", bundle=b, env=env, args={})
    assert_envelope_error(r.payload)
    assert r.payload["error"]["code"] == "AI_ENGINE_UNAVAILABLE"
    # Arg-override validation happens before any network traffic.
    r = cli("provider_test", bundle=b, env=env, args={"endpoint_url": "notaurl"})
    assert_envelope_error(r.payload)
    assert r.payload["error"]["code"] == "AI_ERROR"


def _seed_scene(b, cli):
    r = cli("chapter_create", bundle=b, args={"book_id": 1, "title": "The Harbor", "chapter_number": 1})
    assert_envelope_ok(r.payload)
    cid = r.payload["data"]["id"]
    r = cli("segment_append", bundle=b, args={
        "chapter_id": cid, "segment_type": "NARRATION", "speaker_id": "NARRATOR",
        "text_content": "The harbor lights blinked twice before Kira stepped onto the wet pier."})
    assert_envelope_ok(r.payload)
    return cid


def test_pv6_video_style_default_and_catalog(bundle, cli, env):
    """T1.PV6 — a fresh book resolves the default style; the catalog returns
    the ten novelcast-parity styles with prompts."""
    b = bundle()
    _seed_scene(b, cli)
    r = cli("video_style", bundle=b, env=env, args={"book_id": 1})
    assert_envelope_ok(r.payload)
    d = r.payload["data"]
    assert d["style_id"] == "cinematic"
    assert d["label"] == "Cinematic Realism"
    assert len(d["styles"]) == 10
    ids = [s["id"] for s in d["styles"]]
    for expected in ("cinematic", "storybook", "manhwa", "anime", "manga-ink",
                     "oil", "comic", "pixar", "dark-fantasy", "pulp"):
        assert expected in ids
    assert all(s["prompt"] for s in d["styles"])


def test_pv7_video_style_set_persists_per_book(bundle, cli, env):
    """T1.PV7 — style_id validates against the catalog, persists per book
    (unknown ids rejected), and books resolve independently."""
    b = bundle()
    _seed_scene(b, cli)
    r = cli("video_style", bundle=b, env=env, args={"book_id": 1, "style_id": "manga-ink"})
    assert_envelope_ok(r.payload)
    assert r.payload["data"]["style_id"] == "manga-ink"
    # Read-back.
    r = cli("video_style", bundle=b, env=env, args={"book_id": 1})
    assert r.payload["data"]["style_id"] == "manga-ink"
    # Unknown style rejected.
    assert_envelope_error(
        cli("video_style", bundle=b, env=env, args={"book_id": 1, "style_id": "warp-drive"}).payload)
    # Unknown book rejected.
    assert_envelope_error(
        cli("video_style", bundle=b, env=env, args={"book_id": 99}).payload)
    # Selection lives in provider_settings (section art), never the book row.
    db = b.path / "project.db"
    conn = sqlite3.connect(str(db))
    row = conn.execute(
        "SELECT value FROM provider_settings WHERE section='art' AND key='book:1'").fetchone()
    conn.close()
    assert row == ("manga-ink",)


def test_pv8_media_read_roundtrip_and_sandbox(bundle, cli, env):
    """T1.PV8 — media_read serves a rendered asset (mime + sha matching the
    registry) and rejects traversal, absolute paths and non-image containers."""
    b = bundle()
    cid = _seed_scene(b, cli)
    r = cli("video_render", bundle=b, env=env, args={"chapter_id": cid, "engine": "SCRIPTED"})
    assert_envelope_ok(r.payload)
    assets = r.payload["data"]["assets"]
    scene = next(a for a in assets if a["kind"] == "SCENE")
    r = cli("media_read", bundle=b, env=env, args={"path": scene["path"]})
    assert_envelope_ok(r.payload)
    d = r.payload["data"]
    assert d["mime"] == "image/bmp"
    assert d["sha256"] == scene["sha256"]
    assert d["bytes"] == scene["bytes"]
    assert d["data_b64"], "payload must carry the base64 body"
    # Sandbox: traversal, absolute paths, wrong containers, missing files.
    assert_envelope_error(
        cli("media_read", bundle=b, env=env,
            args={"path": "media/video/../../exports/video_ch1.mp4"}).payload)
    assert_envelope_error(
        cli("media_read", bundle=b, env=env, args={"path": "/etc/passwd"}).payload)
    assert_envelope_error(
        cli("media_read", bundle=b, env=env, args={"path": "media/video/b1/ch1/nope.bmp"}).payload)
    assert_envelope_error(
        cli("media_read", bundle=b, env=env,
            args={"path": "exports/video_ch1.mp4"}).payload)


def test_pv9_style_switch_deterministically_repaints(bundle, cli, env):
    """T1.PV9 — SCRIPTED determinism survives the style seed: same style
    re-render is byte-identical, switching style repaints different bytes, and
    the registry carries the engine + updated sha."""
    b = bundle()
    cid = _seed_scene(b, cli)

    def render_style(style):
        if style:
            assert_envelope_ok(cli("video_style", bundle=b, env=env,
                                   args={"book_id": 1, "style_id": style}).payload)
        r = cli("video_render", bundle=b, env=env, args={"chapter_id": cid, "engine": "SCRIPTED"})
        assert_envelope_ok(r.payload)
        return next(a for a in r.payload["data"]["assets"] if a["kind"] == "SCENE")

    a1 = render_style("cinematic")
    a1b = render_style(None)  # re-render, same style
    assert a1b["sha256"] == a1["sha256"], "same style must stay byte-identical"
    a2 = render_style("pulp")
    assert a2["sha256"] != a1["sha256"], "style switch must repaint"
    assert a2["engine"] == "SCRIPTED"
