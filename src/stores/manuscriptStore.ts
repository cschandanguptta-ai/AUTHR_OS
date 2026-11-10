// AUTHR_OS — manuscript store: books, chapters, segments (NarrativeSegmentAst[]),
// and the optimistic save + reconcile path for sync_lexical_ast_mutation.

import { create } from 'zustand';
import * as api from '../ipc/api';
import { rowToAst } from '../types/ast';
import type {
  AstMutationPayload,
  BookRow,
  ChapterRow,
  ChapterStatus,
  NarrativeSegmentAst,
} from '../types/ast';
import { toast, toastError } from './uiStore';
import { useProjectStore } from './projectStore';

export interface SaveMutationRequest {
  chapter_id: number;
  upserts: NarrativeSegmentAst[];
  deletes: string[];
}

export interface SaveMutationResult {
  ok: boolean;
  rows: NarrativeSegmentAst[];
}

interface ManuscriptState {
  books: BookRow[];
  chapters: ChapterRow[];
  activeBookId: number | null;
  activeChapterId: number | null;
  segments: NarrativeSegmentAst[];
  selectedSegmentId: string | null;
  dirty: boolean;
  saving: boolean;
  loadingSegments: boolean;
  loadedBooks: boolean;
  loadedChapters: boolean;
  liveWordCount: number | null;

  loadBooks: () => Promise<void>;
  loadChapters: (bookId?: number) => Promise<void>;
  loadSegments: (chapterId: number) => Promise<NarrativeSegmentAst[]>;
  selectChapter: (chapterId: number | null) => void;
  selectSegment: (segmentId: string | null) => void;
  createChapter: (args: {
    book_id: number;
    chapter_number: number;
    title: string;
    act_number?: number;
    synopsis?: string;
    target_word_count?: number;
    status?: string;
  }) => Promise<ChapterRow | null>;
  updateChapter: (
    id: number,
    patch: Partial<Pick<ChapterRow, 'title' | 'status' | 'synopsis' | 'target_word_count' | 'act_number'>>,
  ) => Promise<void>;
  deleteChapter: (id: number) => Promise<void>;
  setSegments: (rows: NarrativeSegmentAst[]) => void;
  updateSegment: (segmentId: string, patch: Partial<NarrativeSegmentAst>) => void;
  reorderSegments: (rows: NarrativeSegmentAst[]) => void;
  saveMutation: (req: SaveMutationRequest) => Promise<SaveMutationResult>;
  appendSegments: (rows: NarrativeSegmentAst[]) => void;
  setLiveWordCount: (n: number | null) => void;
  reset: () => void;
}

function assertWritable(action: string): boolean {
  const { readonly } = useProjectStore.getState();
  if (readonly) {
    toast('warn', 'Read-only bundle', `Foreign lease active — ${action} is disabled.`);
    return false;
  }
  return true;
}

export const useManuscriptStore = create<ManuscriptState>((set, get) => ({
  books: [],
  chapters: [],
  activeBookId: null,
  activeChapterId: null,
  segments: [],
  selectedSegmentId: null,
  dirty: false,
  saving: false,
  loadingSegments: false,
  loadedBooks: false,
  loadedChapters: false,
  liveWordCount: null,

  loadBooks: async () => {
    try {
      const { books } = await api.bookList();
      set({ books, loadedBooks: true });
    } catch (err) {
      toastError('Failed to load books', err);
    }
  },

  loadChapters: async () => {
    try {
      const { chapters } = await api.chapterList();
      set({ chapters, loadedChapters: true });
    } catch (err) {
      toastError('Failed to load chapters', err);
    }
  },

  loadSegments: async (chapterId) => {
    set({ loadingSegments: true, activeChapterId: chapterId, selectedSegmentId: null });
    try {
      const { segments } = await api.segmentList(chapterId);
      const rows = segments.map(rowToAst);
      set({ segments: rows, loadingSegments: false, liveWordCount: null });
      return rows;
    } catch (err) {
      set({ loadingSegments: false });
      toastError('Failed to load segments', err);
      return [];
    }
  },

  selectChapter: (chapterId) => {
    const chapter = get().chapters.find((c) => c.id === chapterId);
    set({
      activeChapterId: chapterId,
      activeBookId: chapter ? chapter.book_id : get().activeBookId,
      selectedSegmentId: null,
      segments: [],
      liveWordCount: null,
    });
  },

  selectSegment: (segmentId) => {
    if (get().selectedSegmentId !== segmentId) set({ selectedSegmentId: segmentId });
  },

  createChapter: async (args) => {
    if (!assertWritable('chapter creation')) return null;
    try {
      const row = await api.chapterCreate(args);
      await get().loadChapters();
      get().selectChapter(row.id);
      toast('success', `Chapter ${row.chapter_number} created`, row.title);
      return row;
    } catch (err) {
      toastError('Could not create chapter', err);
      return null;
    }
  },

  updateChapter: async (id, patch) => {
    if (!assertWritable('chapter update')) return;
    try {
      const row = await api.chapterUpdate({ id, ...patch });
      set((s) => ({
        chapters: s.chapters.map((c) => (c.id === id ? row : c)),
      }));
    } catch (err) {
      toastError('Could not update chapter', err);
    }
  },

  deleteChapter: async (id) => {
    if (!assertWritable('chapter deletion')) return;
    try {
      await api.chapterDelete(id);
      await get().loadChapters();
      if (get().activeChapterId === id) {
        const first = get().chapters[0] ?? null;
        get().selectChapter(first ? first.id : null);
      }
      toast('success', 'Chapter deleted', 'Segments, threads and shadow files cascaded.');
    } catch (err) {
      toastError('Could not delete chapter', err);
    }
  },

  setSegments: (rows) => set({ segments: rows }),

  updateSegment: (segmentId, patch) => {
    set((s) => ({
      dirty: true,
      segments: s.segments.map((seg) =>
        seg.segment_id === segmentId ? { ...seg, ...patch } : seg,
      ),
    }));
  },

  reorderSegments: (rows) => {
    set({ dirty: true, segments: rows });
  },

  appendSegments: (rows) => {
    set((s) => ({ dirty: true, segments: [...s.segments, ...rows] }));
  },

  /**
   * Optimistic save: the editor already applied its state; we push the mutation
   * in one BEGIN IMMEDIATE transaction, then reconcile from segment_list.
   */
  saveMutation: async (req) => {
    const chapterId = req.chapter_id;
    if (!assertWritable('editing')) {
      return { ok: false, rows: get().segments };
    }
    set({ saving: true });
    try {
      const payload: AstMutationPayload = {
        chapter_id: chapterId,
        upserts: req.upserts,
        deletes: req.deletes,
      };
      await api.syncLexicalAstMutation(payload);
      // Reconcile from the server (audio_hash invalidation, re-indexed order…).
      const { segments } = await api.segmentList(chapterId);
      const rows = segments.map(rowToAst);
      // Only swap in rows if we are still on the same chapter.
      if (get().activeChapterId === chapterId) {
        set({ segments: rows, dirty: false, saving: false, liveWordCount: null });
      } else {
        set({ saving: false });
      }
      // Quietly refresh chapter word counts.
      void get().loadChapters();
      return { ok: true, rows };
    } catch (err) {
      set({ saving: false, dirty: true });
      toastError('Manuscript sync failed', err);
      return { ok: false, rows: get().segments };
    }
  },

  setLiveWordCount: (n) => set({ liveWordCount: n }),

  reset: () =>
    set({
      books: [],
      chapters: [],
      activeBookId: null,
      activeChapterId: null,
      segments: [],
      selectedSegmentId: null,
      dirty: false,
      saving: false,
      loadingSegments: false,
      loadedBooks: false,
      loadedChapters: false,
      liveWordCount: null,
    }),
}));

/** Chapter status changes go through chapter_update (contract §4.3 #14). */
export async function setChapterStatus(id: number, status: ChapterStatus) {
  await useManuscriptStore.getState().updateChapter(id, { status });
}
