# ADR-0009 — Craft Catalogs & Storefront Packaging (contract v1.3)

**Status**: Accepted (2026-11-09)
**Supersedes**: none · **Extends**: ADR-0001 (frozen dispatcher), ADR-0002
(bundle/lease), ADR-0005 (offline-first AI seams)

## Context

AUTHR_OS could produce publishable artifacts (EPUB 3 / DOCX / PDF / M4B) but
the craft layer was thin: `books.narrative_structure` allowed exactly four
CHECK-frozen plot arcs, `persona_id` was a free-text column with no catalog
behind it, there was no notion of *project format* (novel vs novella vs
screenplay vs memoir), and nothing turned the export pile into a
storefront-ready package (metadata, description HTML, keywords, categories,
copyright page, royalty projection). A one-person authoring/publishing house
needs all four legs.

## Decision

1. **Static, offline catalogs in code** (`craft/catalog.rs`): 25 project
   formats, 37 bestseller-archetype style personas (craft attributes +
   LLM directive — archetypes, not impersonation), 18 plot structures with
   real beat sheets, KDP pricing constants. Pure data, deterministic,
   unit-tested for id uniqueness and shape. No network, no model, no mock.
2. **Purely additive storage** (`craft_state`, `chapter_beats`): the frozen
   §6.1 DDL and all §5 row shapes stay untouched. Migration follows the
   `rag_chunks` idempotent pattern — and runs **eagerly** from `db::open_db`
   so every bundle has one stable shape (lesson: lazy-only migrations fork
   the schema-reference tests between old and new bundles).
3. **Legacy columns stay truthful**: `books.narrative_structure` has a frozen
   CHECK with 4 values. `craft_apply` syncs it **only** when the chosen arc's
   `legacy_structure` is one of those values; the arc of record lives in
   `craft_state.plot_arc_id`. `persona_id` mirrors `style_persona_id`.
4. **Beat scaffolding is deterministic**: chapter *i* of *n* carries beat
   `⌊i·b/n⌋` — monotonic, total coverage for any n ≥ 1, idempotent re-runs,
   per-chapter act derivation clamped 1–3.
5. **Six additive commands** (`craft_catalog`, `craft_apply`,
   `craft_scaffold`, `craft_progress`, `kdp_check`, `kdp_package`) and one
   additive error code (`CRAFT_ERROR`, category `craft`). Writers gated by
   the lease; `kdp_package` writes only inside the `exports/` sandbox through
   the slug machinery (T-11 hash disambiguation applies).
6. **KDP description HTML is generated, never copied**: every input is
   escaped, then the whitelist structure (paragraphs, `- ` bullets, `*bold*`,
   `_italic_`) is rebuilt — no source tag can reach the storefront. The
   copyright page is an escaped XHTML fragment.
7. **Royalty math is a labeled planning approximation** driven by frozen
   constants; every output carries a "verify on kdp.amazon.com" disclaimer.
   This is a calculator, not a rate feed — no invented live data.
8. **Persona → generation**: `local_llm_generate` gains an additive
   `style_persona` arg; the directive is prepended to the prompt for both
   offline and llama-server backends (prompt-shaped, deterministic).

## Consequences

- Contract v1.2 → v1.3: 56 → 62 commands, 13 → 14 error codes. All previous
  codes/semantics untouched; `--contract` emits the new surface.
- Frontend gains the **Craft Studio** (format/persona/arc pickers, scaffold,
  beat progress, KDP packaging + compliance panel).
- Test surface: +17 Rust unit tests (catalog/engine/kdp), +13 E2E
  (`test_t1_craft.py`, `test_t1_kdp.py`, `test_t4_one_person_publisher.py`),
  schema reference now covers `craft_state`/`chapter_beats` and
  `idx_chapter_beats_beat`.
- Non-goals (this ADR): automated cover-image generation, ISBN acquisition,
  live storefront rate lookup, LLM prose quality claims. The app packages and
  projects; the human authors, approves and files.
