import type { JSX } from 'react';
/**
 * Localization Studio (Sprint 8) — multilingual workspace and script
 * import. Pasted YouTube / plain-text scripts parse into segments and
 * import into a chapter through the frozen `segment_create` surface;
 * LLM translation mounts in Phase 5, so the memory table is local until
 * then and clearly labeled.
 */

import { useMemo, useState } from 'react';
import { useBinder } from '../stores/binderStore';
import { useManuscript } from '../stores/manuscriptStore';
import { getTransport } from '../transport';
import { recordCall } from '../stores/uiStore';
import { LANGUAGES, parseScript, parsedToSegmentPayloads } from './localization';

export function LocalizationStudio(): JSX.Element {
  const books = useBinder((s) => s.books);
  const chapters = useBinder((s) => s.chapters);
  const loadChapter = useManuscript((s) => s.load);
  const [bookId, setBookId] = useState<number | null>(null);
  const [chapterId, setChapterId] = useState<number | null>(null);
  const [script, setScript] = useState('');
  const [target, setTarget] = useState<string>(LANGUAGES[1]);
  const [memory, setMemory] = useState<{ source: string; target: string }[]>([]);
  const [imported, setImported] = useState<{ created: number; chapter: string } | null>(null);
  const [error, setError] = useState<string | null>(null);

  const parsed = useMemo(() => parseScript(script), [script]);
  const chapterOptions = chapters.filter((c) => bookId == null || c.book_id === bookId);

  const doImport = async (): Promise<void> => {
    setError(null);
    setImported(null);
    if (chapterId == null) {
      setError('Choose a target chapter first.');
      return;
    }
    if (parsed.length === 0) {
      setError('Nothing to import — paste a script first.');
      return;
    }
    let created = 0;
    let chapterTitle = '';
    for (const payload of parsedToSegmentPayloads(parsed)) {
      const env = await getTransport().invoke('segment_create', { chapter_id: chapterId, ...payload });
      recordCall('segment_create', env);
      if (!env.ok) {
        setError(`${env.error.code}: ${env.error.message}`);
        break;
      }
      created += 1;
    }
    const ch = chapters.find((c) => c.id === chapterId);
    chapterTitle = ch ? `ch${String(ch.chapter_index).padStart(2, '0')} ${ch.title}` : '';
    if (created > 0) {
      // Refresh the store rows for that chapter (source of truth).
      await loadChapter(chapterId);
      setImported({ created, chapter: chapterTitle });
      setScript('');
    }
  };

  return (
    <div className="studio">
      <header className="studio-head">
        <h2>Localization Studio</h2>
        <p className="dim">
          Multilingual translation memory + YouTube script import. Neural translation mounts
          in <b>Phase 5</b>; script import persists through the real segment pipeline today.
        </p>
      </header>

      <section className="studio-grid">
        <div className="studio-card">
          <h3>Script Import</h3>
          <div className="import-targets">
            <select
              value={bookId ?? ''}
              onChange={(e) => {
                setBookId(e.target.value ? Number(e.target.value) : null);
                setChapterId(null);
              }}
            >
              <option value="">— book —</option>
              {books.map((b) => <option key={b.id} value={b.id}>{b.title}</option>)}
            </select>
            <select
              value={chapterId ?? ''}
              onChange={(e) => setChapterId(e.target.value ? Number(e.target.value) : null)}
            >
              <option value="">— target chapter —</option>
              {chapterOptions.map((c) => (
                <option key={c.id} value={c.id}>ch{String(c.chapter_index).padStart(2, '0')} {c.title}</option>
              ))}
            </select>
          </div>
          <textarea
            className="script-input mono"
            rows={10}
            placeholder={'[00:35] MIRA: The marsh has no business knowing my mother’s name.\nNARRATOR: The lanterns answered, one by one.'}
            value={script}
            onChange={(e) => setScript(e.target.value)}
          />
          <div className="mono dim">{parsed.length} parsed lines</div>
          <button className="btn primary" onClick={() => void doImport()}>Import as segments</button>
          {error && <span className="pill bad">{error}</span>}
          {imported && (
            <span className="pill ok" data-testid="import-ok">
              imported {imported.created} segments → {imported.chapter}
            </span>
          )}
        </div>

        <div className="studio-card">
          <h3>Parse Preview</h3>
          <table className="studio-table">
            <thead><tr><th>#</th><th>Speaker</th><th>Text</th></tr></thead>
            <tbody>
              {parsed.slice(0, 12).map((line, i) => (
                <tr key={i}>
                  <td className="mono dim">{i + 1}</td>
                  <td><span className={line.speaker === 'NARRATOR' ? 'chip-cat' : 'chip-role'}>{line.speaker}</span></td>
                  <td>{line.text}</td>
                </tr>
              ))}
            </tbody>
          </table>
          {parsed.length > 12 && <div className="mono dim">…and {parsed.length - 12} more</div>}
        </div>

        <div className="studio-card">
          <h3>Translation Memory <span className="pill">Phase 5 LLM</span></h3>
          <Field label="Target language">
            <select value={target} onChange={(e) => setTarget(e.target.value)}>
              {LANGUAGES.map((l) => <option key={l} value={l}>{l}</option>)}
            </select>
          </Field>
          <div className="mono dim">
            memory rows: {memory.length} · language: {target}
          </div>
          <p className="dim small">
            The memory table and glossary sync activate with the Phase 5 local LLM; the
            import path above already writes through `segment_create` into the bundle.
          </p>
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
