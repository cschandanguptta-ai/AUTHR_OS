// PublishStudio — Publish & Export surface (D7 §5, R-04): EPUB 3 with media
// overlays, DOCX manuscript, Typst print PDF, per-chapter captions, the ACX
// M4B audiobook (disabled with a reason until its scope is fully synthesized),
// a deep link into the Craft KDP panel, and release manifest/verify under an
// Advanced disclosure. Every row runs the real command and reports the real
// artifact path + bytes — bundle ZIP export stays in the binder footer.

import { useCallback, useEffect, useMemo, useState } from 'react';
import {
  BookText,
  Captions,
  CheckCircle2,
  CircleAlert,
  Copy,
  Download,
  FileBadge,
  FileText,
  Loader2,
  PackageCheck,
  Play,
  ShieldCheck,
  Store,
  XCircle,
} from 'lucide-react';
import { StudioShell } from '../shared/StudioShell';
import { EmptyState } from '../shared/EmptyState';
import { useManuscriptStore } from '../../stores/manuscriptStore';
import { useProjectStore } from '../../stores/projectStore';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import {
  publishCaptions,
  publishDocx,
  publishEpub,
  publishM4b,
  publishTypst,
  releaseManifest,
  releaseVerify,
  segmentList,
  type PublishEpubResult,
} from '../../ipc/api';
import { mapError } from '../../ipc/errorCopy';
import { cn, formatBytes, formatMs } from '../../lib/utils';
import { btnPrimary, inputClass } from '../layout/Modal';
import type { ChapterRow } from '../../types/ast';

/** An artifact line: path + size + copy action (no shell-open exists in the
 *  frozen contract, so the path is offered verbatim, copyable). */
function ArtifactLine({ path, bytes, extra }: { path: string; bytes?: number; extra?: string }) {
  const [copied, setCopied] = useState(false);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(path);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    } catch { /* clipboard unavailable — path stays visible */ }
  };
  return (
    <div className="flex items-center gap-2 text-2xs font-mono text-ok/90" data-testid="artifact-line">
      <CheckCircle2 size={11} className="shrink-0" />
      <span className="truncate" title={path}>{path}</span>
      {bytes != null && <span className="text-mut shrink-0">{formatBytes(bytes)}</span>}
      {extra && <span className="text-mut shrink-0">{extra}</span>}
      <button
        type="button"
        onClick={() => void copy()}
        className="text-mut hover:text-ink shrink-0"
        title="Copy artifact path"
        aria-label="Copy artifact path"
      >
        {copied ? <CheckCircle2 size={11} /> : <Copy size={11} />}
      </button>
    </div>
  );
}

interface RowState {
  busy: boolean;
  error: string | null;
}

function useRowState(): [RowState, (r: Partial<RowState>) => void] {
  const [state, setState] = useState<RowState>({ busy: false, error: null });
  const patch = (r: Partial<RowState>) => setState((s) => ({ ...s, ...r }));
  return [state, patch];
}

function RowShell({
  icon: Icon,
  title,
  hint,
  state,
  onRun,
  runLabel = 'Run',
  disabled = false,
  disabledReason,
  children,
}: {
  icon: typeof BookText;
  title: string;
  hint: string;
  state: RowState;
  onRun: () => void;
  runLabel?: string;
  disabled?: boolean;
  disabledReason?: string;
  children?: React.ReactNode;
}) {
  return (
    <div className="border border-line rounded-xl bg-panel2 px-4 py-3 grid gap-2" data-testid={`publish-row-${title.replace(/\s+/g, '-').toLowerCase()}`}>
      <div className="flex items-center gap-2 flex-wrap">
        <Icon size={14} className="text-accent shrink-0" />
        <span className="text-xs font-semibold">{title}</span>
        <span className="text-2xs text-mut flex-1 min-w-[20ch]">{hint}</span>
        {state.busy ? (
          <span className="flex items-center gap-1.5 text-2xs text-accent">
            <Loader2 size={11} className="animate-spin" /> working…
          </span>
        ) : (
          <button
            type="button"
            onClick={onRun}
            disabled={disabled}
            className={cn(btnPrimary, 'text-2xs px-3 py-1.5 flex items-center gap-1')}
            title={disabledReason ?? `Run ${title.toLowerCase()}`}
          >
            <Play size={10} /> {runLabel}
          </button>
        )}
      </div>
      {disabled && disabledReason && (
        <p className="text-2xs text-warn/90 flex items-center gap-1.5">
          <CircleAlert size={11} /> {disabledReason}
        </p>
      )}
      {state.error && (
        <p className="text-2xs text-bad leading-relaxed" role="alert">{state.error}</p>
      )}
      {children}
    </div>
  );
}

function joinAbs(bundlePath: string | null, rel: string): string {
  if (!bundlePath) return rel;
  if (/^[/\\]|^[A-Za-z]:/.test(rel)) return rel; // already absolute
  return `${bundlePath.replace(/[/\\]+$/, '')}/${rel}`;
}

export function PublishStudio() {
  const books = useManuscriptStore((s) => s.books);
  const chapters = useManuscriptStore((s) => s.chapters);
  const loadBooks = useManuscriptStore((s) => s.loadBooks);
  const loadChapters = useManuscriptStore((s) => s.loadChapters);
  const bundlePath = useProjectStore((s) => s.bundlePath);
  const readonly = useProjectStore((s) => s.readonly);
  const activeStudio = useWorkspaceStore((s) => s.activeStudio);

  const [bookId, setBookId] = useState<number | null>(null);
  const [epub, setEpub] = useState<PublishEpubResult | null>(null);
  const [docx, setDocx] = useState<{ out: string; paragraphs: number; words: number } | null>(null);
  const [typ, setTyp] = useState<{ typ: string; compiled: boolean; reason?: string; pdf?: string | null } | null>(null);
  const [caps, setCaps] = useState<{ chapterId: number; lrc?: string; vtt?: string; cues: number; duration_ms: number } | null>(null);
  const [m4b, setM4b] = useState<{ out: string; bytes: number; duration_ms: number; codec: string; scope: string } | null>(null);
  const [m4bScope, setM4bScope] = useState<'book' | number>('book');
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [artifactPath, setArtifactPath] = useState('');
  const [manifestOut, setManifestOut] = useState('');
  const [manifest, setManifest] = useState<{ manifest: string; sha: string; bytes: number; key?: string } | null>(null);
  const [verify, setVerify] = useState<{ valid: boolean; sha256: boolean; signature: boolean } | null>(null);
  /** Per-chapter audio coverage: total segments vs synthesized. */
  const [coverage, setCoverage] = useState<Record<number, { total: number; synth: number }>>({});

  const [epubRow, patchEpubRow] = useRowState();
  const [docxRow, patchDocxRow] = useRowState();
  const [typRow, patchTypRow] = useRowState();
  const [capsRow, patchCapsRow] = useRowState();
  const [m4bRow, patchM4bRow] = useRowState();
  const [relRow, patchRelRow] = useRowState();
  const [verRow, patchVerRow] = useRowState();

  const [capsChapterId, setCapsChapterId] = useState<number | null>(null);

  useEffect(() => {
    void loadBooks();
    void loadChapters();
  }, [loadBooks, loadChapters]);

  useEffect(() => {
    if (bookId == null && books.length > 0) setBookId(books[0].id);
  }, [books, bookId]);

  // Audio coverage per chapter (drives the honest M4B gate).
  const refreshCoverage = useCallback(async () => {
    const next: Record<number, { total: number; synth: number }> = {};
    await Promise.all(
      chapters.map(async (c) => {
        try {
          const { segments } = await segmentList(c.id);
          next[c.id] = {
            total: segments.length,
            synth: segments.filter((s) => Boolean(s.audio_hash)).length,
          };
        } catch {
          next[c.id] = { total: 0, synth: 0 };
        }
      }),
    );
    setCoverage(next);
  }, [chapters]);

  useEffect(() => {
    if (activeStudio === 'publish') void refreshCoverage();
  }, [activeStudio, refreshCoverage]);

  useEffect(() => {
    if (capsChapterId == null && chapters.length > 0) setCapsChapterId(chapters[0].id);
  }, [chapters, capsChapterId]);

  const book = books.find((b) => b.id === bookId) ?? null;
  const chapter = chapters.find((c) => c.id === capsChapterId) ?? null;

  const bookFullySynth = useMemo(() => {
    const rows = chapters.filter((c) => c.book_id === bookId);
    if (rows.length === 0) return { ok: false, reason: 'this book has no chapters yet' };
    for (const c of rows) {
      const cov = coverage[c.id];
      if (!cov || cov.total === 0) return { ok: false, reason: `chapter ${c.chapter_number} has no synthesized segments loaded` };
      if (cov.synth < cov.total) return { ok: false, reason: `chapter ${c.chapter_number}: ${cov.total - cov.synth} of ${cov.total} segments unsynthesized` };
    }
    return { ok: true, reason: null as string | null };
  }, [chapters, bookId, coverage]);

  const chapterSynth = useMemo(() => {
    if (chapter == null) return { ok: false, reason: 'no chapter selected' };
    const cov = coverage[chapter.id];
    if (!cov || cov.total === 0) return { ok: false, reason: 'this chapter has no synthesized segments yet — produce audio first' };
    if (cov.synth < cov.total) return { ok: false, reason: `${cov.total - cov.synth} of ${cov.total} segments unsynthesized — produce audio first` };
    return { ok: true, reason: null as string | null };
  }, [chapter, coverage]);

  const runEpub = async () => {
    if (!book) return;
    patchEpubRow({ busy: true, error: null });
    try {
      const res = await publishEpub({ book_id: book.id });
      setEpub(res);
      setArtifactPath(joinAbs(bundlePath, res.out));
    } catch (err) {
      const mapped = mapError(err);
      patchEpubRow({ error: `${mapped.copy.body} ${mapped.detail}`.trim() });
    } finally {
      patchEpubRow({ busy: false });
    }
  };

  const runDocx = async () => {
    if (!book) return;
    patchDocxRow({ busy: true, error: null });
    try {
      const res = await publishDocx({ book_id: book.id });
      setDocx(res);
    } catch (err) {
      const mapped = mapError(err);
      patchDocxRow({ error: `${mapped.copy.body} ${mapped.detail}`.trim() });
    } finally {
      patchDocxRow({ busy: false });
    }
  };

  const runTyp = async () => {
    if (!book) return;
    patchTypRow({ busy: true, error: null });
    try {
      const res = await publishTypst({ book_id: book.id });
      setTyp(res);
    } catch (err) {
      const mapped = mapError(err);
      patchTypRow({ error: `${mapped.copy.body} ${mapped.detail}`.trim() });
    } finally {
      patchTypRow({ busy: false });
    }
  };

  const runCaps = async () => {
    if (!chapter) return;
    patchCapsRow({ busy: true, error: null });
    try {
      const res = await publishCaptions({ chapter_id: chapter.id });
      setCaps({ chapterId: chapter.id, lrc: res.lrc, vtt: res.vtt, cues: res.cues, duration_ms: res.duration_ms });
    } catch (err) {
      const mapped = mapError(err);
      patchCapsRow({ error: `${mapped.copy.body} ${mapped.detail}`.trim() });
    } finally {
      patchCapsRow({ busy: false });
    }
  };

  const runM4b = async () => {
    if (!book) return;
    patchM4bRow({ busy: true, error: null });
    try {
      const args = m4bScope === 'book' ? { book_id: book.id } : { chapter_id: m4bScope };
      const res = await publishM4b(args);
      setM4b({
        out: res.out,
        bytes: res.bytes,
        duration_ms: res.duration_ms,
        codec: res.codec,
        scope: m4bScope === 'book' ? 'whole book' : `chapter ${chapters.find((c) => c.id === m4bScope)?.chapter_number ?? '?'}`,
      });
    } catch (err) {
      const mapped = mapError(err);
      patchM4bRow({ error: `${mapped.copy.body} ${mapped.detail}`.trim() });
    } finally {
      patchM4bRow({ busy: false });
    }
  };

  const runManifest = async () => {
    patchRelRow({ busy: true, error: null });
    try {
      const out = manifestOut.trim() || joinAbs(bundlePath, 'exports/latest.json');
      const res = await releaseManifest({ artifact: artifactPath.trim(), out });
      setManifest({
        manifest: res.manifest,
        sha: res.artifact_sha256,
        bytes: res.artifact_bytes,
        key: res.generated_key_hex,
      });
      setVerify(null);
    } catch (err) {
      const mapped = mapError(err);
      patchRelRow({ error: `${mapped.copy.body} ${mapped.detail}`.trim() });
    } finally {
      patchRelRow({ busy: false });
    }
  };

  const runVerify = async () => {
    if (!manifest) return;
    patchVerRow({ busy: true, error: null });
    try {
      const res = await releaseVerify({ manifest: manifest.manifest, artifact: artifactPath.trim() || undefined });
      setVerify({ valid: res.valid, sha256: res.checks.sha256, signature: res.checks.signature });
    } catch (err) {
      const mapped = mapError(err);
      patchVerRow({ error: `${mapped.copy.body} ${mapped.detail}`.trim() });
    } finally {
      patchVerRow({ busy: false });
    }
  };

  if (!book) {
    return (
      <StudioShell icon={Store} title="Publish & Export" subtitle="EPUB 3 · DOCX · Typst PDF · captions · M4B · release signing">
        <EmptyState
          icon={Store}
          title="No book in this bundle"
          hint="Publishing attaches to a book row. Create a book first."
        />
      </StudioShell>
    );
  }

  return (
    <StudioShell
      icon={Store}
      title="Publish & Export"
      subtitle="EPUB 3 · DOCX · Typst PDF · captions · ACX M4B · signed release manifests"
      actions={
        <label className="flex items-center gap-2">
          <span className="text-2xs text-mut uppercase tracking-wide">Book</span>
          <select
            className={cn(inputClass, 'w-52')}
            value={bookId ?? ''}
            onChange={(e) => setBookId(Number(e.target.value))}
          >
            {books.map((b) => <option key={b.id} value={b.id}>{b.title}</option>)}
          </select>
        </label>
      }
    >
      <div className="pb-10 max-w-3xl">
        <div className="px-6 pt-5 grid gap-3">
          <div>
            <RowShell
              icon={BookText}
              title="EPUB 3"
              hint="OCF container · SMIL media overlays + embedded audio for synthesized chapters"
              state={epubRow}
              onRun={() => void runEpub()}
            />
            {epub && (
              <div className="grid gap-1 pt-2">
                <ArtifactLine path={joinAbs(bundlePath, epub.out)} extra={`${epub.entries} entries · ${epub.chapters} chapters · ${epub.overlays} overlays`} />
                {epub.cover && !epub.cover.embedded && (
                  <p className="text-2xs text-warn/90">{epub.cover.hint}</p>
                )}
              </div>
            )}
          </div>

          <div>
            <RowShell
              icon={FileText}
              title="DOCX manuscript"
              hint="WordprocessingML with title/heading styles, bold speakers, italic monologue"
              state={docxRow}
              onRun={() => void runDocx()}
            />
            {docx && (
              <div className="grid gap-1 pt-2">
                <ArtifactLine path={joinAbs(bundlePath, docx.out)} extra={`${docx.paragraphs} paragraphs · ${docx.words} words`} />
              </div>
            )}
          </div>

          <div>
            <RowShell
              icon={FileBadge}
              title="Typst print PDF"
              hint="A5 novel template; PDF compiles when a typst binary is present"
              state={typRow}
              onRun={() => void runTyp()}
            />
            {typ && (
              <div className="grid gap-1 pt-2">
                <ArtifactLine path={joinAbs(bundlePath, typ.typ)} extra={typ.compiled ? 'PDF compiled' : 'source only'} />
                {!typ.compiled && typ.reason && <p className="text-2xs text-mut">{typ.reason}</p>}
                {typ.pdf && <ArtifactLine path={joinAbs(bundlePath, typ.pdf)} />}
              </div>
            )}
          </div>

          <div>
            <RowShell
              icon={Captions}
              title="Captions"
              hint="line-synced .lrc + WebVTT .vtt from segment durations"
              state={capsRow}
              onRun={() => void runCaps()}
              disabled={!chapter}
              disabledReason="select a chapter below"
            >
              <label className="flex items-center gap-2">
                <span className="text-2xs text-mut uppercase tracking-wide">Chapter</span>
                <select
                  className={cn(inputClass, 'w-56 text-2xs')}
                  value={capsChapterId ?? ''}
                  onChange={(e) => setCapsChapterId(Number(e.target.value))}
                >
                  {chapters.map((c: ChapterRow) => (
                    <option key={c.id} value={c.id}>Ch {c.chapter_number} — {c.title}</option>
                  ))}
                </select>
              </label>
            </RowShell>
            {caps && (
              <div className="grid gap-1 pt-2">
                {caps.vtt && <ArtifactLine path={joinAbs(bundlePath, caps.vtt)} extra={`${caps.cues} cues · ${formatMs(caps.duration_ms)}`} />}
                {caps.lrc && <ArtifactLine path={joinAbs(bundlePath, caps.lrc)} />}
              </div>
            )}
          </div>

          <div>
            <RowShell
              icon={Download}
              title="M4B audiobook"
              hint="ffmpeg AAC-LC 64k mono 44.1 kHz with chapter atoms — every scoped segment must be synthesized"
              state={m4bRow}
              onRun={() => void runM4b()}
              disabled={m4bScope === 'book' ? !bookFullySynth.ok || readonly : !chapterSynth.ok || readonly}
              disabledReason={
                readonly
                  ? 'bundle is read-only — a foreign lease is active'
                  : (m4bScope === 'book' ? bookFullySynth.reason : chapterSynth.reason) ?? undefined
              }
            >
              <label className="flex items-center gap-2">
                <span className="text-2xs text-mut uppercase tracking-wide">Scope</span>
                <select
                  className={cn(inputClass, 'w-56 text-2xs')}
                  value={m4bScope === 'book' ? 'book' : String(m4bScope)}
                  onChange={(e) => setM4bScope(e.target.value === 'book' ? 'book' : Number(e.target.value))}
                >
                  <option value="book">Whole book</option>
                  {chapters.map((c) => (
                    <option key={c.id} value={c.id}>Chapter {c.chapter_number} — {c.title}</option>
                  ))}
                </select>
              </label>
            </RowShell>
            {m4b && (
              <div className="grid gap-1 pt-2">
                <ArtifactLine path={joinAbs(bundlePath, m4b.out)} bytes={m4b.bytes} extra={`${m4b.scope} · ${formatMs(m4b.duration_ms)} · ${m4b.codec}`} />
              </div>
            )}
          </div>

          <RowShell
            icon={PackageCheck}
            title="KDP package"
            hint="compliance check, pricing and the storefront package live in Craft Studio"
            state={{ busy: false, error: null }}
            runLabel="Open Craft Studio"
            onRun={() => useWorkspaceStore.getState().switchStudio('craft', { arm: 'publish' })}
          />
        </div>

        {/* Advanced: signed release manifest + verification */}
        <div className="px-6 pt-6">
          <button
            type="button"
            onClick={() => setAdvancedOpen((o) => !o)}
            className="flex items-center gap-1.5 text-2xs text-mut hover:text-ink"
            aria-expanded={advancedOpen}
          >
            <ShieldCheck size={12} /> Advanced — signed release manifest (ed25519)
          </button>
          {advancedOpen && (
            <div className="mt-2 border border-line rounded-xl bg-panel2 px-4 py-3 grid gap-2">
              <p className="text-2xs text-mut leading-relaxed">
                Signs the artifact bytes (ed25519) and writes a Tauri-updater-shaped{' '}
                <code>latest.json</code>. The artifact must be an absolute path; the signing
                key comes from <code>AUTHR_RELEASE_KEY_HEX</code> or is generated and returned
                (CI-only).
              </p>
              <label className="grid gap-1">
                <span className="text-2xs text-mut uppercase tracking-wide">Artifact (absolute path)</span>
                <input
                  className={cn(inputClass, 'font-mono text-2xs')}
                  value={artifactPath}
                  onChange={(e) => setArtifactPath(e.target.value)}
                  placeholder="/path/to/bundle/exports/your-book.epub"
                  spellCheck={false}
                />
              </label>
              <div className="flex items-center gap-2 flex-wrap">
                <button
                  type="button"
                  onClick={() => void runManifest()}
                  disabled={relRow.busy || !artifactPath.trim()}
                  className={cn(btnPrimary, 'text-2xs px-3 py-1.5')}
                >
                  Sign manifest
                </button>
                {manifestOut && (
                  <input
                    className={cn(inputClass, 'font-mono text-2xs flex-1 min-w-[24ch]')}
                    value={manifestOut}
                    onChange={(e) => setManifestOut(e.target.value)}
                    placeholder="latest.json output path (absolute)"
                    spellCheck={false}
                  />
                )}
              </div>
              {relRow.error && <p className="text-2xs text-bad">{relRow.error}</p>}
              {manifest && (
                <div className="grid gap-1">
                  <ArtifactLine path={manifest.manifest} bytes={manifest.bytes} extra={`sha ${manifest.sha.slice(0, 12)}…`} />
                  {manifest.key && (
                    <p className="text-2xs text-warn/90">
                      Ephemeral signing key returned (CI-only — store it; it is not saved anywhere).
                    </p>
                  )}
                  <div className="flex items-center gap-2">
                    <button
                      type="button"
                      onClick={() => void runVerify()}
                      disabled={verRow.busy}
                      className="text-2xs px-3 py-1.5 rounded border border-line text-mut hover:text-ink"
                    >
                      Verify against artifact
                    </button>
                    {verify && (
                      <span className={cn('flex items-center gap-1.5 text-2xs', verify.valid ? 'text-ok' : 'text-bad')}>
                        {verify.valid ? <CheckCircle2 size={11} /> : <XCircle size={11} />}
                        sha256 {verify.sha256 ? 'match' : 'MISMATCH'} · signature {verify.signature ? 'valid' : 'INVALID'}
                        {verify.valid ? '' : ' — artifact tampered or wrong key'}
                      </span>
                    )}
                  </div>
                  {verRow.error && <p className="text-2xs text-bad">{verRow.error}</p>}
                </div>
              )}
            </div>
          )}
        </div>
      </div>
    </StudioShell>
  );
}
