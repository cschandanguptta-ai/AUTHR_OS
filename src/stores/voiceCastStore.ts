// AUTHR_OS — voice cast rack (character_profiles store).

import { create } from 'zustand';
import * as api from '../ipc/api';
import type { CharacterRow } from '../types/ast';
import { toast, toastError } from './uiStore';
import { useProjectStore } from './projectStore';

interface VoiceCastState {
  characters: CharacterRow[];
  loaded: boolean;
  loadCharacters: () => Promise<void>;
  upsertCharacter: (args: api.CharacterUpsertArgs) => Promise<CharacterRow | null>;
  patchCharacter: (
    characterKey: string,
    patch: Partial<Pick<CharacterRow, 'voice_engine' | 'voice_id' | 'base_rate' | 'base_pitch' | 'ipa_phonetic' | 'cadence_notes'>>,
  ) => Promise<void>;
  deleteCharacter: (characterKey: string) => Promise<boolean>;
  reset: () => void;
}

export const useVoiceCastStore = create<VoiceCastState>((set, get) => ({
  characters: [],
  loaded: false,

  loadCharacters: async () => {
    try {
      const { characters } = await api.characterList();
      set({ characters, loaded: true });
    } catch (err) {
      toastError('Failed to load voice cast', err);
    }
  },

  upsertCharacter: async (args) => {
    if (useProjectStore.getState().readonly) {
      toast('warn', 'Read-only bundle', 'Foreign lease active — the rack is frozen.');
      return null;
    }
    try {
      const row = await api.characterUpsert(args);
      set((s) => {
        const exists = s.characters.some((c) => c.character_key === row.character_key);
        return {
          characters: exists
            ? s.characters.map((c) => (c.character_key === row.character_key ? row : c))
            : [...s.characters, row],
        };
      });
      return row;
    } catch (err) {
      toastError('Voice profile save failed', err);
      return null;
    }
  },

  patchCharacter: async (characterKey, patch) => {
    const existing = get().characters.find((c) => c.character_key === characterKey);
    if (!existing) return;
    await get().upsertCharacter({
      character_key: existing.character_key,
      display_name: existing.display_name,
      role: existing.role,
      bio: existing.bio,
      ipa_phonetic: existing.ipa_phonetic,
      voice_engine: existing.voice_engine,
      voice_id: existing.voice_id,
      base_rate: existing.base_rate,
      base_pitch: existing.base_pitch,
      cadence_notes: existing.cadence_notes,
      ...patch,
    });
  },

  deleteCharacter: async (characterKey) => {
    if (useProjectStore.getState().readonly) {
      toast('warn', 'Read-only bundle', 'Foreign lease active — the rack is frozen.');
      return false;
    }
    try {
      await api.characterDelete(characterKey);
      set((s) => ({
        characters: s.characters.filter((c) => c.character_key !== characterKey),
      }));
      toast('success', 'Character removed', characterKey);
      return true;
    } catch (err) {
      // Segments referencing the speaker are rejected by FK first (contract §4.4 #18).
      toastError('Could not remove character', err);
      return false;
    }
  },

  reset: () => set({ characters: [], loaded: false }),
}));
