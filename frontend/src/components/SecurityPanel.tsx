import { useEffect, useState } from 'react';
import type { Envelope } from '../api';
import type { Transport } from '../transport';

interface Props {
  transport: Transport;
  mounted: boolean;
  sessionRoot: string | null;
  onEnvelope: (env: Envelope, cmd: string) => void;
}

/** Security panel: sandbox resolver + bundle info (manifest + lock state). */
export function SecurityPanel({ transport, mounted, sessionRoot, onEnvelope }: Props) {
  const [path, setPath] = useState('manuscript/ch01.md');
  const [resolved, setResolved] = useState<string | null>(null);
  const [info, setInfo] = useState<any>(null);

  useEffect(() => {
    if (!mounted) { setInfo(null); return; }
    transport.invoke('project_info').then((res) => {
      onEnvelope(res, 'project_info');
      if (res.ok) setInfo(res.data);
    });
  }, [transport, mounted, sessionRoot, onEnvelope]);

  const resolve = async () => {
    const res = await transport.invoke('sandbox_resolve', { path });
    onEnvelope(res, 'sandbox_resolve');
    setResolved(res.ok ? res.data.resolved : `[${res.error.code}] ${res.error.message}`);
  };

  return (
    <section className="panel">
      <div className="panel-head">
        <h2>Security &amp; sandbox</h2>
        <span className="cmd-chip">sandbox_resolve · project_info</span>
      </div>

      {!mounted && <div className="empty">Mount a bundle to inspect security state.</div>}

      {mounted && (
        <>
          <div className="form-card">
            <h3>Path sandboxing</h3>
            <p className="hint">
              Every bundle-relative path passes through normalization (`\\?\`, UNC, percent-decode),
              then traversal / drive / symlink rejection before touching disk.
            </p>
            <div className="search-row">
              <input value={path} onChange={(e) => setPath(e.target.value)} spellCheck={false} />
              <button className="btn primary" onClick={resolve}>Resolve</button>
            </div>
            {resolved && <div className={`notice ${resolved.startsWith('[') ? 'bad' : ''}`}>{resolved}</div>}
          </div>

          {info && (
            <div className="form-card">
              <h3>Bundle state</h3>
              <table className="kv">
                <tbody>
                  <tr><th>title</th><td>{info.manifest?.title}</td></tr>
                  <tr><th>author</th><td>{info.manifest?.author}</td></tr>
                  <tr><th>schema_version</th><td>{info.manifest?.schema_version}</td></tr>
                  <tr><th>authr_version</th><td>{info.manifest?.authr_version}</td></tr>
                  <tr><th>locked</th><td>{String(info.locked)}</td></tr>
                  <tr><th>read_only</th><td>{String(info.read_only)}</td></tr>
                  {info.lock && (
                    <tr><th>lease</th><td className="mono">
                      session {String(info.lock.session_id ?? '').slice(0, 12)} · expires {info.lock.lease_expires_at ?? '—'}
                    </td></tr>
                  )}
                </tbody>
              </table>
            </div>
          )}
        </>
      )}
    </section>
  );
}
