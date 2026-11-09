// SceneStudio — Studio 3 (v1.6, ADR-0011 + ADR-0012): novelcast UI parity.
//
// The storyboard grid IS the novelcast storyboard-tab experience, folded into
// the offline-first runtime:
//   * scene cards with the painted frame (media_read → data: URL, sha-busted
//     cache), ladder badge, persisted focal selector, per-scene repaint,
//   * "Repaint all" with a bounded painter pool, live progress and stop,
//   * the shared art-style catalog (10 styles, novelcast `art-styles.ts`
//     mirror) — a style switch persists per book and deterministically
//     repaints (style id joins the SHA-256 seed),
//   * a production stage tracker (Derive → Sheets → Scenes → Assemble →
//     Validate) mirroring novelcast's STAGE_META,
//   * engine chip driven by the provider ladder (provider_get): R2I_ENDPOINT
//     when a live image endpoint resolves, SCRIPTED offline otherwise —
//     fail-closed, never fakes output.

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  Brush,
  CheckCircle2,
  Clapperboard,
  Clock,
  Film,
  Images,
  Layers,
  Loader2,
  Palette,
  Play,
  RefreshCw,
  Square,
  TriangleAlert,
  Volume2,
} from 'lucide-react';
import { StudioShell } from '../shared/StudioShell';
import { EmptyState } from '../shared/EmptyState';
import { useManuscriptStore } from '../../stores/manuscriptStore';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import {
  mediaDataUrl,
  providerGet,
  videoAssemble,
  videoPlan,
  videoRender,
  videoStyle,
  videoValidate,
  type ProviderGetResult,
  type VideoAsset,
} from '../../ipc/api';
import { ART_STYLES, getArtStyle } from '../../lib/artStyles';
import { cn, formatMs } from '../../lib/utils';
import { inputClass, btnPrimary } from '../layout/Modal';
import { toast } from '../../stores/uiStore';

interface Block {
  scene_index: number;
  segment_id: string;
  sequence_order: number;
  speaker_id: string;
  text_chars: number;
  duration_ms: number;
  characters: string[];
  focal: string;
  narration_path: string | null;
  ladder: 'REFS_READY' | 'PARTIAL' | 'TEXT_ONLY';
}

interface CheckRow {
  name: string;
  status: 'PASS' | 'WARN' | 'FAIL';
  detail: string;
}

type StageKey = 'derive' | 'sheets' | 'scenes' | 'assemble' | 'validate';
type StageState = 'pending' | 'running' | 'done' | 'failed';

const STAGE_META: Array<{ key: StageKey; label: string }> = [
  { key: 'derive', label: 'Derive' },
  { key: 'sheets', label: 'Sheets' },
  { key: 'scenes', label: 'Scenes' },
  { key: 'assemble', label: 'Assemble' },
  { key: 'validate', label: 'Validate' },
];

const PAINT_POOL = 2; // bounded painter pool (novelcast quota-aware parallelism)

/** One storyboard card — image, ladder, focal, repaint (novelcast ScenePanel). */
function SceneCard({
  scene,
  asset,
  cacheKey,
  engine,
  onRepaint,
  onFocal,
  busy,
}: {
  scene: Block;
  asset: VideoAsset | undefined;
  cacheKey: number;
  engine: string;
  onRepaint: (scene: Block) => void;
  onFocal: (sceneIndex: number, focal: string) => void;
  busy: boolean;
}) {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    let alive = true;
    setUrl(null);
    if (!asset?.path) return () => {};
    mediaDataUrl(asset.path, asset.sha256)
      .then((u) => {
        if (alive) setUrl(u);
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, [asset?.path, asset?.sha256, cacheKey]);

  const ladderTone =
    scene.ladder === 'REFS_READY'
      ? 'border-emerald-500/40 bg-emerald-500/10 text-ok'
      : scene.ladder === 'PARTIAL'
        ? 'border-amber-500/40 bg-amber-500/10 text-warn'
        : 'border-zinc-500/40 bg-zinc-500/10 text-mut';

  return (
    <div className="border border-line rounded-xl bg-panel2 overflow-hidden flex flex-col" data-scene-card={scene.scene_index}>
      <div className="relative aspect-video bg-app/60">
        {url ? (
          <img
            src={url}
            alt={`Scene ${scene.scene_index + 1} frame`}
            loading="lazy"
            className="absolute inset-0 h-full w-full object-cover"
          />
        ) : (
          <div className="absolute inset-0 flex flex-col items-center justify-center gap-1.5 text-mut">
            {busy ? (
              <>
                <Loader2 size={18} className="animate-spin text-accent" />
                <span className="text-2xs font-medium text-accent">Painting scene…</span>
              </>
            ) : (
              <>
                <Clapperboard size={18} />
                <span className="text-2xs">not rendered yet</span>
              </>
            )}
          </div>
        )}
        <span className="absolute left-2 top-2 rounded-full bg-black/60 px-2 py-0.5 text-2xs font-semibold text-white backdrop-blur font-mono">
          SC{scene.scene_index + 1}
        </span>
        <span
          className={cn(
            'absolute left-2 bottom-2 rounded-full border px-1.5 py-0.5 text-2xs font-mono backdrop-blur',
            ladderTone,
          )}
          title="reference ladder — sheets coverage for this scene's cast"
        >
          {scene.ladder}
        </span>
        <button
          type="button"
          disabled={busy}
          onClick={() => onRepaint(scene)}
          aria-label={`Repaint scene ${scene.scene_index + 1}`}
          title={`Repaint with ${engine} engine`}
          className="absolute right-2 top-2 w-7 h-7 rounded-md border border-line bg-panel/90 backdrop-blur text-mut hover:text-accent hover:bg-panel3 flex items-center justify-center transition-colors disabled:opacity-50"
        >
          {busy ? <Loader2 size={12} className="animate-spin" /> : <RefreshCw size={12} />}
        </button>
      </div>
      <div className="px-3 py-2.5 grid gap-2 flex-1">
        <div className="flex items-center gap-2 text-2xs">
          <span className="font-mono text-mut flex items-center gap-1">
            <Clock size={10} /> {formatMs(scene.duration_ms)}
          </span>
          <span className="text-mut truncate flex-1">
            {scene.speaker_id} · {scene.text_chars} chars
          </span>
          {scene.narration_path && (
            <Volume2 size={11} className="text-ok shrink-0" aria-label="narration ready" />
          )}
        </div>
        <div className="flex items-center gap-1.5 flex-wrap">
          {scene.characters.slice(0, 3).map((k) => (
            <span
              key={k}
              className="text-2xs font-mono px-1.5 py-0.5 rounded border border-accent/30 bg-accent/10 text-accent"
            >
              {k}
            </span>
          ))}
          {scene.characters.length > 3 && (
            <span className="text-2xs text-mut">+{scene.characters.length - 3}</span>
          )}
        </div>
        <select
          className={cn(inputClass, 'text-2xs py-1')}
          value={scene.focal}
          onChange={(e) => onFocal(scene.scene_index, e.target.value)}
          aria-label={`Ken Burns anchor for scene ${scene.scene_index + 1}`}
        >
          <option value="center">Focal · center</option>
          <option value="left">Focal · left</option>
          <option value="right">Focal · right</option>
          <option value="top">Focal · upper</option>
          <option value="bottom">Focal · lower</option>
        </select>
      </div>
    </div>
  );
}

export function VideoStudio() {
  const segments = useManuscriptStore((s) => s.segments);
  const chapters = useManuscriptStore((s) => s.chapters);
  const activeChapterId = useManuscriptStore((s) => s.activeChapterId);
  const selectChapter = useManuscriptStore((s) => s.selectChapter);
  const activeStudio = useWorkspaceStore((s) => s.activeStudio);

  const [plan, setPlan] = useState<Awaited<ReturnType<typeof videoPlan>> | null>(null);
  const [assets, setAssets] = useState<VideoAsset[]>([]);
  const [checks, setChecks] = useState<CheckRow[] | null>(null);
  const [mp4, setMp4] = useState<{ out: string; bytes: number; narration: string } | null>(null);
  const [busy, setBusy] = useState<string>('');
  const [engine, setEngine] = useState<'SCRIPTED' | 'R2I_ENDPOINT'>('SCRIPTED');
  const [provider, setProvider] = useState<ProviderGetResult | null>(null);
  const [styleId, setStyleId] = useState<string>(ART_STYLES[0].id);
  const [styleOpen, setStyleOpen] = useState(false);
  const [repaint, setRepaint] = useState<{ done: number; total: number; failed: number } | null>(null);
  const [repaintingScene, setRepaintingScene] = useState<number | null>(null);
  const [stages, setStages] = useState<Record<StageKey, StageState>>({
    derive: 'pending',
    sheets: 'pending',
    scenes: 'pending',
    assemble: 'pending',
    validate: 'pending',
  });
  const cacheKeyRef = useRef(0);
  const repaintCancelRef = useRef(false);
  const aliveRef = useRef(true);
  const [ , forceTick] = useState(0);
  const bumpCache = () => {
    cacheKeyRef.current += 1;
    forceTick((t) => t + 1);
  };

  useEffect(() => {
    aliveRef.current = true;
    return () => {
      aliveRef.current = false;
    };
  }, []);

  const r2iAvailable = (provider?.image.resolved.endpoint_source ?? 'none') !== 'none';

  const markStage = (k: StageKey, v: StageState) =>
    setStages((s) => ({ ...s, [k]: v }));

  // ── Load: plan + style + provider resolution ──────────────────────────────
  const loadPlan = useCallback(async (chapterId: number) => {
    try {
      markStage('derive', 'running');
      const p = await videoPlan({ chapter_id: chapterId });
      setPlan(p);
      setMp4(null);
      setChecks(null);
      setAssets([]);
      markStage('derive', 'done');
      markStage('sheets', 'pending');
      markStage('scenes', 'pending');
      markStage('assemble', 'pending');
      markStage('validate', 'pending');
      // Style + provider in parallel (both tiny reads).
      void videoStyle({ book_id: p.book_id }).then((s) => setStyleId(s.style_id)).catch(() => {});
      void providerGet().then(setProvider).catch(() => {});
      // Offline ensure-render fills the registry so the grid paints (the
      // storyboard shows real pixels, not placeholders). missing_only keeps it
      // a no-op when everything is already rendered.
      try {
        const r = await videoRender({
          chapter_id: chapterId,
          engine: 'SCRIPTED',
          missing_only: true,
        });
        setAssets(r.assets ?? r.rendered);
        markStage('sheets', p.missing_sheets.length === 0 ? 'done' : 'pending');
        markStage('scenes', 'done');
        bumpCache();
      } catch {
        // Read-only lease / fresh bundle: cards render without pixels.
        markStage('sheets', 'pending');
        markStage('scenes', 'pending');
      }
    } catch (e) {
      setPlan(null);
      markStage('derive', 'failed');
      toast('error', 'Video plan failed', String(e));
    }
  }, []);

  useEffect(() => {
    if (activeStudio === 'video' && activeChapterId) void loadPlan(activeChapterId);
  }, [activeStudio, activeChapterId, loadPlan]);

  const blocks: Block[] = useMemo(() => {
    if (!plan) {
      // Pre-plan fallback mirrors the heuristic pacing view.
      return segments.map((s, i) => ({
        scene_index: i,
        segment_id: s.segment_id,
        sequence_order: s.sequence_order ?? 0,
        speaker_id: s.speaker_id ?? '',
        text_chars: s.text_content.length,
        duration_ms: Math.max(1500, s.audio_duration_ms || (s.text_content.split(/\s+/).length || 1) * 420),
        characters: [],
        focal: 'center',
        narration_path: null,
        ladder: 'TEXT_ONLY' as const,
      }));
    }
    return plan.scenes as unknown as Block[];
  }, [plan, segments]);

  const assetByRef = useMemo(() => {
    const m = new Map<string, VideoAsset>();
    for (const a of assets) m.set(`${a.kind}:${a.ref}`, a);
    return m;
  }, [assets]);

  const totalMs = blocks.reduce((a, b) => a + b.duration_ms, 0) + (plan?.title_ms ?? 0);
  const sheetAssets = assets.filter((a) => a.kind === 'SHEET');
  const titleAsset = assetByRef.get('TITLE:title');

  // ── Repaint one scene (per-card RefreshCw) ────────────────────────────────
  const repaintScene = async (scene: Block) => {
    if (!activeChapterId || repaintingScene !== null || repaint) return;
    setRepaintingScene(scene.scene_index);
    try {
      await videoRender({
        chapter_id: activeChapterId,
        engine,
        scene_index: scene.scene_index,
      });
      const p = await videoPlan({ chapter_id: activeChapterId });
      setPlan(p);
      try {
        const r = await videoRender({
          chapter_id: activeChapterId,
          engine: 'SCRIPTED',
          missing_only: true,
        });
        setAssets(r.assets ?? r.rendered);
      } catch { /* read-only: keep old assets */ }
      bumpCache();
      toast('success', 'Scene repainted', `SC${scene.scene_index + 1} · ${engine}`);
    } catch (e) {
      toast('error', 'Repaint failed', String(e));
    } finally {
      setRepaintingScene(null);
    }
  };

  // ── Repaint all (bounded pool + progress + stop) ──────────────────────────
  const repaintAll = async () => {
    if (!activeChapterId || !plan || repaint) return;
    repaintCancelRef.current = false;
    setRepaint({ done: 0, total: blocks.length, failed: 0 });
    let cursor = 0;
    let failed = 0;
    let done = 0;
    const worker = async (): Promise<void> => {
      while (cursor < blocks.length) {
        if (repaintCancelRef.current || !aliveRef.current) return;
        const scene = blocks[cursor];
        cursor += 1;
        setRepaintingScene(scene.scene_index);
        try {
          await videoRender({
            chapter_id: activeChapterId,
            engine,
            scene_index: scene.scene_index,
          });
        } catch {
          failed += 1;
        }
        if (!aliveRef.current) return;
        done += 1;
        setRepaintingScene(null);
        setRepaint({ done, total: blocks.length, failed });
      }
    };
    const painters = Math.max(1, Math.min(PAINT_POOL, blocks.length));
    await Promise.all(Array.from({ length: painters }, () => worker()));
    if (!aliveRef.current) return;
    setRepaintingScene(null);
    const stopped = repaintCancelRef.current;
    try {
      const r = await videoRender({
        chapter_id: activeChapterId,
        engine: 'SCRIPTED',
        missing_only: true,
      });
      setAssets(r.assets ?? r.rendered);
    } catch { /* keep old assets */ }
    bumpCache();
    setRepaint(null);
    const p = await videoPlan({ chapter_id: activeChapterId });
    setPlan(p);
    toast(
      stopped ? 'warn' : 'success',
      stopped ? 'Repaint stopped' : 'Chapter repainted',
      stopped
        ? `Stopped after ${done} of ${blocks.length} scene(s).`
        : failed > 0
          ? `${blocks.length - failed}/${blocks.length} scenes painted · ${failed} failed — retry individually.`
          : `All ${blocks.length} scene(s) painted with the ${getArtStyle(styleId).label} style.`,
    );
  };

  // ── Style switch: persist per book, then deterministic repaint ────────────
  const applyStyle = async (id: string) => {
    if (!plan) return;
    setStyleId(id);
    setStyleOpen(false);
    try {
      await videoStyle({ book_id: plan.book_id, style_id: id });
      toast('success', 'Art style set', `${getArtStyle(id).label} — frames repaint deterministically.`);
      await repaintAll();
    } catch (e) {
      toast('error', 'video_style failed', String(e));
    }
  };

  // ── Full pipeline: Derive → Sheets → Scenes → Assemble → Validate ────────
  const runPipeline = async () => {
    if (!activeChapterId || !plan) return;
    try {
      markStage('derive', 'running');
      const p = await videoPlan({ chapter_id: activeChapterId });
      setPlan(p);
      markStage('derive', 'done');
      markStage('sheets', 'running');
      const sheetRes = await videoRender({
        chapter_id: activeChapterId,
        engine,
        sheets_only: true,
      });
      const sheetAssetsNew: VideoAsset[] = sheetRes.rendered as VideoAsset[];
      setAssets((prev) => [
        ...prev.filter((a) => a.kind !== 'SHEET'),
        ...sheetAssetsNew,
      ]);
      markStage('sheets', 'done');
      markStage('scenes', 'running');
      const r = await videoRender({
        chapter_id: activeChapterId,
        engine,
        focal: undefined,
      });
      setAssets(r.assets ?? r.rendered);
      markStage('scenes', 'done');
      bumpCache();
      markStage('assemble', 'running');
      setBusy('assemble');
      const a = await videoAssemble({ chapter_id: activeChapterId });
      setMp4({ out: a.out, bytes: a.bytes, narration: a.narration });
      markStage('assemble', 'done');
      markStage('validate', 'running');
      setBusy('validate');
      const v = await videoValidate({ chapter_id: activeChapterId });
      setChecks(v.checks);
      markStage('validate', v.ok ? 'done' : 'failed');
      toast('success', 'Video novel ready', `${a.out} · ${(a.bytes / 1e6).toFixed(1)} MB · ${formatMs(a.duration_ms)}`);
    } catch (e) {
      const stage = (Object.entries(stages) as Array<[StageKey, StageState]>).find(
        ([, v]) => v === 'running',
      )?.[0];
      if (stage) markStage(stage, 'failed');
      toast('error', 'Video pipeline failed', String(e));
    } finally {
      setBusy('');
    }
  };

  const setFocalFor = (sceneIndex: number, focal: string) => {
    setPlan((p) =>
      p
        ? {
            ...p,
            scenes: p.scenes.map((s) =>
              s.scene_index === sceneIndex ? { ...s, focal: focal as typeof s.focal } : s,
            ),
          }
        : p,
    );
    // Persisted on the next render/assemble (video_render focal map), same as
    // before — the selector edits local state for instant feedback.
    if (activeChapterId) {
      void videoRender({
        chapter_id: activeChapterId,
        engine: 'SCRIPTED',
        missing_only: true,
        focal: { [sceneIndex]: focal },
      }).catch(() => {});
    }
  };

  const paintedCount = blocks.filter((b) => assetByRef.has(`SCENE:${b.segment_id}`)).length;
  const activeStyle = getArtStyle(styleId);

  return (
    <StudioShell
      icon={Clapperboard}
      title="Scene Studio"
      subtitle="video-novel storyboard · art styles · R2I/SCRIPTED ladder · ffmpeg MP4"
    >
      <div className="pb-10">
        {/* ── Toolbar (novelcast storyboard header parity) ─────────────── */}
        <div className="px-6 pt-5 flex items-center gap-3 flex-wrap">
          <label className="flex items-center gap-2">
            <span className="text-2xs text-mut uppercase tracking-wide">Chapter</span>
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
          {plan && (
            <span className="font-mono text-2xs text-mut">
              {paintedCount}/{plan.scene_count} scenes painted · {formatMs(totalMs)} · {plan.resolution} @{plan.fps}fps
            </span>
          )}

          <div className="ml-auto flex items-center gap-2 flex-wrap">
            {/* Engine chip — the provider ladder, visible */}
            <span
              className={cn(
                'text-2xs font-mono px-2 py-1 rounded border',
                engine === 'R2I_ENDPOINT'
                  ? 'border-accent/50 bg-accent/10 text-accent'
                  : 'border-line bg-panel2 text-mut',
              )}
              title={
                r2iAvailable
                  ? `Live image endpoint resolved (${provider?.image.resolved.endpoint_source}) — repaints use R2I`
                  : 'No image endpoint configured — SCRIPTED offline engine (fail-closed seam)'
              }
            >
              {engine === 'R2I_ENDPOINT' ? 'R2I_ENDPOINT' : 'SCRIPTED'}
            </span>
            {r2iAvailable && (
              <button
                type="button"
                onClick={() => setEngine((e) => (e === 'SCRIPTED' ? 'R2I_ENDPOINT' : 'SCRIPTED'))}
                className="text-2xs text-mut hover:text-ink border border-line rounded px-2 py-1"
                aria-pressed={engine === 'R2I_ENDPOINT'}
              >
                toggle R2I
              </button>
            )}
            {/* Style picker */}
            <div className="relative">
              <button
                type="button"
                onClick={() => setStyleOpen((o) => !o)}
                className="flex items-center gap-1.5 text-2xs border border-line rounded px-2 py-1 hover:bg-panel2 text-ink"
                aria-label={`Art style: ${activeStyle.label}`}
                aria-expanded={styleOpen}
              >
                <Palette size={11} className="text-accent" />
                Style · {activeStyle.label}
              </button>
              {styleOpen && (
                <div className="anim-pop absolute right-0 top-8 z-30 w-72 rounded-lg border border-line2 bg-panel shadow-2xl p-2 grid gap-1 max-h-72 overflow-y-auto">
                  {ART_STYLES.map((s) => (
                    <button
                      key={s.id}
                      type="button"
                      onClick={() => void applyStyle(s.id)}
                      className={cn(
                        'flex items-center gap-2.5 rounded-md px-2 py-1.5 text-left hover:bg-panel2',
                        s.id === styleId && 'bg-panel3',
                      )}
                    >
                      <span className={cn('w-5 h-5 rounded bg-gradient-to-br shrink-0', s.swatch)} />
                      <span className="min-w-0">
                        <span className="block text-2xs text-ink font-medium">{s.label}</span>
                        <span className="block text-2xs text-mut truncate">{s.tagline}</span>
                      </span>
                      {s.id === styleId && <CheckCircle2 size={12} className="ml-auto text-accent" />}
                    </button>
                  ))}
                </div>
              )}
            </div>
            {repaint ? (
              <button
                type="button"
                onClick={() => {
                  repaintCancelRef.current = true;
                }}
                className="flex items-center gap-1.5 text-2xs font-medium border border-rose-500/40 bg-rose-500/10 text-bad rounded px-2 py-1"
              >
                <Square size={11} /> Stop · {repaint.done}/{repaint.total}
              </button>
            ) : (
              <button
                type="button"
                disabled={!plan || blocks.length === 0}
                onClick={() => void repaintAll()}
                className="flex items-center gap-1.5 text-2xs border border-line rounded px-2 py-1 hover:bg-panel2 text-ink disabled:opacity-50"
                title={`Repaint all ${blocks.length} scenes with ${engine}`}
              >
                <Brush size={11} /> Repaint all
              </button>
            )}
          </div>
        </div>

        {/* ── Repaint progress (novelcast parity alert) ────────────────── */}
        {repaint && (
          <div className="mx-6 mt-3 rounded-lg border border-amber-500/40 bg-amber-500/10 px-3.5 py-2.5 flex items-center gap-2">
            <Loader2 size={13} className="animate-spin text-warn" />
            <span className="text-2xs text-warn">
              Repainting with the {engine} engine — {repaint.done}/{repaint.total}
              {repaint.failed > 0 ? ` · ${repaint.failed} failed` : ''}
            </span>
            <span className="ml-auto text-2xs text-mut font-mono">
              style: {activeStyle.label}
            </span>
          </div>
        )}

        {/* ── Stage tracker (novelcast STAGE_META parity) ──────────────── */}
        <div className="px-6 pt-4">
          <div className="flex items-stretch gap-1.5">
            {STAGE_META.map((st, i) => {
              const state = stages[st.key];
              return (
                <div key={st.key} className="flex-1 min-w-0 flex items-center gap-1.5">
                  <div
                    className={cn(
                      'flex-1 rounded-lg border px-2.5 py-1.5 flex items-center gap-1.5 min-w-0',
                      state === 'done' && 'border-emerald-500/40 bg-emerald-500/10',
                      state === 'running' && 'border-accent/50 bg-accent/10',
                      state === 'failed' && 'border-rose-500/40 bg-rose-500/10',
                      state === 'pending' && 'border-line bg-panel2',
                    )}
                    data-stage={st.key}
                  >
                    {state === 'running' ? (
                      <Loader2 size={11} className="animate-spin text-accent shrink-0" />
                    ) : state === 'done' ? (
                      <CheckCircle2 size={11} className="text-ok shrink-0" />
                    ) : state === 'failed' ? (
                      <TriangleAlert size={11} className="text-bad shrink-0" />
                    ) : (
                      <span className="w-[11px] shrink-0" />
                    )}
                    <span
                      className={cn(
                        'text-2xs truncate',
                        state === 'pending' ? 'text-mut' : 'text-ink font-medium',
                      )}
                    >
                      {st.label}
                    </span>
                  </div>
                  {i < STAGE_META.length - 1 && (
                    <span className="text-mut/40 text-2xs select-none" aria-hidden>
                      →
                    </span>
                  )}
                </div>
              );
            })}
          </div>
        </div>

        {!plan && blocks.length === 0 ? (
          <EmptyState
            icon={Film}
            title="No scenes to direct"
            hint="Pick a chapter — every narration segment becomes one derived scene with a timed, seeded frame."
          />
        ) : (
          <>
            {/* ── Storyboard grid ─────────────────────────────────────── */}
            <div className="px-6 pt-4 grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 2xl:grid-cols-4 gap-3">
              {blocks.map((b) => (
                <SceneCard
                  key={b.segment_id}
                  scene={b}
                  asset={assetByRef.get(`SCENE:${b.segment_id}`)}
                  cacheKey={cacheKeyRef.current}
                  engine={engine}
                  busy={repaintingScene === b.scene_index}
                  onRepaint={(s) => void repaintScene(s)}
                  onFocal={setFocalFor}
                />
              ))}
            </div>

            {/* ── Cast reference sheets strip ─────────────────────────── */}
            {sheetAssets.length > 0 && (
              <div className="px-6 pt-5">
                <div className="flex items-center gap-1.5 text-2xs text-mut mb-2">
                  <Layers size={11} />
                  Cast reference sheets ({sheetAssets.length}) — R2I consistency engine, book-level
                </div>
                <div className="flex gap-2 overflow-x-auto pb-1">
                  {sheetAssets.map((a) => (
                    <SheetThumb key={a.ref} asset={a} cacheKey={cacheKeyRef.current} />
                  ))}
                </div>
              </div>
            )}
            {plan && plan.missing_sheets.length > 0 && (
              <p className="px-6 pt-2 text-2xs text-warn/90">
                missing sheets: {plan.missing_sheets.join(', ')} — run the pipeline or{' '}
                <code>video_render sheets_only</code>
              </p>
            )}

            {/* ── Assemble · validate ─────────────────────────────────── */}
            <div className="px-6 pt-5 grid md:grid-cols-2 gap-3">
              <div className="border border-line rounded-xl bg-panel2 px-4 py-3">
                <div className="text-xs font-semibold mb-2 flex items-center gap-2">
                  <Play size={12} className="text-accent" /> Offline render · assemble · validate
                </div>
                <button
                  type="button"
                  onClick={() => void runPipeline()}
                  disabled={busy !== '' || !plan || repaint !== null}
                  className={cn(btnPrimary, 'w-full justify-center flex items-center gap-2 text-xs py-2')}
                >
                  {busy === 'assemble' && 'Encoding MP4 (ffmpeg)…'}
                  {busy === 'validate' && 'Validating…'}
                  {busy === '' && 'Run pipeline → MP4'}
                </button>
                {mp4 && (
                  <p className="text-2xs text-ok/90 mt-2">
                    {mp4.out} · {(mp4.bytes / 1e6).toFixed(1)} MB · narration: {mp4.narration || 'silent'}
                  </p>
                )}
                {checks && (
                  <div className="mt-2 grid gap-1 max-h-32 overflow-y-auto">
                    {checks.map((c) => (
                      <div key={c.name} className="flex items-center gap-2 text-2xs">
                        <CheckCircle2
                          size={11}
                          className={cn(
                            c.status === 'PASS' && 'text-ok',
                            c.status === 'WARN' && 'text-warn',
                            c.status === 'FAIL' && 'text-bad',
                          )}
                        />
                        <span className="font-mono text-mut w-32">{c.name}</span>
                        <span className="text-mut truncate flex-1">{c.detail}</span>
                      </div>
                    ))}
                  </div>
                )}
              </div>

              <div className="border border-line rounded-xl bg-panel2 px-4 py-3">
                <div className="text-xs font-semibold mb-2">Title card</div>
                {titleAsset ? (
                  <TitleThumb asset={titleAsset} cacheKey={cacheKeyRef.current} />
                ) : (
                  <p className="text-2xs text-mut">Rendered by the pipeline (typography card, SCRIPTED on every tier).</p>
                )}
                <p className="text-2xs text-mut mt-2 leading-tight">
                  {engine === 'R2I_ENDPOINT'
                    ? 'R2I tier sends the segment text + cast bios verbatim + the style fragment (consistency ladder, ADR-0011).'
                    : 'SCRIPTED engine is fully offline and deterministic. Configure an image endpoint in Settings → AI providers for the R2I tier.'}
                </p>
              </div>
            </div>
          </>
        )}
      </div>
    </StudioShell>
  );
}

/** Sheet thumbnail (book-level cast sheet). */
function SheetThumb({ asset, cacheKey }: { asset: VideoAsset; cacheKey: number }) {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    let alive = true;
    mediaDataUrl(asset.path, asset.sha256)
      .then((u) => alive && setUrl(u))
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, [asset.path, asset.sha256, cacheKey]);
  return (
    <figure className="shrink-0 w-24 border border-line rounded-lg overflow-hidden bg-panel2" title={`${asset.ref} · ${asset.engine}`}>
      {url ? (
        <img src={url} alt={`Reference sheet ${asset.ref}`} className="w-24 h-24 object-cover" loading="lazy" />
      ) : (
        <div className="w-24 h-24 flex items-center justify-center text-mut">
          <Images size={14} />
        </div>
      )}
      <figcaption className="text-2xs font-mono text-mut px-1.5 py-1 truncate">{asset.ref}</figcaption>
    </figure>
  );
}

/** Title-card preview. */
function TitleThumb({ asset, cacheKey }: { asset: VideoAsset; cacheKey: number }) {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    let alive = true;
    mediaDataUrl(asset.path, asset.sha256)
      .then((u) => alive && setUrl(u))
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, [asset.path, asset.sha256, cacheKey]);
  return (
    <div className="rounded-lg overflow-hidden border border-line aspect-video bg-app/60">
      {url ? (
        <img src={url} alt="Title card" className="w-full h-full object-cover" />
      ) : (
        <div className="h-full flex items-center justify-center text-mut">
          <Loader2 size={14} className="animate-spin" />
        </div>
      )}
    </div>
  );
}

