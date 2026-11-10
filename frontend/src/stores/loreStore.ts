/**
 * Lore store — the Living Story Bible (`lorebook`). Writes go through the
 * frozen `lore_*` IPC commands; the pronunciation dictionary shares the
 * `ipa_phonetic` column with the Audio Studio.
 */

import { create } from 'zustand';
import { getTransport } from '../transport';
import { recordCall } from './uiStore';
import type { LoreRow } from '../types';

interface LoreState {
  lore: LoreRow[];
  loading: boolean;
  error: string | null;
  loadedForBook: number | null;

  reload(bookId: number): Promise<void>;
  create(fields: { book_id: number; term: string } & Partial<LoreRow>): Promise<LoreRow | null>;
  update(id: number, patch: Partial<LoreRow>): Promise<boolean>;
  remove(id: number): Promise<boolean>;
}

export const useLore = create<LoreState>((set) => ({
  lore: [],
  loading: false,
  error: null,
  loadedForBook: null,

  async reload(bookId) {
    const t = getTransport();
    set({ loading: true, error: null });
    const res = await t.invoke('lore_list', { book_id: bookId });
    recordCall('lore_list', res);
    if (res.ok) set({ lore: res.data as LoreRow[], loading: false, loadedForBook: bookId });
    else set({ lore: [], loading: false, error: res.error.message });
  },

  async create(fields) {
    const res = await getTransport().invoke('lore_create', fields);
    recordCall('lore_create', res);
    if (res.ok) {
      set((s) => ({ lore: [...s.lore, res.data as LoreRow] }));
      return res.data as LoreRow;
    }
    set({ error: res.error.message });
    return null;
  },

  async update(id, patch) {
    const res = await getTransport().invoke('lore_update', { id, ...patch });
    recordCall('lore_update', res);
    if (res.ok) {
      set((s) => ({ lore: s.lore.map((l) => (l.id === id ? { ...l, ...patch } : l)) }));
      return true;
    }
    set({ error: res.error.message });
    return false;
  },

  async remove(id) {
    const res = await getTransport().invoke('lore_delete', { id });
    recordCall('lore_delete', res);
    if (res.ok) {
      set((s) => ({ lore: s.lore.filter((l) => l.id !== id) }));
      return true;
    }
    set({ error: res.error.message });
    return false;
  },
}));
