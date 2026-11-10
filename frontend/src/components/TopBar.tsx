import type { JSX } from 'react';
import { useEffect, useState } from 'react';
import { getTransport } from '../transport';
import { useSession } from '../stores/sessionStore';
import { useBinder } from '../stores/binderStore';
import { useVoiceCast } from '../stores/voiceCastStore';
import { useLore } from '../stores/loreStore';
import { useLedger } from '../stores/ledgerStore';
import { useUi } from '../stores/uiStore';

/** Top bar: bundle path, open dialog, mount / unmount, shell badges. */
const DEMO_PATH = '/home/author/Projects/lantern-marshes.authr';

export function TopBar(): JSX.Element {
  const transport = getTransport();
  const mountedRoot = useSession((s) => s.mountedRoot);
  const busy = useSession((s) => s.busy);
  const lastError = useSession((s) => s.lastError);
  const version = useSession((s) => s.version);
  const keychain = useSession((s) => s.keychain);
  const platform = useSession((s) => s.platform);
  const mount = useSession((s) => s.mount);
  const unmount = useSession((s) => s.unmount);

  const [path, setPath] = useState(transport.kind === 'mock' ? DEMO_PATH : '');

  useEffect(() => {
    void useSession.getState().bootstrap();
  }, []);

  const pickFolder = async (): Promise<string> => {
    if (transport.kind === 'tauri') {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({ directory: true, multiple: false, title: 'Choose an .authr bundle' });
      return typeof selected === 'string' ? selected : '';
    }
    // Browser demo: accept any path; default to the fixture bundle.
    return window.prompt('Demo mode: bundle path to mount', DEMO_PATH) ?? '';
  };

  const doMount = async (rootArg?: string): Promise<void> => {
    const root = rootArg ?? (path.trim() ? path.trim() : await pickFolder());
    if (!root) return;
    const ok = await mount(root, 300);
    if (ok) {
      setPath(root);
      void reloadAllStores();
    }
  };

  const doUnmount = async (): Promise<void> => {
    await unmount();
  };

  return (
    <header className="topbar">
      <div className="brand">
        <span className="brand-mark">A</span>
        <span className="brand-name">AUTHR_OS</span>
        <span className="brand-ver">{version ?? ''}</span>
      </div>
      <div className="topbar-controls">
        <input
          className="path-input"
          placeholder="path/to/project.authr"
          value={path}
          onChange={(e) => setPath(e.target.value)}
          spellCheck={false}
        />
        {mountedRoot === null ? (
          <button className="btn primary" disabled={busy} onClick={() => void doMount()}>Mount</button>
        ) : (
          <button className="btn danger" disabled={busy} onClick={() => void doUnmount()}>Unmount</button>
        )}
      </div>
      <div className="badges">
        <span className={`badge ${transport.kind}`}>{transport.kind === 'tauri' ? 'native' : 'demo mode'}</span>
        {keychain && <span className="badge">keychain: {keychain}</span>}
        {platform && <span className="badge">{platform}</span>}
      </div>
      {lastError && <div className="topbar-error" title={lastError}>{lastError}</div>}
    </header>
  );
}

/**
 * Reload every domain store after mount / mutating git events. The cast,
 * lore and ledger list commands are book-scoped by the frozen contract,
 * so they load for the active book (explicit selection, else the first).
 */
export async function reloadAllStores(): Promise<void> {
  if (!useSession.getState().mountedRoot) return;
  await useBinder.getState().reload();
  const books = useBinder.getState().books;
  const bookId = useUi.getState().selection.bookId ?? books[0]?.id ?? null;
  if (bookId == null) return;
  await Promise.all([
    useVoiceCast.getState().reload(bookId),
    useLore.getState().reload(bookId),
    useLedger.getState().reload(bookId),
  ]);
}
