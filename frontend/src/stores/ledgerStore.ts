/**
 * Ledger store — Chekhov's Gun ledger (`unresolved_threads`), presented in
 * the Binder as a kanban board: planted → active → paid_off (abandoned
 * aside). Status transitions go through `thread_update`.
 */

import { create } from 'zustand';
import { getTransport } from '../transport';
import { recordCall } from './uiStore';
import type { ThreadRow } from '../types';

export type ThreadStatus = ThreadRow['status'];
export const THREAD_STATUSES: ThreadStatus[] = ['planted', 'active', 'paid_off', 'abandoned'];

interface LedgerState {
  threads: ThreadRow[];
  loading: boolean;
  error: string | null;
  loadedForBook: number | null;

  reload(bookId: number): Promise<void>;
  plant(fields: { book_id: number; title: string } & Partial<ThreadRow>): Promise<ThreadRow | null>;
  remove(id: number): Promise<boolean>;
  /** Kanban drag & drop / status dropdown. */
  move(id: number, status: ThreadStatus, payoffChapterIndex?: number | null): Promise<boolean>;
}

export const useLedger = create<LedgerState>((set) => ({
  threads: [],
  loading: false,
  error: null,
  loadedForBook: null,

  async reload(bookId) {
    const t = getTransport();
    set({ loading: true, error: null });
    const res = await t.invoke('thread_list', { book_id: bookId });
    recordCall('thread_list', res);
    if (res.ok) set({ threads: res.data as ThreadRow[], loading: false, loadedForBook: bookId });
    else set({ threads: [], loading: false, error: res.error.message });
  },

  async plant(fields) {
    const res = await getTransport().invoke('thread_create', { status: 'planted', ...fields });
    recordCall('thread_create', res);
    if (res.ok) {
      set((s) => ({ threads: [...s.threads, res.data as ThreadRow] }));
      return res.data as ThreadRow;
    }
    set({ error: res.error.message });
    return null;
  },

  async remove(id) {
    const res = await getTransport().invoke('thread_delete', { id });
    recordCall('thread_delete', res);
    if (res.ok) {
      set((s) => ({ threads: s.threads.filter((t) => t.id !== id) }));
      return true;
    }
    set({ error: res.error.message });
    return false;
  },

  async move(id, status, payoffChapterIndex) {
    const patch: Record<string, unknown> = { id, status };
    if (status === 'paid_off') {
      patch.payoff_chapter_index = payoffChapterIndex ?? null;
    }
    const res = await getTransport().invoke('thread_update', patch);
    recordCall('thread_update', res);
    if (res.ok) {
      set((s) => ({
        threads: s.threads.map((t) =>
          t.id === id ? { ...t, status, payoff_chapter_index: (patch.payoff_chapter_index as number | null) ?? t.payoff_chapter_index } : t,
        ),
      }));
      return true;
    }
    set({ error: res.error.message });
    return false;
  },
}));

/** Group threads into kanban columns. */
export function kanban(threads: ThreadRow[]): Record<ThreadStatus, ThreadRow[]> {
  const cols: Record<ThreadStatus, ThreadRow[]> = { planted: [], active: [], paid_off: [], abandoned: [] };
  for (const t of threads) cols[t.status]?.push(t);
  return cols;
}
