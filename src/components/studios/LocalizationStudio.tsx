// LocalizationStudio — Restored Studio 4 (Sprint 8): multilingual translation
// workspace over chapter segments plus a YouTube script import parser.
// Translation memory / auto-translate arrive with later phases; the import
// parser and per-language draft storage are fully local and real.

import { useEffect, useMemo, useState } from 'react';
import { Globe2, Import, Languages, Plus, Trash2 } from 'lucide-react';
import { StudioShell } from '../shared/StudioShell';
import { EmptyState } from '../shared/EmptyState';
import { useManuscriptStore } from '../../stores/manuscriptStore';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { btnDanger, btnPrimary, inputClass } from '../layout/Modal';
import { cn } from '../../lib/utils';
import { toast } from '../../stores/uiStore';

interface LangEntry {
  id: string;
  name: string;
  translations: Record<string, string>; // segment_id → draft
}

const STARTER_LANGUAGES = ['Español', 'Français', 'Deutsch', 'Português (BR)'];

/** Segment-order-preserving YouTube script import (two-space or blank-line breaks). */
function parseScript(raw: string): Array<{ text: string; type: string }> {
  return raw
    .split(/\n\s*\n/)
    .map((chunk) => chunk.trim())
    .filter(Boolean)
    .map((chunk) => {
      const lines = chunk.split('\n').map((l) => l.trim()).filter(Boolean);
      const text = lines.join(' ').replace(/\s+/g, ' ');
      const isDialogue = /^(["“].*["”]|—\s*|-\s*)/.test(text);
      return { text, type: isDialogue ? 'DIALOGUE' : 'NARRATION' };
    });
}

export function LocalizationStudio() {
  const segments = useManuscriptStore((s) => s.segments);
  const chapters = useManuscriptStore((s) => s.chapters);
  const activeChapterId = useManuscriptStore((s) => s.activeChapterId);
  const selectChapter = useManuscriptStore((s) => s.selectChapter);
  const loadSegments = useManuscriptStore((s) => s.loadSegments);
  const activeStudio = useWorkspaceStore((s) => s.activeStudio);

  const [languages, setLanguages] = useState<LangEntry[]>([]);
  const [activeLang, setActiveLang] = useState<string>('');
  const [importText, setImportText] = useState('');
  const [newLang, setNewLang] = useState('');

  useEffect(() => {
    if (activeStudio === 'localization') void loadSegments(activeChapterId ?? 0);
  }, [activeStudio, activeChapterId, loadSegments]);

  const lang = languages.find((l) => l.id === activeLang) ?? null;
  const chapter = chapters.find((c) => c.id === activeChapterId) ?? null;

  const parsed = useMemo(() => parseScript(importText), [importText]);

  const addLanguage = (name: string) => {
    const id = `${name}-${Date.now()}`;
    setLanguages((l) => [...l, { id, name, translations: {} }]);
    setActiveLang(id);
  };

  const setTranslation = (segmentId: string, value: string) => {
    setLanguages((ls) =>
      ls.map((l) =>
        l.id === activeLang ? { ...l, translations: { ...l.translations, [segmentId]: value } } : l,
      ),
    );
  };

  const importScript = () => {
    if (!chapter) {
      toast('warn', 'No chapter', 'Select a chapter to import into first.');
      return;
    }
    if (parsed.length === 0) {
      toast('warn', 'Nothing to import', 'Paste a script with blank lines between paragraphs.');
      return;
    }
    toast(
      'success',
      `Parsed ${parsed.length} paragraphs`,
      'Import lands as chapter segments in a later phase — preview validated.',
    );
  };

  return (
    <StudioShell
      icon={Globe2}
      title="Localization Studio"
      subtitle="Multilingual translation drafts · YouTube script import · sync-ready segment tables"
    >
      <div className="pb-10">
        {/* Language rail */}
        <div className="px-6 pt-5 flex items-center gap-2 flex-wrap">
          <Languages size={13} className="text-accent" />
          {languages.map((l) => (
            <span key={l.id} className="inline-flex items-center">
              <button
                type="button"
                onClick={() => setActiveLang(l.id)}
                className={cn(
                  'px-2.5 py-1 text-2xs rounded-lg border transition-colors',
                  activeLang === l.id
                    ? 'border-accent/60 bg-accent/10 text-accent font-semibold'
                    : 'border-line text-mut hover:text-ink',
                )}
              >
                {l.name}
              </button>
              <button
                type="button"
                aria-label={`Remove ${l.name}`}
                onClick={() => {
                  setLanguages((ls) => ls.filter((x) => x.id !== l.id));
                  if (activeLang === l.id) setActiveLang('');
                }}
                className="ml-0.5 text-mut hover:text-red-400"
              >
                <Trash2 size={11} />
              </button>
            </span>
          ))}
          <span className="flex items-center gap-1.5 ml-2">
            <input
              className={cn(inputClass, 'w-40 py-1 text-2xs')}
              placeholder="Add language…"
              value={newLang}
              onChange={(e) => setNewLang(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && newLang.trim()) {
                  addLanguage(newLang.trim());
                  setNewLang('');
                }
              }}
            />
            <button
              type="button"
              aria-label="Add language"
              className={cn(btnPrimary, 'text-2xs px-2 py-1.5 flex items-center gap-1')}
              onClick={() => {
                if (newLang.trim()) {
                  addLanguage(newLang.trim());
                  setNewLang('');
                }
              }}
            >
              <Plus size={11} /> Add
            </button>
            {languages.length === 0 && STARTER_LANGUAGES.map((name) => (
              <button
                key={name}
                type="button"
                className="text-2xs px-2 py-1 rounded-lg border border-dashed border-line text-mut hover:text-ink"
                onClick={() => addLanguage(name)}
              >
                + {name}
              </button>
            ))}
          </span>
        </div>

        <div className="px-6 pt-4 flex items-center gap-3 flex-wrap">
          <label className="flex items-center gap-2">
            <span className="text-2xs text-mut uppercase tracking-wide">Source chapter</span>
            <select
              className={cn(inputClass, 'w-56')}
              value={activeChapterId ?? ''}
              onChange={(e) => selectChapter(Number(e.target.value))}
            >
              {chapters.map((c) => (
                <option key={c.id} value={c.id}>Ch {c.chapter_number} — {c.title}</option>
              ))}
            </select>
          </label>
          {lang && (
            <span className="font-mono text-2xs text-mut">
              {Object.values(lang.translations).filter(Boolean).length}/{segments.length} translated
            </span>
          )}
        </div>

        {segments.length === 0 || !lang ? (
          <EmptyState
            icon={Languages}
            title={lang ? 'No segments in this chapter' : 'Add a target language'}
            hint={
              lang
                ? 'Pick a chapter with drafted segments to start translating.'
                : 'Choose a starter language above or type your own to open the translation table.'
            }
          />
        ) : (
          <div className="px-6 pt-4 grid gap-1.5">
            {segments.map((s) => (
              <div key={s.segment_id} className="grid grid-cols-[1fr_1fr] gap-2 items-start border border-line rounded-lg bg-panel2 px-3 py-2">
                <div>
                  <div className="text-2xs text-mut font-mono mb-0.5">
                    #{s.sequence_order} · {s.segment_type.toLowerCase().replace('_', ' ')}
                  </div>
                  <div className="text-xs leading-relaxed">{s.text_content}</div>
                </div>
                <textarea
                  className={cn(inputClass, 'text-xs min-h-[54px] leading-relaxed')}
                  placeholder={`${lang.name} translation…`}
                  value={lang.translations[s.segment_id] ?? ''}
                  onChange={(e) => setTranslation(s.segment_id, e.target.value)}
                />
              </div>
            ))}
          </div>
        )}

        {/* YouTube script import */}
        <div className="px-6 pt-6">
          <div className="border border-line rounded-xl bg-panel2 px-4 py-3">
            <div className="flex items-center gap-2 text-xs font-semibold mb-2">
              <Import size={13} className="text-accent" /> YouTube script import
            </div>
            <textarea
              className={cn(inputClass, 'text-xs min-h-[110px] font-mono')}
              placeholder="Paste a script. Separate paragraphs with blank lines — dialogue starting with quotes or dashes is detected automatically."
              value={importText}
              onChange={(e) => setImportText(e.target.value)}
            />
            <div className="flex items-center gap-3 mt-2">
              <span className="text-2xs text-mut font-mono">{parsed.length} paragraphs parsed</span>
              <button type="button" onClick={importScript} className={cn(btnPrimary, 'text-2xs px-3 py-1.5 flex items-center gap-1')}>
                <Import size={11} /> Import preview
              </button>
              <button
                type="button"
                onClick={() => setImportText('')}
                className={cn(btnDanger, 'text-2xs px-2 py-1.5')}
              >
                Clear
              </button>
            </div>
            {parsed.length > 0 && (
              <div className="mt-3 grid gap-1 max-h-40 overflow-y-auto">
                {parsed.slice(0, 12).map((p, i) => (
                  <div key={i} className="text-2xs text-mut border-b border-line/60 py-1">
                    <span className="font-mono text-accent mr-2">{p.type}</span>
                    {p.text.slice(0, 110)}
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      </div>
    </StudioShell>
  );
}
