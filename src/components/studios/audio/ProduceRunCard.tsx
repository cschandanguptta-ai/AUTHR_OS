// ProduceRunCard — one Run action producing a validated chapter audiobook
// inside the window (D7 §2, R-01): audio_plan → audio_synthesize →
// audio_master → audio_validate → publish_captions → optional publish_m4b,
// with per-stage progress, stage-boundary cancel, an ACX results panel and
// the artifact list. The mastering knobs are the REAL backend parameters.

import { useEffect, useMemo, useState } from 'react';
import {
  CheckCircle2,
  CircleAlert,
  Clapperboard,
  Loader2,
  MinusCircle,
  Play,
  Square,
  XCircle,
} from 'lucide-react';
import { useManuscriptStore } from '../../../stores/manuscriptStore';
import { useAudioRunStore, AUDIO_RUN_STAGES, type AudioRunStageId } from '../../../stores/audioRunStore';
import { useWorkspaceStore } from '../../../stores/workspaceStore';
import { VOICE_ENGINES } from '../../../lib/constants';
import { cn, formatMs } from '../../../lib/utils';
import { btnPrimary, inputClass } from '../../layout/Modal';

const CLOUD_ENGINES = VOICE_ENGINES.filter((e) => e !== 'KOKORO_LOCAL');

function StageIcon({ status }: { status: string }) {
  if (status === 'running') return <Loader2 size={11} className="animate-spin text-accent shrink-0" />;
  if (status === 'done') return <CheckCircle2 size={11} className="text-ok shrink-0" />;
  if (status === 'failed') return <XCircle size={11} className="text-bad shrink-0" />;
  if (status === 'skipped') return <MinusCircle size={11} className="text-mut/70 shrink-0" />;
  return <span className="w-[11px] shrink-0" />;
}

function AcxChip({ label, value, ok, unit = ' dB' }: { label: string; value: number; ok: boolean; unit?: string }) {
  return (
    <span
      className={cn(
        'text-2xs font-mono px-1.5 py-0.5 rounded border',
        ok ? 'border-emerald-500/40 bg-emerald-500/10 text-ok' : 'border-rose-500/40 bg-rose-500/10 text-bad',
      )}
      title={ok ? `${label} within ACX range` : `${label} outside the ACX target`}
    >
      {label} {value.toFixed(1)}{unit} {ok ? '✓' : '✗'}
    </span>
  );
}

export function ProduceRunCard() {
  const chapters = useManuscriptStore((s) => s.chapters);
  const activeChapterId = useManuscriptStore((s) => s.activeChapterId);
  const selectChapter = useManuscriptStore((s) => s.selectChapter);
  const segments = useManuscriptStore((s) => s.segments);
  const loadSegments = useManuscriptStore((s) => s.loadSegments);
  const activeStudio = useWorkspaceStore((s) => s.activeStudio);
  const consumeIntent = useWorkspaceStore((s) => s.consumeIntent);

  const run = useAudioRunStore((s) => s.current);
  const cancelRequested = useAudioRunStore((s) => s.cancelRequested);
  const startRun = useAudioRunStore((s) => s.startRun);
  const requestCancel = useAudioRunStore((s) => s.requestCancel);

  const [engine, setEngine] = useState('SCRIPTED');
  const [includeM4b, setIncludeM4b] = useState(false);
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [targetRmsDb, setTargetRmsDb] = useState(-20);
  const [ceilingDb, setCeilingDb] = useState(-3.1);
  const [duck, setDuck] = useState(true);
  const [duckDepthDb, setDuckDepthDb] = useState(18);
  const [duckAttackMs, setDuckAttackMs] = useState(120);
  const [duckReleaseMs, setDuckReleaseMs] = useState(750);
  const [ambientPath, setAmbientPath] = useState('');

  // Chapter select follows the shared active chapter; a Scene→produce handoff
  // consumes the one-shot intent (R-07) and pre-arms the chapter.
  useEffect(() => {
    const intent = consumeIntent('audio');
    if (intent?.chapterId != null) {
      selectChapter(intent.chapterId);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (activeStudio === 'audio' && activeChapterId) void loadSegments(activeChapterId);
  }, [activeStudio, activeChapterId, loadSegments]);

  // A finished run invalidates the local audio picture — reload the chapter's
  // segments so the M4B gate and the unrendered counts reflect reality.
  const runFinished = run?.status === 'done' || run?.status === 'canceled';
  useEffect(() => {
    if (runFinished && activeChapterId) void loadSegments(activeChapterId);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [runFinished, run?.id]);

  const chapter = chapters.find((c) => c.id === activeChapterId) ?? null;
  const running = run?.status === 'running';
  const allSynthesized = segments.length > 0 && segments.every((s) => Boolean(s.audio_hash));
  const synthesizedCount = segments.filter((s) => Boolean(s.audio_hash)).length;
  const unrendered = segments.length - synthesizedCount;

  const m4bBlockedReason = useMemo(() => {
    if (segments.length === 0) return 'no segments loaded for this chapter yet';
    if (!allSynthesized) return `${unrendered} of ${segments.length} segments are not synthesized yet`;
    return null;
  }, [segments, allSynthesized, unrendered]);

  useEffect(() => {
    if (m4bBlockedReason) setIncludeM4b(false);
  }, [m4bBlockedReason]);

  const canRun = Boolean(chapter) && !running && segments.length > 0;

  const go = () => {
    if (!chapter) return;
    void startRun({
      chapterId: chapter.id,
      chapterLabel: `Ch ${chapter.chapter_number} — ${chapter.title}`,
      engine,
      targetRmsDb,
      ceilingDb,
      duck,
      duckDepthDb,
      duckAttackMs,
      duckReleaseMs,
      ambientPath,
      includeM4b,
    });
  };

  const useInSceneStudio = () => {
    if (!chapter) return;
    useWorkspaceStore.getState().switchStudio('video', { chapterId: chapter.id, arm: 'assemble' });
  };

  return (
    <div className="border border-line2 rounded-xl bg-panel2/60 px-4 py-3.5 grid gap-3" data-testid="produce-run-card">
      <div className="flex items-center gap-2 flex-wrap">
        <Play size={13} className="text-accent" />
        <h3 className="text-xs font-semibold">Produce chapter audio</h3>
        <span className="text-2xs text-mut">
          plan → synthesize → master → ACX → captions{includeM4b ? ' → M4B' : ''}
        </span>
        <label className="ml-auto flex items-center gap-2">
          <span className="text-2xs text-mut uppercase tracking-wide">Chapter</span>
          <select
            className={cn(inputClass, 'w-56')}
            value={activeChapterId ?? ''}
            onChange={(e) => selectChapter(Number(e.target.value))}
            disabled={running}
          >
            {chapters.map((c) => (
              <option key={c.id} value={c.id}>Ch {c.chapter_number} — {c.title}</option>
            ))}
          </select>
        </label>
      </div>

      <div className="flex items-center gap-3 flex-wrap">
        <label className="flex items-center gap-2">
          <span className="text-2xs text-mut uppercase tracking-wide">Engine</span>
          <select
            className={cn(inputClass, 'w-44')}
            value={engine}
            onChange={(e) => setEngine(e.target.value)}
            disabled={running}
          >
            <option value="SCRIPTED">SCRIPTED (offline)</option>
            {CLOUD_ENGINES.map((e) => (
              <option key={e} value={e}>{e} — requires network</option>
            ))}
          </select>
        </label>

        <label
          className={cn(
            'flex items-center gap-1.5 text-2xs',
            m4bBlockedReason ? 'text-mut/60' : 'text-mut',
          )}
          title={m4bBlockedReason ?? 'ffmpeg AAC-LC audiobook with chapter atoms'}
        >
          <input
            type="checkbox"
            checked={includeM4b}
            disabled={running || Boolean(m4bBlockedReason)}
            onChange={(e) => setIncludeM4b(e.target.checked)}
          />
          Also render M4B audiobook
          {m4bBlockedReason && <span className="text-warn/80">— {m4bBlockedReason}</span>}
        </label>

        <button
          type="button"
          onClick={() => setAdvancedOpen((o) => !o)}
          className="ml-auto text-2xs text-mut hover:text-ink underline decoration-dotted"
          aria-expanded={advancedOpen}
        >
          Mastering {advancedOpen ? 'hide' : 'advanced'}
        </button>

        {running ? (
          <button
            type="button"
            onClick={requestCancel}
            className="flex items-center gap-1.5 text-2xs font-medium border border-rose-500/40 bg-rose-500/10 text-bad rounded px-3 py-1.5"
            title="Stops at the next stage boundary — completed stages keep their files"
          >
            <Square size={11} /> {cancelRequested ? 'Stopping…' : 'Cancel'}
          </button>
        ) : (
          <button
            type="button"
            onClick={go}
            disabled={!canRun}
            className={cn(btnPrimary, 'text-2xs px-3.5 py-1.5')}
            title={chapter ? `Run the ${engine} pipeline on this chapter` : 'Select a chapter first'}
          >
            <Play size={11} /> Run
          </button>
        )}
      </div>

      {advancedOpen && (
        <div className="grid gap-2 border border-line rounded-lg bg-panel px-3 py-2.5" data-testid="mastering-advanced">
          <div className="text-2xs text-mut uppercase tracking-wide font-semibold">Mastering chain — real audio_master parameters</div>
          <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
            <label className="grid gap-1">
              <span className="text-2xs text-mut flex justify-between">
                Target RMS <span className="font-mono text-ink">{targetRmsDb.toFixed(1)} dB</span>
              </span>
              <input type="range" min={-24} max={-16} step={0.1} value={targetRmsDb}
                onChange={(e) => setTargetRmsDb(Number(e.target.value))} disabled={running} />
            </label>
            <label className="grid gap-1">
              <span className="text-2xs text-mut flex justify-between">
                Ceiling <span className="font-mono text-ink">{ceilingDb.toFixed(1)} dBFS</span>
              </span>
              <input type="range" min={-6} max={-1} step={0.1} value={ceilingDb}
                onChange={(e) => setCeilingDb(Number(e.target.value))} disabled={running} />
            </label>
            <label className="grid gap-1">
              <span className="text-2xs text-mut flex items-center gap-1.5">
                <input type="checkbox" checked={duck} onChange={(e) => setDuck(e.target.checked)} disabled={running} />
                Sidechain ducking
              </span>
              <span className="text-2xs text-mut/70 leading-tight">
                ambient bed ducks under speech (default −18 dB · 120 ms · 750 ms)
              </span>
            </label>
            <label className="grid gap-1">
              <span className="text-2xs text-mut uppercase tracking-wide">Ambient layer (wav path)</span>
              <input
                className={cn(inputClass, 'font-mono text-2xs')}
                value={ambientPath}
                onChange={(e) => setAmbientPath(e.target.value)}
                placeholder="media/ambient/rain_loop.wav"
                disabled={running}
              />
            </label>
          </div>
          {duck && (
            <div className="grid grid-cols-3 gap-3">
              <label className="grid gap-1">
                <span className="text-2xs text-mut flex justify-between">
                  Duck depth <span className="font-mono text-ink">{duckDepthDb} dB</span>
                </span>
                <input type="range" min={6} max={30} step={1} value={duckDepthDb}
                  onChange={(e) => setDuckDepthDb(Number(e.target.value))} disabled={running} />
              </label>
              <label className="grid gap-1">
                <span className="text-2xs text-mut flex justify-between">
                  Attack <span className="font-mono text-ink">{duckAttackMs} ms</span>
                </span>
                <input type="range" min={20} max={400} step={10} value={duckAttackMs}
                  onChange={(e) => setDuckAttackMs(Number(e.target.value))} disabled={running} />
              </label>
              <label className="grid gap-1">
                <span className="text-2xs text-mut flex justify-between">
                  Release <span className="font-mono text-ink">{duckReleaseMs} ms</span>
                </span>
                <input type="range" min={200} max={2000} step={50} value={duckReleaseMs}
                  onChange={(e) => setDuckReleaseMs(Number(e.target.value))} disabled={running} />
              </label>
            </div>
          )}
        </div>
      )}

      {/* Per-stage progress */}
      {run && (
        <div className="grid gap-1" data-testid="run-progress">
          {AUDIO_RUN_STAGES.map((st) => {
            const s = run.stages[st.id as AudioRunStageId];
            return (
              <div key={st.id} className="flex items-center gap-2 text-2xs">
                <StageIcon status={s.status} />
                <span className={cn('w-44', s.status === 'pending' ? 'text-mut/70' : 'text-ink')}>{st.label}</span>
                <span className="font-mono text-mut">{s.ms != null ? `${s.ms} ms` : ''}</span>
                {s.status === 'skipped' && <span className="text-mut/60">not requested</span>}
              </div>
            );
          })}
        </div>
      )}

      {/* ACX panel — real audio_validate numbers, never a faked pass */}
      {run?.acx && (
        <div className="grid gap-1.5" data-testid="acx-panel">
          <div className="flex items-center gap-2 flex-wrap">
            <span className={cn('text-2xs font-semibold px-2 py-0.5 rounded-full border',
              run.acx.pass ? 'border-emerald-500/40 bg-emerald-500/10 text-ok' : 'border-amber-500/40 bg-amber-500/10 text-warn')}>
              ACX {run.acx.pass ? 'PASS' : 'CHECK'}
            </span>
            <AcxChip label="RMS" value={run.acx.rms_db} ok={run.acx.checks.rms_ok} />
            <AcxChip label="Peak" value={run.acx.peak_db} ok={run.acx.checks.peak_ok} />
            <AcxChip label="Noise" value={run.acx.noise_floor_db} ok={run.acx.checks.noise_ok} />
            <span className="text-2xs text-mut">targets: −23…−18 dB · ≤−3 dB · ≤−60 dB</span>
          </div>
          {run.acx.issues.length > 0 && (
            <ul className="text-2xs text-mut list-disc pl-5">
              {run.acx.issues.map((i, k) => <li key={k}>{i}</li>)}
            </ul>
          )}
          {!run.acx.pass && run.acx.checks.noise_ok === false && (
            <p className="text-2xs text-mut/80 leading-relaxed">
              The SCRIPTED engine emits continuous tones, so the noise gate reads the signal itself
              as floor. Cloud engines with natural pauses report true noise floors.
            </p>
          )}
        </div>
      )}

      {/* Artifacts */}
      {run && (run.status === 'done' || run.status === 'canceled') && (
        <div className="grid gap-1" data-testid="run-artifacts">
          <div className="text-2xs text-mut uppercase tracking-wide font-semibold">Artifacts — {run.chapterLabel}</div>
          {run.artifacts.masterWav && (
            <div className="text-2xs font-mono text-mut">✓ {run.artifacts.masterWav}</div>
          )}
          {run.artifacts.vtt && <div className="text-2xs font-mono text-mut">✓ {run.artifacts.vtt}</div>}
          {run.artifacts.lrc && <div className="text-2xs font-mono text-mut">✓ {run.artifacts.lrc}</div>}
          {run.artifacts.m4b && (
            <div className="text-2xs font-mono text-mut">
              ✓ {run.artifacts.m4b} · {((run.artifacts.m4bBytes ?? 0) / 1e6).toFixed(2)} MB
            </div>
          )}
          <div className="text-2xs text-mut">
            {run.segments} segments · {formatMs(run.audioMs)} audio
            {run.finishedAt ? ` · finished ${new Date(run.finishedAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}` : ''}
          </div>
          {run.status === 'done' && (
            <div>
              <button
                type="button"
                onClick={useInSceneStudio}
                className="text-2xs text-accent hover:underline flex items-center gap-1"
                data-testid="handoff-scene"
              >
                <Clapperboard size={11} /> Use in Scene Studio → assemble with narration
              </button>
            </div>
          )}
        </div>
      )}

      {/* Failed state */}
      {run?.status === 'failed' && (
        <div className="flex items-start gap-2 text-2xs text-bad" data-testid="run-error">
          <CircleAlert size={12} className="mt-0.5 shrink-0" />
          <div>
            <div className="font-medium">{run.error?.title}</div>
            <div className="text-mut break-words">{run.error?.detail}</div>
          </div>
        </div>
      )}
      {run?.status === 'canceled' && (
        <div className="text-2xs text-warn" data-testid="run-canceled">
          {run.error?.title} — stopped at a stage boundary. Completed stages keep their real files.
        </div>
      )}
    </div>
  );
}
