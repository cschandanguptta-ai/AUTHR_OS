import { useState } from 'react';
import {
  BookMarked,
  BookOpen,
  ChevronDown,
  ChevronRight,
  CircleDot,
  Crosshair,
  Download,
  FilePlus2,
  Loader2,
  Mic2,
  Pencil,
  Plus,
  ScrollText,
  Search,
  Trash2,
  Users,
} from 'lucide-react';
import { useManuscriptStore } from '../../stores/manuscriptStore';
import type { SegmentSearchHit } from '../../types/ast';
import { useVoiceCastStore } from '../../stores/voiceCastStore';
import { useLoreStore } from '../../stores/loreStore';
import { useThreadStore } from '../../stores/threadStore';
import { useProjectStore } from '../../stores/projectStore';
import * as api from '../../ipc/api';
import { toast, toastError } from '../../stores/uiStore';
import {
  actLabel,
  CHAPTER_STATUS_DOTS,
  LORE_CATEGORIES,
  LORE_CATEGORY_BADGES,
  ROLE_BADGES,
} from '../../lib/constants';
import { cn, formatNumber, truncate } from '../../lib/utils';
import { SkeletonList } from '../shared/Skeleton';
import { EmptyState } from '../shared/EmptyState';
import { NewChapterModal } from './modals/NewChapterModal';
import { CharacterModal } from './modals/CharacterModal';
import { LoreModal } from './modals/LoreModal';
import { ThreadModal } from './modals/ThreadModal';
import { BinderAskBody } from './BinderAskPanel';
import { ConfirmDialog } from '../shared/ConfirmDialog';
import type { ChapterRow, CharacterRow, LoreRow } from '../../types/ast';

function SectionHeader({
  icon: Icon,
  title,
  open,
  onToggle,
  trailing,
}: {
  icon: typeof BookOpen;
  title: string;
  open: boolean;
  onToggle: () => void;
  trailing?: React.ReactNode;
}) {
  return (
    <div className="flex items-center gap-1.5 px-3 h-8 select-none">
      <button
        type="button"
        onClick={onToggle}
        className="flex items-center gap-1.5 flex-1 min-w-0 text-left group"
        aria-expanded={open}
      >
        {open ? (
          <ChevronDown size={12} className="text-mut shrink-0" />
        ) : (
          <ChevronRight size={12} className="text-mut shrink-0" />
        )}
        <Icon size={12} className="text-mut shrink-0" />
        <span className="text-2xs font-semibold tracking-wider text-mut group-hover:text-ink uppercase">
          {title}
        </span>
      </button>
      {trailing}
    </div>
  );
}

/** Manuscript tree: books → grouped by act_number → chapters with status dots. */
function ManuscriptSection({ open, onToggle }: { open: boolean; onToggle: () => void }) {
  const books = useManuscriptStore((s) => s.books);
  const chapters = useManuscriptStore((s) => s.chapters);
  const activeChapterId = useManuscriptStore((s) => s.activeChapterId);
  const selectChapter = useManuscriptStore((s) => s.selectChapter);
  const deleteChapter = useManuscriptStore((s) => s.deleteChapter);
  const loadedBooks = useManuscriptStore((s) => s.loadedBooks);
  const loadedChapters = useManuscriptStore((s) => s.loadedChapters);

  const [query, setQuery] = useState('');
  const [hits, setHits] = useState<SegmentSearchHit[] | null>(null);
  const [searching, setSearching] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<ChapterRow | null>(null);

  const doSearch = async () => {
    const q = query.trim();
    if (!q) {
      setHits(null);
      return;
    }
    setSearching(true);
    try {
      const res = await api.segmentSearch({ query: q, limit: 8 });
      setHits(res.hits);
    } catch (err) {
      toastError('segment_search failed', err);
    } finally {
      setSearching(false);
    }
  };

  return (
    <div>
      <SectionHeader icon={BookOpen} title="Manuscript" open={open} onToggle={onToggle} />
      {open && (
        <div className="pb-2">
          <div className="px-3 pb-2">
            <div className="flex items-center gap-1.5 rounded-md border border-line bg-panel2 px-2 py-1">
              <Search size={11} className="text-mut shrink-0" />
              <input
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') void doSearch();
                  if (e.key === 'Escape') {
                    setQuery('');
                    setHits(null);
                  }
                }}
                placeholder="FTS search — Enter…"
                className="bg-transparent outline-none text-2xs w-full placeholder:text-mut/60"
                aria-label="Search manuscript segments"
              />
              {searching && <Loader2 size={11} className="animate-spin text-mut" />}
            </div>
            {hits && (
              <div className="mt-1.5 rounded-md border border-line bg-panel2 p-1.5 max-h-40 overflow-y-auto">
                {hits.length === 0 ? (
                  <p className="text-2xs text-mut px-1 py-1">
                    No segment matches “{truncate(query, 24)}”.
                  </p>
                ) : (
                  hits.map((h) => (
                    <button
                      key={h.segment_id}
                      type="button"
                      onClick={() => {
                        selectChapter(h.chapter_id);
                        setHits(null);
                        setQuery('');
                      }}
                      className="w-full text-left text-2xs text-mut hover:text-ink px-1 py-1 rounded hover:bg-panel3"
                    >
                      <span className="font-mono text-accent/80">ch{h.chapter_id}</span>{' '}
                      {truncate(h.text_content, 60)}
                    </button>
                  ))
                )}
              </div>
            )}
          </div>

          {!loadedBooks || !loadedChapters ? (
            <div className="px-3">
              <SkeletonList rows={5} />
            </div>
          ) : books.length === 0 ? (
            <EmptyState
              icon={BookOpen}
              title="No books yet"
              hint="Initialize a bundle to scaffold your first working-title book."
            />
          ) : (
            books.map((book) => {
              const bookChapters = chapters.filter((c) => c.book_id === book.id);
              const acts = Array.from(new Set(bookChapters.map((c) => c.act_number))).sort(
                (a, b) => a - b,
              );
              return (
                <div key={book.id} className="mb-2">
                  <div className="flex items-center gap-2 px-3 py-1 bg-panel2/40">
                    <BookOpen size={12} className="text-accent/80 shrink-0" />
                    <span className="text-[13px] font-medium truncate">{book.title}</span>
                    <span className="ml-auto text-2xs font-mono text-mut shrink-0">
                      {bookChapters.length} ch
                    </span>
                  </div>
                  {acts.map((act) => (
                    <div key={act}>
                      <div className="px-3 pt-1.5 pb-0.5 text-2xs text-mut/80 uppercase tracking-wider">
                        {actLabel(act)}
                      </div>
                      {bookChapters
                        .filter((c) => c.act_number === act)
                        .map((chapter) => (
                          <ChapterRowButton
                            key={chapter.id}
                            chapter={chapter}
                            active={chapter.id === activeChapterId}
                            onSelect={() => selectChapter(chapter.id)}
                            onDelete={() => setDeleteTarget(chapter)}
                          />
                        ))}
                    </div>
                  ))}
                  {bookChapters.length === 0 && (
                    <p className="px-3 py-1.5 text-2xs text-mut">
                      No chapters — start with “New Chapter” below.
                    </p>
                  )}
                </div>
              );
            })
          )}
        </div>
      )}
      <ConfirmDialog
        open={deleteTarget !== null}
        title={`Delete chapter ${deleteTarget ? String(deleteTarget.chapter_number).padStart(2, '0') : ''}?`}
        body={deleteTarget
          ? `“${deleteTarget.title}” and all of its segments, threads and shadow files will be removed from the bundle. This cascades and cannot be undone.`
          : ''}
        confirmLabel="Delete chapter"
        onConfirm={() => {
          if (deleteTarget) void deleteChapter(deleteTarget.id);
          setDeleteTarget(null);
        }}
        onClose={() => setDeleteTarget(null)}
      />
    </div>
  );
}

function ChapterRowButton({
  chapter,
  active,
  onSelect,
  onDelete,
}: {
  chapter: ChapterRow;
  active: boolean;
  onSelect: () => void;
  onDelete: () => void;
}) {
  return (
    <div
      className={cn(
        'group flex items-center transition-colors rounded-sm',
        active ? 'bg-panel3' : 'hover:bg-panel2',
      )}
    >
      <button
        type="button"
        onClick={onSelect}
        className={cn(
          'flex-1 min-w-0 flex items-center gap-2 px-3 py-1 text-left text-[13px]',
          active ? 'text-ink' : 'text-mut hover:text-ink',
        )}
        aria-current={active ? 'true' : undefined}
      >
        <span
          className={cn('w-1.5 h-1.5 rounded-full shrink-0', CHAPTER_STATUS_DOTS[chapter.status])}
          title={chapter.status}
        />
        <span className="font-mono text-2xs text-mut/70 shrink-0 w-7">
          {String(chapter.chapter_number).padStart(2, '0')}
        </span>
        <span className="truncate">{chapter.title}</span>
      </button>
      <button
        type="button"
        onClick={onDelete}
        className="opacity-0 group-hover:opacity-100 mr-2 text-mut hover:text-bad shrink-0"
        aria-label={`Delete chapter ${chapter.title}`}
        title="Delete chapter — asks for confirmation"
      >
        <Trash2 size={11} />
      </button>
    </div>
  );
}

/** Story Bible: lore entries grouped by category. */
function LoreSection({ open, onToggle }: { open: boolean; onToggle: () => void }) {
  const lore = useLoreStore((s) => s.lore);
  const loaded = useLoreStore((s) => s.loaded);
  const deleteLore = useLoreStore((s) => s.deleteLore);
  const [deleteTarget, setDeleteTarget] = useState<LoreRow | null>(null);

  return (
    <div>
      <SectionHeader icon={ScrollText} title="Story Bible" open={open} onToggle={onToggle} />
      {open && (
        <div className="pb-2">
          {!loaded ? (
            <div className="px-3">
              <SkeletonList rows={3} />
            </div>
          ) : lore.length === 0 ? (
            <EmptyState
              icon={ScrollText}
              title="Lorebook empty"
              hint="Record locations, factions and magic-tech rules so the prose stays consistent."
            />
          ) : (
            LORE_CATEGORIES.map((category) => {
              const entries = lore.filter((l) => l.category === category);
              if (!entries.length) return null;
              return (
                <div key={category}>
                  <div className="px-3 pt-1.5 pb-0.5 text-2xs text-mut/80 uppercase tracking-wider">
                    {category.replace('_', ' ')}
                  </div>
                  {entries.map((entry) => (
                    <div
                      key={entry.key}
                      className="group flex items-center gap-2 px-3 py-1 hover:bg-panel2"
                      title={entry.content}
                    >
                      <CircleDot size={10} className="text-mut shrink-0" />
                      <span className="text-[13px] text-mut group-hover:text-ink truncate flex-1">
                        {entry.key}
                      </span>
                      <span
                        className={cn(
                          'text-2xs border rounded px-1 py-px shrink-0',
                          LORE_CATEGORY_BADGES[entry.category] ?? LORE_CATEGORY_BADGES.GENERAL,
                        )}
                      >
                        {entry.ipa_phonetic ? 'IPA' : ''}
                      </span>
                      <button
                        type="button"
                        onClick={() => setDeleteTarget(entry)}
                        className="opacity-0 group-hover:opacity-100 text-mut hover:text-bad"
                        aria-label={`Delete lore entry ${entry.key}`}
                        title="Delete lore entry — asks for confirmation"
                      >
                        <Trash2 size={11} />
                      </button>
                    </div>
                  ))}
                </div>
              );
            })
          )}
        </div>
      )}
      <ConfirmDialog
        open={deleteTarget !== null}
        title="Delete lore entry?"
        body={deleteTarget
          ? `“${deleteTarget.key}” will be removed from the Story Bible. Its IPA pronunciation stops applying at the next synthesis.`
          : ''}
        confirmLabel="Delete entry"
        onConfirm={() => {
          if (deleteTarget) void deleteLore(deleteTarget.key);
          setDeleteTarget(null);
        }}
        onClose={() => setDeleteTarget(null)}
      />
    </div>
  );
}

/** Ask your book: local corpus Q&A (rag_index/search/status). */
function AskSection({ open, onToggle }: { open: boolean; onToggle: () => void }) {
  return (
    <div>
      <SectionHeader icon={BookMarked} title="Ask your book" open={open} onToggle={onToggle} />
      {open && <BinderAskBody />}
    </div>
  );
}

/** Voice Cast Rack: characters with role badge, engine chip and IPA phonetic. */
function VoiceCastSection({ open, onToggle }: { open: boolean; onToggle: () => void }) {
  const characters = useVoiceCastStore((s) => s.characters);
  const loaded = useVoiceCastStore((s) => s.loaded);
  const [editTarget, setEditTarget] = useState<CharacterRow | null>(null);

  return (
    <div>
      <SectionHeader icon={Users} title="Voice Cast Rack" open={open} onToggle={onToggle} />
      {open && (
        <div className="pb-2">
          {!loaded ? (
            <div className="px-3">
              <SkeletonList rows={4} />
            </div>
          ) : characters.length === 0 ? (
            <EmptyState
              icon={Users}
              title="No voices cast"
              hint="Add your protagonist, antagonist and supporting cast with engine + prosody baselines."
            />
          ) : (
            characters.map((c) => (
              <div key={c.character_key} className="px-3 py-1.5 hover:bg-panel2 group">
                <div className="flex items-center gap-2">
                  <Mic2 size={11} className="text-mut shrink-0" />
                  <span className="text-[13px] truncate">{c.display_name}</span>
                  <span
                    className={cn(
                      'ml-auto text-2xs border rounded px-1 py-px shrink-0',
                      ROLE_BADGES[c.role] ?? ROLE_BADGES.SUPPORTING,
                    )}
                  >
                    {c.role}
                  </span>
                  <button
                    type="button"
                    onClick={() => setEditTarget(c)}
                    className="opacity-0 group-hover:opacity-100 text-mut hover:text-accent shrink-0"
                    aria-label={`Edit ${c.display_name}`}
                    title="Edit voice profile — opens the cast editor"
                  >
                    <Pencil size={11} />
                  </button>
                </div>
                <div className="flex items-center gap-2 pl-5 mt-0.5">
                  <span className="text-2xs font-mono text-accent/80 border border-accent/30 bg-accent/10 rounded px-1">
                    {c.voice_engine || 'unassigned'}
                  </span>
                  {c.ipa_phonetic && (
                    <span className="text-2xs font-mono text-mut truncate" title={c.ipa_phonetic}>
                      /{c.ipa_phonetic}/
                    </span>
                  )}
                </div>
              </div>
            ))
          )}
        </div>
      )}
      <CharacterModal key={editTarget?.character_key ?? 'cast-new'} open={editTarget !== null} edit={editTarget ?? undefined} onClose={() => setEditTarget(null)} />
    </div>
  );
}

/** Chekhov Kanban: threads grouped by status in three mini-columns. */
function ChekhovSection({ open, onToggle }: { open: boolean; onToggle: () => void }) {
  const threads = useThreadStore((s) => s.threads);
  const loaded = useThreadStore((s) => s.loaded);
  const resolveThread = useThreadStore((s) => s.resolveThread);
  const [newThread, setNewThread] = useState(false);

  const columns: Array<'INTRODUCED' | 'ESCALATED' | 'RESOLVED'> = [
    'INTRODUCED',
    'ESCALATED',
    'RESOLVED',
  ];
  const columnTone: Record<string, string> = {
    INTRODUCED: 'text-warn/90',
    ESCALATED: 'text-bad/90',
    RESOLVED: 'text-ok/90',
  };

  return (
    <div>
      <SectionHeader
        icon={Crosshair}
        title="Chekhov Kanban"
        open={open}
        onToggle={onToggle}
        trailing={
          open ? (
            <button
              type="button"
              onClick={() => setNewThread(true)}
              className="flex items-center gap-1 text-2xs text-mut hover:text-accent border border-line rounded px-1.5 py-0.5 hover:border-accent/40"
              aria-label="New thread"
            >
              <Plus size={10} /> New
            </button>
          ) : undefined
        }
      />
      {open && (
        <div className="pb-2 px-2">
          {!loaded ? (
            <div className="px-1">
              <SkeletonList rows={3} />
            </div>
          ) : threads.length === 0 ? (
            <EmptyState
              icon={Crosshair}
              title="No threads on the board"
              hint="Log every planted gun — the ledger tracks intro chapter to payoff."
              action={
                <button
                  type="button"
                  onClick={() => setNewThread(true)}
                  className="text-2xs text-accent hover:underline"
                >
                  Open your first thread
                </button>
              }
            />
          ) : (
            <div className="grid grid-cols-3 gap-1.5">
              {columns.map((status) => {
                const items = threads.filter((t) => t.status === status);
                return (
                  <div key={status} className="rounded-md border border-line bg-panel2/50 p-1.5">
                    <div
                      className={cn(
                        'text-2xs font-semibold tracking-wide mb-1',
                        columnTone[status],
                      )}
                    >
                      {status.slice(0, 4)} · {items.length}
                    </div>
                    <div className="flex flex-col gap-1">
                      {items.map((t) => (
                        <div
                          key={t.id}
                          className="rounded border border-line bg-panel px-1.5 py-1 group"
                          title={t.payoff_requirement}
                        >
                          <div className="text-2xs leading-tight text-ink">
                            {truncate(t.thread_title, 26)}
                          </div>
                          <div className="flex items-center justify-between mt-0.5">
                            <span className="font-mono text-2xs text-mut">
                              ch{t.introduced_in_chapter}
                            </span>
                            {status !== 'RESOLVED' && (
                              <button
                                type="button"
                                onClick={() => void resolveThread(t.id)}
                                className="text-2xs text-mut hover:text-ok"
                                title="Resolve in the open chapter"
                                aria-label={`Resolve thread ${t.thread_title}`}
                              >
                                resolve
                              </button>
                            )}
                            {status === 'RESOLVED' && t.resolved_in_chapter && (
                              <span className="font-mono text-2xs text-ok/80">
                                →ch{t.resolved_in_chapter}
                              </span>
                            )}
                          </div>
                        </div>
                      ))}
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      )}
      <ThreadModal open={newThread} onClose={() => setNewThread(false)} />
    </div>
  );
}

export function ProjectBinder() {
  const [sections, setSections] = useState({
    manuscript: true,
    lore: true,
    cast: true,
    chekhov: true,
    ask: false,
  });
  const [newChapter, setNewChapter] = useState(false);
  const [newCharacter, setNewCharacter] = useState(false);
  const [newLore, setNewLore] = useState(false);
  const chapterCount = useManuscriptStore((s) => s.chapters.length);
  const totalWords = useManuscriptStore((s) =>
    s.chapters.reduce((acc, c) => acc + (c.actual_word_count ?? 0), 0),
  );
  const bundlePath = useProjectStore((s) => s.bundlePath);

  const toggle = (key: keyof typeof sections) =>
    setSections((s) => ({ ...s, [key]: !s[key] }));

  const exportZip = async () => {
    if (!bundlePath) return;
    const out = `${bundlePath.replace(/\/$/, '')}/exports/authr-export-${new Date()
      .toISOString()
      .slice(0, 19)
      .replace('T', '-')}.zip`;
    try {
      const res = await api.projectExport({ out });
      toast('success', 'Bundle exported', `${res.out} · ${res.bytes} bytes`);
    } catch (err) {
      toastError('project_export failed', err);
    }
  };

  return (
    <div className="flex flex-col min-h-0 flex-1">
      <div className="flex-1 min-h-0 overflow-y-auto pt-1">
        <ManuscriptSection open={sections.manuscript} onToggle={() => toggle('manuscript')} />
        <div className="mx-3 my-1 border-t border-line" />
        <LoreSection open={sections.lore} onToggle={() => toggle('lore')} />
        <div className="mx-3 my-1 border-t border-line" />
        <VoiceCastSection open={sections.cast} onToggle={() => toggle('cast')} />
        <div className="mx-3 my-1 border-t border-line" />
        <ChekhovSection open={sections.chekhov} onToggle={() => toggle('chekhov')} />
        <div className="mx-3 my-1 border-t border-line" />
        <AskSection open={sections.ask} onToggle={() => toggle('ask')} />
      </div>

      <footer className="border-t border-line bg-panel2/40 p-2 grid grid-cols-2 gap-1.5">
        <button
          type="button"
          onClick={() => setNewChapter(true)}
          className="flex items-center justify-center gap-1.5 text-2xs px-2 py-1.5 rounded-md border border-line bg-panel hover:bg-panel3 transition-colors text-ink"
        >
          <FilePlus2 size={12} className="text-accent" /> New Chapter
        </button>
        <button
          type="button"
          onClick={() => setNewCharacter(true)}
          className="flex items-center justify-center gap-1.5 text-2xs px-2 py-1.5 rounded-md border border-line bg-panel hover:bg-panel3 transition-colors text-ink"
        >
          <Users size={12} className="text-accent" /> New Character
        </button>
        <button
          type="button"
          onClick={() => setNewLore(true)}
          className="flex items-center justify-center gap-1.5 text-2xs px-2 py-1.5 rounded-md border border-line bg-panel hover:bg-panel3 transition-colors text-ink"
        >
          <ScrollText size={12} className="text-accent" /> New Lore
        </button>
        <button
          type="button"
          onClick={() => void exportZip()}
          className="flex items-center justify-center gap-1.5 text-2xs px-2 py-1.5 rounded-md border border-line bg-panel hover:bg-panel3 transition-colors text-ink"
        >
          <Download size={12} className="text-accent" /> Export ZIP
        </button>
        <div className="col-span-2 flex items-center justify-between px-1 pt-1 text-2xs text-mut">
          <span>{formatNumber(chapterCount)} chapters</span>
          <span className="font-mono">{formatNumber(totalWords)} words (saved)</span>
        </div>
      </footer>

      <NewChapterModal open={newChapter} onClose={() => setNewChapter(false)} />
      <CharacterModal open={newCharacter} onClose={() => setNewCharacter(false)} />
      <LoreModal open={newLore} onClose={() => setNewLore(false)} />
    </div>
  );
}

