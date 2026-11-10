/**
 * Studio logic unit tests (T6.UF) — marketing generators, video timeline,
 * localization parsing, audio DAW stats + mastering defaults.
 */

import { describe, expect, it } from 'vitest';
import { buildKeywords, buildMidjourneyPrompt, buildSocialHooks } from '../studios/marketing';
import { buildRenderScript, buildTimeline, formatMs } from '../studios/video';
import { LANGUAGES, parseScript, parsedToSegmentPayloads } from '../studios/localization';
import { dawLayerStats, pronunciationRows, MASTERING_DEFAULTS } from '../studios/audio';
import type { BookRow, CharacterRow, LoreRow, SegmentRow, ThreadRow } from '../types';

const book: BookRow = {
  id: 1, title: 'The Lantern Marshes', author: 'A. Calloway', genre: 'gothic mystery',
  narrative_structure: 'three_act', status: 'drafting', target_word_count: 95000, word_count: 1200,
};
const chars: CharacterRow[] = [
  { id: 1, book_id: 1, character_key: 'NARRATOR', display_name: 'Narrator', role: 'narrator', voice_engine: 'kokoro', voice_config: '{}', ipa_phonetic: null },
  { id: 2, book_id: 1, character_key: 'MIRA', display_name: 'Mira Vance', role: 'protagonist', voice_engine: 'kokoro', voice_config: '{}', ipa_phonetic: '/ˈmɪərə/' },
  { id: 3, book_id: 1, character_key: 'FERRYMAN', display_name: 'The Ferryman', role: 'secondary', voice_engine: 'edge_tts', voice_config: '{}', ipa_phonetic: null },
];
const lore: LoreRow[] = [
  { id: 1, book_id: 1, category: 'location', term: 'Lantern Marsh', content: '…', ipa_phonetic: null },
  { id: 2, book_id: 1, category: 'rule', term: 'The Toll', content: '…', ipa_phonetic: null },
];
const threads: ThreadRow[] = [
  { id: 1, book_id: 1, chapter_id: 1, title: 'The ferryman knows her mother', status: 'active', planted_chapter_index: 1, payoff_chapter_index: null, notes: '' },
];

describe('Marketing Studio generators', () => {
  it('builds a deterministic Midjourney prompt anchored in bundle data', () => {
    const p1 = buildMidjourneyPrompt(book, chars, lore);
    const p2 = buildMidjourneyPrompt(book, chars, lore);
    expect(p1).toBe(p2);
    expect(p1).toContain('gothic mystery');
    expect(p1).toContain('Mira Vance');
    expect(p1).toContain('Lantern Marsh');
    expect(p1).toContain('--ar 2:3');
  });

  it('extracts keywords from cast, lore, genre and author (NARRATOR excluded)', () => {
    const kw = buildKeywords(book, chars, lore);
    expect(kw).toContain('mira vance');
    expect(kw).toContain('lantern marsh');
    expect(kw).not.toContain('narrator');
    expect(kw.length).toBeLessThanOrEqual(12);
  });

  it('derives social hooks from open Chekhov threads', () => {
    const hooks = buildSocialHooks(book, threads);
    expect(hooks[0]).toContain('the ferryman knows her mother');
  });
});

describe('Video Studio timeline', () => {
  const chapter = { id: 1, book_id: 1, chapter_index: 1, title: 'The Ferryman’s Toll', summary: '', status: 'draft' };
  const seg = (id: number, text: string, order: number, durMs?: number): SegmentRow => ({
    id, segment_id: `s${id}`, chapter_id: 1, speaker_id: 'NARRATOR', segment_type: 'NARRATION',
    text_content: text, emotion: 'neutral', prosody_rate: 1, prosody_pitch: 1,
    ambient_cue: null, sfx_cue: null, audio_hash: null, sequence_order: order,
    audio_duration_ms: durMs,
  });

  it('estimates pacing from word counts at 150 wpm', () => {
    const tl = buildTimeline([chapter], [seg(1, 'one two three four five six', 1)]);
    expect(tl.slides[0].durationMs).toBe(Math.round((6 / 150) * 60000));
    expect(tl.totalMs).toBe(tl.slides[0].durationMs);
  });

  it('applies per-chapter pacing multipliers', () => {
    const base = buildTimeline([chapter], [seg(1, 'one two three four', 1)]);
    const paced = buildTimeline([chapter], [seg(1, 'one two three four', 1)], { 1: 2 });
    expect(paced.slides[0].durationMs).toBe(base.slides[0].durationMs * 2);
  });

  it('prefers cached audio durations and formats mm:ss', () => {
    const tl = buildTimeline([chapter], [seg(1, 'tiny', 1, 65_000)]);
    expect(tl.slides[0].durationMs).toBe(65_000);
    expect(formatMs(65_000)).toBe('1:05');
  });

  it('emits an inspectable FFmpeg render plan', () => {
    const tl = buildTimeline([chapter], [seg(1, 'one two', 1)]);
    const script = buildRenderScript(tl, 'The Lantern Marshes');
    expect(script).toContain('The Lantern Marshes');
    expect(script).toContain('ffmpeg -loop 1');
    expect(script).toContain('libx264');
    expect(script).toContain('Phase 6');
  });
});

describe('Localization Studio parsing', () => {
  it('parses timestamped speaker lines', () => {
    const lines = parseScript('[00:35] MIRA: The marsh has no business knowing my mother’s name.');
    expect(lines).toEqual([{ speaker: 'MIRA', text: 'The marsh has no business knowing my mother’s name.', tms: 35_000 }]);
  });

  it('parses h:mm:ss timestamps', () => {
    const lines = parseScript('[1:02:03] NARRATOR: Deep night.');
    expect(lines[0].tms).toBe((3600 + 2 * 60 + 3) * 1000);
  });

  it('parses speaker lines without timestamps and plain prose', () => {
    const lines = parseScript('FERRYMAN: State your toll.\nJust narration here.');
    expect(lines[0]).toMatchObject({ speaker: 'FERRYMAN', text: 'State your toll.' });
    expect(lines[1]).toMatchObject({ speaker: 'NARRATOR', text: 'Just narration here.', tms: null });
  });

  it('maps parsed lines onto segment_create payloads', () => {
    const payloads = parsedToSegmentPayloads(parseScript('MIRA: Hello.\nPlain prose.'));
    expect(payloads[0]).toMatchObject({ speaker_id: 'MIRA', segment_type: 'DIALOGUE', sequence_order: 1 });
    expect(payloads[1]).toMatchObject({ speaker_id: 'NARRATOR', segment_type: 'NARRATION', sequence_order: 2 });
  });

  it('catalogs the target languages', () => {
    expect(LANGUAGES.length).toBeGreaterThanOrEqual(8);
  });
});

describe('Audio Studio rack', () => {
  const seg = (id: number, sfx: string | null, ambient: string | null): SegmentRow => ({
    id, segment_id: `s${id}`, chapter_id: 1, speaker_id: 'NARRATOR', segment_type: 'NARRATION',
    text_content: 'x', emotion: 'neutral', prosody_rate: 1, prosody_pitch: 1,
    ambient_cue: ambient, sfx_cue: sfx, audio_hash: null, sequence_order: id,
  });

  it('census the 3-layer DAW rack', () => {
    const stats = dawLayerStats([seg(1, 'door_creak', 'rain_heavy'), seg(2, null, 'rain_heavy'), seg(3, null, null)]);
    expect(stats).toEqual({ speech: 3, sfx: 1, ambient: 2, total: 3 });
  });

  it('flags proper nouns missing IPA pronunciation', () => {
    const rows = pronunciationRows(chars, lore);
    const ferryman = rows.find((r) => r.key === 'FERRYMAN')!;
    const mira = rows.find((r) => r.key === 'MIRA')!;
    const narrator = rows.find((r) => r.key === 'NARRATOR')!;
    expect(ferryman.needsPronunciation).toBe(true);
    expect(mira.needsPronunciation).toBe(false);
    expect(narrator.needsPronunciation).toBe(false); // NARRATOR exempt
    expect(rows.find((r) => r.key === 'Lantern Marsh')!.needsPronunciation).toBe(true);
  });

  it('freezes the ACX / EBU R128 mastering targets', () => {
    expect(MASTERING_DEFAULTS.targetRmsDb).toBe(-20.0);
    expect(MASTERING_DEFAULTS.peakCeilingDbfs).toBe(-3.1);
    expect(MASTERING_DEFAULTS.duckingDb).toBe(-18);
    expect(MASTERING_DEFAULTS.duckingAttackMs).toBe(120);
    expect(MASTERING_DEFAULTS.duckingReleaseMs).toBe(750);
  });
});
