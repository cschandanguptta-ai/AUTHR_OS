// AUTHR_OS — reader/editor preferences, persisted to localStorage.

import { create } from 'zustand';
import { persist } from 'zustand/middleware';

export type AppTheme = 'dark' | 'midnight' | 'sepia';

interface SettingsState {
  theme: AppTheme;
  fontSize: number;
  typewriterMode: boolean;
  setTheme: (theme: AppTheme) => void;
  setFontSize: (size: number) => void;
  setTypewriterMode: (on: boolean) => void;
}

export const useSettingsStore = create<SettingsState>()(
  persist(
    (set) => ({
      theme: 'dark',
      fontSize: 16,
      typewriterMode: false,
      setTheme: (theme) => set({ theme }),
      setFontSize: (fontSize) => set({ fontSize: Math.min(24, Math.max(12, fontSize)) }),
      setTypewriterMode: (typewriterMode) => set({ typewriterMode }),
    }),
    { name: 'authr_os.settings' },
  ),
);
