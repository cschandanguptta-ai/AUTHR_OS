/**
 * In-memory mock bundle for browser preview / CI screenshots.
 *
 * Implements the full 37-command IPC surface with the frozen envelope
 * semantics (validation errors, ENTITY_NOT_FOUND, read-only rejection) so
 * the console is fully explorable without the Rust shell. Every command
 * name in `IPC_COMMANDS` is handled here — enforced by tier5 parity tests.
 */

import type { AppEvent, Envelope } from './api';
import type { Transport } from './transport';

const ok = (data: unknown): Envelope => ({ ok: true, data: data } as Envelope);
const err = (code: string, message: string, category = 'storage'): Envelope =>
  ({ ok: false, error: { code, message, category } } as Envelope);

type Row = Record<string, any> & { id: number };

interface MockState {
  mounted: { root: string; session_id: string; lease_secs: number } | null;
  manifest: Record<string, any>;
  books: Row[];
  chapters: Row[];
  characters: Row[];
  lore: Row[];
  threads: Row[];
  segments: Row[];
  commits: { id: string; message: string; timestamp: number }[];
  seq: number;
  readonly: boolean;
}

function nowIso(): string {
  return new Date().toISOString().replace(/\.\d+Z$/, 'Z');
}

function seed(): MockState {
  const t0 = Date.parse('2026-11-01T08:00:00Z') / 1000;
  const commit = (n: number, message: string): { id: string; message: string; timestamp: number } =>
    ({ id: `f1xture00000000000000000000000000000000${n}`.slice(0, 40), message, timestamp: t0 + n * 600 });
  return {
    mounted: null,
    manifest: {
      schema_version: 1,
      title: 'The Lantern Marshes',
      author: 'A. Calloway',
      created_at: '2026-11-01T08:00:00Z',
      last_modified_at: '2026-11-05T17:12:00Z',
      authr_version: '3.1.0-PROD',
    },
    books: [
      {
        id: 1, title: 'The Lantern Marshes', author: 'A. Calloway', genre: 'gothic mystery',
        narrative_structure: 'three_act', status: 'drafting', target_word_count: 95000,
        created_at: '2026-11-01T08:00:00Z', last_modified_at: '2026-11-05T17:12:00Z',
      },
      {
        id: 2, title: 'Signal Fires', author: 'A. Calloway', genre: 'literary sf',
        narrative_structure: 'kishotenketsu', status: 'outlining', target_word_count: 78000,
        created_at: '2026-11-02T09:30:00Z', last_modified_at: '2026-11-04T11:00:00Z',
      },
    ],
    chapters: [
      { id: 1, book_id: 1, chapter_index: 1, title: 'The Ferryman\u2019s Toll', summary: 'Mira crosses at dusk; the lanterns answer.', status: 'revised', created_at: '2026-11-01T08:05:00Z', last_modified_at: '2026-11-05T17:12:00Z' },
      { id: 2, book_id: 1, chapter_index: 2, title: 'Salt and Ash', summary: 'The marsh gives back a name it should not know.', status: 'draft', created_at: '2026-11-01T09:00:00Z', last_modified_at: '2026-11-05T12:00:00Z' },
      { id: 3, book_id: 1, chapter_index: 3, title: 'The Drowned Bell', summary: '', status: 'outline', created_at: '2026-11-02T10:00:00Z', last_modified_at: '2026-11-03T09:00:00Z' },
    ],
    characters: [
      { id: 1, book_id: 1, character_key: 'MIRA', display_name: 'Mira Vance', role: 'protagonist', voice_engine: 'kokoro', voice_config: '{"voice":"af_heart"}', ipa_phonetic: '\u02c8m\u026a\u0259r\u0259', created_at: '2026-11-01T08:10:00Z', last_modified_at: '2026-11-04T10:00:00Z' },
      { id: 2, book_id: 1, character_key: 'NARRATOR', display_name: 'Narrator', role: 'narrator', voice_engine: 'kokoro', voice_config: '{}', ipa_phonetic: null, created_at: '2026-11-01T08:00:00Z', last_modified_at: '2026-11-01T08:00:00Z' },
      { id: 3, book_id: 1, character_key: 'FERRYMAN', display_name: 'The Ferryman', role: 'secondary', voice_engine: 'edge_tts', voice_config: '{"rate":"-10%"}', ipa_phonetic: null, created_at: '2026-11-01T08:20:00Z', last_modified_at: '2026-11-04T10:00:00Z' },
    ],
    lore: [
      { id: 1, book_id: 1, category: 'location', term: 'Lantern Marsh', content: 'Tidal wetland ringed with votive lanterns; passage is refused after dark.', ipa_phonetic: null, created_at: '2026-11-01T08:30:00Z', last_modified_at: '2026-11-01T08:30:00Z' },
      { id: 2, book_id: 1, category: 'rule', term: 'The Toll', content: 'Every crossing demands a spoken memory; refused tolls are collected later.', ipa_phonetic: null, created_at: '2026-11-01T08:31:00Z', last_modified_at: '2026-11-01T08:31:00Z' },
    ],
    threads: [
      { id: 1, book_id: 1, chapter_id: 1, title: 'Why the ferryman knows Mira\u2019s mother', status: 'active', planted_chapter_index: 1, payoff_chapter_index: null, notes: 'Seed doubt in ch 2; payoff in act III.', created_at: '2026-11-01T08:40:00Z', last_modified_at: '2026-11-05T09:00:00Z' },
      { id: 2, book_id: 1, chapter_id: 2, title: 'The bell that rings under water', status: 'planted', planted_chapter_index: 2, payoff_chapter_index: null, notes: '', created_at: '2026-11-02T08:00:00Z', last_modified_at: '2026-11-02T08:00:00Z' },
    ],
    segments: [
      { id: 1, segment_id: 'ch01-0001', chapter_id: 1, speaker_id: 'NARRATOR', segment_type: 'NARRATION', text_content: 'The ferryman counted no coins; he counted the things you left unsaid.', emotion: 'neutral', prosody_rate: 1.0, prosody_pitch: 1.0, ambient_cue: 'water_lapping', sfx_cue: null, audio_hash: null, sequence_order: 1 },
      { id: 2, segment_id: 'ch01-0002', chapter_id: 1, speaker_id: 'FERRYMAN', segment_type: 'DIALOGUE', text_content: 'State your toll, traveler. A memory will do.', emotion: 'grim', prosody_rate: 0.95, prosody_pitch: 0.9, ambient_cue: null, sfx_cue: 'oar_creak', audio_hash: null, sequence_order: 2 },
      { id: 3, segment_id: 'ch02-0001', chapter_id: 2, speaker_id: 'MIRA', segment_type: 'DIALOGUE', text_content: 'The marsh has no business knowing my mother\u2019s name.', emotion: 'angry', prosody_rate: 1.05, prosody_pitch: 1.1, ambient_cue: 'rain', sfx_cue: null, audio_hash: null, sequence_order: 1 },
    ],
    commits: [
      commit(4, 'chore(manuscript): revise chapter 02'),
      commit(3, 'chore(manuscript): draft chapter 02'),
      commit(2, 'feat(cast): add FERRYMAN profile'),
      commit(1, 'chore(bundle): initial scaffold'),
    ],
    seq: 100,
    readonly: false,
  };
}

function nextCommit(state: MockState, message: string): void {
  state.commits.unshift({
    id: `mock${Date.now().toString(16)}${Math.floor(Math.random() * 1e6).toString(16)}`.padEnd(40, '0').slice(0, 40),
    message,
    timestamp: Math.floor(Date.now() / 1000),
  });
}

function requireMount(state: MockState): Envelope | null {
  if (!state.mounted) {
    return err('BUNDLE_ERROR', 'desktop session not mounted; call desktop_mount first', 'bundle');
  }
  if (state.readonly) {
    return err('READ_ONLY_PROJECT', 'project is read-only: active lease held by another host', 'bundle');
  }
  return null;
}

function requireFields(p: Record<string, any>, names: string[]): Envelope | null {
  for (const n of names) {
    const v = p[n];
    if (typeof v === 'string' ? v.trim() === '' : v === undefined) {
      return err('VALIDATION_ERROR', `invalid payload: missing field \`${n}\``, 'security');
    }
  }
  return null;
}

function findOr404(rows: Row[], id: number, entity: string): Envelope | Row {
  const row = rows.find((r) => r.id === id);
  if (!row) return err('ENTITY_NOT_FOUND', `entity not found: ${entity} id ${id}`, 'storage');
  return row;
}

const isErr = (x: unknown): x is Envelope =>
  typeof x === 'object' && x !== null && (x as any).ok === false;

function touch(row: Row): void {
  row.last_modified_at = nowIso();
}

export class MockTransport implements Transport {
  readonly kind = 'mock' as const;
  private state: MockState = seed();
  private listeners = new Set<(evt: AppEvent) => void>();

  onEvent(cb: (evt: AppEvent) => void): () => void {
    this.listeners.add(cb);
    return () => this.listeners.delete(cb);
  }

  private emit(event: string, payload: unknown): void {
    for (const cb of this.listeners) cb({ event, payload });
  }

  private commitIfMutating(cmd: string): void {
    const messages: Record<string, string> = {
      book_create: 'feat(book): add book',
      book_update: 'chore(manuscript): refresh metadata',
      book_delete: 'chore(manuscript): remove book',
      chapter_create: 'chore(manuscript): add chapter',
      chapter_update: 'chore(manuscript): revise chapter',
      chapter_delete: 'chore(manuscript): remove chapter',
      character_create: 'feat(cast): add cast member',
      character_update: 'chore(manuscript): refresh cast',
      character_delete: 'chore(manuscript): remove cast member',
      segment_create: 'chore(manuscript): draft chapter',
      segment_update: 'chore(manuscript): revise chapter',
      segment_delete: 'chore(manuscript): trim chapter',
      segment_reorder: 'chore(manuscript): reorder chapter',
      lore_create: 'feat(lore): add lore entry',
      lore_update: 'feat(lore): revise lore entry',
      lore_delete: 'feat(lore): remove lore entry',
      thread_create: 'feat(threads): plant thread',
      thread_update: 'feat(threads): revise thread',
      thread_delete: 'feat(threads): remove thread',
      project_mount: 'chore(bundle): mount session',
      project_unmount: 'chore(bundle): unmount session',
    };
    const message = messages[cmd];
    if (message) {
      nextCommit(this.state, message);
      this.emit('authr://git', { latest: this.state.commits[0] });
    }
  }

  async invoke(cmd: string, payload: unknown = {}): Promise<Envelope> {
    await new Promise((r) => setTimeout(r, 25 + Math.random() * 40));
    const p = (payload ?? {}) as Record<string, any>;
    const s = this.state;
    let envelope: Envelope;

    try {
      envelope = this.handle(cmd, p, s);
    } catch (e: any) {
      envelope = err('STORAGE_ERROR', `mock error: ${e?.message ?? e}`);
    }
    this.emit('authr://command', { command: cmd, ok: envelope.ok, duration_ms: 30 });
    if (envelope.ok) this.commitIfMutating(cmd);
    return envelope;
  }

  private handle(cmd: string, p: Record<string, any>, s: MockState): Envelope {
    // ---------- desktop session ----------
    if (cmd === 'desktop_mount') {
      const root = String(p.root ?? '').trim();
      if (!root) return err('VALIDATION_ERROR', 'invalid payload: root must not be empty', 'security');
      s.mounted = { root, session_id: `mock-${Math.random().toString(16).slice(2, 10)}`, lease_secs: Number(p.lease_secs ?? 120) };
      s.readonly = false;
      return ok({ root, session_id: s.mounted.session_id, read_only: false });
    }
    if (cmd === 'desktop_unmount') {
      const was = s.mounted !== null;
      s.mounted = null;
      return ok({ unmounted: was, reason: was ? undefined : 'not mounted' });
    }
    if (cmd === 'desktop_status') {
      if (!s.mounted) return ok({ mounted: false });
      return ok({
        mounted: true, root: s.mounted.root, session_id: s.mounted.session_id,
        lease_secs: s.mounted.lease_secs, lease: { expires_at: new Date(Date.now() + s.mounted.lease_secs * 1000).toISOString() },
      });
    }
    if (cmd === 'desktop_info') {
      return ok({ version: '3.1.0-PROD', keychain: 'mock', platform: 'browser', mounted: s.mounted !== null });
    }

    // ---------- project ----------
    if (cmd === 'project_info') {
      if (!s.mounted) return err('BUNDLE_ERROR', 'desktop session not mounted; call desktop_mount first', 'bundle');
      return ok({
        manifest: s.manifest,
        locked: s.mounted !== null,
        lock: s.mounted && {
          pid: 4242, hostname: 'mock-host', user: 'author',
          locked_at: nowIso(), lease_expires_at: new Date(Date.now() + 60000).toISOString(),
          session_id: s.mounted.session_id, expired: false,
        },
        read_only: false,
      });
    }
    if (cmd === 'project_mount') {
      if (!s.mounted) return err('BUNDLE_ERROR', 'desktop session not mounted; call desktop_mount first', 'bundle');
      return ok({ root: s.mounted.root, session_id: s.mounted.session_id, read_only: false });
    }
    if (cmd === 'project_unmount') {
      return err('VALIDATION_ERROR', 'invalid payload for \'project_unmount\': desktop sessions unmount via desktop_unmount', 'security');
    }
    if (cmd === 'project_export') {
      const guard = requireMount(s); if (guard) return guard;
      return ok({ path: p.out ?? 'exports/author_bundle.zip', books: s.books.length, chapters: s.chapters.length, segments: s.segments.length, excluded: ['project.db', '.authr_lock'] });
    }
    if (cmd === 'project_git_log') {
      if (!s.mounted) return err('BUNDLE_ERROR', 'desktop session not mounted; call desktop_mount first', 'bundle');
      return ok({ commits: s.commits.slice(0, Math.min(p.limit ?? 20, 200)) });
    }

    // ---------- generic entity CRUD ----------
    return this.entityCommand(cmd, p, s);
  }

  private entityCommand(cmd: string, p: Record<string, any>, s: MockState): Envelope {
    const map: Record<string, { rows: keyof MockState; label: string; parent?: 'book_id' | 'chapter_id'; required?: string[] }> = {
      book: { rows: 'books', label: 'book', required: ['title'] },
      chapter: { rows: 'chapters', label: 'chapter', parent: 'book_id' },
      character: { rows: 'characters', label: 'character', parent: 'book_id', required: ['character_key', 'display_name'] },
      lore: { rows: 'lore', label: 'lore entry', parent: 'book_id', required: ['term'] },
      thread: { rows: 'threads', label: 'thread', parent: 'book_id', required: ['title'] },
      segment: { rows: 'segments', label: 'segment', parent: 'chapter_id', required: ['text_content'] },
    };
    const m = /^(book|chapter|character|lore|thread|segment)_(create|get|list|update|delete)$/.exec(cmd);
    if (!m) return this.otherCommand(cmd, p, s);
    const [, entity, action] = m;
    const spec = map[entity];

    if (action === 'list') {
      if (!s.mounted) return err('BUNDLE_ERROR', 'desktop session not mounted; call desktop_mount first', 'bundle');
      const rows = s[spec.rows] as Row[];
      if (spec.parent) {
        const pf: string = spec.parent;
        return ok(rows.filter((r) => r[pf] === p[pf]));
      }
      return ok(rows);
    }
    if (action === 'get') {
      const guard = requireFields(p, ['id']); if (guard) return guard;
      const found = findOr404(s[spec.rows] as Row[], Number(p.id), spec.label);
      return isErr(found) ? found : ok(found);
    }
    if (action === 'create') {
      const guard = requireMount(s); if (guard) return guard;
      const fguard = requireFields(p, spec.required ?? []); if (fguard) return fguard;
      if (spec.parent) {
        const parentRows = entity === 'segment' ? 'chapters' : 'books';
        const parent = findOr404(s[parentRows] as Row[], Number(p[spec.parent!]), spec.parent);
        if (isErr(parent)) return parent;
      }
      const row: Row = { id: s.seq++, ...p };
      touch(row);
      if (entity === 'segment') {
        row.segment_id = `ch${String(row.chapter_id).padStart(2, '0')}-${String(row.id).padStart(4, '0')}`;
        row.speaker_id = row.speaker_id ?? 'NARRATOR';
        row.segment_type = row.segment_type ?? 'NARRATION';
        row.emotion = row.emotion ?? 'neutral';
        row.prosody_rate = row.prosody_rate ?? 1.0;
        row.prosody_pitch = row.prosody_pitch ?? 1.0;
      }
      if (entity === 'chapter' && row.chapter_index === undefined) {
        const siblings = (s.chapters as Row[]).filter((c) => c.book_id === row.book_id);
        row.chapter_index = siblings.length + 1;
      }
      (s[spec.rows] as Row[]).push(row);
      return ok(row);
    }
    if (action === 'update') {
      const guard = requireMount(s); if (guard) return guard;
      const idguard = requireFields(p, ['id']); if (idguard) return idguard;
      const rows = s[spec.rows] as Row[];
      const row = findOr404(rows, Number(p.id), spec.label);
      if (isErr(row)) return row;
      const target = row as Row;
      for (const [k, v] of Object.entries(p)) {
        if (k !== 'id' && v !== undefined) target[k] = v;
      }
      touch(target);
      return ok(target);
    }
    // delete
    {
      const guard = requireMount(s); if (guard) return guard;
      const idguard = requireFields(p, ['id']); if (idguard) return idguard;
      const rows = s[spec.rows] as Row[];
      const row = findOr404(rows, Number(p.id), spec.label);
      if (isErr(row)) return row;
      const target = row as Row;
      const idx = rows.indexOf(target);
      rows.splice(idx, 1);
      if (entity === 'book') {
        (s.chapters as Row[]).filter((c) => c.book_id === target.id)
          .forEach((c) => (s.chapters as Row[]).splice((s.chapters as Row[]).indexOf(c), 1));
      }
      return ok({ deleted: target.id });
    }
  }

  private otherCommand(cmd: string, p: Record<string, any>, s: MockState): Envelope {
    if (cmd === 'search_segments') {
      if (!s.mounted) return err('BUNDLE_ERROR', 'desktop session not mounted; call desktop_mount first', 'bundle');
      const query = String(p.query ?? '').trim();
      if (!query) return err('VALIDATION_ERROR', 'query must not be empty', 'security');
      const limit = Math.min(Number(p.limit ?? 50), 500);
      const results = (s.segments as Row[])
        .filter((seg) => seg.text_content.toLowerCase().includes(query.toLowerCase()))
        .slice(0, limit)
        .map((seg, i) => ({
          segment_id: seg.segment_id, chapter_id: seg.chapter_id,
          text_content: seg.text_content, rank: -(1 + i) * 0.5,
        }));
      return ok({ query, results });
    }
    if (cmd === 'sandbox_resolve') {
      const guard = requireMount(s); if (guard) return guard;
      const path = String(p.path ?? '');
      if (path.includes('..') || path.startsWith('/') || path.includes('\\')) {
        return err('PATH_TRAVERSAL_VIOLATION', `path escapes bundle root: ${path}`, 'security');
      }
      return ok({ resolved: `${s.mounted!.root}/${path}`, within_root: true });
    }
    if (cmd === 'segment_reorder') {
      const guard = requireMount(s); if (guard) return guard;
      const chapterId = Number(p.chapter_id);
      const order: number[] = Array.isArray(p.order) ? p.order.map(Number) : [];
      const rows = (s.segments as Row[]).filter((r) => r.chapter_id === chapterId);
      if (!rows.length && !order.length) {
        return err('ENTITY_NOT_FOUND', `entity not found: chapter id ${chapterId}`, 'storage');
      }
      const known = new Set(rows.map((r) => r.id));
      const seen = new Set<number>();
      for (const id of order) {
        if (!known.has(id)) {
          return err('VALIDATION_ERROR', `segment ${id} does not belong to chapter ${chapterId}`, 'security');
        }
        if (seen.has(id)) {
          return err('VALIDATION_ERROR', `duplicate segment id in reorder payload: ${id}`, 'security');
        }
        seen.add(id);
      }
      if (order.length !== rows.length) {
        return err('VALIDATION_ERROR', `reorder payload lists ${order.length} of ${rows.length} segments in chapter ${chapterId}`, 'security');
      }
      order.forEach((id, idx) => {
        const row = rows.find((r) => r.id === id)!;
        row.sequence_order = idx + 1;
      });
      return ok({ chapter_id: chapterId, reordered: order.length });
    }
    // Unknown command — same envelope as the Rust dispatcher.
    return err('VALIDATION_ERROR', `unknown IPC command '${cmd}'`, 'security');
  }
}
