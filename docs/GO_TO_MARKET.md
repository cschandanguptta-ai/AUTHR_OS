# GO_TO_MARKET.md — One-Person Authoring & Publishing House Playbook

**The question**: *Can one person use AUTHR_OS to write, produce and sell
books — and is the output actually sellable on Amazon KDP and similar
storefronts?*

**The answer**: Yes. Everything between "idea" and "upload-ready package" is
implemented, offline-first and test-verified end to end
(`tests/tier4_scenarios/test_t4_one_person_publisher.py`). The steps that
remain manual are the ones that legally or commercially require a human:
creating the storefront account, uploading, ticking the marketplace's own
checkboxes, and marketing spend.

---

## 1. What the pipeline covers (all offline, zero keys)

| Stage | Tooling | Contract surface |
|---|---|---|
| Plan shape | 25 project formats (novel → picture book → screenplay → memoir → self-help) | `craft_catalog kind=formats`, `craft_apply` |
| Structure | 18 plot arcs with real beat sheets (Save the Cat, Hero's Journey, Kishōtenketsu, Story Circle, Seven-Point, Romance beats, Mystery architecture, non-fiction Problem→Solution, Memoir arc, Dual Timeline…) | `craft_scaffold` (maps beats to chapters), `craft_progress` |
| Voice | 37 bestseller-archetype style personas with POV/tense/rhythm/pacing + LLM directives | `craft_apply` + `local_llm_generate {style_persona}` |
| Write | Lexical editor, AST segments, Chekhov thread ledger, FTS search | §4.3–4.8 |
| Continuity | Hybrid RAG (BM25 + dense) over lore/cast/manuscript | §4.12 |
| Audio | Scripted offline synthesis, per-character voices, ACX mastering + validation | §4.11 |
| Export | EPUB 3 (media overlays), DOCX, print PDF (Typst), M4B audiobook, LRC/VTT captions | §4.13 |
| Package | KDP metadata.json, KDP-safe description.html, copyright page, launch checklist, royalty projection | §4.14 |

## 2. Amazon KDP: what "ready to sell" means here

`kdp_package` produces `exports/kdp/<slug>/`:

1. **metadata.json** — title/subtitle/author/language/date/ISBN/DRM, up to 7
   keywords (≤50 chars each, enforced), 1–2 categories, description — the
   exact fields KDP's upload forms ask for, in copy-paste shape.
2. **description.html** — KDP-allowed HTML subset only, rebuilt by a
   whitelist sanitizer (input is escaped, structure is generated — no tag
   from your source can smuggle through).
3. **copyright-page.xhtml** — escaped front-matter fragment: © line, ISBN,
   edition, publisher, optional AI-assistance disclosure.
4. **checklist.md** — the launch runbook: eBook upload, paperback print
   pricing (est. pages, print cost, minimum list price), wide-distribution
   channels, ACX audiobook steps.

`kdp_check` scores readiness 0–100 across manuscript/EPUB/cover/metadata/
print/audio/polish and reports every gap with a fix. It never lies: missing
artifacts are `FAIL`s, not warnings.

**Royalty model (offline planning approximation — always re-verify on
kdp.amazon.com):**

- eBook: 70% tier for list $2.99–$9.99 minus delivery ($0.15/MB); 35% tier
  elsewhere (no delivery fee).
- Paperback: 60% minus print cost (US B&W model: $0.85 + $0.012/page);
  minimum list price = print cost ÷ 0.60.
- Hardcover: 60% minus ($5.65 + $0.0122/page).
- Audiobook: via ACX/Audible — 40% exclusive / 25% non-exclusive share.

## 3. Other storefronts

The same package re-targets: **Apple Books**, **Kobo Writing Life**,
**Google Play Books**, **Draft2Digital** (which also reaches Baker & Taylor,
OverDrive/Libby, Tolino). Use `metadata.json` + `description.html`; keep
pricing parity. ACX covers Audible; findaway voices-style aggregators are a
manual step.

## 4. What stays manual (by design, not by gap)

- **Cover art**: generate from `books.cover_prompt` in any image tool; KDP
  wants ~2560×1600 JPG (ratio 1.6:1). The bundle stores the path via
  `book_update`.
- **KDP account, tax interview, upload**: your account, your signatures.
- **ISBN choice**: KDP free ISBN vs own block — set it in `kdp_package`.
- **Marketing spend**: ads, newsletter swaps, launch teams.

## 5. Money math sanity example (from the shipped constants)

A 50,000-word novel → ~200 print pages. Print cost ≈ $3.25 → paperback
minimum list ≈ $5.42; at $9.99 the paperback royalty ≈ $2.74/copy. The same
book as a 2 MB eBook at $4.99: 70% tier → ≈ $3.40/copy after delivery. At
100 copies/month mixed format that is roughly $300–400/month from one title —
the whole point of the one-person house model: **catalog depth × evergreen
royalties**, which the craft catalogs (formats × arcs × personas) are built
to accelerate. (Illustrative arithmetic from `catalog::pricing()` constants,
not a promise.)

## 6. Reader retention & the serial revenue loop (v1.4)

Storefronts pay on **page-through**: Kindle Unlimited royalties are computed
from pages readers actually finish, and serial platforms retain or churn
chapter by chapter. v1.4 therefore ships the Serialized Fiction Blueprint
2.0 (SFB-2.0) as executable craft machinery (`craft_catalog kind=retention`):

- **Seven-beat chapter model** — opening continuity → objective → resistance
  → escalation → turning point → payoff → forward pull; the with-chapter
  skeleton every KU page is spent inside.
- **Six ending types with rotation control** — danger, emotional revelation,
  mystery/discovery, irreversible choice, reversal, anticipation;
  `craft_retention_worksheet` blocks whatever the last three chapters used,
  so cliffhanger fatigue never accumulates mechanically.
- **Micro-arc cycle (10–15 chapters × 4 phases)** — the retention unit
  between "chapter" and "book"; phase maps wrap every 15 chapters.
- **QC gate before publication** — `craft_retention_check` scores 7 criteria
  ×0–2 (max 14) with editorial bands (12–14 strong … 0–5 reconsider); a zero
  on *narrative objective* or *meaningful payoff* forces revision regardless
  of the total. Only chapters that pass this gate should reach `kdp_package`.
- **Serialization cadence** — 2–3 chapters/week on a 4–8 chapter reserve;
  `craft_retention_report` derives the buffer directly from the chapter
  pipeline (`DRAFTING/POLISHED/AUDIO_READY`) and converts it to weeks of
  schedule protection.
- **Word-length conformance** — chapters are checked against the 1000–2000
  serial range (platform reading format) separately from the project target.

Workflow: scaffold the macro arc (§4.14) → draft against the A–E worksheet →
run the QC gate → keep the buffer ≥ 4 → publish on cadence → package with
`kdp_package`. Retention is treated as an editorial self-assessment tool
(the blueprint's own caveat), not a guarantee — but it is now enforced,
logged and queryable inside the bundle instead of living in the author's
head.

## 7. The video-novel product line (v1.5)

NovelCast Studio's method — a voiced, illustrated, timed video per chapter —
now ships inside the same offline runtime as the audio pipeline (ADR-0011),
which opens a third storefront lane beside the ebook and the audiobook:
faceless "video novel" channels (YouTube/TikTok serialized fiction, Patreon
early-access screenings) monetize the same manuscript a third time.

- **One derivation, three media**: the draft-time hook
  (`video::derive::refresh_chapter_media`, wired into `segment_append` and
  `sync_lexical_ast_mutation`) computes scenes, durations, cast mentions and
  narration paths once per change — audio, image and video stages consume the
  same rows, so the LLM (or the author) never re-analyzes a chapter to plan
  another medium.
- **Consistency ladder**: every principal character owns an R2I-style
  reference sheet (book-level, 1024×1024); scene frames embed the appearance
  bio verbatim on every tier (multi-ref → text-only degradation), so faces
  and palettes stay stable across a chapter — the #1 reported failure of
  naive text-to-scene pipelines.
- **SCRIPTED engine is the default**: fully offline, deterministic,
  zero-dependency (BMP + embedded 5×7 font + seeded palettes) — the video
  pipeline works on a plane. `AUTHR_IMG_ENDPOINT` upgrades sheets/scenes to a
  hosted image model when one is configured, and fails closed when it is not.
- **Narration reuse**: `video_assemble` auto-concatenates the per-scene audio
  the TTS pipeline already produced (`auto:pipeline-audio`) — the audiobook
  and the video novel share one voice track; silent track only when no
  synthesis has run.
- **Workflow**: draft (derivation happens on save) → `video_plan` (ladder +
  missing sheets) → `video_render` (sheets first, then scenes) →
  `video_assemble` (MP4 with Ken Burns focal anchors + subtitles) →
  `video_validate` (plan-vs-file gate, 1.5 s tolerance) → upload per the
  platform's specs.
