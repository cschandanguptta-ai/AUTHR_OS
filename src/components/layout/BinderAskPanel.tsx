// AUTHR_OS — "Ask your book" binder panel (D7 §4, R-03): rag_index /
// rag_search / rag_status over lorebook + characters + chapters. Hybrid mode
// fuses BM25 + dense via RRF. Empty corpus gets an honest EmptyState with the
// Index-now CTA; failures surface mapped copy — never a fake answer.

import { useEffect, useState } from 'react';
import { BookMarked, Database, Loader2, Search } from 'lucide-react';
import { ragIndex, ragSearch, ragStatus, type RagHit, type RagStatusResult } from '../../ipc/api';
import { useManuscriptStore } from '../../stores/manuscriptStore';
import { mapError } from '../../ipc/errorCopy';
import { cn, truncate } from '../../lib/utils';

const SOURCE_TONE: Record<string, string> = {
  lorebook: 'border-emerald-500/40 bg-emerald-500/10 text-ok',
  characters: 'border-amber-500/40 bg-amber-500/10 text-warn',
  chapters: 'border-violet-500/40 bg-violet-500/10 text-accent2',
};

export function BinderAskBody() {
  const chapters = useManuscriptStore((s) => s.chapters);
  const books = useManuscriptStore((s) => s.books);
  const activeBookId = useManuscriptStore((s) => s.activeBookId);

  const [status, setStatus] = useState<RagStatusResult | null>(null);
  const [hits, setHits] = useState<RagHit[] | null>(null);
  const [latencyMs, setLatencyMs] = useState<number | null>(null);
  const [query, setQuery] = useState('');
  const [busy, setBusy] = useState<'' | 'index' | 'search'>('');
  const [error, setError] = useState<string | null>(null);
  const [indexedAt, setIndexedAt] = useState<number | null>(null);

  const refreshStatus = () => {
    ragStatus()
      .then((s) => setStatus(s))
      .catch(() => setStatus(null));
  };

  useEffect(() => {
    refreshStatus();
  }, []);

  const bookId = activeBookId ?? books[0]?.id ?? undefined;
  const corpusEmpty = (status?.chunks ?? 0) === 0;

  const indexNow = async () => {
    setBusy('index');
    setError(null);
    try {
      await ragIndex({
        sources: ['lorebook', 'characters', 'chapters'],
        book_id: bookId,
      });
      setIndexedAt(Date.now());
      refreshStatus();
    } catch (err) {
      const mapped = mapError(err);
      setError(`${mapped.copy.body} ${mapped.detail}`.trim());
    } finally {
      setBusy('');
    }
  };

  const ask = async () => {
    const q = query.trim();
    if (!q || busy) return;
    setBusy('search');
    setError(null);
    try {
      const res = await ragSearch({ query: q, mode: 'hybrid', k: 5 });
      setHits(res.hits);
      setLatencyMs(res.latency_ms);
    } catch (err) {
      const mapped = mapError(err);
      setError(`${mapped.copy.body} ${mapped.detail}`.trim());
      setHits(null);
    } finally {
      setBusy('');
    }
  };

  return (
    <div className="pb-2" data-testid="ask-your-book">
      <div className="px-3 pb-2">
        {/* Status line */}
        <div className="flex items-center gap-2 flex-wrap text-2xs text-mut">
          <Database size={10} className="text-accent shrink-0" />
          {status ? (
            <>
              <span className="font-mono">{status.embedder}</span>
              <span>· {status.chunks} chunks</span>
              {Object.entries(status.by_source).map(([src, n]) => (
                <span key={src} className="font-mono">· {src}:{n}</span>
              ))}
              {indexedAt && <span className="text-ok">· indexed</span>}
            </>
          ) : (
            <span>corpus status unknown — index to populate</span>
          )}
          <button
            type="button"
            onClick={() => void indexNow()}
            disabled={busy === 'index'}
            className="ml-auto flex items-center gap-1 text-2xs px-2 py-0.5 rounded border border-line hover:border-accent/50 hover:text-ink disabled:opacity-50"
            title="rag_index over lorebook + characters + chapters"
          >
            {busy === 'index' ? <Loader2 size={10} className="animate-spin" /> : <Database size={10} />}
            Index now
          </button>
        </div>

        {/* Query box */}
        <div className="mt-1.5 flex items-center gap-1.5 rounded-md border border-line bg-panel2 px-2 py-1">
          <Search size={11} className="text-mut shrink-0" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') void ask();
              if (e.key === 'Escape') {
                setQuery('');
                setHits(null);
                setError(null);
              }
            }}
            placeholder={corpusEmpty ? 'Index your book first, then ask…' : 'Ask your book — Enter…'}
            className="bg-transparent outline-none text-2xs w-full placeholder:text-mut/60"
            aria-label="Ask your book"
          />
          {busy === 'search' && <Loader2 size={11} className="animate-spin text-mut" />}
        </div>

        {error && <p className="mt-1.5 text-2xs text-bad leading-relaxed">{error}</p>}

        {/* Results */}
        {hits && (
          <div className="mt-1.5 rounded-md border border-line bg-panel2 p-1.5 max-h-44 overflow-y-auto">
            {hits.length === 0 ? (
              <p className="text-2xs text-mut px-1 py-1">No passage matches that yet.</p>
            ) : (
              <>
                {hits.map((h) => (
                  <div key={h.chunk_id} className="px-1 py-1 text-2xs">
                    <div className="flex items-center gap-1.5">
                      <span className={cn('border rounded px-1 py-px font-mono', SOURCE_TONE[h.source] ?? 'border-line text-mut')}>
                        {h.source}
                      </span>
                      <span className="text-mut truncate flex-1" title={h.ref_id}>{truncate(h.ref_id, 24)}</span>
                      <span className="font-mono text-mut/70">{h.score.toFixed(3)}</span>
                    </div>
                    <p className="text-mut leading-relaxed mt-0.5">{h.snippet}</p>
                  </div>
                ))}
                {latencyMs != null && (
                  <p className="px-1 pt-1 font-mono text-mut/60">hybrid retrieval · {latencyMs.toFixed(1)} ms</p>
                )}
              </>
            )}
          </div>
        )}

        {corpusEmpty && !hits && !error && (
          <p className="mt-2 text-2xs text-mut leading-relaxed">
            The corpus is empty — <span className="text-ink">Index now</span> embeds every lore entry,
            character profile and chapter segment ({chapters.length} chapters) locally.
          </p>
        )}
      </div>
    </div>
  );
}

export const ASK_SECTION_ICON = BookMarked;
