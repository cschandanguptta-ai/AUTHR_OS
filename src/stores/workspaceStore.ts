// AUTHR_OS — cockpit shell state: active studio, pane geometry, view mode.

import { create } from 'zustand';
import { clamp } from '../lib/utils';

export type StudioId = 'novel' | 'audio' | 'video' | 'localization' | 'marketing' | 'craft' | 'publish';
export type ViewMode = 'clean' | 'script' | 'readalong';
export type PaneKey = 'binder' | 'inspector';

export const PANE_DEFAULTS: Record<PaneKey, number> = { binder: 280, inspector: 320 };
export const PANE_MIN: Record<PaneKey, number> = { binder: 220, inspector: 260 };
export const PANE_MAX: Record<PaneKey, number> = { binder: 480, inspector: 560 };

/** One-shot cross-studio handoff payload (D7 §8, R-07) — consumed on mount. */
export interface StudioIntent {
  chapterId?: number;
  arm?: 'produce' | 'assemble' | 'publish';
}

interface WorkspaceState {
  activeStudio: StudioId;
  viewMode: ViewMode;
  paneWidths: Record<PaneKey, number>;
  binderOpen: boolean;
  inspectorOpen: boolean;
  paletteOpen: boolean;
  /** Pending one-shot intents per studio — switchStudio(id, payload) writes, the studio consumes. */
  intents: Partial<Record<StudioId, StudioIntent>>;
  setActiveStudio: (studio: StudioId) => void;
  switchStudio: (studio: StudioId, intent?: StudioIntent) => void;
  consumeIntent: (studio: StudioId) => StudioIntent | undefined;
  setViewMode: (mode: ViewMode) => void;
  setPaneWidth: (pane: PaneKey, width: number) => void;
  resetPaneWidths: () => void;
  toggleBinder: () => void;
  toggleInspector: () => void;
  setPaletteOpen: (open: boolean) => void;
}

export const useWorkspaceStore = create<WorkspaceState>((set, get) => ({
  activeStudio: 'novel',
  viewMode: 'script',
  paneWidths: { ...PANE_DEFAULTS },
  binderOpen: true,
  inspectorOpen: true,
  paletteOpen: false,
  intents: {},
  setActiveStudio: (studio) => set({ activeStudio: studio }),
  switchStudio: (studio, intent) =>
    set((s) => ({
      activeStudio: studio,
      intents: intent ? { ...s.intents, [studio]: intent } : s.intents,
    })),
  consumeIntent: (studio) => {
    const intent = get().intents[studio];
    if (intent) {
      set((s) => {
        const intents = { ...s.intents };
        delete intents[studio];
        return { intents };
      });
    }
    return intent;
  },
  setViewMode: (mode) => set({ viewMode: mode }),
  setPaneWidth: (pane, width) =>
    set((s) => ({
      paneWidths: {
        ...s.paneWidths,
        [pane]: clamp(width, PANE_MIN[pane], PANE_MAX[pane]),
      },
    })),
  resetPaneWidths: () => set({ paneWidths: { ...PANE_DEFAULTS } }),
  toggleBinder: () => set((s) => ({ binderOpen: !s.binderOpen })),
  toggleInspector: () => set((s) => ({ inspectorOpen: !s.inspectorOpen })),
  setPaletteOpen: (paletteOpen) => set({ paletteOpen }),
}));
