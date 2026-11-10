/**
 * Bundled audio cue catalogs (build plan §5 — "bundled Foley sound IDs").
 * The LLM grammar emits these IDs into `ambient_cue` / `sfx_cue`; the
 * canvas Foley triggers and the Audio Studio racks select from the same
 * frozen lists so the TTS/DSP engines (Phase 4) can resolve every ID.
 */

export const AMBIENT_CUES = [
  'room_tone',
  'water_lapping',
  'rain_heavy',
  'wind_low',
  'forest_night',
  'city_traffic',
  'tavern_walla',
  'engine_hum',
  'cave_drip',
  'snow_blizzard',
] as const;

export const SFX_CUES = [
  'door_creak',
  'oar_creak',
  'glass_break',
  'footsteps_gravel',
  'bell_distant',
  'page_turn',
  'knife_slice',
  'thermal_cutter_ignite',
  'thunder_strike',
  'heartbeat_slow',
] as const;

export type AmbientCue = (typeof AMBIENT_CUES)[number];
export type SfxCue = (typeof SFX_CUES)[number];

export const isAmbientCue = (s: string): s is AmbientCue =>
  (AMBIENT_CUES as readonly string[]).includes(s);
export const isSfxCue = (s: string): s is SfxCue =>
  (SFX_CUES as readonly string[]).includes(s);

export const EMOTION_TAGS = [
  'neutral',
  'warm',
  'tense',
  'grim',
  'angry',
  'cold_anger',
  'whisper',
  'joyful',
  'sorrowful',
  'wry',
] as const;

export type EmotionTag = (typeof EMOTION_TAGS)[number];
