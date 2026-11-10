import type { JSX } from 'react';
/**
 * AUTHR_OS app root (Phase 3).
 *
 * Top bar (session) → workstation switcher → active workstation:
 *
 *   Cockpit | Audio Studio | Video Studio | Localization | Marketing | Console
 *
 * The app shell pumps every transport event into the shared uiStore so
 * the Cockpit's Dynamic Inspector and the Phase 2 console behave
 * identically, and reloads the domain stores whenever a mutating command
 * lands (`authr://git` → session.revision).
 */

import { useEffect } from 'react';
import { getTransport } from './transport';
import { TopBar, reloadAllStores } from './components/TopBar';
import { Cockpit } from './cockpit/Cockpit';
import { ConsoleWorkstation } from './ConsoleWorkstation';
import { AudioStudio } from './studios/AudioStudio';
import { VideoStudio } from './studios/VideoStudio';
import { LocalizationStudio } from './studios/LocalizationStudio';
import { MarketingStudio } from './studios/MarketingStudio';
import { useSession } from './stores/sessionStore';
import { useBinder } from './stores/binderStore';
import { useUi } from './stores/uiStore';
import { useManuscript } from './stores/manuscriptStore';
import { useVoiceCast } from './stores/voiceCastStore';
import { useLore } from './stores/loreStore';
import { useLedger } from './stores/ledgerStore';
import { WORKSTATIONS, type Workstation } from './types';

export default function App(): JSX.Element {
  const workstation = useUi((s) => s.workstation);
  const mountedRoot = useSession((s) => s.mountedRoot);
  const revision = useSession((s) => s.revision);
  const toast = useUi((s) => s.toast);
  const books = useBinder((s) => s.books);
  const selectedBookId = useUi((s) => s.selection.bookId);
  // The active book drives the book-scoped stores (cast / lore / ledger).
  const activeBookId = selectedBookId ?? books[0]?.id ?? null;

  // Pump shell events into the shared store exactly once.
  useEffect(() => {
    const offUi = getTransport().onEvent((evt) => {
      useUi.getState().pushEvent(evt);
      if (evt.event === 'authr://lease' && evt.payload?.state === 'unmounted') {
        useManuscript.getState().unload();
      }
    });
    const offSession = useSession.getState().wireEvents();
    return () => {
      offUi();
      offSession?.();
    };
  }, []);

  // (Re)load domain stores on mount and after any mutating command.
  useEffect(() => {
    if (!mountedRoot) {
      useManuscript.getState().unload();
      return;
    }
    void reloadAllStores();
  }, [mountedRoot, revision]);

  // Book-scoped stores follow the active book.
  useEffect(() => {
    if (!mountedRoot || activeBookId == null) return;
    void Promise.all([
      useVoiceCast.getState().reload(activeBookId),
      useLore.getState().reload(activeBookId),
      useLedger.getState().reload(activeBookId),
    ]);
  }, [mountedRoot, activeBookId, revision]);

  return (
    <div className="app">
      <TopBar />
      <nav className="workstation-switcher" aria-label="Workstations">
        {WORKSTATIONS.map((w) => (
          <button
            key={w.key}
            className={workstation === w.key ? 'active' : ''}
            title={w.hint}
            onClick={() => useUi.getState().setWorkstation(w.key as Workstation)}
          >
            {w.label}
          </button>
        ))}
      </nav>
      <div className="workstation-host">
        {workstation === 'cockpit' && <Cockpit />}
        {workstation === 'audio' && <AudioStudio />}
        {workstation === 'video' && <VideoStudio />}
        {workstation === 'localization' && <LocalizationStudio />}
        {workstation === 'marketing' && <MarketingStudio />}
        {workstation === 'console' && <ConsoleWorkstation />}
      </div>
      <footer className="statusbar">
        <span className={mountedRoot ? 'dot on' : 'dot'} />
        <span>{mountedRoot ? `mounted: ${mountedRoot}` : 'no session — mount an .authr bundle'}</span>
        <LeaseChip />
        <span className="spacer" />
        <span className="mono">contract: docs/CLI_CONTRACT.md · shell: Tauri v2 · cockpit: Phase 3</span>
      </footer>
      {toast && (
        <div className="toast" role="status" onClick={() => useUi.getState().setToast(null)}>
          {toast}
        </div>
      )}
    </div>
  );
}

function LeaseChip(): JSX.Element {
  const mountedRoot = useSession((s) => s.mountedRoot);
  const leaseExpiresAt = useSession((s) => s.leaseExpiresAt);
  if (!mountedRoot || !leaseExpiresAt) return <span />;
  return <span>lease expires {leaseExpiresAt}</span>;
}
