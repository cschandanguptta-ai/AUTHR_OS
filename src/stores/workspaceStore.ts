// AUTHR_OS — cockpit shell state: active studio, pane geometry, view mode.

import { create } from 'zustand';
import { clamp } from '../lib/utils';

export type StudioId = 'novel' | 'audio' | 'video' | 'localization' | 'marketing' | 'craft';
export type ViewMode = 'clean' | 'script' | 'readalong';
export type PaneKey = 'binder' | 'inspector';

export const PANE_DEFAULTS: Record<PaneKey, number> = { binder: 280, inspector: 320 };
export const PANE_MIN: Record<PaneKey, number> = { binder: 220, inspector: 260 };
export const PANE_MAX: Record<PaneKey, number> = { binder: 480, inspector: 560 };

interface WorkspaceState {
  activeStudio: StudioId;
  viewMode: ViewMode;
  paneWidths: Record<PaneKey, number>;
  binderOpen: boolean;
  inspectorOpen: boolean;
  setActiveStudio: (studio: StudioId) => void;
  setViewMode: (mode: ViewMode) => void;
  setPaneWidth: (pane: PaneKey, width: number) => void;
  resetPaneWidths: () => void;
  toggleBinder: () => void;
  toggleInspector: () => void;
}

export const useWorkspaceStore = create<WorkspaceState>((set) => ({
  activeStudio: 'novel',
  viewMode: 'script',
  paneWidths: { ...PANE_DEFAULTS },
  binderOpen: true,
  inspectorOpen: true,
  setActiveStudio: (studio) => set({ activeStudio: studio }),
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
}));
