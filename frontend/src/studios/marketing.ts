/**
 * Marketing Studio logic (Sprint 8) — pure, deterministic generators for
 * SEO metadata, keywords and Midjourney cover prompts derived from real
 * bundle data (books / characters / lore / threads).
 *
 * Persisted via the frozen surface: `book_update` (genre, status, target
 * word count). SEO copy is a session-local workspace (the publishing
 * pipeline, including front/back-matter fields, lands in Phase 6).
 */

import type { BookRow, CharacterRow, LoreRow, ThreadRow } from '../types';

const STYLES = [
  'cinematic book cover art',
  'dramatic rim lighting',
  'moody atmospheric depth',
  'hyper-detailed, 8k',
];

export function buildMidjourneyPrompt(
  book: BookRow,
  characters: CharacterRow[],
  lore: LoreRow[],
): string {
  const genre = book.genre?.trim() || 'literary fiction';
  const castAnchor = characters
    .filter((c) => c.character_key !== 'NARRATOR')
    .slice(0, 2)
    .map((c) => c.display_name)
    .join(' and ');
  const placeAnchor =
    lore.find((l) => l.category === 'location')?.term ??
    lore[0]?.term ??
    'an unnamed, mist-shrouded place';
  const subject = castAnchor
    ? `${castAnchor} at ${placeAnchor}`
    : `a lone figure at ${placeAnchor}`;
  return [
    `book cover, ${genre} mood, ${subject}`,
    ...STYLES,
    `--ar 2:3 --v 6 --style raw`,
  ].join(', ');
}

export function buildKeywords(
  book: BookRow,
  characters: CharacterRow[],
  lore: LoreRow[],
): string[] {
  const words = new Set<string>();
  if (book.genre) book.genre.split(/[\s,]+/).filter(Boolean).forEach((w) => words.add(w.toLowerCase()));
  characters.filter((c) => c.character_key !== 'NARRATOR').forEach((c) => words.add(c.display_name.toLowerCase()));
  lore.forEach((l) => words.add(l.term.toLowerCase()));
  words.add(book.author.toLowerCase());
  return [...words].filter((w) => w.length > 2).slice(0, 12);
}

export function buildSocialHooks(book: BookRow, threads: ThreadRow[]): string[] {
  const open = threads.filter((t) => t.status === 'planted' || t.status === 'active');
  const hooks = open.slice(0, 3).map((t) => `What if ${lowerFirst(t.title)}?`);
  if (hooks.length === 0) {
    hooks.push(`${book.title} — ${book.genre ?? 'a story'} where every promise is kept.`);
  }
  return hooks;
}

function lowerFirst(s: string): string {
  return s.charAt(0).toLowerCase() + s.slice(1);
}
