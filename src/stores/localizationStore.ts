// AUTHR_OS — localization draft persistence (D7 §3, R-02).
//
// Translation drafts used to live in LocalizationStudio component state and
// died on every tab switch. They now persist to localStorage (key
// authr_os.loc_drafts) keyed `${bundlePath}::${chapterId}::${lang}` so drafts
// survive studio switches and bundle switches stay isolated.
//
// Removal rule: entries for a deleted bundle path cannot be stat'ed from the
// browser, so Welcome-open pruning drops every entry that is (a) not the last
// opened bundle AND (b) untouched for 14 days — an age-based proxy for
// deletion; the studio's Clear-drafts button covers the explicit case.

import { create } from 'zustand';
import { persist } from 'zustand/middleware';

export interface LocDraftEntry {
  /** Language rail for the owning bundle+chapter (kept in sync on writes). */
  languages: string[];
  /** segment_id → translated text for ONE language. */
  drafts: Record<string, string>;
  savedAt: number;
}

const PRUNE_AFTER_MS = 14 * 24 * 60 * 60 * 1000;

export function locKey(bundlePath: string, chapterId: number, lang: string): string {
  return `${bundlePath}::${chapterId}::${lang}`;
}

interface LocalizationState {
  entries: Record<string, LocDraftEntry>;
  /** Write one segment's draft for (bundle, chapter, lang). */
  saveDraft: (bundle: string, chapterId: number, lang: string, segmentId: string, text: string, languages: string[]) => void;
  /** Create the per-language entry (empty drafts) and sync the shared rail. */
  ensureLanguage: (bundle: string, chapterId: number, lang: string, languages: string[]) => void;
  removeLanguage: (bundle: string, chapterId: number, lang: string) => void;
  /** Sync the language rail across every entry of this bundle+chapter. */
  syncRail: (bundle: string, chapterId: number, languages: string[]) => void;
  clearBundle: (bundle: string) => number;
  /** Age+membership proxy prune — called on Welcome open. Returns removed count. */
  pruneStale: (keepBundle: string) => number;
}

function ensureEntry(
  entries: Record<string, LocDraftEntry>,
  key: string,
  languages: string[],
): LocDraftEntry {
  return entries[key] ?? { languages: [...languages], drafts: {}, savedAt: Date.now() };
}

export const useLocalizationStore = create<LocalizationState>()(
  persist(
    (set, get) => ({
      entries: {},

      saveDraft: (bundle, chapterId, lang, segmentId, text, languages) => {
        const key = locKey(bundle, chapterId, lang);
        set((s) => {
          const entry = ensureEntry(s.entries, key, languages);
          return {
            entries: {
              ...s.entries,
              [key]: {
                languages: entry.languages.length ? entry.languages : [...languages],
                drafts: { ...entry.drafts, [segmentId]: text },
                savedAt: Date.now(),
              },
            },
          };
        });
      },

      ensureLanguage: (bundle, chapterId, lang, languages) => {
        const key = locKey(bundle, chapterId, lang);
        set((s) => {
          const entry = s.entries[key] ?? { languages: [...languages], drafts: {}, savedAt: Date.now() };
          return {
            entries: {
              ...s.entries,
              [key]: { ...entry, languages: [...languages] },
            },
          };
        });
        // Keep the rail identical on sibling language entries.
        get().syncRail(bundle, chapterId, languages);
      },

      removeLanguage: (bundle, chapterId, lang) => {
        const key = locKey(bundle, chapterId, lang);
        set((s) => {
          if (!(key in s.entries)) return s;
          const next = { ...s.entries };
          delete next[key];
          return { entries: next };
        });
      },

      syncRail: (bundle, chapterId, languages) => {
        // Sync the rail on every entry of this bundle+chapter (any language).
        const prefix = `${bundle}::${chapterId}::`;
        set((s) => {
          const next: Record<string, LocDraftEntry> = {};
          for (const [k, v] of Object.entries(s.entries)) {
            next[k] = k.startsWith(prefix) ? { ...v, languages: [...languages] } : v;
          }
          return { entries: next };
        });
      },

      clearBundle: (bundle) => {
        const prefix = `${bundle}::`;
        let removed = 0;
        set((s) => {
          const next: Record<string, LocDraftEntry> = {};
          for (const [k, v] of Object.entries(s.entries)) {
            if (k.startsWith(prefix)) removed += 1;
            else next[k] = v;
          }
          return { entries: next };
        });
        return removed;
      },

      pruneStale: (keepBundle) => {
        const cutoff = Date.now() - PRUNE_AFTER_MS;
        let removed = 0;
        set((s) => {
          const next: Record<string, LocDraftEntry> = {};
          for (const [k, v] of Object.entries(s.entries)) {
            const bundle = k.split('::')[0];
            if (bundle !== keepBundle && v.savedAt < cutoff) {
              removed += 1;
              continue;
            }
            next[k] = v;
          }
          return { entries: next };
        });
        return removed;
      },
    }),
    { name: 'authr_os.loc_drafts' },
  ),
);

/** Non-reactive helpers for one-shot callers (Welcome prune, project close). */
export function pruneStaleDrafts(keepBundle: string): number {
  return useLocalizationStore.getState().pruneStale(keepBundle);
}

export function clearBundleDrafts(bundle: string): number {
  return useLocalizationStore.getState().clearBundle(bundle);
}
