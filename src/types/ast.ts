// AUTHR_OS — Frozen IPC contract types (docs/CLI_CONTRACT.md §4/§5).
// All row shapes are snake_case exactly as SQLite returns them.

export type SegmentType = 'NARRATION' | 'DIALOGUE' | 'INTERNAL_MONOLOGUE';

/**
 * NarrativeSegmentAst (§5.2 of the build plan / contract §4.8).
 * The editor bridge syncs arrays of these objects.
 */
export interface NarrativeSegmentAst {
  segment_id: string;
  chapter_id: number;
  sequence_order: number;
  segment_type: SegmentType;
  speaker_id: string;
  emotion: string;
  prosody_rate: number;
  prosody_pitch: number;
  text_content: string;
  ambient_cue: string | null;
  sfx_cue: string | null;
  prev_request_id: string | null;
  audio_hash: string | null;
  audio_start_ms: number;
  audio_duration_ms: number;
}

/** Payload accepted by `sync_lexical_ast_mutation` (contract §4.8). */
export interface AstMutationPayload {
  chapter_id: number;
  upserts: NarrativeSegmentAst[];
  deletes: string[];
}

/** `book` row (contract §5). */
export interface BookRow {
  id: number;
  title: string;
  author: string;
  genre: string;
  premise: string;
  theme: string;
  narrative_structure: string;
  target_word_count: number;
  target_chapters: number;
  persona_id: string;
  cover_image_path: string;
  cover_prompt: string;
  marketing_copy: string;
  isbn: string;
  readability_score: number;
  created_at: number;
  last_modified_at: number;
}

/** `chapter` row (contract §5). */
export interface ChapterRow {
  id: number;
  book_id: number;
  act_number: number;
  chapter_number: number;
  title: string;
  synopsis: string;
  target_word_count: number;
  actual_word_count: number;
  status: ChapterStatus;
  created_at: number;
  last_modified_at: number;
}

export type ChapterStatus =
  | 'PLANNED'
  | 'DRAFTING'
  | 'POLISHED'
  | 'AUDIO_READY'
  | 'PUBLISHED';

/** `character` row (contract §5, character_profiles). */
export interface CharacterRow {
  character_key: string;
  display_name: string;
  role: string;
  bio: string;
  ipa_phonetic: string;
  voice_engine: string;
  voice_id: string;
  base_rate: number;
  base_pitch: number;
  cadence_notes: string;
  created_at: number;
}

/** `lore` row (contract §5). */
export interface LoreRow {
  id: number;
  key: string;
  category: string;
  content: string;
  ipa_phonetic: string;
  created_at: number;
  last_modified_at: number;
}

/** `thread` row (contract §5, unresolved_threads). */
export interface ThreadRow {
  id: number;
  thread_title: string;
  thread_type: string;
  introduced_in_chapter: number;
  resolved_in_chapter: number | null;
  status: ThreadStatus;
  payoff_requirement: string;
  created_at: number;
  last_modified_at: number;
}

export type ThreadStatus = 'INTRODUCED' | 'ESCALATED' | 'RESOLVED';

/** `segment` row (contract §5) — the full SQLite shape incl. audit columns. */
export interface SegmentRow {
  segment_id: string;
  chapter_id: number;
  sequence_order: number;
  segment_type: SegmentType;
  speaker_id: string;
  emotion: string;
  prosody_rate: number;
  prosody_pitch: number;
  text_content: string;
  ambient_cue: string | null;
  sfx_cue: string | null;
  prev_request_id: string | null;
  audio_hash: string | null;
  audio_cache_path: string | null;
  audio_start_ms: number;
  audio_duration_ms: number;
  created_at: number;
  last_modified_at: number;
}

/** `manifest.json` v1 (contract §6). */
export interface BundleManifest {
  schema_version: number;
  project_id: string;
  title: string;
  author: string;
  created_at: number;
  last_modified_at: number;
  authr_version: string;
}

/** `project_lock_status` result (contract §4.1 #4). */
export interface LockStatus {
  locked: boolean;
  pid?: number;
  hostname?: string;
  user?: string;
  locked_at?: string;
  lease_expires_at?: string;
  session_id?: string;
  expired?: boolean;
  readonly: boolean;
}

/** `segment_search` hit row (contract §4.7 #27). */
export interface SegmentSearchHit {
  segment_id: string;
  chapter_id: number;
  text_content: string;
  rank: number;
}

/** Map a full SegmentRow to the editor-facing NarrativeSegmentAst shape. */
export function rowToAst(row: SegmentRow): NarrativeSegmentAst {
  return {
    segment_id: row.segment_id,
    chapter_id: row.chapter_id,
    sequence_order: row.sequence_order,
    segment_type: row.segment_type,
    speaker_id: row.speaker_id,
    emotion: row.emotion,
    prosody_rate: row.prosody_rate,
    prosody_pitch: row.prosody_pitch,
    text_content: row.text_content,
    ambient_cue: row.ambient_cue,
    sfx_cue: row.sfx_cue,
    prev_request_id: row.prev_request_id,
    audio_hash: row.audio_hash,
    audio_start_ms: row.audio_start_ms,
    audio_duration_ms: row.audio_duration_ms,
  };
}
