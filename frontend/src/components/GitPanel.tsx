import { useCallback, useEffect, useState } from 'react';
import type { Envelope } from '../api';
import type { Transport } from '../transport';

interface Props {
  transport: Transport;
  mounted: boolean;
  onEnvelope: (env: Envelope, cmd: string) => void;
  refreshSignal: number;
}

/** Commit history panel (`project_git_log` — Phase 2 additive command). */
export function GitPanel({ transport, mounted, onEnvelope, refreshSignal }: Props) {
  const [commits, setCommits] = useState<any[]>([]);

  const load = useCallback(async () => {
    if (!mounted) return;
    const res = await transport.invoke('project_git_log', { limit: 50 });
    onEnvelope(res, 'project_git_log');
    if (res.ok) setCommits(res.data.commits);
  }, [transport, mounted, onEnvelope]);

  useEffect(() => { void load(); }, [load, refreshSignal]);

  return (
    <section className="panel">
      <div className="panel-head">
        <h2>Version history</h2>
        <span className="cmd-chip">project_git_log</span>
      </div>
      {!mounted && <div className="empty">Mount a bundle to inspect history.</div>}
      {mounted && (
        <ul className="commit-list">
          {commits.length === 0 && <li className="empty">No commits.</li>}
          {commits.map((c) => (
            <li key={c.id}>
              <code className="mono commit-id">{c.id.slice(0, 10)}</code>
              <span className="commit-msg">{c.message}</span>
              <time>{new Date(c.timestamp * 1000).toLocaleString()}</time>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
