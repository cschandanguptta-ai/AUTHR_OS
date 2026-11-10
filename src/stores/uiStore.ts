// AUTHR_OS — UI toast bus. No component library; a tiny zustand store + Toaster.

import { create } from 'zustand';
import { mapError } from '../ipc/mapError';

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

/**
 * Failure path used by stores/components. The canonical code on the error
 * decides the copy (mapError) so command names never reach the user; a
 * caller-supplied human title still wins. The raw backend message lands in
 * the details line — never a fake success.
 */
export function toastError(title: string, err: unknown) {
  const e = err as { code?: string; message?: string } | null;
  const humanTitle = Boolean(title) && !/_failed$/.test(title) && !/^[a-z][a-z0-9_]*$/.test(title);
  if (typeof e?.code === 'string' && e.code) {
    const { copy, detail } = mapError(err);
    useUiStore.getState().push(
      'error',
      humanTitle ? title : copy.title,
      copy.action ? `${copy.body}\n${copy.action}\n${detail}` : `${copy.body}\n${detail}`,
    );
    return;
  }
  const message = err instanceof Error ? err.message : String(err);
  useUiStore.getState().push('error', humanTitle ? title : 'That did not go through', message);
}
