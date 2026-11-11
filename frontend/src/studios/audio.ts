/**
 * Audio Studio logic (Sprint 8) — DAW layer stats, mastering rack
 * defaults (the Phase 4 DSP targets, already frozen in the build plan)
 * and the pronunciation dictionary table.
 */

import type { CharacterRow, LoreRow, SegmentRow } from '../types';

export interface DawLayerStats {
  speech: number;
  sfx: number;
  ambient: number;
  total: number;
}

/** 3-layer rack census: Speech + Spot-SFX Foley + Ambient Bed. */
export function dawLayerStats(segments: SegmentRow[]): DawLayerStats {
  let sfx = 0;
  let ambient = 0;
  for (const seg of segments) {
    if (seg.sfx_cue) sfx += 1;
    if (seg.ambient_cue) ambient += 1;
  }
  return { speech: segments.length, sfx, ambient, total: segments.length };
}

/** ACX / EBU R128 mastering targets (build plan §5.4, frozen numbers). */
export const MASTERING_DEFAULTS = {
  targetRmsDb: -20.0,
  peakCeilingDbfs: -3.1,
  duckingDb: -18,
  duckingAttackMs: 120,
  duckingReleaseMs: 750,
} as const;

export interface PronunciationRow {
  key: string;
  display: string;
  source: 'character' | 'lore';
  id: number;
  ipa: string | null;
  needsPronunciation: boolean;
}

/**
 * The pronunciation dictionary: proper nouns (cast + lore terms). Names
 * missing `ipa_phonetic` are flagged so the G2P fallback (Phase 4) can be
 * reviewed before synthesis.
 */
export function pronunciationRows(characters: CharacterRow[], lore: LoreRow[]): PronunciationRow[] {
  const rows: PronunciationRow[] = [];
  for (const c of characters) {
    rows.push({
      key: c.character_key,
      display: c.display_name,
      source: 'character',
      id: c.id,
      ipa: c.ipa_phonetic,
      needsPronunciation: !c.ipa_phonetic && c.character_key !== 'NARRATOR',
    });
  }
  for (const l of lore) {
    rows.push({
      key: l.term,
      display: l.term,
      source: 'lore',
      id: l.id,
      ipa: l.ipa_phonetic,
      needsPronunciation: !l.ipa_phonetic,
    });
  }
  return rows;
}
