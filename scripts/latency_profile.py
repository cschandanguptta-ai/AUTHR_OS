#!/usr/bin/env python3
"""Per-command end-to-end latency profile for the authr CLI (contract v1.2).

Complements `benchmark_run` (process cold start / RSS / binary size) by
measuring the COMMANDS themselves: a representative authoring workload is
driven against a throwaway bundle — CRUD, bulk AST import, FTS + vector RAG
over the imported corpus, SCRIPTED synthesis, mastering, publishing and
export — and every invocation is wall-clock timed.

Reports min / p50 / p95 / max (ms) per command. Deterministic: fixed text
seeds, fixed iteration counts, no wall-clock content anywhere in the data.

Usage:
  python3 scripts/latency_profile.py                    # table on stdout
  python3 scripts/latency_profile.py --out baseline.json
  python3 scripts/latency_profile.py --out after.json --compare baseline.json
"""

import argparse
import json
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

WORDS = ["lighthouse", "emberlight", "lantern", "harbor", "tide", "wick",
         "keeper", "voyage", "compass", "driftwood", "saltpan", "gunwale"]


def envelope(bin_path: Path, command: str, bundle: str | None, args: dict) -> dict:
    cmd = [str(bin_path), command]
    if bundle is not None:
        cmd += ["--bundle", bundle]
    payload = json.dumps(args)
    stdin_data = None
    if len(payload) > 96 * 1024:
        # Oversized payloads (bulk AST mutations) travel over stdin via the
        # additive --args-stdin transport — argv caps out around 128 KB.
        cmd += ["--args-stdin"]
        stdin_data = payload
    else:
        cmd += ["--args", payload]
    try:
        proc = subprocess.run(cmd, capture_output=True, text=True, input=stdin_data)
    except OSError as e:
        # E2BIG: payload exceeds the OS argv limit (baseline binaries without
        # --args-stdin). Recorded as an errored sample instead of crashing.
        return {"ok": False, "error": {"code": "ARGV_LIMIT", "message": str(e)}}
    for line in reversed((proc.stdout or "").strip().splitlines()):
        line = line.strip()
        if not line:
            continue
        try:
            parsed = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(parsed, dict) and isinstance(parsed.get("ok"), bool):
            return parsed
    return {"ok": False, "error": {"code": "INVALID_ENVELOPE",
                                   "message": (proc.stderr or proc.stdout or "")[:300]}}


def timed(fn, iterations):
    """Run fn() `iterations` times (fn returns (args, bundle)); time each."""
    samples, last_result, last_err = [], None, None
    for _ in range(iterations):
        args, bundle = fn()
        t0 = time.perf_counter()
        res = envelope(BIN, args["__cmd"], bundle,
                       {k: v for k, v in args.items() if k != "__cmd"})
        dt = (time.perf_counter() - t0) * 1000.0
        samples.append(dt)
        if res.get("ok"):
            last_result = res["data"]
        else:
            last_err = res.get("error")
    return samples, last_result, last_err


BIN = None


def pct(samples, p):
    s = sorted(samples)
    idx = max(0, min(len(s) - 1, round(p / 100 * len(s) + 0.5) - 1))
    return s[idx]


def profile(scale: int) -> tuple[dict, dict]:
    rows: dict[str, dict] = {}

    def record(name, samples, last_err):
        rows[name] = {
            "n": len(samples),
            "min_ms": round(min(samples), 3),
            "p50_ms": round(statistics.median(samples), 3),
            "p95_ms": round(pct(samples, 95), 3),
            "max_ms": round(max(samples), 3),
            "error": (last_err or {}).get("code"),
        }

    def step(name, iterations, args_fn):
        samples, data, err = timed(args_fn, iterations)
        record(name, samples, err)
        return samples, data, err

    # ── workload fixtures ────────────────────────────────────────────────────
    tmp_root = Path(tempfile.mkdtemp(prefix="authr_lat_"))

    # project_init on a fresh dir per iteration
    def a_init():
        d = tempfile.mkdtemp(prefix="authr_lat_init_")
        shutil.rmtree(d)
        return {"__cmd": "project_init", "path": d, "title": "Latency Probe",
                "author": "Profiler"}, None
    samples, res, err = step("project_init", 3, a_init)
    bundle = (res or {}).get("path") or ""
    book_id = (res or {}).get("book_id") or 1
    if not bundle or err:
        print(f"FATAL: project_init failed: {err}", file=sys.stderr)
        sys.exit(1)

    def a(cmd, **args):
        return lambda: ({"__cmd": cmd, **args}, bundle)

    # ── books / chapters ─────────────────────────────────────────────────────
    step("book_create", 1, a("book_create", title="Probe Book", author="P",
                             genre="nautical", premise="p", theme="t"))
    chapters = []
    t0 = time.perf_counter()
    for i in range(1, 9):
        r = envelope(BIN, "chapter_create", bundle,
                     {"book_id": book_id, "chapter_id": None,
                      "chapter_number": i, "title": f"Chapter {i}: {WORDS[i % len(WORDS)].title()}",
                      "synopsis": f"Act arc {i}", "target_word_count": 3500})
        if not r.get("ok"):
            # retry without the bogus chapter_id key if rejected
            r = envelope(BIN, "chapter_create", bundle,
                         {"book_id": book_id, "chapter_number": i,
                          "title": f"Chapter {i}: {WORDS[i % len(WORDS)].title()}"})
        if not r.get("ok"):
            print(f"FATAL: chapter_create {i} failed: {r.get('error')}", file=sys.stderr)
            sys.exit(1)
        chapters.append(r["data"]["id"])
    rows["chapter_create"] = {
        "n": 8, "min_ms": None, "p50_ms": round((time.perf_counter() - t0) * 1000 / 8, 3),
        "p95_ms": None, "max_ms": None, "error": None, "note": "mean of 8 sequential creates"}

    # ── segments: 240 appends (30/chapter) + one bulk AST import ─────────────
    ss_samples: list[float] = []
    rh_samples: list[float] = []
    rd_samples: list[float] = []
    t0 = time.perf_counter()
    n_appended = 0
    for ch in chapters:
        for j in range(30):
            text = (f"{WORDS[(ch + j) % len(WORDS)].title()} keeps the {WORDS[j % len(WORDS)]} "
                    f"burning segment {ch}-{j} beyond the {WORDS[(j + 3) % len(WORDS)]}.")
            r = envelope(BIN, "segment_append", bundle,
                         {"chapter_id": ch, "segment_type": "NARRATION",
                          "speaker_id": "NARRATOR", "text_content": text})
            if r.get("ok"):
                n_appended += 1
    per = (time.perf_counter() - t0) * 1000 / max(n_appended, 1)
    rows["segment_append"] = {"n": n_appended, "min_ms": None, "p50_ms": round(per, 3),
                              "p95_ms": None, "max_ms": None, "error": None,
                              "note": "mean of sequential appends"}

    upserts = [{"segment_id": f"bulk-{i:05d}",
                "text_content": f"bulk passage {i} about {WORDS[i % len(WORDS)]} and {WORDS[(i * 7) % len(WORDS)]}",
                "segment_type": "NARRATION", "speaker_id": "NARRATOR",
                "sequence_order": 30 + i}   # after the 30 appended segments
               for i in range(scale)]
    step("sync_lexical_ast_mutation[bulk]", 1,
         a("sync_lexical_ast_mutation", chapter_id=chapters[0], upserts=upserts, deletes=[]))
    small = [{"segment_id": f"bulk-{i:05d}",
              "text_content": f"edited bulk passage {i} — continuity pass",
              "segment_type": "NARRATION", "speaker_id": "NARRATOR",
              "sequence_order": 30 + i}
             for i in range(20)]
    step("sync_lexical_ast_mutation[20]", 5,
         a("sync_lexical_ast_mutation", chapter_id=chapters[0], upserts=small, deletes=[]))

    # ── reads ────────────────────────────────────────────────────────────────
    step("chapter_list", 5, a("chapter_list", book_id=book_id))
    step("segment_list", 5, a("segment_list", chapter_id=chapters[0]))
    for w in WORDS[:10]:
        s, _, e = step("segment_search", 1, a("segment_search", query=w, limit=10))
        ss_samples.extend(s)
    record("segment_search", ss_samples, e)
    rows["segment_search"]["n"] = 10

    # ── voice cast / lore / threads ──────────────────────────────────────────
    step("character_upsert", 12, a("character_upsert", character_key="probe_char",
                                   display_name="Probe", role="SUPPORTING", bio="b",
                                   voice_engine="KOKORO_LOCAL", voice_id="af_bella"))
    step("lore_upsert", 30, a("lore_upsert", key="probe_lore", category="LOCATION",
                              content="the lighthouse above the saltpan"))

    # ── RAG over the imported corpus ─────────────────────────────────────────
    samples, _, err = step("rag_index", 2, a("rag_index"))
    r = envelope(BIN, "rag_status", bundle, {})
    meta_corpus = (r.get("data") or {}).get("chunks")
    for w in WORDS[:10]:
        s, _, e = step("rag_search[hybrid]", 1, a("rag_search", query=w, k=5, mode="hybrid"))
        rh_samples.extend(s)
    record("rag_search[hybrid]", rh_samples, e)
    rows["rag_search[hybrid]"]["n"] = 10
    for w in WORDS[:5]:
        s, _, e = step("rag_search[dense]", 1, a("rag_search", query=w, k=5, mode="dense"))
        rd_samples.extend(s)
    record("rag_search[dense]", rd_samples, e)
    rows["rag_search[dense]"]["n"] = 5
    step("rag_status", 3, a("rag_status"))

    # ── audio studio (SCRIPTED, deterministic) ───────────────────────────────
    step("audio_plan", 3, a("audio_plan", chapter_id=chapters[0]))
    _, res, err = step("audio_synthesize[chapter]", 1,
                       a("audio_synthesize", chapter_id=chapters[0], engine="SCRIPTED",
                         sample_rate=24000))
    wav_rel = ((res or {}).get("results") or [{}])[0].get("path")
    if wav_rel:
        step("audio_master", 1, a("audio_master",
                                  layers=[{"kind": "SPEECH", "wav": wav_rel, "gain_db": -1.5}],
                                  sample_rate=24000, out="probe_master.wav"))
    step("audio_validate", 2, a("audio_validate", wav=wav_rel or "audio/segments/none.wav"))

    # ── publishing / export / maintenance ────────────────────────────────────
    step("publish_captions", 1, a("publish_captions", chapter_id=chapters[0]))
    step("publish_epub", 2, a("publish_epub", book_id=book_id))
    step("publish_docx", 2, a("publish_docx", book_id=book_id))
    # `out` for project_export resolves against the CLI's CWD when relative —
    # always pass an ABSOLUTE path inside the throwaway tmp root so profiling
    # never writes an ~90 MB bundle export into the repo (Phase 7 lesson).
    step("project_export", 2, a("project_export", out=str(tmp_root / "probe_export.zip")))
    step("git_snapshot", 2, a("git_snapshot", message="chore: manual snapshot"))
    step("clean_sweep", 1, a("clean_sweep"))

    meta = {
        "binary": str(BIN),
        "bundle_scale": {"chapters": 8, "appended_segments": n_appended,
                         "bulk_upserts": scale},
        "corpus_total": meta_corpus,
    }
    return rows, meta


def main() -> None:
    global BIN
    ap = argparse.ArgumentParser()
    ap.add_argument("--bin", default=str(REPO / "src-tauri" / "target" / "release" / "authr"))
    ap.add_argument("--out", default=None)
    ap.add_argument("--compare", default=None)
    ap.add_argument("--scale", type=int, default=2000, help="bulk AST upserts (RAG corpus seed)")
    args = ap.parse_args()
    BIN = Path(args.bin)

    rows, meta = profile(args.scale)
    result = {"schema": "authr-latency-profile/1", "meta": meta, "commands": rows}

    print(f"\n{'command':38s} {'n':>4s} {'min':>9s} {'p50':>9s} {'p95':>9s} {'max':>9s}  err")
    print("-" * 92)
    for name, r in sorted(rows.items(), key=lambda kv: kv[1]["p95_ms"] or kv[1]["p50_ms"] or 0, reverse=True):
        def f(x):
            return f"{x:9.2f}" if isinstance(x, float) else f"{x or 0:9.2f}"
        print(f"{name:38s} {r['n']:4d} {f(r['min_ms'])} {f(r['p50_ms'])} "
              f"{f(r['p95_ms'])} {f(r['max_ms'])}  {r['error'] or ''}")

    if args.compare and Path(args.compare).exists():
        base = json.loads(Path(args.compare).read_text())["commands"]
        print(f"\n{'command':38s} {'p50 base':>10s} {'p50 now':>10s} {'p95 base':>10s} {'p95 now':>10s} {'Δp95':>9s}")
        print("-" * 92)
        for name, r in rows.items():
            b = base.get(name)
            if not b:
                continue
            b95 = b.get("p95_ms") or b.get("p50_ms") or 0
            n95 = r.get("p95_ms") or r.get("p50_ms") or 0
            delta = n95 - b95
            flag = "  ◀" if abs(delta) > max(2.0, b95 * 0.25) else ""
            print(f"{name:38s} {b.get('p50_ms') or 0:10.2f} {r.get('p50_ms') or 0:10.2f} "
                  f"{b95:10.2f} {n95:10.2f} {delta:+9.2f}{flag}")

    if args.out:
        Path(args.out).write_text(json.dumps(result, indent=2))
        print(f"\nwrote {args.out}")


if __name__ == "__main__":
    main()
