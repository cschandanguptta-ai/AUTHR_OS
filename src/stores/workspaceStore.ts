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
  /** Provider settings modal open state — any surface can raise it (R-07 CTA). */
  providerModalOpen: boolean;
  providerModalFocus: 'image' | 'llm' | null;
  /** Project dashboard overlay (R-12) — header button, palette action, and
   *  auto-raised right after a fresh project_init so the bootstrap is visible. */
  dashboardOpen: boolean;
  /** True only when the dashboard was opened by a fresh bootstrap (R-12/F-14). */
  dashboardFresh: boolean;
  openDashboard: (opts?: { fresh?: boolean }) => void;
  closeDashboard: () => void;
  /** RA-01 deep-link: bumped when any surface requests the New Chapter dialog
   *  (dashboard NBA ladder, palette). ProjectBinder watches the nonce. */
  newChapterNonce: number;
  requestNewChapter: () => void;
  /** RA-08 companion: prefill speaker key for "add to cast" from the canvas
   *  gutter. ProjectBinder opens CharacterModal with this pre-seeded. */
  castModalPrefill: string | null;
  openCastModal: (prefillSpeaker?: string) => void;
  closeCastModal: () => void;
  /** Pending one-shot intents per studio — switchStudio(id, payload) writes, the studio consumes. */
  intents: Partial<Record<StudioId, StudioIntent>>;
  setActiveStudio: (studio: StudioId) => void;
  switchStudio: (studio: StudioId, intent?: StudioIntent) => void;
  consumeIntent: (studio: StudioId) => StudioIntent | undefined;
  openProviderModal: (focus?: 'image' | 'llm') => void;
  closeProviderModal: () => void;
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
  dashboardOpen: false,
  dashboardFresh: false,
  openDashboard: (opts) => set({ dashboardOpen: true, dashboardFresh: opts?.fresh ?? false }),
  closeDashboard: () => set({ dashboardOpen: false, dashboardFresh: false }),
  newChapterNonce: 0,
  requestNewChapter: () => set((s) => ({ newChapterNonce: s.newChapterNonce + 1 })),
  castModalPrefill: null,
  openCastModal: (prefillSpeaker) =>
    set({ castModalPrefill: prefillSpeaker ?? null }),
  closeCastModal: () => set({ castModalPrefill: null }),
  providerModalOpen: false,
  providerModalFocus: null,
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
  openProviderModal: (focus) =>
    set({ providerModalOpen: true, providerModalFocus: focus ?? null }),
  closeProviderModal: () => set({ providerModalOpen: false, providerModalFocus: null }),
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
