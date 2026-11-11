# ADR-0003 — Shadow layer: manuscript mirrors + git auto-commit

**Status**: Accepted (Phase 1/3; `test_t1_shadow_markdown.py`,
`test_t1_git_integration.py`)

**Context**: The authoritative story text lives in SQLite rows
(`chapter_segments`), but authors need human-readable files, diffable history,
and external-tool access — without making files the sync source of truth.

**Decision**: Every write path re-renders `manuscript/chNN.md` (YAML
frontmatter + formatted body) and `manuscript/chNN.ast.json` (full AST array)
via `shadow.rs::render_chapter:55` — atomic tmp+rename per file, contract §7.
Chapter deletes remove the pair (`shadow.rs::remove_chapter:144`). After
render, `storage/git.rs::auto_sync_commit:80` commits `Auto-sync chNN` with
the fixed signature `AUTHR_OS <authr@local>` (`git.rs:9-10`), skipping clean
trees (`git.rs:54-61`). Ordering is fixed: manifest touch → shadow → git
(`ipc/mod.rs:43-49`; invariant `invariants-and-gotchas.md §1.2`).

**Alternatives**: files-as-source-of-truth with DB as cache — not implemented;
no-git revisioning — `git2` vendored dependency already present.

**Consequences**: the bundle is readable in any markdown viewer and diffable;
DB stays authoritative; editor round-trip (`sync_lexical_ast_mutation`) is the
only writer, so manual file edits are overwritten on next sync (INFERRED from
render-on-write behavior).

**Do not change unless**: the contract §7 format changes with a version bump
and `shadow_markdown` tests are updated — downstream consumers (publishers,
git history readers) depend on the exact shape.

**Evidence**: `src-tauri/src/shadow.rs`, `src-tauri/src/storage/git.rs`,
`docs/CLI_CONTRACT.md §7-8`.
