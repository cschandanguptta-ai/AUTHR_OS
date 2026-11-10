// AUTHR_OS — Chekhov's Gun ledger store (unresolved_threads).

import { create } from 'zustand';
import * as api from '../ipc/api';
import type { ThreadRow } from '../types/ast';
import { toast, toastError } from './uiStore';
import { useProjectStore } from './projectStore';
import { useManuscriptStore } from './manuscriptStore';

interface ThreadState {
  threads: ThreadRow[];
  loaded: boolean;
  loadThreads: () => Promise<void>;
  openThread: (args: api.ThreadOpenArgs) => Promise<ThreadRow | null>;
  resolveThread: (id: number, resolvedInChapter?: number) => Promise<boolean>;
  reset: () => void;
}

export const useThreadStore = create<ThreadState>((set) => ({
  threads: [],
  loaded: false,

  loadThreads: async () => {
    try {
      const { threads } = await api.threadList();
      set({ threads, loaded: true });
    } catch (err) {
      toastError('Failed to load Chekhov ledger', err);
    }
  },

  openThread: async (args) => {
    if (useProjectStore.getState().readonly) {
      toast('warn', 'Read-only bundle', 'Foreign lease active — the ledger is frozen.');
      return null;
    }
    try {
      const row = await api.threadOpen(args);
      set((s) => ({ threads: [...s.threads, row] }));
      toast('success', 'Thread introduced', row.thread_title);
      return row;
    } catch (err) {
      toastError('Could not open thread', err);
      return null;
    }
  },

  resolveThread: async (id, resolvedInChapter) => {
    if (useProjectStore.getState().readonly) {
      toast('warn', 'Read-only bundle', 'Foreign lease active — the ledger is frozen.');
      return false;
    }
    const chapterId =
      resolvedInChapter ?? useManuscriptStore.getState().activeChapterId ?? undefined;
    if (!chapterId) {
      toast('warn', 'No chapter selected', 'Open a chapter to resolve threads against it.');
      return false;
    }
    try {
      const row = await api.threadResolve({ id, resolved_in_chapter: chapterId });
      set((s) => ({ threads: s.threads.map((t) => (t.id === id ? row : t)) }));
      toast('success', 'Thread paid off', `${row.thread_title} → resolved`);
      return true;
    } catch (err) {
      toastError('Could not resolve thread', err);
      return false;
    }
  },

  reset: () => set({ threads: [], loaded: false }),
}));
