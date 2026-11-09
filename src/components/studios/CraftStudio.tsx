// CraftStudio — v1.3 studio: project formats, bestseller style personas,
// plot-arc beat scaffolding and the KDP storefront packaging panel.
// All craft knowledge comes from the offline `craft_catalog`; nothing here
// calls the network.

import { useEffect, useMemo, useState } from 'react';
import {
  BookTemplate,
  CheckCircle2,
  Package,
  PenTool,
  RefreshCw,
  ShieldCheck,
  Sparkles,
  TriangleAlert,
} from 'lucide-react';
import { StudioShell } from '../shared/StudioShell';
import { EmptyState } from '../shared/EmptyState';
import { useManuscriptStore } from '../../stores/manuscriptStore';
import {
  craftApply,
  craftCatalog,
  craftProgress,
  craftScaffold,
  kdpCheck,
  kdpPackage,
  type CatalogEntry,
  type CraftProgressResult,
  type KdpCheckResult,
} from '../../ipc/api';
import { btnPrimary, inputClass } from '../layout/Modal';
import { cn } from '../../lib/utils';
import { toast, toastError } from '../../stores/uiStore';

function Select<T extends CatalogEntry>(props: {
  label: string;
  entries: T[];
  value: string;
  onChange: (id: string) => void;
}) {
  return (
    <label className="grid gap-1">
      <span className="text-2xs text-mut uppercase tracking-wide">{props.label}</span>
      <select
        className={cn(inputClass, 'text-xs')}
        value={props.value}
        onChange={(e) => props.onChange(e.target.value)}
      >
        {props.entries.map((e) => (
          <option key={e.id} value={e.id}>
            {e.label}
          </option>
        ))}
      </select>
    </label>
  );
}

export function CraftStudio() {
  const books = useManuscriptStore((s) => s.books);
  const loadBooks = useManuscriptStore((s) => s.loadBooks);
  const book = books[0] ?? null;

  const [catalog, setCatalog] = useState<{
    formats: CatalogEntry[];
    personas: CatalogEntry[];
    arcs: CatalogEntry[];
  }>({ formats: [], personas: [], arcs: [] });

  const [formatId, setFormatId] = useState('NOVEL');
  const [arcId, setArcId] = useState('SAVE_THE_CAT');
  const [personaId, setPersonaId] = useState('EDGE_THRILLER');
  const [adoptDefaults, setAdoptDefaults] = useState(false);
  const [chapterCount, setChapterCount] = useState(25);
  const [overwrite, setOverwrite] = useState(false);
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<CraftProgressResult | null>(null);
  const [personaQuery, setPersonaQuery] = useState('');

  // KDP panel state
  const [price, setPrice] = useState('4.99');
  const [keywords, setKeywords] = useState('');
  const [categories, setCategories] = useState('');
  const [description, setDescription] = useState('');
  const [aiDisclosure, setAiDisclosure] = useState(false);
  const [checkResult, setCheckResult] = useState<KdpCheckResult | null>(null);
  const [packageResult, setPackageResult] = useState<string | null>(null);

  useEffect(() => {
    void loadBooks();
    void craftCatalog()
      .then((c) => {
        setCatalog({
          formats: c.formats ?? [],
          personas: c.personas ?? [],
          arcs: c.arcs ?? [],
        });
      })
      .catch((err) => toastError('craft_catalog failed', err));
  }, [loadBooks]);

  useEffect(() => {
    if (!book) return;
    setDescription(book.marketing_copy || '');
    void craftProgress({ book_id: book.id })
      .then(setProgress)
      .catch(() => setProgress(null));
  }, [book]);

  const personaList = useMemo(() => {
    const q = personaQuery.trim().toLowerCase();
    if (!q) return catalog.personas;
    return catalog.personas.filter(
      (p) =>
        p.label.toLowerCase().includes(q) ||
        (p.family ?? '').toLowerCase().includes(q) ||
        p.id.toLowerCase().includes(q),
    );
  }, [catalog.personas, personaQuery]);

  const selectedPersona = useMemo(
    () => catalog.personas.find((p) => p.id === personaId),
    [catalog.personas, personaId],
  );
  const selectedArc = useMemo(
    () => catalog.arcs.find((a) => a.id === arcId),
    [catalog.arcs, arcId],
  );

  const apply = async () => {
    if (!book) return;
    setBusy(true);
    try {
      await craftApply({
        book_id: book.id,
        format_id: formatId,
        plot_arc_id: arcId,
        style_persona_id: personaId,
        adopt_defaults: adoptDefaults,
      });
      toast('success', 'Craft applied', `${formatId} · ${arcId} · ${personaId}`);
      await loadBooks();
      setProgress(await craftProgress({ book_id: book.id }));
    } catch (err) {
      toastError('craft_apply failed', err);
    } finally {
      setBusy(false);
    }
  };

  const scaffold = async () => {
    if (!book) return;
    setBusy(true);
    try {
      const res = await craftScaffold({
        book_id: book.id,
        plot_arc_id: arcId,
        chapter_count: chapterCount,
        overwrite_synopsis: overwrite,
      });
      toast(
        'success',
        'Beat scaffold mapped',
        `${res.created} created · ${res.mapped} mapped onto ${res.beats_total} beats`,
      );
      await loadBooks();
      setProgress(await craftProgress({ book_id: book.id }));
    } catch (err) {
      toastError('craft_scaffold failed', err);
    } finally {
      setBusy(false);
    }
  };

  const runCheck = async () => {
    if (!book) return;
    try {
      const res = await kdpCheck({ book_id: book.id, price_usd: Number(price) || 4.99 });
      setCheckResult(res);
    } catch (err) {
      toastError('kdp_check failed', err);
    }
  };

  const buildPackage = async () => {
    if (!book) return;
    setBusy(true);
    try {
      const res = await kdpPackage({
        book_id: book.id,
        price_usd: Number(price) || 4.99,
        keywords: keywords.split(',').map((s) => s.trim()).filter(Boolean),
        categories: categories.split(',').map((s) => s.trim()).filter(Boolean),
        description: description.trim() || book.marketing_copy,
        ai_disclosure: aiDisclosure,
        isbn: book.isbn || undefined,
      });
      setPackageResult(
        `${res.package_dir} — ${res.files.map((f) => f.name).join(', ')}`,
      );
      toast('success', 'KDP package written', res.package_dir);
      setCheckResult(await kdpCheck({ book_id: book.id, price_usd: Number(price) || 4.99 }));
    } catch (err) {
      toastError('kdp_package failed', err);
    } finally {
      setBusy(false);
    }
  };

  if (!book) {
    return (
      <StudioShell icon={PenTool} title="Craft Studio" subtitle="Formats · personas · plot arcs · KDP packaging">
        <EmptyState
          icon={PenTool}
          title="No book in this bundle"
          hint="Craft presets attach to a book row. Create a book first."
        />
      </StudioShell>
    );
  }

  const fmt = catalog.formats.find((f) => f.id === formatId);

  return (
    <StudioShell
      icon={PenTool}
      title="Craft Studio"
      subtitle="25 formats · 37 bestseller personas · 18 plot arcs · KDP packaging"
    >
      <div className="pb-10">
        <div className="px-6 pt-5 grid gap-3 max-w-4xl">
          {/* ── Craft apply ── */}
          <div className="border border-line rounded-xl bg-panel2 px-4 py-3 grid gap-2">
            <div className="flex items-center gap-2 text-xs font-semibold">
              <BookTemplate size={14} className="text-accent" /> Craft presets
              <span className="text-2xs text-mut font-normal">
                {book.title} · offline catalog
              </span>
            </div>
            <div className="grid grid-cols-3 gap-3">
              <Select label="Project format" entries={catalog.formats} value={formatId} onChange={setFormatId} />
              <Select label="Plot arc" entries={catalog.arcs} value={arcId} onChange={setArcId} />
              <Select
                label="Style persona"
                entries={personaList}
                value={personaId}
                onChange={setPersonaId}
              />
            </div>
            <input
              className={cn(inputClass, 'text-xs')}
              placeholder="Filter personas (genre, family, id)…"
              value={personaQuery}
              onChange={(e) => setPersonaQuery(e.target.value)}
            />
            {selectedPersona && (
              <p className="text-2xs text-mut leading-snug">
                <span className="text-ink font-semibold">{selectedPersona.label}</span> ·{' '}
                {selectedPersona.pov} · {selectedPersona.tense} · {selectedPersona.pacing} —{' '}
                {selectedPersona.directive}
              </p>
            )}
            {fmt && (
              <p className="text-2xs text-mut leading-snug">
                {fmt.label}: default {fmt.default_word_count?.toLocaleString('en-US')} words /{' '}
                {fmt.default_chapters} {fmt.unit} — {fmt.kdp_note}
              </p>
            )}
            <div className="flex items-center gap-4 flex-wrap">
              <label className="flex items-center gap-1.5 text-2xs text-mut">
                <input
                  type="checkbox"
                  checked={adoptDefaults}
                  onChange={(e) => setAdoptDefaults(e.target.checked)}
                />
                adopt format defaults as targets
              </label>
              <button
                type="button"
                onClick={() => void apply()}
                disabled={busy}
                className={cn(btnPrimary, 'text-2xs px-3 py-1.5 flex items-center gap-1.5')}
              >
                <Sparkles size={12} /> {busy ? 'Working…' : 'Apply craft'}
              </button>
              {progress && (
                <span className="text-2xs font-mono text-mut">
                  arc: {progress.arc_label ?? progress.plot_arc_id} · persona:{' '}
                  {progress.style_persona_id}
                </span>
              )}
            </div>
          </div>

          {/* ── Scaffold ── */}
          <div className="border border-line rounded-xl bg-panel2 px-4 py-3 grid gap-2">
            <div className="flex items-center gap-2 text-xs font-semibold">
              <RefreshCw size={14} className="text-accent" /> Beat scaffolding
              <span className="text-2xs text-mut font-normal">
                {selectedArc ? `${selectedArc.beats?.length ?? 0} beats` : '—'} ·{' '}
                {selectedArc?.lineage ?? ''}
              </span>
            </div>
            <div className="flex items-end gap-3 flex-wrap">
              <label className="grid gap-1">
                <span className="text-2xs text-mut uppercase tracking-wide">Chapters</span>
                <input
                  type="number"
                  min={1}
                  max={500}
                  className={cn(inputClass, 'w-24 text-xs')}
                  value={chapterCount}
                  onChange={(e) => setChapterCount(Number(e.target.value) || 1)}
                />
              </label>
              <label className="flex items-center gap-1.5 text-2xs text-mut pb-2">
                <input
                  type="checkbox"
                  checked={overwrite}
                  onChange={(e) => setOverwrite(e.target.checked)}
                />
                overwrite existing synopses
              </label>
              <button
                type="button"
                onClick={() => void scaffold()}
                disabled={busy}
                className={cn(btnPrimary, 'text-2xs px-3 py-1.5')}
              >
                {busy ? 'Working…' : 'Scaffold chapters'}
              </button>
            </div>
            {progress && (
              <div className="grid gap-1">
                <div className="flex items-center gap-2 text-2xs text-mut">
                  <span className="font-mono">
                    {progress.covered_beats}/{progress.total_beats} beats started
                  </span>
                  <div className="flex-1 h-1.5 rounded-full bg-panel3 overflow-hidden">
                    <div
                      className="h-full bg-accent"
                      style={{ width: `${Math.min(100, progress.coverage_pct)}%` }}
                    />
                  </div>
                  <span className="font-mono">
                    {progress.total_words.toLocaleString('en-US')} /{' '}
                    {progress.target_words.toLocaleString('en-US')} words ·{' '}
                    {progress.unresolved_threads} open threads
                  </span>
                </div>
                <div className="grid grid-cols-2 md:grid-cols-3 gap-1">
                  {progress.beats.map((b) => (
                    <div
                      key={b.beat_id}
                      title={b.guidance}
                      className={cn(
                        'text-2xs px-2 py-1 rounded border truncate',
                        b.chapters.length > 0
                          ? b.started
                            ? 'border-emerald-500/40 bg-emerald-500/10 text-emerald-300'
                            : 'border-line text-mut'
                          : 'border-line/50 text-mut/50',
                      )}
                    >
                      {b.beat_label}
                      {b.chapters.length > 0 && ` → ch ${b.chapters.map((c) => c.chapter_number).join(',')}`}
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>

          {/* ── KDP packaging ── */}
          <div className="border border-line rounded-xl bg-panel2 px-4 py-3 grid gap-2">
            <div className="flex items-center gap-2 text-xs font-semibold">
              <Package size={14} className="text-accent" /> KDP storefront package
              <span className="text-2xs text-mut font-normal">offline royalty projection — verify rates on kdp.amazon.com</span>
            </div>
            <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
              <label className="grid gap-1">
                <span className="text-2xs text-mut uppercase tracking-wide">Price (USD)</span>
                <input
                  className={cn(inputClass, 'text-xs')}
                  value={price}
                  onChange={(e) => setPrice(e.target.value)}
                />
              </label>
              <label className="grid gap-1 col-span-1 md:col-span-3">
                <span className="text-2xs text-mut uppercase tracking-wide">
                  Keywords (max 7, comma-separated, ≤50 chars each)
                </span>
                <input
                  className={cn(inputClass, 'text-xs')}
                  placeholder="lighthouse mystery, coastal thriller, slow burn suspense"
                  value={keywords}
                  onChange={(e) => setKeywords(e.target.value)}
                />
              </label>
              <label className="grid gap-1 md:col-span-2">
                <span className="text-2xs text-mut uppercase tracking-wide">Categories (max 2)</span>
                <input
                  className={cn(inputClass, 'text-xs')}
                  placeholder="Fiction / Mystery & Detective / Cozy"
                  value={categories}
                  onChange={(e) => setCategories(e.target.value)}
                />
              </label>
              <label className="flex items-center gap-1.5 text-2xs text-mut pb-2">
                <input
                  type="checkbox"
                  checked={aiDisclosure}
                  onChange={(e) => setAiDisclosure(e.target.checked)}
                />
                include AI-assistance disclosure
              </label>
            </div>
            <label className="grid gap-1">
              <span className="text-2xs text-mut uppercase tracking-wide flex items-center justify-between">
                Storefront description (20–4000 chars; KDP-safe HTML enforced)
                <span className="font-mono">{description.length}</span>
              </span>
              <textarea
                className={cn(inputClass, 'min-h-[80px] text-xs leading-relaxed')}
                value={description}
                onChange={(e) => setDescription(e.target.value)}
              />
            </label>
            <div className="flex items-center gap-3 flex-wrap">
              <button
                type="button"
                onClick={() => void runCheck()}
                className="text-2xs px-3 py-1.5 rounded-lg border border-line text-mut hover:text-ink flex items-center gap-1.5"
              >
                <ShieldCheck size={12} /> Run compliance check
              </button>
              <button
                type="button"
                onClick={() => void buildPackage()}
                disabled={busy}
                className={cn(btnPrimary, 'text-2xs px-3 py-1.5 flex items-center gap-1.5')}
              >
                <Package size={12} /> {busy ? 'Working…' : 'Build KDP package'}
              </button>
              {packageResult && (
                <span className="font-mono text-2xs text-mut truncate max-w-[380px]" title={packageResult}>
                  {packageResult}
                </span>
              )}
            </div>
            {checkResult && (
              <div className="grid gap-1.5 mt-1">
                <div className="flex items-center gap-2 text-2xs">
                  <span className={cn('font-mono font-bold', checkResult.pass ? 'text-emerald-400' : 'text-amber-400')}>
                    readiness {checkResult.readiness_score}/100 {checkResult.pass ? '· PASS' : '· GAPS'}
                  </span>
                  <span className="font-mono text-mut">
                    est. {checkResult.est_pages} print pages
                  </span>
                </div>
                {checkResult.issues.map((i, idx) => (
                  <div
                    key={idx}
                    className={cn(
                      'text-2xs px-2 py-1 rounded border flex items-start gap-1.5',
                      i.severity === 'FAIL'
                        ? 'border-rose-500/40 bg-rose-500/10 text-rose-300'
                        : i.severity === 'WARN'
                          ? 'border-amber-500/40 bg-amber-500/10 text-amber-300'
                          : 'border-line text-mut',
                    )}
                  >
                    {i.severity === 'FAIL' ? (
                      <TriangleAlert size={11} className="mt-0.5 shrink-0" />
                    ) : (
                      <CheckCircle2 size={11} className="mt-0.5 shrink-0" />
                    )}
                    <span>
                      <b>[{i.area}]</b> {i.message}
                      {i.fix && <span className="text-mut"> — {i.fix}</span>}
                    </span>
                  </div>
                ))}
                <p className="text-2xs text-mut font-mono">
                  ebook(70%): $
                  {String(
                    (checkResult.pricing_hint as { ebook?: { tier_70?: { royalty?: number } } })
                      .ebook?.tier_70?.royalty ?? '—',
                  )}{' '}
                  · paperback: $
                  {String(
                    (checkResult.pricing_hint as { paperback?: { royalty?: number } }).paperback
                      ?.royalty ?? '—',
                  )}{' '}
                  · min list $
                  {String(
                    (checkResult.pricing_hint as { paperback?: { min_list_price?: number } })
                      .paperback?.min_list_price ?? '—',
                  )}
                </p>
              </div>
            )}
          </div>
        </div>
      </div>
    </StudioShell>
  );
}
