/**
 * Localization Studio logic (Sprint 8) — YouTube/plain-text script import
 * parsing and the target-language catalog.
 *
 * `parseScript` accepts the formats:
 *   [00:35] MIRA: The marsh has no business knowing my mother's name.
 *   MIRA: The marsh has no business knowing my mother's name.
 *   Plain prose paragraphs (speaker = NARRATOR)
 * `parsedToSegments` maps parsed lines onto `segment_create` payloads
 * (DIALOGUE for any non-NARRATOR speaker) so imports persist through the
 * frozen IPC surface.
 */

export interface ParsedLine {
  speaker: string;
  text: string;
  tms: number | null;
}

export const LANGUAGES = [
  'English',
  'Español',
  'Français',
  'Deutsch',
  'Português (BR)',
  'Italiano',
  '日本語',
  '한국어',
  'हिन्दी',
  '中文（简体）',
] as const;

const TIMESTAMP = /^\[(\d{1,2}):(\d{2})(?::(\d{2}))?\]\s*/;
const SPEAKER_LINE = /^([A-Za-z][A-Za-z0-9_']*)\s*:\s*(.+)$/;

export function parseScript(text: string): ParsedLine[] {
  const out: ParsedLine[] = [];
  for (const rawLine of text.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (line === '') continue;
    let rest = line;
    let tms: number | null = null;
    const ts = TIMESTAMP.exec(rest);
    if (ts) {
      const a = Number(ts[1]);
      const b = Number(ts[2]);
      const c = ts[3] != null ? Number(ts[3]) : null;
      tms = c == null ? (a * 60 + b) * 1000 : (a * 3600 + b * 60 + c) * 1000;
      rest = rest.slice(ts[0].length);
    }
    const sp = SPEAKER_LINE.exec(rest);
    if (sp) {
      out.push({ speaker: sp[1].toUpperCase(), text: sp[2].trim(), tms });
    } else {
      out.push({ speaker: 'NARRATOR', text: rest, tms });
    }
  }
  return out;
}

export function parsedToSegmentPayloads(parsed: ParsedLine[]): Record<string, unknown>[] {
  return parsed.map((line, idx) => ({
    speaker_id: line.speaker,
    segment_type: line.speaker === 'NARRATOR' ? 'NARRATION' : 'DIALOGUE',
    emotion: 'neutral',
    text_content: line.text,
    sequence_order: idx + 1,
  }));
}

/** Session-local translation memory row (LLM translation lands in Phase 5). */
export interface MemoryRow {
  source: string;
  target: string;
  language: string;
}
