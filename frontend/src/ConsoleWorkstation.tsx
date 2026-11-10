import type { JSX } from 'react';
/**
 * The Operator Console — the Phase 2 workstation, preserved verbatim as
 * one switcher destination of the Phase 3 app shell. Schema-driven
 * resource tables, FTS search, git history, security inspector and the
 * raw envelope log.
 */

import { useCallback, useEffect, useState } from 'react';
import type { Envelope } from './api';
import { getTransport, type Transport } from './transport';
import { ENTITY_SPECS } from './schema';
import { Sidebar, type PanelKey } from './components/Sidebar';
import { ResourcePanel } from './components/ResourcePanel';
import { SearchPanel } from './components/SearchPanel';
import { GitPanel } from './components/GitPanel';
import { SecurityPanel } from './components/SecurityPanel';
import { Inspector } from './components/Inspector';
import { useSession } from './stores/sessionStore';
import { useUi } from './stores/uiStore';

export function ConsoleWorkstation(): JSX.Element {
  const transport: Transport = getTransport();
  const sessionRoot = useSession((s) => s.mountedRoot);
  const revision = useSession((s) => s.revision);
  const events = useUi((s) => s.events);
  const lastEnvelope = useUi((s) => s.lastEnvelope);
  const recordEnvelope = useUi((s) => s.recordEnvelope);
  const [panel, setPanel] = useState<PanelKey>('books');
  const [chapterBookId, setChapterBookId] = useState<number | null>(null);
  const [segmentChapterId, setSegmentChapterId] = useState<number | null>(null);
  const [books, setBooks] = useState<{ id: number; label: string }[]>([]);
  const [chapters, setChapters] = useState<{ id: number; label: string }[]>([]);
  const [lease, setLease] = useState<any>(null);

  const onEnvelope = useCallback((env: Envelope, cmd: string) => {
    recordEnvelope(cmd, env);
  }, [recordEnvelope]);

  // Keep book/chapter selectors fresh (console-local, envelope-true).
  const refreshSelectors = useCallback(async () => {
    if (!sessionRoot) return;
    const b = await transport.invoke('book_list', {});
    if (b.ok) setBooks(b.data.map((x: any) => ({ id: x.id, label: x.title })));
    if (chapterBookId !== null) {
      const c = await transport.invoke('chapter_list', { book_id: chapterBookId });
      if (c.ok) setChapters(c.data.map((x: any) => ({ id: x.id, label: `#${x.chapter_index} ${x.title}` })));
    }
  }, [transport, sessionRoot, chapterBookId]);

  useEffect(() => { void refreshSelectors(); }, [refreshSelectors, revision]);

  // Poll the session status bar (lease expiry countdown).
  useEffect(() => {
    if (!sessionRoot) return;
    const t = setInterval(() => {
      transport.invoke('desktop_status').then((res) => {
        if (res.ok && res.data.mounted) setLease(res.data);
      });
    }, 10_000);
    return () => clearInterval(t);
  }, [transport, sessionRoot]);

  const onMutation = useCallback(() => {
    void refreshSelectors();
  }, [refreshSelectors]);

  const spec = ENTITY_SPECS.find((s) => s.key === panel);
  const parents = panel === 'segments' ? chapters : books;

  return (
    <div className="console-workstation">
      <div className="body">
        <Sidebar active={panel} onSelect={setPanel} enabled={sessionRoot !== null} />
        <main className="main">
          {spec && (
            <ResourcePanel
              key={spec.key}
              spec={spec}
              transport={transport}
              mounted={sessionRoot !== null}
              parentId={spec.key === 'chapters' ? chapterBookId : spec.key === 'segments' ? segmentChapterId : null}
              parentLabel={spec.parent?.label ?? ''}
              parents={parents}
              onParentChange={(id) => {
                if (spec.key === 'chapters') setChapterBookId(id);
                else setSegmentChapterId(id);
              }}
              onEnvelope={onEnvelope}
              onMutation={onMutation}
            />
          )}
          {panel === 'search' && (
            <SearchPanel transport={transport} mounted={sessionRoot !== null} onEnvelope={onEnvelope} />
          )}
          {panel === 'git' && (
            <GitPanel
              transport={transport}
              mounted={sessionRoot !== null}
              onEnvelope={onEnvelope}
              refreshSignal={revision}
            />
          )}
          {panel === 'security' && (
            <SecurityPanel
              transport={transport}
              mounted={sessionRoot !== null}
              sessionRoot={sessionRoot}
              onEnvelope={onEnvelope}
            />
          )}
        </main>
        <Inspector envelope={lastEnvelope} events={events} />
      </div>
      {lease?.lease?.expires_at && (
        <footer className="statusbar">
          <span className="mono">lease expires {lease.lease.expires_at}</span>
        </footer>
      )}
    </div>
  );
}
