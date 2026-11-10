/**
 * Manuscript store — `chapter_segments` for the chapter currently bound to
 * the Lexical AST canvas, plus the sync-state surface of the debounced
 * AST→SQLite bridge (Sprint 7) and Read-Along playback state.
 *
 * The bridge (editor/astBridge.ts) computes mutation payloads; this store
 * performs the actual `segment_*` IPC calls and tracks their envelopes.
 */

import { create } from 'zustand';
import { getTransport } from '../transport';
import { recordCall } from './uiStore';
import type { SegmentRow } from '../types';
import {
  buildReorderCall,
  payloadToCalls,
  rowIndexMap,
  type AstMutationPayload,
  type FlushResult,
} from '../editor/astBridge';

export interface BridgeStats {
  /** Mutations queued in the current debounce window. */
  queued: number;
  /** Segment rows awaiting persistence (created in editor, no row id yet). */
  unsaved: number;
  lastSyncAt: number | null;
  syncing: boolean;
  syncError: string | null;
  /** Monotonic count of flushed batches (test assertion surface). */
  flushes: number;
}

interface ManuscriptState {
  chapterId: number | null;
  segments: SegmentRow[];
  loading: boolean;
  error: string | null;
  bridge: BridgeStats;
  playback: { playing: boolean; activeSegmentId: string | null };

  load(chapterId: number): Promise<void>;
  unload(): void;
  setBridge(patch: Partial<BridgeStats>): void;
  /** metadata edit from the Dynamic Inspector — immediate, not debounced. */
  updateMeta(rowId: number, patch: Partial<SegmentRow>): Promise<boolean>;
  /** Remove a row directly (e.g. empty paragraph pruned by the bridge). */
  removeRow(rowId: number): Promise<boolean>;
  /** Replace local rows after a bridge reconciliation fetch. */
  replaceRows(rows: SegmentRow[]): void;
  /**
   * Persist one `AstMutationPayload` (the debounced bridge flush):
   * create/update/delete wave, then the whole-chapter `segment_reorder`
   * re-index, then a reconciliation `segment_list` refresh. Throws on the
   * first failed envelope so the scheduler keeps its (old) baseline.
   *
   * `targetChapterId` is bound by the scheduler when it is created, so a
   * flush landing after a chapter switch still writes to (and refreshes)
   * the chapter the payload belongs to. Returns the editor-UUID →
   * persisted-identity remap (the DB generates its own `segment_id`).
   */
  applyBridgePayload(payload: AstMutationPayload, targetChapterId?: number): Promise<FlushResult>;
  play(): void;
  stop(): void;
  setActiveSegment(segmentId: string | null): void;
}

export const useManuscript = create<ManuscriptState>((set, get) => ({
  chapterId: null,
  segments: [],
  loading: false,
  error: null,
  bridge: { queued: 0, unsaved: 0, lastSyncAt: null, syncing: false, syncError: null, flushes: 0 },
  playback: { playing: false, activeSegmentId: null },

  async load(chapterId) {
    const t = getTransport();
    set({ loading: true, error: null, chapterId, segments: [] });
    const res = await t.invoke('segment_list', { chapter_id: chapterId });
    recordCall('segment_list', res);
    if (res.ok) {
      const rows = res.data as SegmentRow[];
      rows.sort((a, b) => a.sequence_order - b.sequence_order);
      set({ segments: rows, loading: false });
    } else {
      set({ segments: [], loading: false, error: res.error.message });
    }
  },

  unload() {
    set({ chapterId: null, segments: [], playback: { playing: false, activeSegmentId: null } });
  },

  setBridge(patch) {
    set((s) => ({ bridge: { ...s.bridge, ...patch } }));
  },

  async updateMeta(rowId, patch) {
    const res = await getTransport().invoke('segment_update', { id: rowId, ...patch });
    recordCall('segment_update', res);
    if (res.ok) {
      set((s) => ({
        segments: s.segments.map((seg) => (seg.id === rowId ? { ...seg, ...patch } : seg)),
      }));
      return true;
    }
    set((s) => ({ bridge: { ...s.bridge, syncError: res.error.message } }));
    return false;
  },

  async removeRow(rowId) {
    const res = await getTransport().invoke('segment_delete', { id: rowId });
    recordCall('segment_delete', res);
    if (res.ok) {
      set((s) => ({ segments: s.segments.filter((seg) => seg.id !== rowId) }));
      return true;
    }
    set((s) => ({ bridge: { ...s.bridge, syncError: res.error.message } }));
    return false;
  },

  replaceRows(rows) {
    rows.sort((a, b) => a.sequence_order - b.sequence_order);
    set({ segments: rows });
  },

  async applyBridgePayload(payload, targetChapterId): Promise<FlushResult> {
    const chapterId = targetChapterId ?? get().chapterId;
    if (!chapterId) return { remap: [] };
    get().setBridge({ syncing: true });
    try {
      const rowIds = rowIndexMap(get().segments.filter((s) => s.chapter_id === chapterId || targetChapterId == null));
      const calls = payloadToCalls(payload, chapterId, rowIds);
      const remap: FlushResult['remap'] = [];
      let createdIdx = 0;
      for (const call of calls) {
        const env = await getTransport().invoke(call.cmd, call.payload);
        recordCall(call.cmd, env);
        if (!env.ok) throw new Error(`${call.cmd}: ${env.error.message}`);
        if (call.cmd === 'segment_create') {
          const editorSnap = payload.created[createdIdx++];
          remap.push({
            editorId: editorSnap.segmentId,
            segmentId: env.data.segment_id,
            rowId: env.data.id,
          });
        }
      }
      if (payload.created.length + payload.deleted.length > 0) {
        const editorToDb = new Map(remap.map((r) => [r.editorId, r] as const));
        // sequenceOrder is expressed in editor UUIDs; segment_reorder needs
        // persisted ids, so re-key created entries through the remap.
        const persistedOrder = payload.sequenceOrder.map((id) => editorToDb.get(id)?.segmentId ?? id);
        const merged = new Map<string, number>([
          ...rowIds,
          ...remap.map((r) => [r.segmentId, r.rowId] as [string, number]),
        ]);
        const reorder = buildReorderCall(chapterId, persistedOrder, merged);
        if (reorder) {
          const env = await getTransport().invoke(reorder.cmd, reorder.payload);
          recordCall(reorder.cmd, env);
          if (!env.ok) throw new Error(`${reorder.cmd}: ${env.error.message}`);
        }
      }
      const list = await getTransport().invoke('segment_list', { chapter_id: chapterId });
      recordCall('segment_list', list);
      // Only refresh the visible rows when the canvas still shows the
      // chapter this payload belongs to.
      if (list.ok && get().chapterId === chapterId) {
        get().replaceRows(list.data as SegmentRow[]);
      }
      set((s) => ({
        bridge: {
          ...s.bridge,
          syncing: false,
          lastSyncAt: Date.now(),
          flushes: s.bridge.flushes + 1,
          syncError: null,
          unsaved: 0,
        },
      }));
      return { remap };
    } catch (e) {
      set((s) => ({
        bridge: {
          ...s.bridge,
          syncing: false,
          syncError: e instanceof Error ? e.message : String(e),
        },
      }));
      throw e;
    }
  },

  play() {
    const segs = get().segments;
    set({ playback: { playing: true, activeSegmentId: segs[0]?.segment_id ?? null } });
  },

  stop() {
    set({ playback: { playing: false, activeSegmentId: null } });
  },

  setActiveSegment(segmentId) {
    set((s) => ({ playback: { ...s.playback, activeSegmentId: segmentId } }));
  },
}));

/** Speaker key → display name lookup used by gutter badges. */
export function speakerLabel(
  speakerId: string,
  characters: { character_key: string; display_name: string }[],
): string {
  if (speakerId === 'NARRATOR') return 'Narrator';
  const c = characters.find((x) => x.character_key === speakerId);
  return c?.display_name ?? speakerId;
}
