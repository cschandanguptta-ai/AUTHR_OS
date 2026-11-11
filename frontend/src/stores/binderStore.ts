/**
 * Binder store — books + chapters (the Manuscript tree) with a shared
 * "loaded bundle revision" marker so all domain stores reload together
 * after mount / unmount / external git events.
 */

import { create } from 'zustand';
import { getTransport } from '../transport';
import { recordCall } from './uiStore';
import type { BookRow, ChapterRow } from '../types';

interface BinderState {
  books: BookRow[];
  chapters: ChapterRow[];
  loading: boolean;
  error: string | null;
  /** Bumped by every successful mutation; components key reloads off it. */
  dataVersion: number;

  reload(): Promise<void>;
  createBook(fields: Partial<BookRow> & { title: string }): Promise<BookRow | null>;
  updateBook(id: number, patch: Partial<BookRow>): Promise<boolean>;
  deleteBook(id: number): Promise<boolean>;
  createChapter(bookId: number, fields: Partial<ChapterRow> & { title: string }): Promise<ChapterRow | null>;
  updateChapter(id: number, patch: Partial<ChapterRow>): Promise<boolean>;
  deleteChapter(id: number): Promise<boolean>;
}

export const useBinder = create<BinderState>((set, get) => ({
  books: [],
  chapters: [],
  loading: false,
  error: null,
  dataVersion: 0,

  async reload() {
    const t = getTransport();
    set({ loading: true, error: null });
    const b = await t.invoke('book_list');
    recordCall('book_list', b);
    if (!b.ok) {
      // Not mounted (or read-only session) — clear to an empty binder.
      set({ books: [], chapters: [], loading: false, error: b.ok ? null : b.error.message });
      return;
    }
    const books = b.data as BookRow[];
    const chapters: ChapterRow[] = [];
    for (const book of books) {
      const c = await t.invoke('chapter_list', { book_id: book.id });
      recordCall('chapter_list', c);
      if (c.ok) chapters.push(...(c.data as ChapterRow[]));
    }
    set({ books, chapters, loading: false, dataVersion: get().dataVersion + 1 });
  },

  async createBook(fields) {
    const res = await getTransport().invoke('book_create', fields);
    recordCall('book_create', res);
    if (res.ok) {
      await get().reload();
      return res.data as BookRow;
    }
    set({ error: res.error.message });
    return null;
  },

  async updateBook(id, patch) {
    const res = await getTransport().invoke('book_update', { id, ...patch });
    recordCall('book_update', res);
    if (res.ok) {
      await get().reload();
      return true;
    }
    set({ error: res.error.message });
    return false;
  },

  async deleteBook(id) {
    const res = await getTransport().invoke('book_delete', { id });
    recordCall('book_delete', res);
    if (res.ok) {
      await get().reload();
      return true;
    }
    set({ error: res.error.message });
    return false;
  },

  async createChapter(bookId, fields) {
    const res = await getTransport().invoke('chapter_create', { book_id: bookId, ...fields });
    recordCall('chapter_create', res);
    if (res.ok) {
      await get().reload();
      return res.data as ChapterRow;
    }
    set({ error: res.error.message });
    return null;
  },

  async updateChapter(id, patch) {
    const res = await getTransport().invoke('chapter_update', { id, ...patch });
    recordCall('chapter_update', res);
    if (res.ok) {
      await get().reload();
      return true;
    }
    set({ error: res.error.message });
    return false;
  },

  async deleteChapter(id) {
    const res = await getTransport().invoke('chapter_delete', { id });
    recordCall('chapter_delete', res);
    if (res.ok) {
      await get().reload();
      return true;
    }
    set({ error: res.error.message });
    return false;
  },
}));

/** Chapters of one book ordered by chapter_index. */
export function chaptersOf(chapters: ChapterRow[], bookId: number | null): ChapterRow[] {
  return chapters.filter((c) => c.book_id === bookId).sort((a, b) => a.chapter_index - b.chapter_index);
}
