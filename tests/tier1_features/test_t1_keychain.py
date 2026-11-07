"""Tier 1 — F3: OS Keychain credential manager & headless mock fallback.

Test IDs T1.K1–T1.K5 (5 tests).
"""

from helpers.assertions import assert_error, assert_ok


def test_k1_store_api_key(cli):
    """T1.K1 — stores an API key successfully."""
    r = cli("keychain_store", args={
        "service": "anthropic", "key": "api_key", "value": "sk-ant-test-001",
    })
    assert_ok(r, "keychain_store")


def test_k2_get_api_key_roundtrip(cli):
    """T1.K2 — retrieves the exact stored value."""
    cli("keychain_store", args={
        "service": "anthropic", "key": "api_key", "value": "sk-ant-test-002",
    })
    r = cli("keychain_get", args={"service": "anthropic", "key": "api_key"})
    data = assert_ok(r, "keychain_get")
    assert data["value"] == "sk-ant-test-002"


def test_k3_update_api_key(cli):
    """T1.K3 — re-storing overwrites the previous value."""
    cli("keychain_store", args={"service": "gemini", "key": "api_key", "value": "v1"})
    cli("keychain_store", args={"service": "gemini", "key": "api_key", "value": "v2"})
    r = cli("keychain_get", args={"service": "gemini", "key": "api_key"})
    assert_ok(r).get("value") == "v2"
    assert assert_ok(r, "keychain_get")["value"] == "v2"


def test_k4_delete_api_key(cli):
    """T1.K4 — delete removes the credential; later read is ENTITY_NOT_FOUND."""
    cli("keychain_store", args={"service": "groq", "key": "api_key", "value": "gq-1"})
    r = cli("keychain_delete", args={"service": "groq", "key": "api_key"})
    assert_ok(r, "keychain_delete")
    r = cli("keychain_get", args={"service": "groq", "key": "api_key"})
    assert_error(r, code="ENTITY_NOT_FOUND", what="get after delete")


def test_k5_mock_keyring_file_fallback(cli, env, tmp_path):
    """T1.K5 — AUTHR_MOCK_KEYCHAIN=1 uses the file-backed mock store."""
    keyring_file = tmp_path / "kr.json"
    e = dict(env)
    e["AUTHR_MOCK_KEYRING_FILE"] = str(keyring_file)
    r = cli("keychain_store", args={
        "service": "openai", "key": "api_key", "value": "sk-oai-777",
    }, env=e)
    assert_ok(r, "store with mock env")
    import json

    assert keyring_file.exists(), "mock keyring file not created"
    store = json.loads(keyring_file.read_text())
    assert store["openai"]["api_key"] == "sk-oai-777"
    r = cli("keychain_get", args={"service": "openai", "key": "api_key"}, env=e)
    assert assert_ok(r, "get with mock env")["value"] == "sk-oai-777"
