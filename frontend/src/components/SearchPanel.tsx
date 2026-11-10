import { useState } from 'react';
import type { Envelope } from '../api';
import type { Transport } from '../transport';

interface Props {
  transport: Transport;
  mounted: boolean;
  onEnvelope: (env: Envelope, cmd: string) => void;
}

/** FTS5 / BM25 search panel (`search_segments`). */
export function SearchPanel({ transport, mounted, onEnvelope }: Props) {
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<any[] | null>(null);

  const run = async () => {
    if (!query.trim()) return;
    const res = await transport.invoke('search_segments', { query, limit: 50 });
    onEnvelope(res, 'search_segments');
    setResults(res.ok ? res.data.results : null);
  };

  return (
    <section className="panel">
      <div className="panel-head"><h2>Search segments</h2><span className="cmd-chip">search_segments</span></div>
      <div className="search-row">
        <input
          placeholder="MATCH query — e.g. toll, ferryman, marsh…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && run()}
          spellCheck={false}
        />
        <button className="btn primary" onClick={run} disabled={!mounted}>Search</button>
      </div>
      {!mounted && <div className="empty">Mount a bundle to search.</div>}
      {results !== null && (
        <div className="table-wrap">
          <table>
            <thead><tr><th>Rank</th><th>Segment</th><th>Ch.</th><th>Text</th></tr></thead>
            <tbody>
              {results.length === 0 && <tr><td colSpan={4} className="empty">No matches.</td></tr>}
              {results.map((r) => (
                <tr key={r.segment_id}>
                  <td className="mono">{r.rank.toFixed(2)}</td>
                  <td className="mono">{r.segment_id}</td>
                  <td>{r.chapter_id}</td>
                  <td>{r.text_content}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}
