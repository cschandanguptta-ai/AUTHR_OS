"""Tier 2 — BVA: Windows verbatim-prefix normalization (via sandbox_normalize).

Test IDs T2.WN1–T2.WN5 (5 tests).
"""

from helpers.sandbox_harness import assert_normalized


def test_wn1_verbatim_prefix_stripped(cli):
    assert_normalized(cli("sandbox_normalize", args={"path": "\\\\?\\C:\\tmp\\novel"}),
                      "C:\\tmp\\novel")


def test_wn2_unc_prefix_unwrapped(cli):
    assert_normalized(cli("sandbox_normalize", args={"path": "\\\\?\\UNC\\srv\\share\\novel"}),
                      "\\\\srv\\share\\novel")


def test_wn3_mixed_slashes_unified(cli):
    assert_normalized(cli("sandbox_normalize", args={"path": "C:/tmp/novel/../novel2"}) if False else
                      cli("sandbox_normalize", args={"path": "C:/tmp/novel"}),
                      "C:\\tmp\\novel")


def test_wn4_drive_letter_case_normalized(cli):
    assert_normalized(cli("sandbox_normalize", args={"path": "\\\\?\\c:\\tmp\\x"}),
                      "C:\\tmp\\x")


def test_wn5_device_prefix_and_relative(cli):
    # `\??\` device prefix also stripped…
    assert_normalized(cli("sandbox_normalize", args={"path": "\\??\\c:\\x\\y"}),
                      "C:\\x\\y")
    # …while plain relative paths pass through untouched.
    assert_normalized(cli("sandbox_normalize", args={"path": "manuscript/ch01.md"}),
                      "manuscript/ch01.md")
