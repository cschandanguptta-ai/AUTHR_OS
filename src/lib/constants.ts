// AUTHR_OS — domain constants shared across the cockpit.

import type { ChapterStatus, SegmentType } from '../types/ast';

export const APP_VERSION = '3.0.0-PROD';

/** The five synthesis engines selectable in the Voice Cast Rack. */
/** The five synthesis engines — frozen by the §6.1 DDL CHECK constraint. */
export const VOICE_ENGINES = [
  'KOKORO_LOCAL',
  'EDGE_TTS',
  'ELEVEN_LABS',
  'CARTESIA',
  'OPENAI_TTS',
] as const;

export const SEGMENT_TYPES: SegmentType[] = [
  'NARRATION',
  'DIALOGUE',
  'INTERNAL_MONOLOGUE',
];

export const SEGMENT_TYPE_LABELS: Record<SegmentType, string> = {
  NARRATION: 'Narration',
  DIALOGUE: 'Dialogue',
  INTERNAL_MONOLOGUE: 'Internal monologue',
};

export const SEGMENT_TYPE_HINTS: Record<SegmentType, string> = {
  NARRATION: 'Verbatim paragraph, narrator voice',
  DIALOGUE: 'Attributed spoken line with speaker tag',
  INTERNAL_MONOLOGUE: 'Interior thought, italicized in shadow files',
};

export const CHAPTER_STATUSES: ChapterStatus[] = [
  'PLANNED',
  'DRAFTING',
  'POLISHED',
  'AUDIO_READY',
  'PUBLISHED',
];

export const CHAPTER_STATUS_DOTS: Record<ChapterStatus, string> = {
  PLANNED: 'bg-zinc-500',
  DRAFTING: 'bg-amber-400',
  POLISHED: 'bg-indigo-400',
  AUDIO_READY: 'bg-violet-400',
  PUBLISHED: 'bg-emerald-400',
};

export const CHARACTER_ROLES = [
  'PROTAGONIST',
  'ANTAGONIST',
  'SUPPORTING',
  'NARRATOR',
] as const;

export const ROLE_BADGES: Record<string, string> = {
  PROTAGONIST: 'border-amber-500/40 bg-amber-500/10 text-warn',
  ANTAGONIST: 'border-rose-500/40 bg-rose-500/10 text-bad',
  SUPPORTING: 'border-zinc-500/40 bg-zinc-500/10 text-mut',
  NARRATOR: 'border-violet-500/40 bg-violet-500/10 text-accent2',
};

export const ROLE_SPEAKER_COLORS: Record<string, string> = {
  PROTAGONIST: 'text-warn',
  ANTAGONIST: 'text-bad',
  SUPPORTING: 'text-mut',
  NARRATOR: 'text-accent2',
};

export const LORE_CATEGORIES = [
  'GENERAL',
  'LOCATION',
  'FACTION',
  'MAGIC_TECH',
  'HISTORICAL',
] as const;

export const LORE_CATEGORY_BADGES: Record<string, string> = {
  GENERAL: 'border-zinc-500/40 bg-zinc-500/10 text-mut',
  LOCATION: 'border-emerald-500/40 bg-emerald-500/10 text-ok',
  FACTION: 'border-amber-500/40 bg-amber-500/10 text-warn',
  MAGIC_TECH: 'border-violet-500/40 bg-violet-500/10 text-accent2',
  HISTORICAL: 'border-rose-500/40 bg-rose-500/10 text-bad',
};

export const THREAD_STATUSES = ['INTRODUCED', 'ESCALATED', 'RESOLVED'] as const;

/** Frozen by the unresolved_threads CHECK constraint (§6.1). */
export const THREAD_TYPE_OPTIONS = [
  'CHEKHOV_GUN',
  'MYSTERY',
  'SUBPLOT',
  'PROMISE',
] as const;

export interface EmotionPreset {
  id: string;
  label: string;
  hint: string;
  chip: string;
}

export const EMOTION_PRESETS: EmotionPreset[] = [
  {
    id: 'neutral',
    label: 'Neutral',
    hint: 'Baseline delivery — measured pace, level pitch.',
    chip: 'border-zinc-500/40 bg-zinc-500/10 text-mut',
  },
  {
    id: 'tense',
    label: 'Tense',
    hint: 'Ratcheted urgency — slightly fast, clipped breaths.',
    chip: 'border-amber-500/40 bg-amber-500/10 text-warn',
  },
  {
    id: 'whisper',
    label: 'Whisper',
    hint: 'Close-mic intimacy — slow, airy, low volume.',
    chip: 'border-violet-500/40 bg-violet-500/10 text-accent2',
  },
  {
    id: 'cold_anger',
    label: 'Cold anger',
    hint: 'Constrained fury — very slow, pitch pressed down.',
    chip: 'border-rose-500/40 bg-rose-500/10 text-bad',
  },
  {
    id: 'joyful',
    label: 'Joyful',
    hint: 'Bright and lifted — quick, pitch on the rise.',
    chip: 'border-emerald-500/40 bg-emerald-500/10 text-ok',
  },
  {
    id: 'sorrowful',
    label: 'Sorrowful',
    hint: 'Heavy grief — dragging tempo, softened endings.',
    chip: 'border-teal-500/40 bg-teal-500/10 chip-teal',
  },
  {
    id: 'defiant',
    label: 'Defiant',
    hint: 'Chin-up resolve — punchy attacks, firm pitch.',
    chip: 'border-orange-500/40 bg-orange-500/10 chip-orange',
  },
  {
    id: 'serene',
    label: 'Serene',
    hint: 'Still water — long pauses, gentle contour.',
    chip: 'border-lime-500/40 bg-lime-500/10 chip-lime',
  },
  {
    id: 'urgent',
    label: 'Urgent',
    hint: 'Alarm state — fastest rate, wide pitch swings.',
    chip: 'border-red-500/40 bg-red-500/10 text-bad',
  },
  {
    id: 'tender',
    label: 'Tender',
    hint: 'Warm closeness — slow, rounded, low-lit.',
    chip: 'border-pink-500/40 bg-pink-500/10 chip-pink',
  },
];

export function emotionChipClass(emotion: string): string {
  return (
    EMOTION_PRESETS.find((e) => e.id === emotion)?.chip ??
    'border-zinc-500/40 bg-zinc-500/10 text-mut'
  );
}

export function emotionLabel(emotion: string): string {
  return EMOTION_PRESETS.find((e) => e.id === emotion)?.label ?? emotion;
}

export const ACT_LABELS: Record<number, string> = {
  1: 'Act I',
  2: 'Act II',
  3: 'Act III',
};

export function actLabel(act: number): string {
  return ACT_LABELS[act] ?? `Act ${act}`;
}

export const VIDEO_RESOLUTIONS = [
  '1920 × 1080',
  '2560 × 1440',
  '1280 × 720',
  '1080 × 1920',
] as const;

export const VIDEO_FPS = [24, 30, 60] as const;
