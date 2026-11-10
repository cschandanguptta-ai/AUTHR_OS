import type { JSX } from 'react';
/**
 * The Project Binder — left pane of the Author's Cockpit (Sprint 6).
 *
 * Tree sections:
 *   Manuscript        books → chapters (click = open in the canvas)
 *   Story Bible       lorebook entries by category
 *   Voice Cast Rack   character profiles (click = inspect / cast)
 *   Chekhov Kanban    unresolved threads by status (click = inspect)
 */

import { useState } from 'react';
import { useBinder, chaptersOf } from '../stores/binderStore';
import { useLore } from '../stores/loreStore';
import { useLedger, THREAD_STATUSES, kanban } from '../stores/ledgerStore';
import { useVoiceCast, castOrder } from '../stores/voiceCastStore';
import { useManuscript } from '../stores/manuscriptStore';
import { useUi } from '../stores/uiStore';
import { useSession } from '../stores/sessionStore';

type SectionKey = 'manuscript' | 'bible' | 'cast' | 'ledger';

export function ProjectBinder(): JSX.Element {
  const [open, setOpen] = useState<Record<SectionKey, boolean>>({
    manuscript: true,
    bible: false,
    cast: true,
    ledger: true,
  });
  const toggle = (k: SectionKey): void => setOpen((o) => ({ ...o, [k]: !o[k] }));

  const mounted = useSession((s) => s.mountedRoot !== null);
  const selection = useUi((s) => s.selection);
  const select = useUi((s) => s.select);
  const books = useBinder((s) => s.books);
  const chapters = useBinder((s) => s.chapters);
  const lore = useLore((s) => s.lore);
  const threads = useLedger((s) => s.threads);
  const characters = useVoiceCast((s) => s.characters);
  const canvasChapterId = selection.canvasChapterId;

  if (!mounted) {
    return <div className="binder"><div className="empty">Mount a bundle to open the binder.</div></div>;
  }

  return (
    <nav className="binder" aria-label="Project Binder">
      <BinderSection
        title="Manuscript"
        open={open.manuscript}
        onToggle={() => toggle('manuscript')}
        meta={`${books.length} books`}
      >
        {books.map((book) => (
          <div key={book.id} className="binder-book">
            <div className="binder-row strong">
              <span className="binder-caret" />
              <span className="binder-label" title={book.title}>{book.title}</span>
              <span className="mono dim">{book.status}</span>
            </div>
            <div className="binder-chapters">
              {chaptersOf(chapters, book.id).map((ch) => (
                <button
                  key={ch.id}
                  className={`binder-row leaf ${canvasChapterId === ch.id ? 'active' : ''}`}
                  onClick={() => {
                    select({ bookId: book.id, chapterId: ch.id, canvasChapterId: ch.id, selectedSegmentId: null });
                    void useManuscript.getState().load(ch.id);
                  }}
                  title={ch.summary || ch.title}
                >
                  <span className="mono dim">ch{String(ch.chapter_index).padStart(2, '0')}</span>
                  <span className="binder-label">{ch.title}</span>
                  <span className="mono dim">{ch.status}</span>
                </button>
              ))}
              <button
                className="binder-row leaf ghost"
                onClick={() => {
                  const title = window.prompt(`New chapter in "${book.title}"`, 'Untitled Chapter');
                  if (title && title.trim()) void useBinder.getState().createChapter(book.id, { title: title.trim() });
                }}
              >＋ new chapter</button>
            </div>
          </div>
        ))}
        <button
          className="binder-row leaf ghost"
          onClick={() => {
            const title = window.prompt('New book title', 'Untitled Novel');
            if (title && title.trim()) void useBinder.getState().createBook({ title: title.trim() });
          }}
        >＋ new book</button>
      </BinderSection>

      <BinderSection
        title="Story Bible"
        open={open.bible}
        onToggle={() => toggle('bible')}
        meta={`${lore.length} entries`}
      >
        {lore.map((entry) => (
          <button
            key={entry.id}
            className={`binder-row leaf ${selection.selectedThreadId == null && selection.selectedCastId == null ? '' : ''}`}
            onClick={() => useUi.getState().setToast(`${entry.category} · ${entry.term}`)}
            title={entry.content}
          >
            <span className="chip-cat">{entry.category}</span>
            <span className="binder-label">{entry.term}</span>
          </button>
        ))}
        {lore.length === 0 && <div className="binder-row leaf dim">no lore yet</div>}
      </BinderSection>

      <BinderSection
        title="Voice Cast Rack"
        open={open.cast}
        onToggle={() => toggle('cast')}
        meta={`${characters.length} voices`}
      >
        {castOrder(characters).map((c) => (
          <button
            key={c.id}
            className={`binder-row leaf ${selection.selectedCastId === c.id ? 'active' : ''}`}
            onClick={() => select({ selectedCastId: c.id, selectedThreadId: null, selectedSegmentId: null })}
          >
            <span className="chip-role">{c.role}</span>
            <span className="binder-label">{c.display_name}</span>
            <span className="mono dim">{c.voice_engine}</span>
          </button>
        ))}
        {characters.length === 0 && <div className="binder-row leaf dim">no cast yet</div>}
      </BinderSection>

      <BinderSection
        title="Chekhov Kanban"
        open={open.ledger}
        onToggle={() => toggle('ledger')}
        meta={`${threads.filter((t) => t.status !== 'paid_off').length} open`}
      >
        <div className="binder-kanban">
          {THREAD_STATUSES.map((status) => (
            <div key={status} className={`kanban-col st-${status}`}>
              <div className="kanban-head mono">{status} · {kanban(threads)[status].length}</div>
              {kanban(threads)[status].map((t) => (
                <button
                  key={t.id}
                  className={`binder-row leaf ${selection.selectedThreadId === t.id ? 'active' : ''}`}
                  onClick={() => select({ selectedThreadId: t.id, selectedCastId: null, selectedSegmentId: null })}
                  title={t.notes || t.title}
                >
                  <span className="binder-label">{t.title}</span>
                </button>
              ))}
            </div>
          ))}
        </div>
      </BinderSection>
    </nav>
  );
}

function BinderSection(props: {
  title: string;
  open: boolean;
  onToggle: () => void;
  meta: string;
  children: React.ReactNode;
}): JSX.Element {
  return (
    <div className="binder-section">
      <button className="binder-head" onClick={props.onToggle} aria-expanded={props.open}>
        <span className={`binder-caret ${props.open ? 'open' : ''}`}>▸</span>
        <span className="binder-title">{props.title}</span>
        <span className="mono dim">{props.meta}</span>
      </button>
      {props.open && <div className="binder-body">{props.children}</div>}
    </div>
  );
}
