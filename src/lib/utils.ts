// AUTHR_OS — small shared helpers.

import type { ChapterRow } from '../types/ast';

/** Tailwind-friendly className joiner (keeps false/undefined out). */
export function cn(...parts: Array<string | false | null | undefined>): string {
  return parts.filter(Boolean).join(' ');
}

/** New segment UUID (contract §4.8 — new segments get UUID v4 ids). */
export function newSegmentId(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return crypto.randomUUID();
  }
  // Deterministic-enough fallback for exotic browsers.
  return `xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx`.replace(/[xy]/g, (c) => {
    const r = (Math.random() * 16) | 0;
    const v = c === 'x' ? r : (r & 0x3) | 0x8;
    return v.toString(16);
  });
}

export function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

/** Word count = whitespace-split tokens (mirrors the inspector's definition). */
export function wordCount(text: string): number {
  if (!text) return 0;
  return text.trim().split(/\s+/).filter(Boolean).length;
}

export function totalWordCount(texts: string[]): number {
  return texts.reduce((acc, t) => acc + wordCount(t), 0);
}

export function slugify(value: string): string {
  return value
    .toLowerCase()
    .trim()
    .replace(/['’]/g, '')
    .replace(/[^a-z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '');
}

export function formatNumber(n: number): string {
  return new Intl.NumberFormat('en-US').format(n);
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}

export function formatMs(ms: number): string {
  if (!ms) return '0.0s';
  return `${(ms / 1000).toFixed(1)}s`;
}

export function truncate(text: string, max: number): string {
  if (text.length <= max) return text;
  return `${text.slice(0, max - 1)}…`;
}

export function timestampSlug(): string {
  const d = new Date();
  const p = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}${p(d.getMonth() + 1)}${p(d.getDate())}-${p(d.getHours())}${p(
    d.getMinutes(),
  )}${p(d.getSeconds())}`;
}

/** Next chapter number within a book (max + 1). */
export function nextChapterNumber(chapters: ChapterRow[], bookId: number): number {
  const nums = chapters.filter((c) => c.book_id === bookId).map((c) => c.chapter_number);
  return nums.length ? Math.max(...nums) + 1 : 1;
}

/** Is the error one of the contract's taxonomy errors? */
export function errorMessage(err: unknown): string {
  if (err instanceof Error) return err.message;
  return String(err);
}
