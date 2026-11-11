// AUTHR_OS — deterministic next-best-action ladder (RA-01, UX re-assessment D6
// Phase 1). Maturity first: nothing ranks above creating or writing chapter
// content while the manuscript has no chapters or no words; storefront work
// (KDP) is ranked below word-count progress and only surfaces once words
// exist. Ladder order: first-chapter → write → cast → tag → produce →
// scene → kdp. Pure function — the Project Dashboard overlay supplies icons
// and navigation, this module owns the ordering and the copy.

import type { ChapterRow } from '../types/ast';

export interface NbaUntagged {
  chapterId: number;
  chapterNumber: number;
  count: number;
}

export interface NbaInput {
  chapters: ChapterRow[];
  /** Sum of actual_word_count across all loaded chapters. */
  words: number;
  cast: number;
  untagged: NbaUntagged | null;
  /** Per-chapter audio coverage: total segments vs synthesized. */
  coverage: Record<number, { total: number; synth: number }>;
  kdp: { pass: boolean; score: number } | null;
}

export type NbaId =
  | 'first-chapter'
  | 'write'
  | 'cast'
  | 'tag'
  | 'produce'
  | 'scene'
  | 'kdp';

export interface Nba {
  id: NbaId;
  label: string;
  detail: string;
  /** Chapter the action targets, when one exists (deep-link support). */
  chapterId?: number;
}

/** First chapter with zero words — the "start writing" target. */
export function writingTarget(chapters: ChapterRow[]): ChapterRow | null {
  if (chapters.length === 0) return null;
  return chapters.find((c) => (c.actual_word_count ?? 0) === 0) ?? chapters[0];
}

export function buildNbaLadder(input: NbaInput): Nba[] {
  const out: Nba[] = [];
  const { chapters, words, cast, untagged, coverage, kdp } = input;

  // ── Maturity tier — always outranks production and storefront work ──────
  if (chapters.length === 0) {
    out.push({
      id: 'first-chapter',
      label: 'Create your first chapter',
      detail:
        'The book has no chapters yet — audio, video and the storefront all hang off chapter content. The New Chapter dialog opens with the next number pre-filled.',
    });
  }
  if (chapters.length > 0 && words === 0) {
    const target = writingTarget(chapters);
    if (target) {
      const tgt = target.target_word_count;
      out.push({
        id: 'write',
        label: `Start writing — chapter ${target.chapter_number} has 0 words`,
        detail: tgt
          ? `Draft in the cockpit; the chapter target is ${tgt.toLocaleString('en-US')} words. Produce and storefront steps unlock as words land.`
          : 'Draft in the cockpit — produce and storefront steps unlock as words land.',
        chapterId: target.id,
      });
    }
  }

  // ── Production readiness tier ───────────────────────────────────────────
  if (cast === 0) {
    out.push({
      id: 'cast',
      label: 'Build your cast',
      detail:
        "No characters yet — audio synthesis needs a voice cast before any chapter can be produced.",
    });
  }
  if (untagged) {
    out.push({
      id: 'tag',
      label: `Tag dialogue in chapter ${untagged.chapterNumber}`,
      detail: `${untagged.count} dialogue segment${untagged.count === 1 ? '' : 's'} still speak with the narrator's voice — bulk-tag them in the inspector.`,
      chapterId: untagged.chapterId,
    });
  }
  const produceTarget = chapters.find(
    (c) => (coverage[c.id]?.total ?? 0) > 0 && coverage[c.id].synth === 0,
  );
  if (produceTarget) {
    out.push({
      id: 'produce',
      label: `Produce audio for chapter ${produceTarget.chapter_number}`,
      detail: `${coverage[produceTarget.id].total} segments have no audio yet — run the produce pipeline (plan → synthesize → master → validate).`,
      chapterId: produceTarget.id,
    });
  }
  const assembled = chapters.find(
    (c) =>
      (coverage[c.id]?.total ?? 0) > 0 &&
      coverage[c.id].synth === coverage[c.id].total,
  );
  if (assembled) {
    out.push({
      id: 'scene',
      label: `Assemble the video novel for chapter ${assembled.chapter_number}`,
      detail:
        'Fully synthesized narration is ready — storyboard the scenes and render the MP4.',
      chapterId: assembled.id,
    });
  }

  // ── Storefront tier — gated on real content (RA-01 acceptance) ──────────
  if (words > 0 && kdp && !kdp.pass) {
    out.push({
      id: 'kdp',
      label: 'Complete your KDP package',
      detail: `Storefront readiness is ${kdp.score}/100 — finish keywords, categories and the description in the KDP panel.`,
    });
  }

  return out;
}
