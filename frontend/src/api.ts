/**
 * Frozen IPC envelope contract + command catalog.
 *
 * The wire format is byte-identical to the CLI (`docs/CLI_CONTRACT.md`):
 *   success: { ok: true,  data }
 *   failure: { ok: false, error: { code, message, category } }
 *
 * `IPC_COMMANDS` mirrors the match arms of `ipc::dispatch` in
 * `src-tauri/src/ipc/mod.rs` (enforced by tests/tier5_desktop_shell).
 */

export type Envelope = {
  ok: true;
  data: any;
} | {
  ok: false;
  error: { code: string; message: string; category: string };
};

export const isOk = (e: Envelope): e is Extract<Envelope, { ok: true }> => e.ok;

/** The 38 IPC dispatch commands (36 frozen Phase 1 + `project_git_log`
 * + `segment_reorder`, the Phase 3 AST-bridge re-index command). */
export const IPC_COMMANDS = [
  // Project lifecycle
  'project_info',
  'project_mount',
  'project_unmount',
  'project_export',
  'project_git_log',
  // Books
  'book_create',
  'book_get',
  'book_list',
  'book_update',
  'book_delete',
  // Chapters
  'chapter_create',
  'chapter_get',
  'chapter_list',
  'chapter_update',
  'chapter_delete',
  // Characters
  'character_create',
  'character_get',
  'character_list',
  'character_update',
  'character_delete',
  // Lorebook
  'lore_create',
  'lore_get',
  'lore_list',
  'lore_update',
  'lore_delete',
  // Chekhov threads
  'thread_create',
  'thread_get',
  'thread_list',
  'thread_update',
  'thread_delete',
  // Segments
  'segment_create',
  'segment_get',
  'segment_list',
  'segment_update',
  'segment_delete',
  'segment_reorder',
  // Search & security
  'search_segments',
  'sandbox_resolve',
] as const;

export type IpcCommand = (typeof IPC_COMMANDS)[number];

/** Desktop session commands handled by the Tauri shell itself. */
export const DESKTOP_COMMANDS = [
  'desktop_mount',
  'desktop_unmount',
  'desktop_status',
  'desktop_info',
] as const;

/** Shell event channels (mirrors `tauri_shell::mod` constants). */
export const EVT_COMMAND = 'authr://command';
export const EVT_GIT = 'authr://git';
export const EVT_LEASE = 'authr://lease';

export type AppEvent = { event: string; payload: any };

/** Convenience wrappers used by the panels. */
export const api = {
  desktopInfo: () => ({ cmd: 'desktop_info' as const, payload: {} }),
  desktopStatus: () => ({ cmd: 'desktop_status' as const, payload: {} }),
};

/** Enum value domains (mirror the SQLite CHECK constraints / Rust DTOs). */
export const ENUMS = {
  narrative_structure: [
    'three_act', 'save_the_cat', 'fichtean_curve', 'heros_journey',
    'kishotenketsu', 'seven_point', 'non_linear',
  ],
  book_status: [
    'draft', 'outlining', 'drafting', 'revising', 'polishing',
    'published', 'archived',
  ],
  chapter_status: ['outline', 'draft', 'revised', 'final'],
  character_role: [
    'protagonist', 'antagonist', 'secondary', 'supporting',
    'narrator', 'minor',
  ],
  voice_engine: [
    'kokoro', 'edge_tts', 'kitten_tts', 'hf_tts', 'elevenlabs',
    'windows_sapi', 'macos_avspeech', 'none',
  ],
  lore_category: [
    'location', 'faction', 'character', 'magic_system', 'technology',
    'item', 'event', 'rule', 'custom',
  ],
  thread_status: ['planted', 'active', 'paid_off', 'abandoned'],
  // UPPERCASE domain per models/ast.rs SegmentType
  segment_type: ['NARRATION', 'DIALOGUE', 'INTERNAL_MONOLOGUE'],
} as const;
