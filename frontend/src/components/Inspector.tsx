import { useEffect, useRef, useState } from 'react';
import type { AppEvent, Envelope } from '../api';

interface Props {
  envelope: { cmd: string; env: Envelope } | null;
  events: AppEvent[];
}

/** Right-hand inspector: last raw envelope + live shell event log. */
export function Inspector({ envelope, events }: Props) {
  const [tab, setTab] = useState<'envelope' | 'events'>('envelope');
  const logRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (tab === 'events' && logRef.current) {
      logRef.current.scrollTop = 0;
    }
  }, [events, tab]);

  return (
    <aside className="inspector">
      <div className="inspector-tabs">
        <button className={tab === 'envelope' ? 'active' : ''} onClick={() => setTab('envelope')}>Envelope</button>
        <button className={tab === 'events' ? 'active' : ''} onClick={() => setTab('events')}>
          Events <span className="count">{events.length}</span>
        </button>
      </div>

      {tab === 'envelope' && (
        <div className="inspector-body">
          {!envelope && <div className="empty">Invoke a command to inspect its envelope.</div>}
          {envelope && (
            <>
              <div className="cmd-line">
                <span className="mono cmd-chip">{envelope.cmd}</span>
                <span className={`pill ${envelope.env.ok ? 'ok' : 'bad'}`}>{envelope.env.ok ? 'ok' : 'error'}</span>
              </div>
              <pre className="mono envelope-view">{JSON.stringify(envelope.env, null, 2)}</pre>
            </>
          )}
        </div>
      )}

      {tab === 'events' && (
        <div className="inspector-body" ref={logRef}>
          {events.length === 0 && <div className="empty">No shell events yet.</div>}
          <ul className="event-log">
            {events.map((e, i) => (
              <li key={i}>
                <span className={`evt ${e.event.replace('authr://', '')}`}>{e.event}</span>
                <span className="mono evt-payload">{JSON.stringify(e.payload)}</span>
              </li>
            ))}
          </ul>
        </div>
      )}
    </aside>
  );
}
