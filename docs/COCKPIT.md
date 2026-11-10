# The Author's Cockpit — Phase 3 (Sprints 6–8)

Phase 3 turns the Phase 2 desktop shell into the **Author's Cockpit**: a
3-pane workstation with a Project Binder, a triple-view Lexical AST canvas
bound to `chapter_segments` through the debounced DOM→SQLite bridge, the
Dynamic Inspector, and the four restored studios. The storage core (Phase 1)
and the frozen IPC envelope contract are untouched; the only additive IPC
command is `segment_reorder` (documented in `DESKTOP_SHELL.md`).

```
┌──────────────────────────────────────────────────────────────────────┐
│  Top bar (session)  ·  Workstation switcher                          │
│  Cockpit | Audio Studio | Video Studio | Localization | Marketing    │
├──────────────────┬──────────────────────────────┬────────────────────┤
│  Project Binder  │  Triple-View Lexical Canvas  │ Dynamic Inspector  │
│  · Manuscript    │  · Clean Literary            │ · Segment (AST)    │
│  · Story Bible   │  · Script & Cast             │ · Cast / Ledger    │
│  · Voice Cast    │  · Read-Along (karaoke)      │ · Envelope/Events  │
│  · Chekhov Kanban│  400 ms AST bridge ▶ SQLite  │  resizable panes   │
└──────────────────┴──────────────────────────────┴────────────────────┘
```

## Sprint 6 — 3-Pane Workstation & Project Binder

* **Layout** (`src/cockpit/Cockpit.tsx`): pointer-drag resizers between the
  panes; fractions persist in `localStorage` (`authr.cockpit.panes`);
  double-click a divider resets to 20 / 56 / 24.
* **Project Binder** (`ProjectBinder.tsx`): tree of Manuscript (books →
  chapters, click opens the canvas), Story Bible (lorebook), Voice Cast
  Rack (characters) and the Chekhov Kanban (threads grouped by status).
  Creating books / chapters writes straight through `book_create` /
  `chapter_create`.
* **Dynamic Inspector** (`InspectorPane.tsx`): context tabs — Segment
  (speaker / type / emotion / prosody / cues of the segment under the
  caret), Cast, Ledger, plus the Phase 2 Envelope and Events tabs. Segment
  metadata edits persist immediately via `segment_update`.
* **State** (`src/stores/`): Zustand domain stores — `sessionStore`
  (mount/lease + event pump), `uiStore` (workstation, view mode, pane
  geometry, shared event/envelope log), `binderStore`, `voiceCastStore`,
  `loreStore`, `ledgerStore`, `manuscriptStore` (segments + bridge stats).
  All loads are book-scoped: `character_list`, `lore_list`, `thread_list`
  require `book_id` in the frozen contract, so the stores reload for the
  active book (explicit selection, else the first).

## Sprint 7 — Triple-View Lexical AST Canvas

* **Custom nodes** (`src/editor/nodes.tsx`) — Speaker Gutter Badges,
  Emotion Chips and Foley triggers ride along on every segment:

| Node | Lexical kind | Persisted counterpart |
| --- | --- | --- |
| `SegmentNode` | paragraph subclass | one `chapter_segments` row (UUID, speaker, type, emotion, prosody, cues, row id) |
| `SpeakerBadgeNode` | inline decorator | `speaker_id` (display from the cast) |
| `EmotionChipNode` | inline decorator | `emotion` |
| `FoleyTriggerNode` | inline decorator | `ambient_cue` (∞) / `sfx_cue` (⚡) |

  Decorators render pure DOM and return `''` from `getTextContent`, so
  metadata chrome never leaks into `text_content`.

* **Triple view** (`viewMode` in `uiStore`, CSS class on the canvas):
  * `clean` — Clean Literary: all chrome hidden, prose only;
  * `script` — Script & Cast: badges, chips and Foley triggers visible;
  * `readalong` — Read-Along: editor read-only, karaoke ticker walks the
    segments using cached `audio_duration_ms` when present, else a 150 wpm
    estimate.

* **Split / merge rules** (`segmentOps.ts`, §6.4):
  * **Enter** splits at the caret: the leading paragraph keeps its UUID
    (and its audio-cache identity); the new paragraph gets a fresh UUID v4
    and inherits speaker / type / emotion.
  * **Backspace at offset 0** merges into the LEADING segment: only prose
    text moves, the trailing decorators are discarded, and the trailing
    UUID disappears from the export → the bridge deletes its row.

* **The debounced AST bridge** (`astBridge.ts` + `manuscriptStore`):
  1. Every editor transaction exports the AST (`$exportAst`).
  2. `AstSyncScheduler` diffs against the last persisted baseline and
     buffers for **400 ms** of quiet.
  3. The flush persists the `AstMutationPayload` over the frozen surface:
     `segment_create` (with `sequence_order`), `segment_update`
     (`audio_hash: null` — audio invalidation), `segment_delete`, then the
     additive `segment_reorder` re-indexes the whole chapter 1..N in one
     transaction, then a reconciliation `segment_list` refreshes the rows.
  4. `segment_create` returns the DB-generated UUID; the bridge remaps the
     editor node (`setSegmentId`) and re-keys the baseline, so the next
     diff is an `update`, never a duplicate `create`.
  5. A failed envelope keeps the old baseline and surfaces `syncError` in
     the canvas toolbar; the next edit retries implicitly.

## Sprint 8 — Restored Studio Workstations

All five studios live behind the top workstation switcher; the Phase 2
console remains available as the sixth destination. Full names: **Audio
Studio**, **Video Studio**, **Localization Studio** and **Marketing Studio**
(plus the Cockpit itself).

| Studio | Live now (persists through the IPC surface) | Pending phases |
| --- | --- | --- |
| **Audio** | voice profiling + casting (`character_update` with engine + JSON config), pronunciation dictionary IPA (`character_update` / `lore_update`), Foley/ambient cue assignment per segment (`segment_update`), 3-layer DAW census + frozen ACX/EBU mastering targets | synthesis engines, DSP rack (Phase 4) |
| **Video** | slide pacing timeline from real segment timings, per-chapter pacing multipliers, copyable FFmpeg render plan | MP4 render pipeline (Phase 6) |
| **Localization** | YouTube/plain-text script import → parsed → `segment_create` into a chosen chapter; language catalog | neural translation, memory sync (Phase 5) |
| **Marketing** | genre / status / target word count (`book_update`), deterministic keyword + Midjourney cover prompt generators, social hooks from the Chekhov ledger | publishing fields (Phase 6) |

Pending-phase controls are explicitly badged (`Phase 4` / `Phase 5` /
`Phase 6`) rather than silently disabled.

## Testing

* **Vitest unit suite** (`frontend/src/__tests__/`, run via
  `npm --prefix frontend run test`):
  * `astBridge.test.ts` — diff rules, wire-format serialization, 400 ms
    debounce with fake timers, baseline re-keying, Read-Along timings;
  * `editorHeadless.test.ts` — `@lexical/headless` round-trips and the
    §6.4 split / merge UUID rules in a DOM-less editor;
  * `stores.test.ts` — stores driven through the mock transport with the
    frozen envelope semantics, including a full bridge flush over the wire
    and the `segment_reorder` validation contract;
  * `studios.test.ts` — deterministic studio logic.
* **Tier 6 pytest** (`tests/tier6_cockpit/`): dist markers for every
  cockpit/studio surface, dependency surface (Lexical + Zustand + React
  19), a live CLI smoke of `segment_reorder` against the real binary,
  the vitest run itself, and documentation binding.

```bash
python tests/run_e2e.py --tier=6      # cockpit tier only
python tests/run_e2e.py               # full 6-tier matrix
```

## Browser demo

`npm --prefix frontend run dev` still works without the Rust shell: the
mock transport implements the full 38-command surface (now including
`segment_reorder`), so every workstation — including the AST canvas and
the studios — is explorable in a plain browser.
