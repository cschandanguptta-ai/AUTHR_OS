/**
 * Shared frontend domain types (Phase 3).
 *
 * Row shapes mirror the SQLite schema DTOs returned by the frozen IPC
 * envelope (see docs/CLI_CONTRACT.md and src-tauri/src/models/ast.rs).
 */

/** A `chapter_segments` row as returned by `segment_list` / `segment_create`. */
export interface SegmentRow {
  id: number;
  segment_id: string;
  chapter_id: number;
  speaker_id: string;
  segment_type: 'NARRATION' | 'DIALOGUE' | 'INTERNAL_MONOLOGUE';
  text_content: string;
  emotion: string;
  prosody_rate: number;
  prosody_pitch: number;
  ambient_cue: string | null;
  sfx_cue: string | null;
  audio_hash: string | null;
  sequence_order: number;
  audio_start_ms?: number;
  audio_duration_ms?: number;
}

export interface BookRow {
  id: number;
  title: string;
  author: string;
  genre: string | null;
  narrative_structure: string;
  status: string;
  target_word_count: number | null;
  word_count: number;
}

export interface ChapterRow {
  id: number;
  book_id: number;
  chapter_index: number;
  title: string;
  summary: string | null;
  status: string;
}

export interface CharacterRow {
  id: number;
  book_id: number;
  character_key: string;
  display_name: string;
  role: string;
  voice_engine: string;
  voice_config: string | null;
  ipa_phonetic: string | null;
}

export interface LoreRow {
  id: number;
  book_id: number;
  category: string;
  term: string;
  content: string;
  ipa_phonetic: string | null;
}

export interface ThreadRow {
  id: number;
  book_id: number;
  chapter_id: number | null;
  title: string;
  status: 'planted' | 'active' | 'paid_off' | 'abandoned';
  planted_chapter_index: number | null;
  payoff_chapter_index: number | null;
  notes: string | null;
}

/** The three canvas view modes (Sprint 7). */
export type ViewMode = 'clean' | 'script' | 'readalong';

/** The five restored workstations + the Phase 2 operator console. */
export type Workstation = 'cockpit' | 'audio' | 'video' | 'localization' | 'marketing' | 'console';

export const WORKSTATIONS: { key: Workstation; label: string; hint: string }[] = [
  { key: 'cockpit', label: 'Cockpit', hint: '3-pane author workstation' },
  { key: 'audio', label: 'Audio Studio', hint: 'voice profiling · casting · pronunciation' },
  { key: 'video', label: 'Video Studio', hint: 'slide pacing timeline' },
  { key: 'localization', label: 'Localization', hint: 'translation · script import' },
  { key: 'marketing', label: 'Marketing', hint: 'SEO · Midjourney prompts' },
  { key: 'console', label: 'Console', hint: 'raw envelope operator console' },
];

/** Unwrap a successful envelope or throw — store actions use this. */
export function unwrap<T>(env: { ok: boolean; data?: T; error?: { message: string } }): T {
  if (!env.ok) throw new Error(env.error?.message ?? 'envelope error');
  return env.data as T;
}
