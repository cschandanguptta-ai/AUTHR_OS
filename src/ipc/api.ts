// AUTHR_OS — Typed IPC surface: one function per contract command (33 total).
// Every store and component goes through this module only.
// Contract reference: docs/CLI_CONTRACT.md §4.

import { invoke } from './bridge';
import type {
  BookRow,
  BundleManifest,
  ChapterRow,
  CharacterRow,
  AstMutationPayload,
  LockStatus,
  LoreRow,
  NarrativeSegmentAst,
  SegmentRow,
  SegmentSearchHit,
  ThreadRow,
} from '../types/ast';

// ── 4.1 Project lifecycle ────────────────────────────────────────────────────

export interface ProjectInitArgs {
  path: string;
  title: string;
  author?: string;
}
export interface ProjectInitResult {
  path: string;
  manifest: BundleManifest;
  book_id: number;
  readonly: boolean;
}

export function projectInit(args: ProjectInitArgs) {
  return invoke<ProjectInitResult>('project_init', args);
}

export interface ProjectOpenResult {
  readonly: boolean;
  manifest: BundleManifest;
  cloud_sync_guard: boolean;
}

export function projectOpen(args: { path: string }) {
  return invoke<ProjectOpenResult>('project_open', args);
}

export function projectClose() {
  return invoke<{ released: true }>('project_close');
}

export function projectLockStatus() {
  return invoke<LockStatus>('project_lock_status');
}

export function projectExport(args: { out: string }) {
  return invoke<{ out: string; bytes: number }>('project_export', args);
}

export function projectImport(args: { zip: string; dest: string }) {
  return invoke<{ path: string; manifest: BundleManifest }>('project_import', args);
}

// ── 4.2 Books ────────────────────────────────────────────────────────────────

export interface BookCreateArgs {
  title: string;
  author?: string;
  genre?: string;
  premise?: string;
  theme?: string;
  narrative_structure?: string;
  target_word_count?: number;
  target_chapters?: number;
}

export function bookCreate(args: BookCreateArgs) {
  return invoke<BookRow>('book_create', args);
}

export function bookList() {
  return invoke<{ books: BookRow[] }>('book_list');
}

export function bookUpdate(args: { id: number } & Partial<BookRow>) {
  return invoke<BookRow>('book_update', args);
}

export function bookDelete(id: number) {
  return invoke<{ deleted: number }>('book_delete', { id });
}

// ── 4.3 Chapters ─────────────────────────────────────────────────────────────

export interface ChapterCreateArgs {
  book_id: number;
  chapter_number: number;
  title: string;
  act_number?: number;
  synopsis?: string;
  target_word_count?: number;
  status?: string;
}

export function chapterCreate(args: ChapterCreateArgs) {
  return invoke<ChapterRow>('chapter_create', args);
}

export function chapterList(args: { book_id?: number } = {}) {
  return invoke<{ chapters: ChapterRow[] }>('chapter_list', args);
}

export function chapterGet(args: { id: number }) {
  return invoke<ChapterRow>('chapter_get', args);
}

export function chapterUpdate(args: { id: number } & Partial<ChapterRow>) {
  return invoke<ChapterRow>('chapter_update', args);
}

export function chapterDelete(id: number) {
  return invoke<{ deleted: number }>('chapter_delete', { id });
}

// ── 4.4 Voice Cast (character_profiles) ──────────────────────────────────────

export function characterList() {
  return invoke<{ characters: CharacterRow[] }>('character_list');
}

export interface CharacterUpsertArgs {
  character_key: string;
  display_name: string;
  role?: string;
  bio?: string;
  ipa_phonetic?: string;
  voice_engine?: string;
  voice_id?: string;
  base_rate?: number;
  base_pitch?: number;
  cadence_notes?: string;
}

export function characterUpsert(args: CharacterUpsertArgs) {
  return invoke<CharacterRow>('character_upsert', args);
}

export function characterDelete(character_key: string) {
  return invoke<{ deleted: string }>('character_delete', { character_key });
}

// ── 4.5 Lorebook ─────────────────────────────────────────────────────────────

export function loreList() {
  return invoke<{ lore: LoreRow[] }>('lore_list');
}

export function loreUpsert(args: {
  key: string;
  content: string;
  category?: string;
  ipa_phonetic?: string;
}) {
  return invoke<LoreRow>('lore_upsert', args);
}

export function loreDelete(key: string) {
  return invoke<{ deleted: string }>('lore_delete', { key });
}

// ── 4.6 Chekhov's Gun Ledger (unresolved_threads) ────────────────────────────

export interface ThreadOpenArgs {
  thread_title: string;
  introduced_in_chapter: number;
  payoff_requirement: string;
  thread_type?: string;
  status?: string;
}

export function threadOpen(args: ThreadOpenArgs) {
  return invoke<ThreadRow>('thread_open', args);
}

export function threadList(args: { status?: string } = {}) {
  return invoke<{ threads: ThreadRow[] }>('thread_list', args);
}

export function threadResolve(args: {
  id: number;
  resolved_in_chapter: number;
  status?: string;
}) {
  return invoke<ThreadRow>('thread_resolve', args);
}

// ── 4.7 Segments (AST storage + FTS) ─────────────────────────────────────────

export function segmentAppend(args: {
  chapter_id: number;
  segment_type: string;
  speaker_id: string;
  text_content: string;
  emotion?: string;
  prosody_rate?: number;
  prosody_pitch?: number;
  ambient_cue?: string;
  sfx_cue?: string;
  sequence_order?: number;
}) {
  return invoke<SegmentRow>('segment_append', args);
}

export function segmentList(chapter_id: number) {
  return invoke<{ segments: SegmentRow[] }>('segment_list', { chapter_id });
}

export function segmentSearch(args: { query: string; limit?: number }) {
  return invoke<{ hits: SegmentSearchHit[] }>('segment_search', args);
}

// ── 4.8 Editor Bridge ────────────────────────────────────────────────────────

export function syncLexicalAstMutation(args: AstMutationPayload) {
  return invoke<{ applied: number; commit?: string }>(
    'sync_lexical_ast_mutation',
    args as unknown as Record<string, unknown>,
  );
}

// ── 4.9 Keychain (no bundle required) ────────────────────────────────────────

export function keychainStore(args: { service: string; key: string; value: string }) {
  return invoke<{ stored: true }>('keychain_store', args);
}

export function keychainGet(args: { service: string; key: string }) {
  return invoke<{ value: string }>('keychain_get', args);
}

export function keychainDelete(args: { service: string; key: string }) {
  return invoke<{ deleted: true }>('keychain_delete', args);
}

// ── 4.10 Maintenance ─────────────────────────────────────────────────────────

export function gitSnapshot(args: { message?: string } = {}) {
  return invoke<{ commit: string }>('git_snapshot', args);
}

export function cleanSweep() {
  return invoke<{ purged_segments: number; purged_threads: number; fts_rebuilt: boolean }>(
    'clean_sweep',
  );
}

// Convenience re-export of the segment AST type for API consumers.
export type { NarrativeSegmentAst };
