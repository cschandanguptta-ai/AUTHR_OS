// AUTHR_OS — chapter audio produce-run state machine (D7 §2, R-01).
//
// The run owns the full orchestration audio_plan → audio_synthesize →
// audio_master → audio_validate → publish_captions → (optional) publish_m4b.
// State lives in a zustand store (NOT component memory) so it survives studio
// tab switches; Cancel arms a flag honored at stage boundaries only — stages
// that already completed keep their real, complete artifacts and the run is
// marked `canceled`, never `done`.

import { create } from 'zustand';
import * as api from '../ipc/api';
import { errorInline } from '../ipc/errorCopy';

export type AudioRunStageId = 'plan' | 'synthesize' | 'master' | 'validate' | 'captions' | 'm4b';

export const AUDIO_RUN_STAGES: Array<{ id: AudioRunStageId; label: string }> = [
  { id: 'plan', label: 'Schedule (audio_plan)' },
  { id: 'synthesize', label: 'Synthesize voices' },
  { id: 'master', label: 'Master & duck' },
  { id: 'validate', label: 'ACX check' },
  { id: 'captions', label: 'Captions (VTT + LRC)' },
  { id: 'm4b', label: 'Audiobook M4B' },
];

export type StageStatus = 'pending' | 'running' | 'done' | 'skipped' | 'failed';

export interface AudioRunStage {
  status: StageStatus;
  ms?: number;
}

export interface AcxReport {
  pass: boolean;
  rms_db: number;
  peak_db: number;
  noise_floor_db: number;
  checks: { rms_ok: boolean; peak_ok: boolean; noise_ok: boolean };
  issues: string[];
}

export interface AudioRunArtifacts {
  masterWav?: string;
  lrc?: string;
  vtt?: string;
  m4b?: string;
  m4bBytes?: number;
}

export interface AudioRunRecord {
  id: string;
  startedAt: number;
  finishedAt?: number;
  chapterId: number;
  chapterLabel: string;
  engine: string;
  segments: number;
  audioMs: number;
  status: 'running' | 'done' | 'failed' | 'canceled';
  stages: Record<AudioRunStageId, AudioRunStage>;
  acx?: AcxReport;
  artifacts: AudioRunArtifacts;
  error?: { title: string; detail: string };
}

export interface RunOptions {
  chapterId: number;
  chapterLabel: string;
  engine: string;
  /** Advanced mastering knobs — the REAL backend parameters. */
  targetRmsDb: number;
  ceilingDb: number;
  duck: boolean;
  duckDepthDb: number;
  duckAttackMs: number;
  duckReleaseMs: number;
  ambientPath: string;
  includeM4b: boolean;
}

interface AudioRunState {
  current: AudioRunRecord | null;
  history: AudioRunRecord[];
  cancelRequested: boolean;
  startRun: (opts: RunOptions) => Promise<void>;
  requestCancel: () => void;
  clearHistory: () => void;
}

function blankStages(): Record<AudioRunStageId, AudioRunStage> {
  return {
    plan: { status: 'pending' },
    synthesize: { status: 'pending' },
    master: { status: 'pending' },
    validate: { status: 'pending' },
    captions: { status: 'pending' },
    m4b: { status: 'pending' },
  };
}

/** audio_master `out` takes a BARE filename — the backend prepends exports/. */
function masterOutName(chapterNumber: number): string {
  return `audiobook_ch${String(chapterNumber).padStart(2, '0')}_master.wav`;
}

function chapterNumberFromLabel(label: string): number {
  const m = label.match(/(\d+)/);
  return m ? Number(m[1]) : 1;
}

export const useAudioRunStore = create<AudioRunState>((set, get) => ({
  current: null,
  history: [],
  cancelRequested: false,

  startRun: async (opts) => {
    if (get().current?.status === 'running') return;
    const record: AudioRunRecord = {
      id: `run-${Date.now()}`,
      startedAt: Date.now(),
      chapterId: opts.chapterId,
      chapterLabel: opts.chapterLabel,
      engine: opts.engine,
      segments: 0,
      audioMs: 0,
      status: 'running',
      stages: blankStages(),
      artifacts: {},
    };
    set({ current: record, cancelRequested: false });

    const patch = (fn: (r: AudioRunRecord) => AudioRunRecord) =>
      set((s) => ({ current: s.current ? fn(s.current) : s.current }));
    const stage = (id: AudioRunStageId, status: StageStatus, ms?: number) =>
      patch((r) => ({
        ...r,
        stages: { ...r.stages, [id]: { status, ms } },
      }));

    try {
      // ── 1 · plan ────────────────────────────────────────────────────────
      stage('plan', 'running');
      const t0 = performance.now();
      const plan = await api.audioPlan({ chapter_id: opts.chapterId });
      stage('plan', 'done', Math.round(performance.now() - t0));
      patch((r) => ({ ...r, segments: plan.turns }));

      if (get().cancelRequested) throw { canceled: true };

      // ── 2 · synthesize (paths live on segments, NOT the response) ───────
      stage('synthesize', 'running');
      const t1 = performance.now();
      const synth = await api.audioSynthesize({ chapter_id: opts.chapterId, engine: opts.engine });
      stage('synthesize', 'done', Math.round(performance.now() - t1));
      if (!('results' in synth)) {
        throw new Error('Unexpected single-segment synthesis response for a chapter run.');
      }
      const segRows = await api.segmentList(opts.chapterId);
      const speechPaths = segRows.segments
        .map((s) => s.audio_cache_path)
        .filter((p): p is string => Boolean(p));
      const allSynthesized = segRows.segments.every((s) => Boolean(s.audio_hash));
      const audioMs = segRows.segments.reduce((a, s) => a + (s.audio_duration_ms ?? 0), 0);
      patch((r) => ({ ...r, audioMs }));

      if (speechPaths.length === 0) {
        throw new Error('No segment produced an audio cache path — nothing to master.');
      }
      if (get().cancelRequested) throw { canceled: true };

      // ── 3 · master ──────────────────────────────────────────────────────
      stage('master', 'running');
      const t2 = performance.now();
      const layers: api.AudioMasterLayer[] = speechPaths.map((wav) => ({ kind: 'SPEECH' as const, wav }));
      if (opts.ambientPath.trim()) {
        layers.push({ kind: 'AMBIENT', wav: opts.ambientPath.trim() });
      }
      const master = await api.audioMaster({
        layers,
        out: masterOutName(chapterNumberFromLabel(opts.chapterLabel)),
        target_rms_db: opts.targetRmsDb,
        ceiling_db: opts.ceilingDb,
        duck: opts.duck,
        duck_params: { depth_db: opts.duckDepthDb, attack_ms: opts.duckAttackMs, release_ms: opts.duckReleaseMs },
      });
      stage('master', 'done', Math.round(performance.now() - t2));
      patch((r) => ({ ...r, artifacts: { ...r.artifacts, masterWav: master.out } }));

      if (get().cancelRequested) throw { canceled: true };

      // ── 4 · validate ────────────────────────────────────────────────────
      stage('validate', 'running');
      const t3 = performance.now();
      const acx = await api.audioValidate({ wav: master.out });
      stage('validate', 'done', Math.round(performance.now() - t3));
      patch((r) => ({ ...r, acx }));

      if (get().cancelRequested) throw { canceled: true };

      // ── 5 · captions ────────────────────────────────────────────────────
      stage('captions', 'running');
      const t4 = performance.now();
      const caps = await api.publishCaptions({ chapter_id: opts.chapterId });
      stage('captions', 'done', Math.round(performance.now() - t4));
      patch((r) => ({
        ...r,
        artifacts: { ...r.artifacts, lrc: caps.lrc, vtt: caps.vtt },
      }));

      // ── 6 · optional M4B (every segment must be synthesized) ────────────
      if (opts.includeM4b && !allSynthesized) {
        stage('m4b', 'skipped');
      } else if (opts.includeM4b) {
        stage('m4b', 'running');
        const t5 = performance.now();
        const m4b = await api.publishM4b({ chapter_id: opts.chapterId });
        stage('m4b', 'done', Math.round(performance.now() - t5));
        patch((r) => ({
          ...r,
          artifacts: { ...r.artifacts, m4b: m4b.out, m4bBytes: m4b.bytes },
        }));
      } else {
        stage('m4b', 'skipped');
      }

      patch((r) => ({ ...r, status: 'done', finishedAt: Date.now() }));
    } catch (err) {
      const canceled = (err as { canceled?: boolean })?.canceled === true;
      const finished: AudioRunRecord = {
        ...(get().current ?? record),
        status: canceled ? 'canceled' : 'failed',
        finishedAt: Date.now(),
        error: canceled
          ? { title: 'Run canceled', detail: 'Stopped at a stage boundary — completed stages keep their files.' }
          : { title: errorInline(err), detail: err instanceof Error ? err.message : String(err) },
      };
      // Mark the running stage failed (unless canceled — boundaries only).
      if (!canceled) {
        for (const st of AUDIO_RUN_STAGES) {
          if (finished.stages[st.id].status === 'running') {
            finished.stages[st.id].status = 'failed';
            break;
          }
        }
      }
      set((s) => ({
        current: finished,
        history: [finished, ...s.history].slice(0, 12),
        cancelRequested: false,
      }));
    }
  },

  requestCancel: () => set({ cancelRequested: true }),

  clearHistory: () => set({ history: [] }),
}));
