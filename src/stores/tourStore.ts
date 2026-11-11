// AUTHR_OS — first-run product tour state (F-14 completion, UX re-assessment
// D6 Phase 2). Three steps — WRITE / PRODUCE / SELL — shown once after the
// first bootstrap, dismissible forever (localStorage flag), replayable from
// the maintenance menu. Zero interference for returning users: the flag is
// checked before anything mounts.

import { create } from 'zustand';

const TOUR_DONE_KEY = 'authr_os.tour_done';

export const TOUR_STEPS = [
  {
    id: 'write',
    title: 'WRITE — the cockpit',
    body: 'Draft in the manuscript canvas: every paragraph is a queryable segment with a speaker and emotion. Enter splits segments; the inspector bulk-tags dialogue. Ctrl/Cmd+K opens the command palette from anywhere.',
  },
  {
    id: 'produce',
    title: 'PRODUCE — media studios',
    body: 'The Audio studio runs the full chain (plan → synthesize → master → ACX validate); the Video studio storyboards and renders the MP4; Localization manages translation drafts per language.',
  },
  {
    id: 'sell',
    title: 'SELL — storefront & releases',
    body: 'Craft QC scores retention and cadence, the KDP panel checks compliance live, and Publish & Export ships EPUB 3, M4B, captions and signed release manifests.',
  },
] as const;

function tourDone(): boolean {
  try {
    return window.localStorage.getItem(TOUR_DONE_KEY) === '1';
  } catch {
    return false;
  }
}

function markTourDone(): void {
  try {
    window.localStorage.setItem(TOUR_DONE_KEY, '1');
  } catch {
    /* private mode — tour just replays next launch */
  }
}

interface TourState {
  /** 0-based step index, or null when the tour is not showing. */
  step: number | null;
  /** Fresh bootstrap path — never shows for returning users. */
  maybeStart: () => void;
  next: () => void;
  /** Skip or finish — marks done either way (dismissible forever). */
  end: () => void;
  /** Maintenance-menu entry: replays even when the flag is set. */
  replay: () => void;
}

export const useTourStore = create<TourState>((set, get) => ({
  step: null,

  maybeStart: () => {
    if (tourDone()) return;
    set({ step: 0 });
  },

  next: () => {
    const cur = get().step;
    if (cur == null) return;
    if (cur + 1 >= TOUR_STEPS.length) {
      markTourDone();
      set({ step: null });
    } else {
      set({ step: cur + 1 });
    }
  },

  end: () => {
    markTourDone();
    set({ step: null });
  },

  replay: () => set({ step: 0 }),
}));
