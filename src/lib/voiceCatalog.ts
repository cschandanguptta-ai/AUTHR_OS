// AUTHR_OS — bundled voice catalogs per engine (D7 §6, R-05).
//
// Static, offline catalogs so no one has to paste raw voice IDs for supported
// engines. SCRIPTED and KOKORO_LOCAL share the Kokoro-82M voice naming
// (contract §4.11: local_tts_generate voice?="af_bella"). Cloud engines carry
// their vendors' documented sample IDs; CARTESIA ships no catalog — its voices
// are account-private UUIDs, so the picker offers the paste-custom-ID path
// with an honest explanation instead of inventing IDs.

export interface VoiceDef {
  id: string;
  label: string;
  hint?: string;
}

export interface EngineCatalog {
  engine: string;
  offline: boolean;
  /** False = voices are account-private; the picker leads with paste-custom-ID. */
  cataloged: boolean;
  voices: VoiceDef[];
  customHint?: string;
}

const KOKORO_VOICES: VoiceDef[] = [
  { id: 'af_bella', label: 'Bella', hint: 'female · warm, expressive' },
  { id: 'af_heart', label: 'Heart', hint: 'female · bright' },
  { id: 'af_nicole', label: 'Nicole', hint: 'female · soft' },
  { id: 'am_adam', label: 'Adam', hint: 'male · grounded' },
  { id: 'am_michael', label: 'Michael', hint: 'male · steady' },
  { id: 'bf_emma', label: 'Emma', hint: 'female · British' },
  { id: 'bm_george', label: 'George', hint: 'male · British' },
];

const EDGE_VOICES: VoiceDef[] = [
  { id: 'en-US-AriaNeural', label: 'Aria', hint: 'en-US · female' },
  { id: 'en-US-JennyNeural', label: 'Jenny', hint: 'en-US · female' },
  { id: 'en-US-GuyNeural', label: 'Guy', hint: 'en-US · male' },
  { id: 'en-GB-SoniaNeural', label: 'Sonia', hint: 'en-GB · female' },
  { id: 'en-GB-RyanNeural', label: 'Ryan', hint: 'en-GB · male' },
];

const ELEVEN_VOICES: VoiceDef[] = [
  { id: 'EXAVITQu4vr4xnSDjMa5', label: 'Sarah', hint: 'soft, news-like' },
  { id: '21m00Tcm4TlvDq8ikWAM', label: 'Rachel', hint: 'calm narration' },
  { id: 'AZnzlk1XvdvUeBnXmlld', label: 'Domi', hint: 'strong, confident' },
  { id: 'ErXwobaYiN019PkySvjV', label: 'Antoni', hint: 'well-rounded' },
  { id: 'TxGEqnHWrfWFTfGW9XjX', label: 'Josh', hint: 'deep, young' },
];

const OPENAI_VOICES: VoiceDef[] = [
  { id: 'alloy', label: 'Alloy', hint: 'balanced' },
  { id: 'echo', label: 'Echo', hint: 'male' },
  { id: 'fable', label: 'Fable', hint: 'British, storyteller' },
  { id: 'onyx', label: 'Onyx', hint: 'deep male' },
  { id: 'nova', label: 'Nova', hint: 'female, bright' },
  { id: 'shimmer', label: 'Shimmer', hint: 'female, soft' },
];

export const VOICE_CATALOGS: Record<string, EngineCatalog> = {
  SCRIPTED: {
    engine: 'SCRIPTED',
    offline: true,
    cataloged: true,
    voices: KOKORO_VOICES,
    customHint: 'SCRIPTED renders deterministic placeholder speech offline — any Kokoro-style id works and previews instantly.',
  },
  KOKORO_LOCAL: {
    engine: 'KOKORO_LOCAL',
    offline: true,
    cataloged: true,
    voices: KOKORO_VOICES,
    customHint: 'Kokoro-82M runs locally when the model is installed (see Local AI status); ids follow the Kokoro voice set.',
  },
  EDGE_TTS: {
    engine: 'EDGE_TTS',
    offline: false,
    cataloged: true,
    voices: EDGE_VOICES,
    customHint: 'Any Edge TTS locale voice id (e.g. en-US-AriaNeural). Requires network at synthesis time.',
  },
  ELEVEN_LABS: {
    engine: 'ELEVEN_LABS',
    offline: false,
    cataloged: true,
    voices: ELEVEN_VOICES,
    customHint: 'Documented public sample IDs — or paste the voice_id from your ElevenLabs voice library.',
  },
  CARTESIA: {
    engine: 'CARTESIA',
    offline: false,
    cataloged: false,
    voices: [],
    customHint: 'Cartesia voices are private to your account — paste the voice UUID from your Cartesia dashboard.',
  },
  OPENAI_TTS: {
    engine: 'OPENAI_TTS',
    offline: false,
    cataloged: true,
    voices: OPENAI_VOICES,
    customHint: 'OpenAI preset voices; needs OPENAI_API_KEY at synthesis time.',
  },
};

/** The full engine list incl. the offline SCRIPTED default (frozen DDL set). */
export const VOICE_ENGINE_OPTIONS = ['SCRIPTED', 'KOKORO_LOCAL', 'EDGE_TTS', 'ELEVEN_LABS', 'CARTESIA', 'OPENAI_TTS'] as const;

export function engineCatalog(engine: string): EngineCatalog {
  return VOICE_CATALOGS[engine] ?? {
    engine,
    offline: false,
    cataloged: false,
    voices: [],
    customHint: 'Unknown engine — paste the exact voice id your engine expects.',
  };
}

export const PREVIEW_LINE = 'This is how I will sound in your audiobook.';
