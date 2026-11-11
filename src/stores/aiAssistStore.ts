// AUTHR_OS — inline AI assist state (D7 §4, R-03).
//
// The editor toolbar / command palette request an action; LexicalCanvas owns
// the Lexical transaction and consumes requests from here. Proposals never
// auto-commit: the accept/discard bar decides. Provenance (backend + seed)
// rides on every proposal chip — OFFLINE_GRAMMAR output is deterministic.

import { create } from 'zustand';

export type AiAction = 'continue' | 'rewrite' | 'summarize';

export const AI_ACTION_LABEL: Record<AiAction, string> = {
  continue: 'Continue writing',
  rewrite: 'Rewrite selection',
  summarize: 'Summarize chapter',
};

export interface AiProposal {
  action: AiAction;
  text: string;
  backend: string;
  seed: number;
  /** What Accept will do — shown verbatim on the bar. */
  effect: string;
}

interface AiAssistState {
  /** One-shot request consumed by the editor canvas. */
  pendingRequest: AiAction | null;
  busy: AiAction | null;
  proposal: AiProposal | null;
  error: string | null;
  requestAction: (action: AiAction) => void;
  consumeRequest: () => AiAction | null;
  setBusy: (action: AiAction | null) => void;
  propose: (proposal: AiProposal) => void;
  fail: (message: string) => void;
  resolve: () => void;
}

export const useAiAssistStore = create<AiAssistState>((set, get) => ({
  pendingRequest: null,
  busy: null,
  proposal: null,
  error: null,
  requestAction: (action) => set({ pendingRequest: action, error: null }),
  consumeRequest: () => {
    const req = get().pendingRequest;
    if (req) set({ pendingRequest: null });
    return req;
  },
  setBusy: (busy) => set({ busy }),
  propose: (proposal) => set({ proposal, busy: null, error: null }),
  fail: (message) => set({ error: message, busy: null }),
  resolve: () => set({ proposal: null, error: null }),
}));

/** FNV-1a over the selection text → deterministic uint32 seed (D7 §4). */
export function seedFromText(text: string): number {
  let h = 0x811c9dc5;
  for (let i = 0; i < text.length; i += 1) {
    h ^= text.charCodeAt(i);
    h = Math.imul(h, 0x01000193);
  }
  return h >>> 0;
}
