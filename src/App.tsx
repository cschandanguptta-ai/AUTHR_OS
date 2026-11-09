import { useEffect } from 'react';
import { useProjectStore } from './stores/projectStore';
import { useSettingsStore } from './stores/settingsStore';
import { useBootstrapProject } from './hooks/useBootstrapProject';
import { useGlobalShortcuts } from './hooks/useGlobalShortcuts';
import { WelcomeScreen } from './components/layout/WelcomeScreen';
import { WorkstationSwitcher } from './components/layout/WorkstationSwitcher';
import { CommandPalette } from './components/layout/CommandPalette';
import { Cockpit } from './components/layout/Cockpit';
import { AudioStudio } from './components/studios/AudioStudio';
import { VideoStudio } from './components/studios/VideoStudio';
import { LocalizationStudio } from './components/studios/LocalizationStudio';
import { MarketingStudio } from './components/studios/MarketingStudio';
import { CraftStudio } from './components/studios/CraftStudio';
import { Toaster } from './components/layout/Toaster';
import { useWorkspaceStore } from './stores/workspaceStore';

export default function App() {
  const status = useProjectStore((s) => s.status);
  const manifest = useProjectStore((s) => s.manifest);
  const theme = useSettingsStore((s) => s.theme);
  const fontSize = useSettingsStore((s) => s.fontSize);
  const activeStudio = useWorkspaceStore((s) => s.activeStudio);
  useBootstrapProject();

  // Shortcuts + palette are cockpit-level features (need an open bundle).
  useGlobalShortcuts(status === 'open');

  useEffect(() => {
    document.documentElement.className = `theme-${theme}`;
  }, [theme]);

  // Editor type size (Settings popover) → CSS var consumed by the canvas CSS.
  useEffect(() => {
    document.documentElement.style.setProperty('--editor-font', `${fontSize}px`);
  }, [fontSize]);

  useEffect(() => {
    document.title = manifest
      ? `${manifest.title} — AUTHR_OS`
      : "AUTHR_OS — Author's Cockpit";
  }, [manifest]);

  if (status !== 'open') {
    return (
      <div className="h-full flex flex-col bg-app text-ink">
        <WelcomeScreen />
        <Toaster />
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col bg-app text-ink">
      <WorkstationSwitcher />
      <main className="flex-1 min-h-0 flex flex-col" aria-label="Active studio">
        {/* key remount gives each studio switch a soft crossfade (anim-fade) */}
        <div key={activeStudio} className="flex-1 min-h-0 flex flex-col anim-fade">
          {activeStudio === 'novel' && <Cockpit />}
          {activeStudio === 'audio' && <AudioStudio />}
          {activeStudio === 'video' && <VideoStudio />}
          {activeStudio === 'localization' && <LocalizationStudio />}
          {activeStudio === 'marketing' && <MarketingStudio />}
          {activeStudio === 'craft' && <CraftStudio />}
        </div>
      </main>
      <CommandPalette />
      <Toaster />
    </div>
  );
}
