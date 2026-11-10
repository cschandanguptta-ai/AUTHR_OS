/**
 * The pure half of the Sprint 7 AST bridge.
 *
 * `diffAst` compares the current Lexical AST export against the last
 * persisted baseline and produces the `AstMutationPayload` (added /
 * mutated / deleted segment UUIDs + the full sequence_order array), and
 * `AstSyncScheduler` implements the 400 ms debounce buffer from the
 * Lexical DOM→SQLite synchronization contract (build plan §6.4).
 *
 * Split / merge / invalidation rules (§6.4):
 *  - Enter split:  the leading paragraph keeps its UUID, the new paragraph
 *    gets a fresh UUID v4 (this module never rewrites UUIDs).
 *  - Backspace merge: the surviving paragraph is the LEADING one, so the
 *    trailing segment id simply disappears from the export -> `deleted`.
 *  - Any text change marks the segment "unrendered" -> the flusher clears
 *    `audio_hash` on updated rows.
 */

import type { SegmentRow } from '../types';

/** One segment as exported from the Lexical editor state. */
export interface AstSnapshot {
  /** Lexical node key (editor-local, not persisted). */
  key: string;
  /** UUID v4 canonical segment id (persisted, stable across splits). */
  segmentId: string;
  speakerId: string;
  segmentType: 'NARRATION' | 'DIALOGUE' | 'INTERNAL_MONOLOGUE';
  emotion: string;
  prosodyRate: number;
  prosodyPitch: number;
  ambientCue: string | null;
  sfxCue: string | null;
  text: string;
  /** Numeric `chapter_segments.id` once the segment has been persisted. */
  rowId: number | null;
}

export interface AstMutationPayload {
  /** Segments present in the AST but unknown to the persisted baseline. */
  created: AstSnapshot[];
  /** Segments whose text or metadata changed since the baseline. */
  updated: AstSnapshot[];
  /** Segment ids removed from the AST since the baseline. */
  deleted: string[];
  /** Full ordered list of segment ids (index i -> sequence_order i+1). */
  sequenceOrder: string[];
  /** True when the payload is a no-op. */
  empty: boolean;
}

export function emptyPayload(): AstMutationPayload {
  return { created: [], updated: [], deleted: [], sequenceOrder: [], empty: true };
}

export function snapshotFromRow(row: SegmentRow): AstSnapshot {
  return {
    key: `row-${row.id}`,
    segmentId: row.segment_id,
    speakerId: row.speaker_id,
    segmentType: row.segment_type,
    emotion: row.emotion,
    prosodyRate: row.prosody_rate,
    prosodyPitch: row.prosody_pitch,
    ambientCue: row.ambient_cue,
    sfxCue: row.sfx_cue,
    text: row.text_content,
    rowId: row.id,
  };
}

/** Row-id lookup for the current chapter (segment_id -> numeric id). */
export function rowIndexMap(rows: SegmentRow[]): Map<string, number> {
  return new Map(rows.map((r) => [r.segment_id, r.id]));
}

export function diffAst(
  baseline: Map<string, AstSnapshot>,
  current: AstSnapshot[],
): AstMutationPayload {
  const created: AstSnapshot[] = [];
  const updated: AstSnapshot[] = [];
  const seen = new Set<string>();

  for (const seg of current) {
    seen.add(seg.segmentId);
    const before = baseline.get(seg.segmentId);
    if (!before) {
      created.push(seg);
      continue;
    }
    if (
      before.text !== seg.text ||
      before.speakerId !== seg.speakerId ||
      before.segmentType !== seg.segmentType ||
      before.emotion !== seg.emotion ||
      before.prosodyRate !== seg.prosodyRate ||
      before.prosodyPitch !== seg.prosodyPitch ||
      before.ambientCue !== seg.ambientCue ||
      before.sfxCue !== seg.sfxCue
    ) {
      updated.push(seg);
    }
  }

  const deleted: string[] = [];
  for (const id of baseline.keys()) {
    if (!seen.has(id)) deleted.push(id);
  }

  return {
    created,
    updated,
    deleted,
    sequenceOrder: current.map((s) => s.segmentId),
    empty: created.length === 0 && updated.length === 0 && deleted.length === 0,
  };
}

export type AstFlusher = (payload: AstMutationPayload) => Promise<FlushResult | void>;

/** What the flusher reports back so the baseline can re-key itself. */
export interface FlushResult {
  /** editor-side UUID -> persisted (segment_id, row id) replacements. */
  remap: { editorId: string; segmentId: string; rowId: number }[];
}

/**
 * 400 ms debounce buffer. Every editor transaction calls `push()` with the
 * fresh AST export; the flusher runs once the stream goes quiet (and is
 * also callable immediately via `flushNow()` for chapter switches and
 * unmount). `markPersisted` advances the baseline so the next diff is
 * minimal.
 */
export class AstSyncScheduler {
  private baseline: Map<string, AstSnapshot>;
  private timer: ReturnType<typeof setTimeout> | null = null;
  private pending: AstSnapshot[] | null = null;
  private flushing = false;
  private readonly debounceMs: number;
  private readonly flusher: AstFlusher;
  private readonly onError: (e: unknown) => void;

  constructor(
    initialRows: SegmentRow[],
    flusher: AstFlusher,
    debounceMs = 400,
    onError: (e: unknown) => void = () => undefined,
  ) {
    this.baseline = new Map(initialRows.map((r) => [r.segment_id, snapshotFromRow(r)]));
    this.flusher = flusher;
    this.debounceMs = debounceMs;
    this.onError = onError;
  }

  /** Last persisted snapshot id list (test/debug surface). */
  persistedIds(): string[] {
    return [...this.baseline.keys()];
  }

  /** How many segments carry edits not yet flushed. */
  pendingCount(): number {
    if (!this.pending) return 0;
    const d = diffAst(this.baseline, this.pending);
    return d.created.length + d.updated.length + d.deleted.length;
  }

  push(current: AstSnapshot[]): void {
    this.pending = current;
    if (this.timer) clearTimeout(this.timer);
    this.timer = setTimeout(() => {
      void this.flushNow().catch(this.onError);
    }, this.debounceMs);
  }

  /** Cancel the debounce window without flushing. */
  cancel(): void {
    if (this.timer) clearTimeout(this.timer);
    this.timer = null;
    this.pending = null;
  }

  async flushNow(): Promise<AstMutationPayload> {
    if (this.timer) {
      clearTimeout(this.timer);
      this.timer = null;
    }
    if (!this.pending || this.flushing) return emptyPayload();
    const current = this.pending;
    this.pending = null;
    const payload = diffAst(this.baseline, current);
    if (payload.empty) return payload;
    this.flushing = true;
    try {
      const result = await this.flusher(payload);
      // Advance the baseline to exactly what is now persisted. Created
      // segments re-key to the DB-generated segment_id + row id.
      const remap = new Map((result?.remap ?? []).map((r) => [r.editorId, r]));
      this.baseline = new Map();
      for (const snap of current) {
        const r = remap.get(snap.segmentId);
        if (r) {
          this.baseline.set(r.segmentId, { ...snap, segmentId: r.segmentId, rowId: r.rowId });
        } else {
          this.baseline.set(snap.segmentId, snap);
        }
      }
    } finally {
      this.flushing = false;
    }
    return payload;
  }
}

/**
 * `AstMutationPayload` -> concrete IPC call list. Pure so tests can pin the
 * exact wire behavior against `docs/CLI_CONTRACT.md`.
 */
export type SegmentIpcCall =
  | { cmd: 'segment_create'; payload: Record<string, unknown> }
  | { cmd: 'segment_update'; payload: Record<string, unknown> }
  | { cmd: 'segment_delete'; payload: Record<string, unknown> }
  | { cmd: 'segment_reorder'; payload: Record<string, unknown> };

/**
 * The create/update/delete wave of a mutation payload. The whole-chapter
 * `segment_reorder` call is NOT included: created rows have no numeric id
 * until the create envelopes return, so the executor appends it via
 * `buildReorderCall` once every id is known.
 */
export function payloadToCalls(
  payload: AstMutationPayload,
  chapterId: number,
  rowIds: Map<string, number>,
): SegmentIpcCall[] {
  const calls: SegmentIpcCall[] = [];
  for (const seg of payload.created) {
    calls.push({
      cmd: 'segment_create',
      payload: {
        chapter_id: chapterId,
        segment_id: seg.segmentId,
        speaker_id: seg.speakerId,
        segment_type: seg.segmentType,
        emotion: seg.emotion,
        prosody_rate: seg.prosodyRate,
        prosody_pitch: seg.prosodyPitch,
        ambient_cue: seg.ambientCue,
        sfx_cue: seg.sfxCue,
        text_content: seg.text,
        sequence_order: payload.sequenceOrder.indexOf(seg.segmentId) + 1,
      },
    });
  }
  for (const seg of payload.updated) {
    const rowId = seg.rowId ?? rowIds.get(seg.segmentId);
    if (rowId == null) continue;
    calls.push({
      cmd: 'segment_update',
      payload: {
        id: rowId,
        speaker_id: seg.speakerId,
        segment_type: seg.segmentType,
        emotion: seg.emotion,
        prosody_rate: seg.prosodyRate,
        prosody_pitch: seg.prosodyPitch,
        ambient_cue: seg.ambientCue,
        sfx_cue: seg.sfxCue,
        text_content: seg.text,
        // Audio invalidation: any content change unsets the cache pointer.
        audio_hash: null,
      },
    });
  }
  for (const segmentId of payload.deleted) {
    const rowId = rowIds.get(segmentId);
    if (rowId == null) continue;
    calls.push({ cmd: 'segment_delete', payload: { id: rowId } });
  }
  return calls;
}

/**
 * Whole-chapter dense re-index (1-based). `idBySegmentId` must resolve
 * every id in `sequenceOrder` to its persisted numeric row id — created
 * ids come from the create envelopes, the rest from the loaded rows.
 */
export function buildReorderCall(
  chapterId: number,
  sequenceOrder: string[],
  idBySegmentId: Map<string, number>,
): SegmentIpcCall | null {
  const order: number[] = [];
  for (const segmentId of sequenceOrder) {
    const rowId = idBySegmentId.get(segmentId);
    if (rowId == null) return null;
    order.push(rowId);
  }
  return { cmd: 'segment_reorder', payload: { chapter_id: chapterId, order } };
}

/** Read-Along karaoke timing estimate (words @ wpm; real timings when cached). */
export interface SegmentTiming {
  segmentId: string;
  startMs: number;
  durationMs: number;
}

export function computeTimings(segments: SegmentRow[], wpm = 150): SegmentTiming[] {
  const out: SegmentTiming[] = [];
  let cursor = 0;
  for (const seg of segments) {
    const words = seg.text_content.trim().split(/\s+/).filter(Boolean).length;
    const estimated = (words / wpm) * 60_000;
    const durationMs = seg.audio_duration_ms && seg.audio_duration_ms > 0 ? seg.audio_duration_ms : Math.round(estimated);
    out.push({ segmentId: seg.segment_id, startMs: cursor, durationMs });
    cursor += durationMs;
  }
  return out;
}
