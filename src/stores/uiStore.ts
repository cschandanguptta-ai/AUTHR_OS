// AUTHR_OS — UI toast bus. No component library; a tiny zustand store + Toaster.

import { create } from 'zustand';

export type ToastKind = 'info' | 'success' | 'error' | 'warn';

export interface Toast {
  id: number;
  kind: ToastKind;
  title: string;
  message?: string;
}

interface UiState {
  toasts: Toast[];
  push: (kind: ToastKind, title: string, message?: string) => void;
  dismiss: (id: number) => void;
}

let nextToastId = 1;

export const useUiStore = create<UiState>((set, get) => ({
  toasts: [],
  push: (kind, title, message) => {
    const id = nextToastId++;
    set((s) => ({ toasts: [...s.toasts.slice(-5), { id, kind, title, message }] }));
    window.setTimeout(() => get().dismiss(id), 5200);
  },
  dismiss: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
}));

/** Convenience helper usable from stores (non-reactive). */
export function toast(kind: ToastKind, title: string, message?: string) {
  useUiStore.getState().push(kind, title, message);
}

export function toastError(title: string, err: unknown) {
  const message = err instanceof Error ? err.message : String(err);
  useUiStore.getState().push('error', title, message);
}
