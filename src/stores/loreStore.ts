// AUTHR_OS — lorebook store.

import { create } from 'zustand';
import * as api from '../ipc/api';
import type { LoreRow } from '../types/ast';
import { toast, toastError } from './uiStore';
import { useProjectStore } from './projectStore';

interface LoreState {
  lore: LoreRow[];
  loaded: boolean;
  loadLore: () => Promise<void>;
  upsertLore: (args: {
    key: string;
    content: string;
    category?: string;
    ipa_phonetic?: string;
  }) => Promise<LoreRow | null>;
  patchLore: (
    key: string,
    patch: Partial<Pick<LoreRow, 'content' | 'category' | 'ipa_phonetic'>>,
  ) => Promise<void>;
  deleteLore: (key: string) => Promise<boolean>;
  reset: () => void;
}

export const useLoreStore = create<LoreState>((set, get) => ({
  lore: [],
  loaded: false,

  loadLore: async () => {
    try {
      const { lore } = await api.loreList();
      set({ lore, loaded: true });
    } catch (err) {
      toastError('Failed to load lorebook', err);
    }
  },

  upsertLore: async (args) => {
    if (useProjectStore.getState().readonly) {
      toast('warn', 'Read-only bundle', 'Foreign lease active — the lorebook is frozen.');
      return null;
    }
    try {
      const row = await api.loreUpsert(args);
      set((s) => {
        const exists = s.lore.some((l) => l.key === row.key);
        return {
          lore: exists ? s.lore.map((l) => (l.key === row.key ? row : l)) : [...s.lore, row],
        };
      });
      return row;
    } catch (err) {
      toastError('lore_upsert failed', err);
      return null;
    }
  },

  patchLore: async (key, patch) => {
    const existing = get().lore.find((l) => l.key === key);
    if (!existing) return;
    await get().upsertLore({
      key: existing.key,
      content: patch.content ?? existing.content,
      category: patch.category ?? existing.category,
      ipa_phonetic: patch.ipa_phonetic ?? existing.ipa_phonetic,
    });
  },

  deleteLore: async (key) => {
    if (useProjectStore.getState().readonly) {
      toast('warn', 'Read-only bundle', 'Foreign lease active — the lorebook is frozen.');
      return false;
    }
    try {
      await api.loreDelete(key);
      set((s) => ({ lore: s.lore.filter((l) => l.key !== key) }));
      toast('success', 'Lore entry removed', key);
      return true;
    } catch (err) {
      toastError('lore_delete failed', err);
      return false;
    }
  },

  reset: () => set({ lore: [], loaded: false }),
}));
