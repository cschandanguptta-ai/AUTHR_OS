/**
 * UI store — workstation switcher, cockpit pane geometry, canvas view mode,
 * and the shared shell event/envelope log consumed by the Phase 2 console
 * and the Phase 3 Dynamic Inspector alike.
 */

import { create } from 'zustand';
import type { AppEvent, Envelope } from '../api';
import type { ViewMode, Workstation } from '../types';

export interface BinderSelection {
  bookId: number | null;
  chapterId: number | null;
  /** Which chapter the canvas is bound to (mirrors chapterId once loaded). */
  canvasChapterId: number | null;
  selectedSegmentId: string | null;
  selectedCastId: number | null;
  selectedThreadId: number | null;
}

interface UiState {
  workstation: Workstation;
  viewMode: ViewMode;
  /** left | center | right pane fractions (sum to 100). */
  panes: { binder: number; canvas: number; inspector: number };
  selection: BinderSelection;
  events: AppEvent[];
  lastEnvelope: { cmd: string; env: Envelope } | null;
  toast: string | null;

  setWorkstation(w: Workstation): void;
  setViewMode(v: ViewMode): void;
  setPanes(p: { binder: number; canvas: number; inspector: number }): void;
  select(patch: Partial<BinderSelection>): void;
  pushEvent(evt: AppEvent): void;
  recordEnvelope(cmd: string, env: Envelope): void;
  setToast(msg: string | null): void;
}

const EVENTS_CAP = 200;

/** Pane geometry is a pure UI preference — persist it, but never crash. */
const PANES_KEY = 'authr.cockpit.panes';

function loadPanes(): { binder: number; canvas: number; inspector: number } {
  try {
    const raw = typeof localStorage !== 'undefined' ? localStorage.getItem(PANES_KEY) : null;
    if (raw) {
      const p = JSON.parse(raw);
      if ([p.binder, p.canvas, p.inspector].every((n: unknown) => typeof n === 'number')) return p;
    }
  } catch {
    /* ignore */
  }
  return { binder: 20, canvas: 56, inspector: 24 };
}

function savePanes(p: UiState['panes']): void {
  try {
    if (typeof localStorage !== 'undefined') localStorage.setItem(PANES_KEY, JSON.stringify(p));
  } catch {
    /* ignore */
  }
}

export const useUi = create<UiState>((set) => ({
  workstation: 'cockpit',
  viewMode: 'script',
  panes: loadPanes(),
  selection: { bookId: null, chapterId: null, canvasChapterId: null, selectedSegmentId: null, selectedCastId: null, selectedThreadId: null },
  events: [],
  lastEnvelope: null,
  toast: null,

  setWorkstation: (w) => set({ workstation: w }),
  setViewMode: (v) => set({ viewMode: v }),
  setPanes: (p) => {
    set({ panes: p });
    savePanes(p);
  },
  select: (patch) => set((s) => ({ selection: { ...s.selection, ...patch } })),
  pushEvent: (evt) => set((s) => ({ events: [evt, ...s.events].slice(0, EVENTS_CAP) })),
  recordEnvelope: (cmd, env) => set({ lastEnvelope: { cmd, env } }),
  setToast: (msg) => set({ toast: msg }),
}));

/** Record an invocation for the inspector (used by panels and stores). */
export function recordCall(cmd: string, env: Envelope): void {
  useUi.getState().recordEnvelope(cmd, env);
}
