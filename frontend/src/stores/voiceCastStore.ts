/**
 * Voice Cast store — `character_profiles` domain: the Voice Cast Rack,
 * voice profiling (engine + JSON voice config) and the pronunciation
 * dictionary (custom IPA phonetics). All writes go through the frozen
 * `character_*` IPC commands.
 */

import { create } from 'zustand';
import { getTransport } from '../transport';
import { recordCall } from './uiStore';
import type { CharacterRow } from '../types';

export interface VoiceProfile {
  voice?: string;
  rate?: string;
  pitch?: string;
  volume?: string;
  style?: string;
}

interface VoiceCastState {
  characters: CharacterRow[];
  loading: boolean;
  error: string | null;
  /** Book the rack was loaded for (the frozen list command is book-scoped). */
  loadedForBook: number | null;

  reload(bookId: number): Promise<void>;
  create(fields: { character_key: string; display_name: string; book_id: number } & Partial<CharacterRow>): Promise<CharacterRow | null>;
  remove(id: number): Promise<boolean>;
  /** Voice profiling: engine + parsed JSON config persisted atomically. */
  castVoice(id: number, engine: string, profile: VoiceProfile): Promise<boolean>;
  /** Pronunciation dictionary entry (custom IPA). */
  setPronunciation(id: number, ipa: string | null): Promise<boolean>;
  parseConfig(row: { voice_config: string | null }): VoiceProfile;
}

export const useVoiceCast = create<VoiceCastState>((set) => ({
  characters: [],
  loading: false,
  error: null,
  loadedForBook: null,

  async reload(bookId) {
    const t = getTransport();
    set({ loading: true, error: null });
    const res = await t.invoke('character_list', { book_id: bookId });
    recordCall('character_list', res);
    if (res.ok) set({ characters: res.data as CharacterRow[], loading: false, loadedForBook: bookId });
    else set({ characters: [], loading: false, error: res.error.message });
  },

  async create(fields) {
    const res = await getTransport().invoke('character_create', fields);
    recordCall('character_create', res);
    if (res.ok) {
      set((s) => ({ characters: [...s.characters, res.data as CharacterRow] }));
      return res.data as CharacterRow;
    }
    set({ error: res.error.message });
    return null;
  },

  async remove(id) {
    const res = await getTransport().invoke('character_delete', { id });
    recordCall('character_delete', res);
    if (res.ok) {
      set((s) => ({ characters: s.characters.filter((c) => c.id !== id) }));
      return true;
    }
    set({ error: res.error.message });
    return false;
  },

  async castVoice(id, engine, profile) {
    const res = await getTransport().invoke('character_update', {
      id,
      voice_engine: engine,
      voice_config: JSON.stringify(profile),
    });
    recordCall('character_update', res);
    if (res.ok) {
      set((s) => ({
        characters: s.characters.map((c) =>
          c.id === id ? { ...c, voice_engine: engine, voice_config: JSON.stringify(profile) } : c,
        ),
      }));
      return true;
    }
    set({ error: res.error.message });
    return false;
  },

  async setPronunciation(id, ipa) {
    const res = await getTransport().invoke('character_update', { id, ipa_phonetic: ipa });
    recordCall('character_update', res);
    if (res.ok) {
      set((s) => ({
        characters: s.characters.map((c) => (c.id === id ? { ...c, ipa_phonetic: ipa } : c)),
      }));
      return true;
    }
    set({ error: res.error.message });
    return false;
  },

  parseConfig(row) {
    try {
      return row.voice_config ? (JSON.parse(row.voice_config) as VoiceProfile) : {};
    } catch {
      return {};
    }
  },
}));

/** NARRATOR is auto-seeded by the backend; keep it first in rack order. */
export function castOrder(rows: CharacterRow[]): CharacterRow[] {
  return [...rows].sort((a, b) => {
    if (a.character_key === 'NARRATOR') return -1;
    if (b.character_key === 'NARRATOR') return 1;
    return a.character_key.localeCompare(b.character_key);
  });
}

export const VOICE_ENGINES = [
  'kokoro',
  'edge_tts',
  'kitten_tts',
  'hf_tts',
  'elevenlabs',
  'windows_sapi',
  'macos_avspeech',
  'none',
] as const;
