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

// ── 4.14 Craft catalog & storefront packaging (contract v1.3) ───────────────

export interface CatalogEntry {
  id: string;
  label: string;
  category?: string;
  family?: string;
  default_word_count?: number;
  default_chapters?: number;
  unit?: string;
  kdp_note?: string;
  pov?: string;
  tense?: string;
  rhythm?: string;
  diction?: string;
  pacing?: string;
  signature?: string;
  directive?: string;
  lineage?: string;
  legacy_structure?: string | null;
  beats?: Array<{ id: string; label: string; guidance: string }>;
  best_for?: string[];
}

export interface CraftCatalogResult {
  kind: string;
  formats?: CatalogEntry[];
  personas?: CatalogEntry[];
  arcs?: CatalogEntry[];
  pricing?: Record<string, unknown>;
  counts: { formats: number; personas: number; arcs: number };
  offline: boolean;
}

export function craftCatalog(args: { kind?: string } = {}) {
  return invoke<CraftCatalogResult>('craft_catalog', args);
}

export function craftApply(args: {
  book_id: number;
  format_id?: string;
  plot_arc_id?: string;
  style_persona_id?: string;
  adopt_defaults?: boolean;
}) {
  return invoke<{
    book_id: number;
    format: CatalogEntry;
    plot_arc: CatalogEntry;
    style_persona: CatalogEntry;
    legacy_sync: Record<string, unknown>;
  }>('craft_apply', args);
}

export function craftScaffold(args: {
  book_id: number;
  plot_arc_id?: string;
  chapter_count?: number;
  start_chapter?: number;
  per_chapter_words?: number;
  overwrite_synopsis?: boolean;
}) {
  return invoke<{
    book_id: number;
    plot_arc: string;
    arc_label: string;
    chapters_total: number;
    created: number;
    mapped: number;
    beats_total: number;
    plan: Array<{ chapter_number: number; beat_id: string; beat_label: string; created: boolean }>;
  }>('craft_scaffold', args);
}

export interface CraftProgressResult {
  book_id: number;
  format_id: string;
  plot_arc_id: string;
  arc_label: string | null;
  style_persona_id: string;
  total_beats: number;
  covered_beats: number;
  coverage_pct: number;
  beats: Array<{
    beat_index: number;
    beat_id: string;
    beat_label: string;
    guidance: string;
    chapters: Array<{ chapter_id: number; chapter_number: number; title: string; status: string; actual_word_count: number }>;
    started: boolean;
  }>;
  chapters: number;
  total_words: number;
  target_words: number;
  unresolved_threads: number;
}

export function craftProgress(args: { book_id: number }) {
  return invoke<CraftProgressResult>('craft_progress', args);
}

export interface KdpCheckResult {
  book_id: number;
  pass: boolean;
  readiness_score: number;
  checks: Record<string, Record<string, unknown>>;
  issues: Array<{ severity: 'FAIL' | 'WARN' | 'INFO'; area: string; message: string; fix?: string }>;
  pricing_hint: Record<string, Record<string, unknown>>;
  est_pages: number;
}

export function kdpCheck(args: { book_id: number; price_usd?: number }) {
  return invoke<KdpCheckResult>('kdp_check', args);
}

export function kdpPackage(args: {
  book_id: number;
  price_usd?: number;
  keywords?: string[];
  categories?: string[];
  description?: string;
  subtitle?: string;
  language?: string;
  isbn?: string;
  publisher?: string;
  edition?: string;
  ai_disclosure?: boolean;
  drm?: boolean;
}) {
  return invoke<{
    book_id: number;
    package_dir: string;
    files: Array<{ name: string; bytes: number }>;
    royalties: Record<string, Record<string, unknown>>;
    issues: Array<{ severity: string; message: string }>;
    est_pages: number;
  }>('kdp_package', args);
}

// ── v1.4 — retention & serialization kit (SFB-2.0) ──────────────────────────

export interface RetentionQc {
  scores: number[];
  total: number;
  max: number;
  band: 'STRONG' | 'EXAMINE' | 'STRUCTURAL_REVISION' | 'RECONSIDER';
  verdict: string;
  zero_flags: string[];
  revision_required: boolean;
}

export interface RetentionPhase {
  chapter_number: number;
  cycle: number;
  phase_index: number;
  phase_id: 'setup' | 'escalation' | 'reversal_pivot' | 'climax_resolution';
  phase_label: string;
  phase_chapters: string;
  step_in_phase: number;
  phase_span: number;
  goals: string[];
}

export interface RetentionEndingSuggestion {
  id: string;
  label: string;
  example: string;
  when: string;
}

export function craftRetentionCheck(args: {
  book_id: number;
  chapter_number: number;
  scores: number[];
  ending_type?: string;
  notes?: string;
}) {
  return invoke<{
    book_id: number;
    chapter_number: number;
    chapter_id: number;
    qc: RetentionQc;
    ending_type: string;
    next_chapter: {
      number: number;
      phase: RetentionPhase;
      suggested_endings: RetentionEndingSuggestion[];
    };
    blueprint: string;
  }>('craft_retention_check', args);
}

export function craftRetentionReport(args: {
  book_id: number;
  chapters_per_week?: number;
}) {
  return invoke<{
    book_id: number;
    chapters_per_week: number;
    chapters: Array<{
      chapter_number: number;
      title: string;
      status: string;
      word_count: number;
      word_length_guidance: 'unwritten' | 'short' | 'in_range' | 'long';
      phase: RetentionPhase;
      qc?: {
        total: number;
        band: string;
        revision_required: boolean;
        ending_type?: string;
        notes?: string;
      };
    }>;
    cadence: {
      reserve_chapters: number;
      published_chapters: number;
      planned_chapters: number;
      buffer_weeks: number;
      buffer_healthy: boolean;
    };
    summary: {
      chapters_total: number;
      qc_scored: number;
      bands: Record<string, number>;
      revision_required: number;
      endings_used: Record<string, number>;
      recent_endings: string[];
      ending_repetition_in_window: string[];
      next_phase: RetentionPhase;
    };
    blueprint: string;
  }>('craft_retention_report', args);
}

export function craftRetentionWorksheet(args: {
  book_id: number;
  chapter_number: number;
}) {
  return invoke<{
    book_id: number;
    chapter_number: number;
    chapter_exists: boolean;
    chapter?: { title: string; target_word_count: number; actual_word_count: number; status: string };
    phase: RetentionPhase;
    arc?: { id: string; label: string };
    beat_stamp?: { beat_id: string; beat_label: string };
    previous_qc?: { total: number; band: string; revision_required: boolean };
    ending_rotation: {
      recent: string[];
      suggested: RetentionEndingSuggestion[];
      blocked_window: number;
    };
    sections: Array<{ letter: string; id: string; title: string; prompts: string[]; context: string }>;
    qc_criteria: string[];
    blueprint: string;
  }>('craft_retention_worksheet', args);
}

export function localLlmGenerate(args: {
  prompt: string;
  max_tokens?: number;
  temperature?: number;
  seed?: number;
  style_persona?: string;
}) {
  return invoke<{ text: string; backend: string; style_persona?: string }>(
    'local_llm_generate',
    args,
  );
}

// ── 4.16 Video novel studio (v1.5) ───────────────────────────────────────────

export interface VideoScene {
  scene_index: number;
  segment_id: string;
  sequence_order: number;
  speaker_id: string;
  text_chars: number;
  duration_ms: number;
  characters: string[];
  focal: 'center' | 'left' | 'right' | 'top' | 'bottom';
  narration_path: string | null;
  ladder: 'REFS_READY' | 'PARTIAL' | 'TEXT_ONLY';
}

export function videoPlan(args: { chapter_id: number }) {
  return invoke<{
    chapter_id: number;
    book_id: number;
    chapter_number: number;
    derived: { refreshed: boolean; durations_filled: number; source: string };
    scenes: VideoScene[];
    scene_count: number;
    total_ms: number;
    title_ms: number;
    fps: number;
    resolution: string;
    ladder: { refs_ready: number; partial: number; text_only: number };
    missing_sheets: string[];
    narration_ready: boolean;
  }>('video_plan', args);
}

export interface VideoAsset {
  kind: 'TITLE' | 'SHEET' | 'SCENE';
  ref: string;
  scene_index?: number;
  path: string;
  sha256: string;
  width: number;
  height: number;
  bytes: number;
  engine: string;
  characters?: string[];
}

export function videoRender(args: {
  chapter_id: number;
  engine?: 'SCRIPTED' | 'R2I_ENDPOINT';
  scene_index?: number;
  sheets_only?: boolean;
  characters?: string[];
  focal?: Record<string, string>;
}) {
  return invoke<{
    chapter_id: number;
    book_id: number;
    chapter_number: number;
    engine: string;
    rendered: VideoAsset[];
    reused: number;
    assets?: VideoAsset[];
  }>('video_render', args);
}

export function videoAssemble(args: {
  chapter_id: number;
  narration?: string;
  srt?: string;
  fps?: number;
  ffmpeg_bin?: string;
}) {
  return invoke<{
    out: string;
    bytes: number;
    scenes: number;
    clips: number;
    duration_ms: number;
    fps: number;
    resolution: string;
    narration: string;
    srt: string;
    codec: string;
    ffmpeg: string;
  }>('video_assemble', args);
}

export function videoValidate(args: { chapter_id: number; ffmpeg_bin?: string }) {
  return invoke<{
    ok: boolean;
    checks: Array<{ name: string; status: 'PASS' | 'WARN' | 'FAIL'; detail: string }>;
    plan: {
      chapter_id: number;
      chapter_number: number;
      book_id: number;
      scenes: number;
      total_ms: number;
      resolution: string;
    };
  }>('video_validate', args);
}

// ── 4.17 Provider settings ladder (v1.6, ADR-0012) ───────────────────────────

export interface ProviderGetResult {
  image: {
    settings: {
      endpoint_url: string | null;
      model: string | null;
      timeout_s: number | null;
      retries: number | null;
    };
    resolved: {
      endpoint_url: string | null;
      endpoint_source: 'settings' | 'env' | 'none';
      model: string | null;
      model_source: 'settings' | 'env' | 'none';
      timeout_s: number;
      retries: number;
      key_configured: boolean;
      key_source: 'env' | 'keychain' | 'none';
    };
  };
  llm: {
    settings: { server_url: string | null };
    resolved: {
      server_url: string | null;
      source: 'settings' | 'env' | 'offline';
    };
  };
  ladder: string;
}

export interface ProviderTestResult {
  ok: boolean;
  latency_ms: number;
  bytes: number;
  ext: string;
  size: string;
  model: string | null;
  endpoint_source: 'args' | 'settings' | 'env';
  key_source: 'env' | 'keychain' | 'none';
  retries: number;
  note: string;
}

export function providerGet() {
  return invoke<ProviderGetResult>('provider_get', {});
}

export function providerSet(args: {
  endpoint_url?: string | null;
  model?: string | null;
  timeout_s?: number | null;
  retries?: number | null;
  server_url?: string | null;
}) {
  return invoke<ProviderGetResult & { applied: string[] }>('provider_set', args);
}

export function providerTest(args: {
  endpoint_url?: string;
  model?: string;
  size?: string;
  timeout_s?: number;
  retries?: number;
}) {
  return invoke<ProviderTestResult>('provider_test', args);
}

// Convenience re-export of the segment AST type for API consumers.
export type { NarrativeSegmentAst };
