// AUTHR_OS — app-level keyboard map. Deliberately avoids plain Ctrl/Cmd+B/I
// (Lexical bold/italic) and plain Ctrl+S (autosave owns it): everything here
// is Ctrl/Cmd+K or Ctrl/Cmd+Alt+<key>.

import { useEffect } from 'react';
import { useWorkspaceStore, type StudioId } from '../stores/workspaceStore';
import { useSettingsStore } from '../stores/settingsStore';

const STUDIO_KEYS: Record<string, StudioId> = {
  '1': 'novel',
  '2': 'audio',
  '3': 'video',
  '4': 'localization',
  '5': 'marketing',
  '6': 'craft',
};

/** Pretty-print the platform-correct hint for the palette chip / menus. */
export function modKey() {
  return navigator.platform.toLowerCase().includes('mac') ? '⌘' : 'Ctrl';
}

/**
 * Global shortcuts:
 *   Ctrl/Cmd+K        command palette
 *   Ctrl/Cmd+Alt+1–6  switch studio
 *   Ctrl/Cmd+Alt+B    toggle project binder
 *   Ctrl/Cmd+Alt+I    toggle dynamic inspector
 *   Escape            exit focus (typewriter) mode
 */
export function useGlobalShortcuts(active: boolean) {
  useEffect(() => {
    if (!active) return;

    const onKeyDown = (e: KeyboardEvent) => {
      const ws = useWorkspaceStore.getState();
      const mod = e.ctrlKey || e.metaKey;

      if (mod && !e.altKey && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        ws.setPaletteOpen(!ws.paletteOpen);
        return;
      }

      if (mod && e.altKey) {
        const studio = STUDIO_KEYS[e.key];
        if (studio) {
          e.preventDefault();
          ws.setActiveStudio(studio);
          return;
        }
        const k = e.key.toLowerCase();
        if (k === 'b') {
          e.preventDefault();
          ws.toggleBinder();
          return;
        }
        if (k === 'i') {
          e.preventDefault();
          ws.toggleInspector();
          return;
        }
      }

      // Escape is Lexical-free at the app level; use it to leave focus mode.
      if (e.key === 'Escape' && useSettingsStore.getState().typewriterMode) {
        e.preventDefault();
        useSettingsStore.getState().setTypewriterMode(false);
      }
    };

    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [active]);
}
