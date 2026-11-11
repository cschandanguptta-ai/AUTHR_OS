// AUTHR_OS — publish run history (RA-07, UX re-assessment D6 Phase 3). The
// Publish & Export studio previously rendered its cards in a single left
// column with a dead right half at 1440 px; the right column is now the
// artifacts & run history surface, reusing the Audio run-history pattern:
// zustand state (survives studio tab switches), newest first, capped at 12.

import { create } from 'zustand';

export interface PublishArtifact {
  path: string;
  bytes?: number;
  extra?: string;
}

export interface PublishRunRecord {
  id: string;
  startedAt: number;
  finishedAt: number;
  label: string;
  status: 'done' | 'failed';
  artifacts: PublishArtifact[];
  /** Failure detail — errorCopy body + backend detail. */
  error?: string;
}

interface PublishRunState {
  history: PublishRunRecord[];
  /** Append one completed run (done or failed) — call in the run's finally. */
  record: (input: Omit<PublishRunRecord, 'id' | 'startedAt' | 'finishedAt'>) => void;
  clear: () => void;
}

const HISTORY_CAP = 12;

export const usePublishRunStore = create<PublishRunState>((set) => ({
  history: [],

  record: (input) =>
    set((s) => ({
      history: [
        {
          id: `pub-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
          startedAt: Date.now(),
          finishedAt: Date.now(),
          ...input,
        },
        ...s.history,
      ].slice(0, HISTORY_CAP),
    })),

  clear: () => set({ history: [] }),
}));
