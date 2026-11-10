import { lazy, Suspense, useEffect } from 'react';
import { Loader2 } from 'lucide-react';
import { useProjectStore } from './stores/projectStore';
import { useSettingsStore } from './stores/settingsStore';
import { useBootstrapProject } from './hooks/useBootstrapProject';
import { useGlobalShortcuts } from './hooks/useGlobalShortcuts';
import { WelcomeScreen } from './components/layout/WelcomeScreen';
import { WorkstationSwitcher } from './components/layout/WorkstationSwitcher';
import { CommandPalette } from './components/layout/CommandPalette';
import { DashboardOverlay } from './components/layout/DashboardOverlay';
import { Cockpit } from './components/layout/Cockpit';
import { Toaster } from './components/layout/Toaster';
import { AppConfirmHost } from './components/shared/AppConfirmHost';
import { useWorkspaceStore } from './stores/workspaceStore';

// Studio code-splitting (latency workstream): the cockpit is the primary
// surface and loads eagerly; the other five studios are fetched on first
// visit. Vite emits one chunk per studio; the fallback covers the fetch gap.
const AudioStudio = lazy(() =>
  import('./components/studios/AudioStudio').then((m) => ({ default: m.AudioStudio })),
);
const VideoStudio = lazy(() =>
  import('./components/studios/VideoStudio').then((m) => ({ default: m.VideoStudio })),
);
const LocalizationStudio = lazy(() =>
  import('./components/studios/LocalizationStudio').then((m) => ({ default: m.LocalizationStudio })),
);
const MarketingStudio = lazy(() =>
  import('./components/studios/MarketingStudio').then((m) => ({ default: m.MarketingStudio })),
);
const CraftStudio = lazy(() =>
  import('./components/studios/CraftStudio').then((m) => ({ default: m.CraftStudio })),
);
const PublishStudio = lazy(() =>
  import('./components/studios/PublishStudio').then((m) => ({ default: m.PublishStudio })),
);

function StudioFallback() {
  return (
    <div className="flex-1 flex items-center justify-center text-mut" aria-live="polite">
      <Loader2 size={18} className="animate-spin text-accent" />
      <span className="sr-only">Loading studio…</span>
    </div>
  );
}

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
        <AppConfirmHost />
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
          {activeStudio !== 'novel' && (
            <Suspense fallback={<StudioFallback />}>
              {activeStudio === 'audio' && <AudioStudio />}
              {activeStudio === 'video' && <VideoStudio />}
              {activeStudio === 'localization' && <LocalizationStudio />}
              {activeStudio === 'marketing' && <MarketingStudio />}
              {activeStudio === 'craft' && <CraftStudio />}
              {activeStudio === 'publish' && <PublishStudio />}
            </Suspense>
          )}
        </div>
      </main>
      <CommandPalette />
      <DashboardOverlay />
      <Toaster />
      <AppConfirmHost />
    </div>
  );
}
