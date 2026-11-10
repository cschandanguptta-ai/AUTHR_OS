// LocalizationStudio — multilingual translation workspace over chapter
// segments plus a YouTube script import parser (R-02): drafts persist through
// studio switches and bundle switches via localizationStore (debounced 400 ms
// writes, 'Draft saved HH:MM' chip). The import parser is honest about its
// scope: it parses and previews — chapter import is a roadmap item, and the
// copy says exactly that.

import { useEffect, useMemo, useRef, useState } from 'react';
import { Check, Globe2, Import, Languages, Plus, Trash2 } from 'lucide-react';
import { StudioShell } from '../shared/StudioShell';
import { EmptyState } from '../shared/EmptyState';
import { useManuscriptStore } from '../../stores/manuscriptStore';
import { useProjectStore } from '../../stores/projectStore';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import {
  clearBundleDrafts,
  locKey,
  useLocalizationStore,
} from '../../stores/localizationStore';
import { ConfirmDialog } from '../shared/ConfirmDialog';
import { btnDanger, btnPrimary, inputClass } from '../layout/Modal';
import { cn } from '../../lib/utils';
import { toast } from '../../stores/uiStore';

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

function savedClock(savedAt: number): string {
  return new Date(savedAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
}

export function LocalizationStudio() {
  const segments = useManuscriptStore((s) => s.segments);
  const chapters = useManuscriptStore((s) => s.chapters);
  const activeChapterId = useManuscriptStore((s) => s.activeChapterId);
  const selectChapter = useManuscriptStore((s) => s.selectChapter);
  const loadSegments = useManuscriptStore((s) => s.loadSegments);
  const activeStudio = useWorkspaceStore((s) => s.activeStudio);
  const bundlePath = useProjectStore((s) => s.bundlePath) ?? '';

  const entries = useLocalizationStore((s) => s.entries);
  const saveDraft = useLocalizationStore((s) => s.saveDraft);
  const removeLanguage = useLocalizationStore((s) => s.removeLanguage);
  const ensureLanguage = useLocalizationStore((s) => s.ensureLanguage);
  const syncRail = useLocalizationStore((s) => s.syncRail);

  const [activeLang, setActiveLang] = useState<string>('');
  const [importText, setImportText] = useState('');
  const [newLang, setNewLang] = useState('');
  const [clearOpen, setClearOpen] = useState(false);
  const debounceRef = useRef<Map<string, number>>(new Map());
  const pendingRef = useRef<Map<string, { lang: string; chapterId: number; segmentId: string; value: string }>>(new Map());

  const flushKey = (k: string) => {
    const pending = pendingRef.current.get(k);
    if (!pending) return;
    pendingRef.current.delete(k);
    debounceRef.current.delete(k);
    saveDraft(bundlePath, pending.chapterId, pending.lang, pending.segmentId, pending.value, languagesRef.current);
    setEcho((e) => {
      if (e[pending.segmentId] !== pending.value) return e;
      const next = { ...e };
      delete next[pending.segmentId];
      return next;
    });
  };

  const flushAllPending = () => {
    const timers = debounceRef.current;
    timers.forEach((t) => window.clearTimeout(t));
    timers.clear();
    const keys = [...pendingRef.current.keys()];
    keys.forEach((k) => flushKey(k));
  };

  useEffect(() => {
    if (activeStudio === 'localization') void loadSegments(activeChapterId ?? 0);
  }, [activeStudio, activeChapterId, loadSegments]);

  const chapter = chapters.find((c) => c.id === activeChapterId) ?? null;

  // The language rail persists per bundle+chapter (shared across languages).
  const railKey = bundlePath && chapter ? `${bundlePath}::${chapter.id}` : null;
  const languages = useMemo(() => {
    if (!railKey || !bundlePath || !chapter) return [];
    for (const [k, v] of Object.entries(entries)) {
      if (k.startsWith(`${railKey}::`) && v.languages.length > 0) return v.languages;
    }
    return [];
  }, [entries, railKey, bundlePath, chapter]);
  const languagesRef = useRef<string[]>([]);
  languagesRef.current = languages;

  const langEntry = useMemo(() => {
    if (!bundlePath || !chapter || !activeLang) return null;
    return entries[locKey(bundlePath, chapter.id, activeLang)] ?? null;
  }, [entries, bundlePath, chapter, activeLang]);

  const storeDrafts = langEntry?.drafts ?? {};
  // Local echo for snappy typing; the persisted store receives the value on a
  // 400 ms debounce (mirror of the editor's useAutosave pattern).
  const [echo, setEcho] = useState<Record<string, string>>({});
  const translations = useMemo(() => ({ ...storeDrafts, ...echo }), [storeDrafts, echo]);
  const parsed = useMemo(() => parseScript(importText), [importText]);

  // Language/chapter switch: flush pending debounces, reset the echo layer.
  useEffect(() => {
    flushAllPending();
    setEcho({});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeLang, chapter?.id, bundlePath]);

  const addLanguage = (name: string) => {
    if (!bundlePath || !chapter) {
      toast('warn', 'No chapter', 'Select a chapter before adding a target language.');
      return;
    }
    const next = languages.includes(name) ? languages : [...languages, name];
    ensureLanguage(bundlePath, chapter.id, name, next);
    setActiveLang(name);
  };

  const dropLanguage = (name: string) => {
    if (bundlePath && chapter) removeLanguage(bundlePath, chapter.id, name);
    const next = languages.filter((l) => l !== name);
    if (bundlePath && chapter) syncRail(bundlePath, chapter.id, next);
    if (activeLang === name) setActiveLang('');
  };

  const setTranslation = (segmentId: string, value: string) => {
    if (!bundlePath || !chapter || !activeLang) return;
    setEcho((e) => ({ ...e, [segmentId]: value }));
    const k = `${activeLang}::${segmentId}`;
    pendingRef.current.set(k, { lang: activeLang, chapterId: chapter.id, segmentId, value });
    const t = debounceRef.current.get(k);
    if (t) window.clearTimeout(t);
    debounceRef.current.set(
      k,
      window.setTimeout(() => flushKey(k), 400),
    );
  };

  const savedAt = langEntry?.savedAt ?? 0;
  const translatedCount = Object.values(translations).filter(Boolean).length;

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
      'info',
      `Parsed ${parsed.length} paragraphs`,
      'Preview only — parses and checks the segment split; chapter import arrives with the import roadmap item.',
    );
  };

  const clearDrafts = () => {
    if (!bundlePath) return;
    const removed = clearBundleDrafts(bundlePath);
    setActiveLang('');
    setClearOpen(false);
    toast('info', 'Drafts cleared', `${removed} saved language draft${removed === 1 ? '' : 's'} removed for this bundle.`);
  };

  return (
    <StudioShell
      icon={Globe2}
      title="Localization Studio"
      subtitle="Multilingual translation drafts · YouTube script import · sync-ready segment tables"
      actions={
        savedAt > 0 ? (
          <span className="flex items-center gap-1.5 text-2xs text-ok" data-testid="draft-saved-chip">
            <Check size={11} /> Draft saved {savedClock(savedAt)}
          </span>
        ) : undefined
      }
    >
      <div className="pb-10">
        {/* Language rail */}
        <div className="px-6 pt-5 flex items-center gap-2 flex-wrap">
          <Languages size={13} className="text-accent" />
          {languages.map((name) => (
            <span key={name} className="inline-flex items-center">
              <button
                type="button"
                onClick={() => setActiveLang(name)}
                className={cn(
                  'px-2.5 py-1 text-2xs rounded-lg border transition-colors',
                  activeLang === name
                    ? 'border-accent/60 bg-accent/10 text-accent font-semibold'
                    : 'border-line text-mut hover:text-ink',
                )}
              >
                {name}
              </button>
              <button
                type="button"
                aria-label={`Remove ${name}`}
                onClick={() => dropLanguage(name)}
                className="ml-0.5 text-mut hover:text-bad"
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
          {languages.length > 0 && bundlePath && (
            <button
              type="button"
              onClick={() => setClearOpen(true)}
              className={cn(btnDanger, 'ml-auto text-2xs px-2 py-1')}
            >
              Clear drafts
            </button>
          )}
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
          {activeLang && chapter && (
            <span className="font-mono text-2xs text-mut">
              {translatedCount}/{segments.length} translated
            </span>
          )}
        </div>

        {segments.length === 0 || !activeLang ? (
          <EmptyState
            icon={Languages}
            title={activeLang ? 'No segments in this chapter' : 'Add a target language'}
            hint={
              activeLang
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
                  placeholder={`${activeLang} translation…`}
                  value={translations[s.segment_id] ?? ''}
                  onChange={(e) => setTranslation(s.segment_id, e.target.value)}
                />
              </div>
            ))}
          </div>
        )}

        {/* YouTube script import — honest scope */}
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
            <p className="text-2xs text-mut mt-2 leading-tight">
              Parses and previews only — persistence arrives with chapter import (roadmap).
            </p>
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

      <ConfirmDialog
        open={clearOpen}
        title="Clear localization drafts?"
        body="Every saved translation draft for this bundle (all chapters, all languages) will be removed from local storage. Bundle content is untouched — only unsaved-in-bundle drafts are lost."
        confirmLabel="Clear drafts"
        onConfirm={clearDrafts}
        onClose={() => setClearOpen(false)}
      />
    </StudioShell>
  );
}
