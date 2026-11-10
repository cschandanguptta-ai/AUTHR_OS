// AUTHR_OS — app-level confirm requests (R-06): lets any surface (kebab menu,
// command palette) raise the shared ConfirmDialog without prop drilling.

import { create } from 'zustand';

export interface ConfirmRequest {
  title: string;
  body: string;
  confirmLabel?: string;
  typeToConfirm?: string;
  onConfirm: () => void;
}

interface ConfirmState {
  req: ConfirmRequest | null;
  ask: (req: ConfirmRequest) => void;
  close: () => void;
}

export const useConfirmStore = create<ConfirmState>((set) => ({
  req: null,
  ask: (req) => set({ req }),
  close: () => set({ req: null }),
}));
