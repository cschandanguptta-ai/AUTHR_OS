/**
 * Store integration tests (T6.US) — the Zustand domain stores driven
 * through the mock transport (same frozen envelope semantics as the Rust
 * shell), including a full AST-bridge flush over the wire and the
 * additive `segment_reorder` validation contract.
 */

import { beforeEach, describe, expect, it } from 'vitest';
import { getTransport } from '../transport';
import { useSession } from '../stores/sessionStore';
import { useBinder, chaptersOf } from '../stores/binderStore';
import { useVoiceCast, castOrder } from '../stores/voiceCastStore';
import { useLedger, kanban } from '../stores/ledgerStore';
import { useLore } from '../stores/loreStore';
import { useManuscript } from '../stores/manuscriptStore';
import { useUi } from '../stores/uiStore';
import { diffAst, snapshotFromRow } from '../editor/astBridge';
import { snapshotOf } from './helpers';
import type { SegmentRow } from '../types';

async function mountSession(): Promise<void> {
  const t = getTransport();
  const res = await t.invoke('desktop_mount', { root: '/tmp/vitest.authr' });
  if (!res.ok) throw new Error(res.error.message);
  useSession.setState({ mountedRoot: res.data.root, sessionId: res.data.session_id });
  await useBinder.getState().reload();
  const bookId = useBinder.getState().books[0]?.id;
  if (bookId == null) throw new Error('fixture bundle has no books');
  await Promise.all([
    useVoiceCast.getState().reload(bookId),
    useLore.getState().reload(bookId),
    useLedger.getState().reload(bookId),
  ]);
}

beforeEach(async () => {
  // Fresh mock fixture per test (the singleton resets only via reload).
  useSession.setState({
    mountedRoot: null,
    sessionId: null,
    leaseExpiresAt: null,
    lastError: null,
    revision: 0,
  });
  useBinder.setState({ books: [], chapters: [] });
  useVoiceCast.setState({ characters: [] });
  useLore.setState({ lore: [] });
  useLedger.setState({ threads: [] });
  useManuscript.setState({ chapterId: null, segments: [], bridge: { queued: 0, unsaved: 0, lastSyncAt: null, syncing: false, syncError: null, flushes: 0 } });
  useUi.setState({ selection: { bookId: null, chapterId: null, canvasChapterId: null, selectedSegmentId: null, selectedCastId: null, selectedThreadId: null } });
  await mountSession();
});

describe('session + binder stores over the mock bundle', () => {
  it('sees the fixture bundle after mount', () => {
    expect(useSession.getState().mountedRoot).toBe('/tmp/vitest.authr');
    expect(useBinder.getState().books.map((b) => b.title)).toContain('The Lantern Marshes');
    const chs = chaptersOf(useBinder.getState().chapters, useBinder.getState().books[0].id);
    expect(chs.length).toBeGreaterThanOrEqual(3);
  });

  it('creates and deletes books through the frozen envelopes', async () => {
    const created = await useBinder.getState().createBook({ title: 'Night Orchard' });
    expect(created).not.toBeNull();
    expect(useBinder.getState().books.some((b) => b.title === 'Night Orchard')).toBe(true);
    const ok = await useBinder.getState().deleteBook(created!.id);
    expect(ok).toBe(true);
    expect(useBinder.getState().books.some((b) => b.title === 'Night Orchard')).toBe(false);
  });

  it('records envelopes in the console log (Phase 2 parity)', async () => {
    await useBinder.getState().reload();
    const last = useUi.getState().lastEnvelope;
    expect(last).not.toBeNull();
    // binder.reload() walks book_list → chapter_list per book.
    expect(last!.cmd).toBe('chapter_list');
    expect(last!.env.ok).toBe(true);
  });
});

describe('voice cast + lore + ledger stores', () => {
  it('keeps NARRATOR first and parses voice config JSON', () => {
    const ordered = castOrder(useVoiceCast.getState().characters);
    expect(ordered[0].character_key).toBe('NARRATOR');
    const cfg = useVoiceCast.getState().parseConfig(ordered[0]);
    expect(typeof cfg).toBe('object');
  });

  it('casts a voice profile atomically (engine + JSON config)', async () => {
    const mira = useVoiceCast.getState().characters.find((c) => c.character_key === 'MIRA');
    expect(mira).toBeDefined();
    const ok = await useVoiceCast.getState().castVoice(mira!.id, 'elevenlabs', { voice: 'n2_mira', rate: '-5%' });
    expect(ok).toBe(true);
    const after = useVoiceCast.getState().characters.find((c) => c.id === mira!.id)!;
    expect(after.voice_engine).toBe('elevenlabs');
    expect(JSON.parse(after.voice_config!)).toEqual({ voice: 'n2_mira', rate: '-5%' });
  });

  it('moves a Chekhov thread to paid_off (kanban transition)', async () => {
    const thread = useLedger.getState().threads[0];
    const ok = await useLedger.getState().move(thread.id, 'paid_off', 9);
    expect(ok).toBe(true);
    const cols = kanban(useLedger.getState().threads);
    expect(cols.paid_off.some((t) => t.id === thread.id)).toBe(true);
    expect(cols.paid_off.find((t) => t.id === thread.id)?.payoff_chapter_index).toBe(9);
  });
});

describe('AST bridge flush over the wire', () => {
  it('flushes created segments, re-orders the chapter and refreshes rows', async () => {
    const chapter = chaptersOf(useBinder.getState().chapters, useBinder.getState().books[0].id)[0];
    await useManuscript.getState().load(chapter.id);
    const before = useManuscript.getState().segments;
    expect(before.length).toBeGreaterThanOrEqual(2);

    const editorSnap = snapshotOf('editor-uuid-1', 'A brand new segment.', 'MIRA');
    const orderedIds = [before[0].segment_id, 'editor-uuid-1', ...before.slice(1).map((s) => s.segment_id)];
    const payload = {
      created: [editorSnap],
      updated: [],
      deleted: [],
      sequenceOrder: orderedIds,
      empty: false,
    };
    const result = await useManuscript.getState().applyBridgePayload(payload, chapter.id);

    // The DB generated its own segment_id — the remap reports it.
    expect(result.remap).toHaveLength(1);
    expect(result.remap[0].editorId).toBe('editor-uuid-1');
    expect(result.remap[0].segmentId).not.toBe('editor-uuid-1');
    expect(result.remap[0].rowId).toBeGreaterThan(0);

    const after = useManuscript.getState().segments;
    expect(after).toHaveLength(before.length + 1);
    expect(after.map((s) => s.sequence_order)).toEqual(after.map((_, i) => i + 1));
    const inserted = after.find((s: SegmentRow) => s.id === result.remap[0].rowId)!;
    expect(inserted.text_content).toBe('A brand new segment.');
    expect(inserted.speaker_id).toBe('MIRA');
    expect(inserted.segment_type).toBe('DIALOGUE');
    // Dense 1..N re-index landed through segment_reorder.
    expect(after[1].id).toBe(result.remap[0].rowId);
    expect(useManuscript.getState().bridge.flushes).toBe(1);
  });

  it('detects no changes after a flush+reload cycle (baseline stability)', async () => {
    const chapter = chaptersOf(useBinder.getState().chapters, useBinder.getState().books[0].id)[0];
    await useManuscript.getState().load(chapter.id);
    const rows = useManuscript.getState().segments;
    const baseline = new Map(rows.map((r) => [r.segment_id, snapshotFromRow(r)]));
    // Simulate the export right after import — no edits.
    const editorAst = rows.map((r) => ({
      key: 'k' + r.id,
      segmentId: r.segment_id,
      speakerId: r.speaker_id,
      segmentType: r.segment_type,
      emotion: r.emotion,
      prosodyRate: r.prosody_rate,
      prosodyPitch: r.prosody_pitch,
      ambientCue: r.ambient_cue,
      sfxCue: r.sfx_cue,
      text: r.text_content,
      rowId: r.id,
    }));
    expect(diffAst(baseline, editorAst).empty).toBe(true);
  });
});

describe('additive segment_reorder validation contract', () => {
  it('rejects partial lists, duplicates and foreign ids', async () => {
    const chapter = chaptersOf(useBinder.getState().chapters, useBinder.getState().books[0].id)[0];
    await useManuscript.getState().load(chapter.id);
    const rows = useManuscript.getState().segments;
    const t = getTransport();

    const partial = await t.invoke('segment_reorder', {
      chapter_id: chapter.id,
      order: rows.slice(0, 1).map((r) => r.id),
    });
    expect(partial.ok).toBe(false);
    if (!partial.ok) expect(partial.error.code).toBe('VALIDATION_ERROR');

    const dup = await t.invoke('segment_reorder', {
      chapter_id: chapter.id,
      order: [rows[0].id, rows[0].id],
    });
    expect(dup.ok).toBe(false);
    if (!dup.ok) expect(dup.error.message).toContain('duplicate');

    const foreign = await t.invoke('segment_reorder', {
      chapter_id: chapter.id,
      order: [rows[0].id, 999_999],
    });
    expect(foreign.ok).toBe(false);
    if (!foreign.ok) expect(foreign.error.message).toContain('does not belong');
  });

  it('accepts a permutation and persists the new order', async () => {
    const chapter = chaptersOf(useBinder.getState().chapters, useBinder.getState().books[0].id)[0];
    await useManuscript.getState().load(chapter.id);
    const rows = useManuscript.getState().segments;
    if (rows.length < 2) return; // fixture guarantees >= 2 for ch01
    const reversed = [...rows.map((r) => r.id)].reverse();
    const t = getTransport();
    const res = await t.invoke('segment_reorder', { chapter_id: chapter.id, order: reversed });
    expect(res.ok).toBe(true);
    const refreshed = await t.invoke('segment_list', { chapter_id: chapter.id });
    if (!refreshed.ok) throw new Error(refreshed.error.message);
    const ordered = (refreshed.data as SegmentRow[]).sort((a, b) => a.sequence_order - b.sequence_order);
    expect(ordered.map((r) => r.id)).toEqual(reversed);
    expect(ordered.map((r) => r.sequence_order)).toEqual(ordered.map((_, i) => i + 1));
  });
});
