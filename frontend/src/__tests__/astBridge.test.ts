/**
 * AST bridge unit tests (T6.UB) — diff rules, payload serialization and
 * the 400 ms debounce, pinned against the §6.4 contract.
 */

import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import {
  AstSyncScheduler,
  buildReorderCall,
  computeTimings,
  diffAst,
  emptyPayload,
  payloadToCalls,
  rowIndexMap,
  snapshotFromRow,
  type AstSnapshot,
} from '../editor/astBridge';
import type { SegmentRow } from '../types';

const row = (id: number, segmentId: string, text: string, order: number): SegmentRow => ({
  id,
  segment_id: segmentId,
  chapter_id: 1,
  speaker_id: 'NARRATOR',
  segment_type: 'NARRATION',
  text_content: text,
  emotion: 'neutral',
  prosody_rate: 1,
  prosody_pitch: 1,
  ambient_cue: null,
  sfx_cue: null,
  audio_hash: 'hash-' + id,
  sequence_order: order,
});

const snap = (segmentId: string, text: string, extra: Partial<AstSnapshot> = {}): AstSnapshot => ({
  key: 'k-' + segmentId,
  segmentId,
  speakerId: 'NARRATOR',
  segmentType: 'NARRATION',
  emotion: 'neutral',
  prosodyRate: 1,
  prosodyPitch: 1,
  ambientCue: null,
  sfxCue: null,
  text,
  rowId: null,
  ...extra,
});

describe('diffAst', () => {
  const baseline = new Map([
    ['a', snapshotFromRow(row(1, 'a', 'alpha', 1))],
    ['b', snapshotFromRow(row(2, 'b', 'bravo', 2))],
  ]);

  it('detects created segments', () => {
    const d = diffAst(baseline, [snap('a', 'alpha'), snap('b', 'bravo'), snap('c', 'charlie')]);
    expect(d.created.map((s) => s.segmentId)).toEqual(['c']);
    expect(d.updated).toHaveLength(0);
    expect(d.deleted).toHaveLength(0);
    expect(d.sequenceOrder).toEqual(['a', 'b', 'c']);
    expect(d.empty).toBe(false);
  });

  it('detects text mutations', () => {
    const d = diffAst(baseline, [snap('a', 'alpha edited'), snap('b', 'bravo')]);
    expect(d.updated.map((s) => s.segmentId)).toEqual(['a']);
  });

  it('detects metadata mutations (speaker / emotion / cues)', () => {
    const d = diffAst(baseline, [snap('a', 'alpha', { speakerId: 'MIRA' }), snap('b', 'bravo')]);
    expect(d.updated.map((s) => s.segmentId)).toEqual(['a']);
    const d2 = diffAst(baseline, [snap('a', 'alpha'), snap('b', 'bravo', { sfxCue: 'door_creak' })]);
    expect(d2.updated.map((s) => s.segmentId)).toEqual(['b']);
  });

  it('detects deletions', () => {
    const d = diffAst(baseline, [snap('a', 'alpha')]);
    expect(d.deleted).toEqual(['b']);
    expect(d.sequenceOrder).toEqual(['a']);
  });

  it('returns empty payload for no-op', () => {
    const d = diffAst(baseline, [snap('a', 'alpha'), snap('b', 'bravo')]);
    expect(d.empty).toBe(true);
  });
});

describe('payloadToCalls (frozen wire format)', () => {
  it('emits segment_create with sequence_order = list position + 1', () => {
    const p = {
      ...emptyPayload(),
      created: [snap('n1', 'new text')],
      sequenceOrder: ['a', 'n1'],
    };
    const calls = payloadToCalls(p, 7, rowIndexMap([row(1, 'a', 'alpha', 1)]));
    expect(calls).toHaveLength(1);
    expect(calls[0].cmd).toBe('segment_create');
    expect(calls[0].payload).toMatchObject({
      chapter_id: 7,
      segment_id: 'n1',
      text_content: 'new text',
      sequence_order: 2,
    });
  });

  it('emits segment_update with numeric row id and audio_hash invalidation', () => {
    const p = {
      ...emptyPayload(),
      updated: [snap('a', 'alpha v2', { rowId: 42 })],
      sequenceOrder: ['a'],
    };
    const calls = payloadToCalls(p, 7, new Map());
    expect(calls[0]).toEqual({
      cmd: 'segment_update',
      payload: expect.objectContaining({ id: 42, text_content: 'alpha v2', audio_hash: null }),
    });
  });

  it('emits segment_delete keyed by numeric id', () => {
    const p = { ...emptyPayload(), deleted: ['b'], sequenceOrder: ['a'] };
    const calls = payloadToCalls(p, 7, rowIndexMap([row(2, 'b', 'bravo', 2)]));
    expect(calls[0].cmd).toBe('segment_delete');
    expect(calls[0].payload).toEqual({ id: 2 });
  });

  it('skips updates for unknown row ids (still waiting for create)', () => {
    const p = { ...emptyPayload(), updated: [snap('ghost', 'x')], sequenceOrder: ['ghost'] };
    expect(payloadToCalls(p, 7, new Map())).toHaveLength(0);
  });
});

describe('buildReorderCall', () => {
  it('maps the full ordered segment-id list onto numeric row ids', () => {
    const ids = new Map<string, number>([
      ['a', 11],
      ['b', 22],
      ['c', 33],
    ]);
    const call = buildReorderCall(7, ['c', 'a', 'b'], ids);
    expect(call).toEqual({ cmd: 'segment_reorder', payload: { chapter_id: 7, order: [33, 11, 22] } });
  });

  it('returns null when any id is unresolved (defensive)', () => {
    expect(buildReorderCall(7, ['a', 'zzz'], new Map([['a', 1]]))).toBeNull();
  });
});

describe('AstSyncScheduler (400 ms debounce)', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it('debounces bursts into one flush', async () => {
    const flusher = vi.fn(async (_payload: any) => ({ remap: [] }));
    const sched = new AstSyncScheduler([row(1, 'a', 'alpha', 1)], flusher);
    sched.push([snap('a', 'alpha one')]);
    sched.push([snap('a', 'alpha one two')]); // inside the window
    sched.push([snap('a', 'alpha one two three')]);
    expect(flusher).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(400);
    expect(flusher).toHaveBeenCalledTimes(1);
    const [payload] = flusher.mock.calls[0];
    expect(payload.updated.map((s: AstSnapshot) => s.text)).toEqual(['alpha one two three']);
    // Baseline advanced: a second identical push produces an empty diff —
    // the flusher is never invoked for no-ops.
    sched.push([snap('a', 'alpha one two three')]);
    await vi.advanceTimersByTimeAsync(400);
    expect(flusher).toHaveBeenCalledTimes(1);
  });

  it('re-keys created segments after the flusher returns the remap', async () => {
    const flusher = vi.fn(async (_payload: any) => ({
      remap: [{ editorId: 'editor-uuid', segmentId: 'db-uuid', rowId: 99 }],
    }));
    const sched = new AstSyncScheduler([], flusher);
    sched.push([snap('editor-uuid', 'created text')]);
    await vi.advanceTimersByTimeAsync(400);
    expect(sched.persistedIds()).toEqual(['db-uuid']);
    // After the flush the canvas re-keys the node to the DB UUID (that is
    // what setSegmentId does), so the next export carries 'db-uuid'; a
    // follow-up edit therefore diffs as an UPDATE of row 99, not a create.
    sched.push([snap('db-uuid', 'created text!', { rowId: 99 })]);
    await vi.advanceTimersByTimeAsync(400);
    const [p2] = flusher.mock.calls[1];
    expect(p2.created).toHaveLength(0);
    expect(p2.updated.map((s: AstSnapshot) => s.segmentId)).toEqual(['db-uuid']);
    expect(p2.updated[0].rowId).toBe(99);
  });

  it('flushNow flushes immediately and cancels the timer', async () => {
    const flusher = vi.fn(async (_payload: any) => ({ remap: [] }));
    const sched = new AstSyncScheduler([], flusher);
    sched.push([snap('x', 'hello')]);
    await sched.flushNow();
    expect(flusher).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(1000);
    expect(flusher).toHaveBeenCalledTimes(1);
  });

  it('keeps the old baseline when the flusher throws', async () => {
    const flusher = vi.fn(async (_payload: any) => {
      throw new Error('segment_create: BUNDLE_ERROR');
    });
    const sched = new AstSyncScheduler([], flusher, 400, () => undefined);
    sched.push([snap('x', 'hello')]);
    await expect(sched.flushNow()).rejects.toThrow('BUNDLE_ERROR');
    expect(sched.persistedIds()).toEqual([]);
    // The next push still carries the unflushed edit.
    expect(sched.pendingCount()).toBe(0); // pending was consumed; next push re-diffs
    sched.push([snap('x', 'hello')]);
    expect(sched.pendingCount()).toBe(1);
  });
});

describe('computeTimings (Read-Along)', () => {
  it('uses cached audio durations when present', () => {
    const rows = [
      { ...row(1, 'a', 'one two three four', 1), audio_duration_ms: 7000 },
      { ...row(2, 'b', 'one two', 2), audio_duration_ms: 0 },
    ];
    const t = computeTimings(rows, 120);
    expect(t[0]).toEqual({ segmentId: 'a', startMs: 0, durationMs: 7000 });
    expect(t[1].startMs).toBe(7000);
    expect(t[1].durationMs).toBe(Math.round((2 / 120) * 60000));
  });
});
