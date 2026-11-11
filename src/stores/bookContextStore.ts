// AUTHR_OS — global book context (R-13): one header chip, every book-scoped
// panel follows it. Craft, KDP packaging, marketing premise and the publish
// card all read activeBookId instead of defaulting to books[0] independently.

import { create } from 'zustand';

interface BookContextState {
  activeBookId: number | null;
  setActiveBookId: (id: number | null) => void;
  reset: () => void;
}

export const useBookContextStore = create<BookContextState>((set) => ({
  activeBookId: null,
  setActiveBookId: (activeBookId) => set({ activeBookId }),
  reset: () => set({ activeBookId: null }),
}));

/**
 * Resolve the book a book-scoped panel should operate on: the globally picked
 * book when it still exists in the loaded list, else the first book (the
 * single-book default), else null. Callers pass the books they already have.
 */
export function resolveActiveBook<T extends { id: number }>(
  books: T[],
  activeBookId: number | null,
): T | null {
  if (books.length === 0) return null;
  return books.find((b) => b.id === activeBookId) ?? books[0];
}
