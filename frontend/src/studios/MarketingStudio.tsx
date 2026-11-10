import type { JSX } from 'react';
/**
 * Marketing Studio (Sprint 8) — SEO metadata, keywords, Midjourney cover
 * prompts and social hooks, all derived from live bundle data. Genre /
 * status / target word count persist via `book_update`; SEO copy is a
 * session workspace until the Phase 6 publishing fields land.
 */

import { useMemo, useState } from 'react';
import { useBinder } from '../stores/binderStore';
import { useLedger } from '../stores/ledgerStore';
import { useVoiceCast } from '../stores/voiceCastStore';
import { useLore } from '../stores/loreStore';
import { buildKeywords, buildMidjourneyPrompt, buildSocialHooks } from './marketing';
import { ENUMS } from '../api';

export function MarketingStudio(): JSX.Element {
  const books = useBinder((s) => s.books);
  const updateBook = useBinder((s) => s.updateBook);
  const threads = useLedger((s) => s.threads);
  const characters = useVoiceCast((s) => s.characters);
  const lore = useLore((s) => s.lore);
  const [bookId, setBookId] = useState<number | null>(null);
  const [copied, setCopied] = useState<string | null>(null);

  const activeBookId = bookId ?? books[0]?.id ?? null;
  const book = books.find((b) => b.id === activeBookId) ?? null;
  const prompt = useMemo(
    () => (book ? buildMidjourneyPrompt(book, characters, lore) : ''),
    [book, characters, lore],
  );
  const keywords = useMemo(
    () => (book ? buildKeywords(book, characters, lore) : []),
    [book, characters, lore],
  );
  const hooks = useMemo(
    () => (book ? buildSocialHooks(book, threads) : []),
    [book, threads],
  );

  if (!book) {
    return (
      <div className="studio">
        <header className="studio-head"><h2>Marketing Studio</h2></header>
        <div className="empty">Create a book to unlock marketing metadata.</div>
      </div>
    );
  }

  const copy = async (text: string, tag: string): Promise<void> => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(tag);
      window.setTimeout(() => setCopied(null), 1500);
    } catch {
      setCopied(null);
    }
  };

  return (
    <div className="studio">
      <header className="studio-head">
        <h2>Marketing Studio</h2>
        <select value={activeBookId ?? ''} onChange={(e) => setBookId(Number(e.target.value))}>
          {books.map((b) => <option key={b.id} value={b.id}>{b.title}</option>)}
        </select>
      </header>

      <section className="studio-grid">
        <div className="studio-card">
          <h3>SEO Metadata</h3>
          <Field label="Title (read-only from bundle)">
            <input value={book.title} readOnly />
          </Field>
          <Field label="Genre (persists via book_update)">
            <input
              defaultValue={book.genre ?? ''}
              onBlur={(e) => {
                if (e.target.value !== (book.genre ?? '')) void updateBook(book.id, { genre: e.target.value });
              }}
            />
          </Field>
          <Field label="Status">
            <select value={book.status} onChange={(e) => void updateBook(book.id, { status: e.target.value })}>
              {ENUMS.book_status.map((s) => <option key={s} value={s}>{s}</option>)}
            </select>
          </Field>
          <Field label="Target word count">
            <input
              type="number"
              defaultValue={book.target_word_count ?? 0}
              onBlur={(e) => {
                const n = Number(e.target.value);
                if (n > 0 && n !== book.target_word_count) void updateBook(book.id, { target_word_count: n });
              }}
            />
          </Field>
          <div className="mono dim">word_count: {book.word_count.toLocaleString()}</div>
        </div>

        <div className="studio-card">
          <h3>Keywords <span className="mono dim">{keywords.length}</span></h3>
          <div className="keyword-cloud" data-testid="keyword-cloud">
            {keywords.map((k) => <span key={k} className="kw">{k}</span>)}
          </div>
          <button className="btn sm" onClick={() => void copy(keywords.join(', '), 'kw')}>Copy keywords</button>
          {copied === 'kw' && <span className="pill ok">copied</span>}
        </div>

        <div className="studio-card">
          <h3>Midjourney Cover Prompt</h3>
          <pre className="mono prompt-view" data-testid="mj-prompt">{prompt}</pre>
          <button className="btn sm" onClick={() => void copy(prompt, 'mj')}>Copy prompt</button>
          {copied === 'mj' && <span className="pill ok">copied</span>}
        </div>

        <div className="studio-card">
          <h3>Social Hooks <span className="dim">from the Chekhov ledger</span></h3>
          <ul className="hook-list" data-testid="social-hooks">
            {hooks.map((h, i) => <li key={i}>{h}</li>)}
          </ul>
        </div>
      </section>
    </div>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }): JSX.Element {
  return (
    <label className="field">
      <span className="field-label">{label}</span>
      {children}
    </label>
  );
}
