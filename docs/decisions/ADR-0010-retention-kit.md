# ADR-0010 — Retention & Serialization Kit (contract v1.4)

**Status**: Accepted (2026-11-09)
**Supersedes**: none · **Extends**: ADR-0009 (craft catalogs), ADR-0001
(frozen dispatcher), ADR-0002 (bundle/lease)

## Context

ADR-0009 gave every book a *macro* spine: 18 plot arcs whose beat sheets are
scaffolded across the whole manuscript. What it did not give the one-person
publisher was the *chapter-level* machinery that serial platforms (Kindle Vella,
Royal Road, Wattpad, KU page-through economics) actually monetize on: reader
retention. A book can hit every macro beat and still bleed readers chapter by
chapter. The "Serialized Fiction Blueprint 2.0" (SFB-2.0) — a practitioner
framework for chapter pacing and serial drafting — supplies exactly that
layer: the seven-beat chapter model, a six-type chapter-ending taxonomy with
selection rules, a 10–15 chapter micro-arc cycle, a 7×0–2 editorial QC rubric
with a zero-flag rule, an A–E drafting worksheet, opening-hook benchmarks and
serialization cadence/buffer guidance (2–3 chapters/week on a 4–8 chapter
reserve).

## Decision

1. **One static module** (`craft/retention.rs`): the whole blueprint lives in
   code as deterministic data + pure scoring functions — same pattern as the
   v1.3 catalogs, zero network, zero deps, unit-tested kernels
   (`score_chapter`, `phase_for`, `suggest_endings`, `blueprint_json`).
2. **Blueprint rides the existing catalog surface**: `craft_catalog
   kind=retention` (bundle-free) — no new catalog command.
3. **Three new commands (§4.15)**: `craft_retention_check` (writer: QC a
   drafted chapter, persist verdict, steer the next chapter),
   `craft_retention_report` (reader: per-chapter QC, ending rotation, phase
   map, word-length conformance vs the 1000–2000 serial range, cadence
   arithmetic) and `craft_retention_worksheet` (reader: A–E worksheet fused
   with book state — works for chapters that do not exist yet).
4. **Additive storage only**: `chapter_retention(book_id, chapter_number, …)`
   keyed `(book_id, chapter_number)`, eager idempotent migration, no frozen
   DDL touched, no §5 row shape changed, **no new error code** (reuses
   `CRAFT_ERROR`).
5. **The zero-flag rule is law in code**: a 0 on `narrative_objective` or
   `chapter_payoff` sets `revision_required` even at a 12–14 total — the
   blueprint's "clarity and payoff are non-negotiable" is enforced, not
   advisory.
6. **Cadence truth comes from the chapter pipeline**: `status` (PLANNED /
   DRAFTING / POLISHED / AUDIO_READY / PUBLISHED) drives `reserve =
   DRAFTING+POLISHED+AUDIO_READY`, `buffer_weeks = reserve /
   chapters_per_week`, `buffer_healthy = reserve ≥ 4`. No invented
   "published" flag, no schema change.
7. **Ending rotation is data-driven**: suggestions exclude the types used in
   the last three QC'd chapters (blueprint rule 3.2.1), and the report flags
   repetitions inside that window.
8. **Frontend**: the Craft Studio gains a Retention QC panel (cadence strip,
   worksheet loader, 7-criterion scorer with zero-flag marks, ending picker
   seeded from the rotation suggestions). All knowledge still arrives from
   the offline catalog — nothing calls the network.

## Consequences

- 62 → 65 commands; harness, docs and `refresh_acceptance.py` updated
  together (195 E2E, 100 Rust units).
- Serial-platform authors get a quantified pre-publication gate inside the
  same lease/sandbox envelope as every other writer command.
- The QC rubric remains an editorial self-assessment tool (the blueprint's
  own caveat), not a scientifically validated retention score — the UI says
  so nowhere loudly, the docs here say so once.
- Future hooks (not built): per-beat presence scoring against chapter text,
  platform-specific length targets, publication calendar projection.
