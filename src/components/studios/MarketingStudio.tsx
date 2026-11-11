// MarketingStudio — Restored Studio 5 (Sprint 8): SEO metadata, YouTube title/
// description builders and the Midjourney / Stable Diffusion cover prompt
// composer. Marketing fields persist through book_update (contract #9).

import { useEffect, useMemo, useState } from 'react';
import { BadgeCheck, Megaphone, PlayCircle, Sparkles, Wand2 } from 'lucide-react';
import { StudioShell } from '../shared/StudioShell';
import { EmptyState } from '../shared/EmptyState';
import { useManuscriptStore } from '../../stores/manuscriptStore';
import { useBookContextStore, resolveActiveBook } from '../../stores/bookContextStore';
import { bookUpdate } from '../../ipc/api';
import { btnPrimary, inputClass } from '../layout/Modal';
import { cn, plural } from '../../lib/utils';
import { toast, toastError } from '../../stores/uiStore';
import type { BookRow } from '../../types/ast';

const STYLE_PRESETS = [
  'cinematic matte painting',
  'moody ink illustration',
  'vintage paperback cover',
  'hyper-detailed concept art',
  'art-nouveau poster',
];

/** Deterministic local builders — no network calls, clearly labeled as drafts. */
function buildTitles(book: BookRow): string[] {
  const premise = book.premise.trim();
  const hook = premise ? premise.split(/[.!?]/)[0] : book.title;
  return [
    book.title,
    `${book.title} — ${book.genre} Series Starter`,
    `${hook.slice(0, 60)} | ${book.title}`,
    `The Complete ${book.title}: ${book.narrative_structure.replace(/_/g, ' ').toLowerCase()} edition`,
  ];
}

function buildDescription(book: BookRow): string {
  return [
    book.premise || `A ${book.genre.toLowerCase()} story about ${book.theme || 'what we owe each other'}.`,
    book.theme ? `Theme: ${book.theme}.` : '',
    `Narrative structure: ${book.narrative_structure.replace(/_/g, ' ').toLowerCase()}.`,
    `${plural(book.target_chapters, 'chapter')} · ${book.target_word_count.toLocaleString('en-US')} words.`,
    '\nSubscribe for chapter drops and behind-the-scenes audio drama.',
  ].filter(Boolean).join('\n');
}

function buildCoverPrompt(book: BookRow, style: string): string {
  const subject = book.premise.split(/[.!?]/)[0].trim() || book.title;
  return `${book.genre.toLowerCase()} book cover, ${subject.toLowerCase()} --ar 2:3 --style ${style.toLowerCase()}`;
}

function buildSocialTags(book: BookRow): string {
  const base = [book.genre.replace(/\s+/g, ''), 'audiobook', 'audioDrama', 'amwriting'];
  if (book.theme) base.push(book.theme.split(/\s+/).slice(0, 2).join('').replace(/\s+/g, ''));
  return base.map((t) => `#${t.toLowerCase()}`).join(' ');
}

export function MarketingStudio() {
  const books = useManuscriptStore((s) => s.books);
  const loadBooks = useManuscriptStore((s) => s.loadBooks);
  // R-13: premise and copy follow the global book-context chip.
  const activeBookId = useBookContextStore((s) => s.activeBookId);
  const [draft, setDraft] = useState<BookRow | null>(null);
  const [style, setStyle] = useState(STYLE_PRESETS[0]);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    void loadBooks();
  }, [loadBooks]);

  useEffect(() => {
    setDraft(resolveActiveBook(books, activeBookId));
  }, [books, activeBookId]);

  const titles = useMemo(() => (draft ? buildTitles(draft) : []), [draft]);
  const description = useMemo(() => (draft ? buildDescription(draft) : ''), [draft]);
  const coverPrompt = useMemo(() => (draft ? buildCoverPrompt(draft, style) : ''), [draft, style]);
  const tags = useMemo(() => (draft ? buildSocialTags(draft) : ''), [draft]);

  const save = async () => {
    if (!draft) return;
    setSaving(true);
    try {
      await bookUpdate({
        id: draft.id,
        premise: draft.premise,
        theme: draft.theme,
        marketing_copy: draft.marketing_copy,
        cover_prompt: draft.cover_prompt,
        isbn: draft.isbn,
      });
      toast('success', 'Marketing metadata saved', 'Persisted to the book row (contract #9).');
    } catch (err) {
      // R-08: a failed save must visibly fail — mapped copy, not silence.
      toastError('Save marketing metadata', err);
    } finally {
      setSaving(false);
    }
  };

  if (!draft) {
    return (
      <StudioShell icon={Megaphone} title="Marketing Studio" subtitle="SEO metadata · YouTube copy · cover prompts">
        <EmptyState
          icon={Megaphone}
          title="No book in this bundle"
          hint="Marketing metadata attaches to a book row. Create a book first."
        />
      </StudioShell>
    );
  }

  return (
    <StudioShell
      icon={Megaphone}
      title="Marketing Studio"
      subtitle="SEO metadata · YouTube copy builders · Midjourney cover prompts"
      actions={
        <button
          type="button"
          onClick={save}
          disabled={saving}
          className={cn(btnPrimary, 'text-2xs px-3 py-1.5 flex items-center gap-1.5')}
        >
          <BadgeCheck size={12} /> {saving ? 'Saving…' : 'Save to book'}
        </button>
      }
    >
      <div className="pb-10">
        <div className="px-6 pt-5 grid gap-3 max-w-3xl">
          <label className="grid gap-1">
            <span className="text-2xs text-mut uppercase tracking-wide">Premise (drives all builders)</span>
            <textarea
              className={cn(inputClass, 'min-h-[64px] text-xs leading-relaxed')}
              value={draft.premise}
              onChange={(e) => setDraft({ ...draft, premise: e.target.value })}
            />
          </label>
          <div className="grid grid-cols-2 gap-3">
            <label className="grid gap-1">
              <span className="text-2xs text-mut uppercase tracking-wide">Theme</span>
              <input
                className={inputClass}
                value={draft.theme}
                onChange={(e) => setDraft({ ...draft, theme: e.target.value })}
              />
            </label>
            <label className="grid gap-1">
              <span className="text-2xs text-mut uppercase tracking-wide">ISBN</span>
              <input
                className={cn(inputClass, 'font-mono')}
                value={draft.isbn}
                onChange={(e) => setDraft({ ...draft, isbn: e.target.value })}
              />
            </label>
          </div>

          {/* YouTube builders */}
          <div className="border border-line rounded-xl bg-panel2 px-4 py-3 grid gap-2">
            <div className="flex items-center gap-2 text-xs font-semibold">
              <PlayCircle size={14} className="text-bad" /> YouTube SEO builders
              <span className="text-2xs text-mut font-normal">deterministic local drafts — no network</span>
            </div>
            <div className="grid gap-1.5">
              {titles.map((t, i) => (
                <div key={i} className="flex items-center gap-2">
                  <span className="font-mono text-2xs text-mut w-6">{i + 1}</span>
                  <input
                    className={cn(inputClass, 'text-xs')}
                    value={t}
                    onFocus={(e) => e.currentTarget.select()}
                    readOnly
                  />
                </div>
              ))}
            </div>
            <label className="grid gap-1 mt-1">
              <span className="text-2xs text-mut uppercase tracking-wide flex items-center justify-between">
                Description
                <span className="font-mono">{description.length} chars</span>
              </span>
              <textarea
                className={cn(inputClass, 'min-h-[90px] text-xs leading-relaxed')}
                value={draft.marketing_copy || description}
                onChange={(e) => setDraft({ ...draft, marketing_copy: e.target.value })}
              />
            </label>
            <div className="flex items-center gap-2">
              <span className="text-2xs text-mut font-mono flex-1 truncate" title={tags}>
                {tags}
              </span>
              <button
                type="button"
                className="text-2xs px-2 py-1 rounded border border-line text-mut hover:text-ink"
                onClick={() => setDraft({ ...draft, marketing_copy: `${description}\n\n${tags}` })}
              >
                Append tags
              </button>
            </div>
          </div>

          {/* Cover prompt composer */}
          <div className="border border-line rounded-xl bg-panel2 px-4 py-3 grid gap-2">
            <div className="flex items-center gap-2 text-xs font-semibold">
              <Wand2 size={14} className="text-accent" /> Midjourney / Stable Diffusion cover composer
            </div>
            <div className="flex items-center gap-2 flex-wrap">
              {STYLE_PRESETS.map((s) => (
                <button
                  key={s}
                  type="button"
                  onClick={() => setStyle(s)}
                  className={cn(
                    'text-2xs px-2 py-1 rounded-lg border transition-colors',
                    style === s
                      ? 'border-accent/60 bg-accent/10 text-accent font-semibold'
                      : 'border-line text-mut hover:text-ink',
                  )}
                >
                  {s}
                </button>
              ))}
            </div>
            <div className="flex items-start gap-2">
              <textarea
                className={cn(inputClass, 'min-h-[64px] text-xs font-mono leading-relaxed flex-1')}
                value={draft.cover_prompt || coverPrompt}
                onChange={(e) => setDraft({ ...draft, cover_prompt: e.target.value })}
              />
              <button
                type="button"
                title="Rebuild from premise + style"
                onClick={() => setDraft({ ...draft, cover_prompt: coverPrompt })}
                className="mt-1 p-2 rounded-lg border border-line text-mut hover:text-accent"
                aria-label="Rebuild prompt"
              >
                <Sparkles size={14} />
              </button>
            </div>
            <p className="text-2xs text-mut leading-tight">
              Saved to the book row (cover_prompt) — paste it into your image generator; the Scene Studio art pipeline uses its own style catalog.
            </p>
          </div>
        </div>
      </div>
    </StudioShell>
  );
}
