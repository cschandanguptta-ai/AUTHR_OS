// VideoStudio — Studio 3 (v1.5, ADR-0011): the video-novel pipeline is real.
// The pacing timeline reads the draft-time derived `video_plan` (canonical
// durations — no per-UI re-estimation), the focal selector persists into the
// scene rows, and Render → Assemble → Validate drive the offline SCRIPTED
// engine and the ffmpeg MP4 export.

import { useEffect, useMemo, useState } from 'react';
import { Clapperboard, Film, GripVertical, CheckCircle2, Timer } from 'lucide-react';
import { StudioShell } from '../shared/StudioShell';
import { EmptyState } from '../shared/EmptyState';
import { useManuscriptStore } from '../../stores/manuscriptStore';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import {
  videoPlan,
  videoRender,
  videoAssemble,
  videoValidate,
  type VideoScene,
  type VideoAsset,
} from '../../ipc/api';
import { cn, formatMs } from '../../lib/utils';
import { inputClass, btnPrimary } from '../layout/Modal';
import { toast } from '../../stores/uiStore';

interface Block extends VideoScene {
  widthMs: number;
}

interface CheckRow {
  name: string;
  status: 'PASS' | 'WARN' | 'FAIL';
  detail: string;
}

const MS_PER_WORD = 420; // only for segments not yet derived (pre-plan fallback)

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
  const [focal, setFocal] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState<string>('');

  // Derive + plan on mount / chapter switch. `refreshed` tells the author
  // whether the draft-time hook had to catch up (stale chapter) or the plan
  // is served straight from persisted state (the no-double-work contract).
  const loadPlan = async (chapterId: number) => {
    try {
      const p = await videoPlan({ chapter_id: chapterId });
      setPlan(p);
      setMp4(null);
      setChecks(null);
    } catch (e) {
      setPlan(null);
      toast('error', 'Video plan failed', String(e));
    }
  };
  useEffect(() => {
    if (activeStudio === 'video' && activeChapterId) void loadPlan(activeChapterId);
  }, [activeStudio, activeChapterId]);

  const blocks: Block[] = useMemo(() => {
    if (!plan) {
      // Pre-plan fallback mirrors the old heuristic pacing view.
      let offset = 0;
      return segments.map((s) => {
        const widthMs = Math.max(1500, s.audio_duration_ms || (s.text_content.split(/\s+/).length || 1) * MS_PER_WORD);
        const block = {
          scene_index: Number(s.sequence_order ?? 0),
          segment_id: s.segment_id,
          sequence_order: s.sequence_order ?? 0,
          speaker_id: s.speaker_id ?? '',
          text_chars: s.text_content.length,
          duration_ms: widthMs,
          characters: [],
          focal: 'center' as const,
          narration_path: null,
          ladder: 'TEXT_ONLY' as const,
          widthMs,
        };
        offset += widthMs;
        return block;
      });
    }
    return plan.scenes.map((s) => ({ ...s, widthMs: s.duration_ms }));
  }, [plan, segments]);

  const totalMs = blocks.reduce((a, b) => a + b.widthMs, 0) + (plan?.title_ms ?? 0);
  const timelineMs = Math.max(totalMs, 1);

  const runPipeline = async () => {
    if (!activeChapterId || !plan) return;
    try {
      setBusy('render');
      const r = await videoRender({
        chapter_id: activeChapterId,
        engine: 'SCRIPTED',
        focal: Object.keys(focal).length ? focal : undefined,
      });
      setAssets(r.assets ?? r.rendered);
      setBusy('assemble');
      const a = await videoAssemble({ chapter_id: activeChapterId });
      setMp4({ out: a.out, bytes: a.bytes, narration: a.narration });
      setBusy('validate');
      const v = await videoValidate({ chapter_id: activeChapterId });
      setChecks(v.checks);
      toast('success', 'Video novel ready', `${a.out} · ${(a.bytes / 1e6).toFixed(1)} MB · ${formatMs(a.duration_ms)}`);
    } catch (e) {
      toast('error', 'Video pipeline failed', String(e));
    } finally {
      setBusy('');
    }
  };

  const setFocalFor = (sceneIndex: number, value: string) => {
    setFocal((f) => ({ ...f, [sceneIndex]: value }));
  };

  return (
    <StudioShell
      icon={Clapperboard}
      title="Video Studio"
      subtitle="Video novel pipeline · draft-time derived scenes · SCRIPTED engine · ffmpeg MP4"
    >
      <div className="pb-10">
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
              {plan.scene_count} scenes · {formatMs(totalMs)} · {plan.resolution} @{plan.fps}fps
              {plan.derived.refreshed ? ' · derived now' : ' · draft-time derived'}
            </span>
          )}
        </div>

        {!plan && blocks.length === 0 ? (
          <EmptyState
            icon={Film}
            title="No scenes to direct"
            hint="Pick a chapter — every narration segment becomes one derived scene with a timed, seeded frame."
          />
        ) : (
          <>
            <div className="px-6 pt-4">
              <div className="flex items-center gap-2 text-2xs text-mut mb-1.5">
                <Timer size={11} />
                <span>
                  Pacing timeline — {plan ? 'canonical durations from the draft-time derivation' : `estimate (${MS_PER_WORD} ms/word)`}
                  {plan ? ` (+ ${formatMs(plan.title_ms)} title card)` : ''}
                </span>
              </div>
              <div className="relative h-28 rounded-xl border border-line bg-panel2 overflow-hidden">
                <div className="absolute inset-0 flex" style={{ pointerEvents: 'none' }}>
                  {blocks.map((b) => {
                    const pct = (b.widthMs / timelineMs) * 100;
                    return (
                      <div
                        key={b.segment_id}
                        className={cn(
                          'timeline-block relative h-full border-r border-line/70',
                          b.ladder === 'REFS_READY' && 'bg-emerald-500/10',
                          b.ladder === 'PARTIAL' && 'bg-amber-500/10',
                          b.ladder === 'TEXT_ONLY' && 'bg-zinc-500/10',
                        )}
                        style={{ width: `${pct}%` }}
                        data-scene-index={b.scene_index}
                      >
                        <div className="px-2 py-1.5 h-full overflow-hidden select-none" style={{ pointerEvents: 'auto' }}>
                          <div className="flex items-center gap-1 text-2xs text-mut font-mono">
                            <GripVertical size={10} className="opacity-40" />
                            SC{b.scene_index + 1} {formatMs(b.widthMs)}
                          </div>
                          <div className="text-2xs leading-tight mt-1 line-clamp-3">{b.speaker_id}</div>
                        </div>
                      </div>
                    );
                  })}
                </div>
              </div>
              {plan && (
                <div className="flex gap-3 mt-1.5 text-2xs text-mut">
                  <span>ladder: {plan.ladder.refs_ready} ready · {plan.ladder.partial} partial · {plan.ladder.text_only} text-only</span>
                  {plan.missing_sheets.length > 0 && <span className="text-amber-400/90">missing sheets: {plan.missing_sheets.join(', ')}</span>}
                  <span>{plan.narration_ready ? 'narration ready' : 'no narration yet (silent track)'}</span>
                </div>
              )}
            </div>

            <div className="px-6 pt-4 grid md:grid-cols-2 gap-3">
              <div className="border border-line rounded-xl bg-panel2 px-4 py-3">
                <div className="text-xs font-semibold mb-2">Focal points (persisted per scene)</div>
                <div className="grid gap-1.5 max-h-44 overflow-y-auto">
                  {blocks.map((b) => (
                    <div key={b.segment_id} className="flex items-center gap-2">
                      <span className="font-mono text-2xs text-mut w-8">SC{b.scene_index + 1}</span>
                      <select
                        className={cn(inputClass, 'text-2xs py-1')}
                        value={focal[b.scene_index] ?? b.focal}
                        onChange={(e) => setFocalFor(b.scene_index, e.target.value)}
                      >
                        <option value="center">Center</option>
                        <option value="left">Left third</option>
                        <option value="right">Right third</option>
                        <option value="top">Upper</option>
                        <option value="bottom">Lower</option>
                      </select>
                      <span className="text-2xs text-mut truncate flex-1">{b.speaker_id} · {b.text_chars} chars</span>
                    </div>
                  ))}
                </div>
              </div>

              <div className="border border-line rounded-xl bg-panel2 px-4 py-3">
                <div className="text-xs font-semibold mb-2">Offline render · assemble · validate</div>
                <button
                  type="button"
                  onClick={() => void runPipeline()}
                  disabled={busy !== '' || !plan}
                  className={cn(btnPrimary, 'w-full justify-center flex items-center gap-2 text-xs py-2')}
                >
                  {busy === 'render' && 'Painting frames (SCRIPTED)…'}
                  {busy === 'assemble' && 'Encoding MP4 (ffmpeg)…'}
                  {busy === 'validate' && 'Validating…'}
                  {busy === '' && 'Render → Assemble → Validate'}
                </button>
                {assets.length > 0 && (
                  <p className="text-2xs text-mut mt-2">
                    {assets.length} assets: {assets.filter((a) => a.kind === 'SCENE').length} scenes ·{' '}
                    {assets.filter((a) => a.kind === 'SHEET').length} sheets ·{' '}
                    {assets.filter((a) => a.kind === 'TITLE').length} title
                  </p>
                )}
                {mp4 && (
                  <p className="text-2xs text-emerald-400/90 mt-1">
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
                            c.status === 'PASS' && 'text-emerald-400',
                            c.status === 'WARN' && 'text-amber-400',
                            c.status === 'FAIL' && 'text-red-400',
                          )}
                        />
                        <span className="font-mono text-mut w-32">{c.name}</span>
                        <span className="text-mut truncate flex-1">{c.detail}</span>
                      </div>
                    ))}
                  </div>
                )}
                <p className="text-2xs text-mut mt-2 leading-tight">
                  SCRIPTED engine is fully offline and deterministic. Set <code>AUTHR_IMG_ENDPOINT</code> for the R2I reference tier.
                </p>
              </div>
            </div>
          </>
        )}
      </div>
    </StudioShell>
  );
}
