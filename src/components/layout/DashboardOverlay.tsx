// AUTHR_OS — Project dashboard (R-12): real project stats from the frozen
// commands (book_list / chapter_list / character_list / lore_list /
// audio_cache_stats / craft_retention_report / kdp_check), a bootstrap
// summary for fresh bundles (F-14), and a deterministic next-best-action
// ladder (D5 §5.3): no cast → tag → produce audio → scene → publish.
// Every stat fails closed — a failed probe hides its card, never fakes one.

import { useCallback, useEffect, useState } from 'react';
import {
  AudioLines,
  BookOpen,
  Clapperboard,
  Feather,
  HeartPulse,
  Layers,
  LayoutDashboard,
  Loader2,
  Package,
  PackageCheck,
  RefreshCw,
  ScrollText,
  Sparkles,
  Tags,
  Users,
  X,
} from 'lucide-react';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { useProjectStore } from '../../stores/projectStore';
import { useManuscriptStore } from '../../stores/manuscriptStore';
import { useBookContextStore, resolveActiveBook } from '../../stores/bookContextStore';
import * as api from '../../ipc/api';
import type { ChapterRow } from '../../types/ast';
import { cn, formatNumber } from '../../lib/utils';

interface DashboardData {
  books: number;
  chapters: ChapterRow[];
  words: number;
  cast: number;
  lore: number;
  cache: { chunks: number; bytes: number; hit_rate: number } | null;
  retention: { pass: boolean; score: number } | null;
  kdp: { pass: boolean; score: number } | null;
  /** Per-chapter audio coverage for the produce/scene rules. */
  coverage: Record<number, { total: number; synth: number }>;
}

type Nba = {
  id: string;
  label: string;
  detail: string;
  icon: typeof Users;
  go: () => void;
};

function StatCard({
  icon: Icon,
  label,
  value,
  sub,
}: {
  icon: typeof Users;
  label: string;
  value: string;
  sub?: string;
}) {
  return (
    <div className="rounded-lg border border-line bg-panel2 px-3 py-2.5 flex items-center gap-2.5">
      <Icon size={15} className="text-accent shrink-0" />
      <div className="min-w-0">
        <div className="text-sm font-semibold text-ink font-mono tabular-nums leading-tight">{value}</div>
        <div className="text-2xs text-mut truncate">
          {label}
          {sub ? ` · ${sub}` : ''}
        </div>
      </div>
    </div>
  );
}

export function DashboardOverlay() {
  const open = useWorkspaceStore((s) => s.dashboardOpen);
  const fresh = useWorkspaceStore((s) => s.dashboardFresh);
  const closeDashboard = useWorkspaceStore((s) => s.closeDashboard);
  const manifest = useProjectStore((s) => s.manifest);
  const books = useManuscriptStore((s) => s.books);
  const chapters = useManuscriptStore((s) => s.chapters);
  const activeChapterId = useManuscriptStore((s) => s.activeChapterId);
  const ctxBookId = useBookContextStore((s) => s.activeBookId);
  const activeBook = resolveActiveBook(books, ctxBookId);

  const [data, setData] = useState<DashboardData | null>(null);
  const [loading, setLoading] = useState(false);
  const [untagged, setUntagged] = useState<{ chapterId: number; chapterNumber: number; count: number } | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const [charRes, loreRes, cacheRes] = await Promise.all([
        api.characterList().catch(() => null),
        api.loreList().catch(() => null),
        api.audioCacheStats().catch(() => null),
      ]);
      // Per-chapter audio coverage — same honest probe the publish card uses.
      const coverage: Record<number, { total: number; synth: number }> = {};
      await Promise.all(
        chapters.map(async (c) => {
          try {
            const { segments } = await api.segmentList(c.id);
            coverage[c.id] = {
              total: segments.length,
              synth: segments.filter((s) => Boolean(s.audio_hash)).length,
            };
          } catch {
            coverage[c.id] = { total: 0, synth: 0 };
          }
        }),
      );

      // Untagged dialogue probe on the active (or first) chapter — one cheap call.
      const probeId = activeChapterId ?? chapters[0]?.id ?? null;
      let untaggedHit: { chapterId: number; chapterNumber: number; count: number } | null = null;
      if (probeId != null) {
        try {
          const { segments } = await api.segmentList(probeId);
          const count = segments.filter(
            (s) => s.segment_type === 'DIALOGUE' && s.speaker_id === 'NARRATOR',
          ).length;
          const ch = chapters.find((c) => c.id === probeId);
          untaggedHit = count > 0 ? { chapterId: probeId, chapterNumber: ch?.chapter_number ?? 0, count } : null;
        } catch {
          untaggedHit = null;
        }
      }
      setUntagged(untaggedHit);

      // Retention + KDP probes for the active book (fail-closed → card hidden).
      let retention: DashboardData['retention'] = null;
      let kdp: DashboardData['kdp'] = null;
      if (activeBook) {
        try {
          const rt = await api.craftRetentionReport({ book_id: activeBook.id });
          const scored = rt.chapters.filter((c) => c.word_count > 0);
          retention = {
            pass: scored.length > 0 && scored.every((c) => c.word_length_guidance !== 'unwritten'),
            score: scored.length,
          };
        } catch {
          retention = null;
        }
        try {
          const kc = await api.kdpCheck({ book_id: activeBook.id, price_usd: 4.99 });
          kdp = { pass: kc.pass, score: kc.readiness_score };
        } catch {
          kdp = null;
        }
      }

      setData({
        books: books.length,
        chapters,
        words: chapters.reduce((acc, c) => acc + (c.actual_word_count ?? 0), 0),
        cast: charRes?.characters?.length ?? 0,
        lore: loreRes?.lore?.length ?? 0,
        cache: cacheRes ? { chunks: cacheRes.chunks, bytes: cacheRes.bytes, hit_rate: cacheRes.hit_rate } : null,
        retention,
        kdp,
        coverage,
      });
    } finally {
      setLoading(false);
    }
  }, [chapters, books.length, activeChapterId, activeBook]);

  useEffect(() => {
    if (open) void load();
  }, [open, load]);

  if (!open) return null;

  const goto = (
    studio: 'novel' | 'audio' | 'video' | 'publish' | 'craft',
    intent?: { chapterId?: number; arm?: 'produce' | 'assemble' | 'publish' },
  ) => {
    useWorkspaceStore.getState().switchStudio(studio, intent);
    closeDashboard();
  };

  // ── Deterministic NBA ladder (D5 §5.3, first hit = primary) ─────────────
  const nbas: Nba[] = [];
  if (data) {
    if (data.cast === 0) {
      nbas.push({
        id: 'cast',
        label: 'Build your cast',
        detail: 'No characters yet — audio synthesis needs a voice cast before any chapter can be produced.',
        icon: Users,
        go: () => goto('novel'),
      });
    }
    if (untagged) {
      nbas.push({
        id: 'tag',
        label: `Tag dialogue in chapter ${untagged.chapterNumber}`,
        detail: `${untagged.count} dialogue segment${untagged.count === 1 ? '' : 's'} still speak with the narrator's voice — bulk-tag them in the inspector.`,
        icon: Tags,
        go: () => goto('novel'),
      });
    }
    const produceTarget = data.chapters.find(
      (c) => (data.coverage[c.id]?.total ?? 0) > 0 && data.coverage[c.id].synth === 0,
    );
    if (produceTarget) {
      nbas.push({
        id: 'produce',
        label: `Produce audio for chapter ${produceTarget.chapter_number}`,
        detail: `${data.coverage[produceTarget.id].total} segments have no audio yet — run the produce pipeline (plan → synthesize → master → validate).`,
        icon: AudioLines,
        go: () => goto('audio', { chapterId: produceTarget.id, arm: 'produce' }),
      });
    }
    const assembled = data.chapters.find(
      (c) =>
        (data.coverage[c.id]?.total ?? 0) > 0 &&
        data.coverage[c.id].synth === data.coverage[c.id].total,
    );
    if (assembled) {
      nbas.push({
        id: 'scene',
        label: `Assemble the video novel for chapter ${assembled.chapter_number}`,
        detail: 'Fully synthesized narration is ready — storyboard the scenes and render the MP4.',
        icon: Clapperboard,
        go: () => goto('video', { chapterId: assembled.id, arm: 'assemble' }),
      });
    }
    if (data.kdp && !data.kdp.pass) {
      nbas.push({
        id: 'kdp',
        label: 'Complete your KDP package',
        detail: `Storefront readiness is ${data.kdp.score}/100 — finish keywords, categories and the description in the KDP panel.`,
        icon: Package,
        go: () => goto('craft', { arm: 'publish' }),
      });
    }
  }

  const epubReady = data
    ? Object.values(data.coverage).some((c) => c.total > 0 && c.synth === c.total)
    : false;

  return (
    <div
      className="fixed inset-0 z-[65] flex items-start justify-center px-4 pt-[8vh] pb-10 bg-black/60 anim-fade"
      role="dialog"
      aria-modal="true"
      aria-label="Project dashboard"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) closeDashboard();
      }}
    >
      <div className="anim-modal w-full max-w-2xl rounded-xl border border-line2 bg-panel shadow-2xl flex flex-col max-h-[80vh]">
        {/* Header */}
        <div className="flex items-center gap-2.5 px-5 py-3 border-b border-line shrink-0">
          <LayoutDashboard size={15} className="text-accent" />
          <div className="min-w-0">
            <h2 className="text-sm font-semibold">Project dashboard</h2>
            <p className="text-2xs text-mut truncate">
              {manifest?.title ?? 'Untitled bundle'} · live from the bundle — nothing here is estimated
            </p>
          </div>
          <button
            type="button"
            onClick={closeDashboard}
            className="ml-auto w-7 h-7 rounded-md border border-line bg-panel2 hover:bg-panel3 flex items-center justify-center text-mut hover:text-ink transition-colors"
            aria-label="Close dashboard"
          >
            <X size={13} />
          </button>
        </div>

        <div className="flex-1 min-h-0 overflow-y-auto px-5 py-4 grid gap-4">
          {/* Bootstrap summary (F-14) — only after a fresh project_init. */}
          {fresh && data && (
            <div className="rounded-lg border border-accent/40 bg-accent/10 px-4 py-3 flex items-start gap-2.5" data-testid="bootstrap-summary">
              <Sparkles size={14} className="text-accent mt-0.5 shrink-0" />
              <div className="text-2xs leading-relaxed">
                <b className="text-ink">Bundle ready.</b>{' '}
                <span className="text-mut">
                  Created book “{books[0]?.title ?? manifest?.title ?? 'Untitled Novel'}” with{' '}
                  {data.chapters.length === 0 ? 'no chapters yet' : `${data.chapters.length} chapter${data.chapters.length === 1 ? '' : 's'}`}
                  {data.chapters[0] ? ` (“${data.chapters[0].title}”)` : ''}. The next step below takes you straight there.
                </span>
              </div>
            </div>
          )}

          {/* Stats */}
          {loading && !data ? (
            <div className="flex items-center justify-center gap-2 py-10 text-2xs text-mut">
              <Loader2 size={14} className="animate-spin text-accent" /> Reading the bundle…
            </div>
          ) : data ? (
            <>
              <div className="grid grid-cols-2 sm:grid-cols-3 gap-2" data-testid="dashboard-stats">
                <StatCard icon={BookOpen} label="books" value={formatNumber(data.books)} />
                <StatCard icon={Layers} label="chapters" value={formatNumber(data.chapters.length)} />
                <StatCard icon={Feather} label="words written" value={formatNumber(data.words)} />
                <StatCard icon={Users} label="cast members" value={formatNumber(data.cast)} />
                <StatCard icon={ScrollText} label="lore entries" value={formatNumber(data.lore)} />
                {data.cache && (
                  <StatCard
                    icon={AudioLines}
                    label="audio cache"
                    value={formatNumber(data.cache.chunks)}
                    sub={`${Math.round(data.cache.hit_rate * 100)}% hits`}
                  />
                )}
                {data.retention && (
                  <StatCard
                    icon={HeartPulse}
                    label="retention QC"
                    value={`${data.retention.score} ch`}
                    sub={data.retention.pass ? 'in range' : 'needs attention'}
                  />
                )}
                {data.kdp && (
                  <StatCard
                    icon={data.kdp.pass ? PackageCheck : Package}
                    label="KDP readiness"
                    value={`${data.kdp.score}/100`}
                    sub={data.kdp.pass ? 'pass' : 'gaps'}
                  />
                )}
              </div>

              {/* Next-best-action ladder */}
              <div data-testid="dashboard-nba">
                <div className="flex items-center gap-1.5 mb-2">
                  <Sparkles size={12} className="text-accent" />
                  <span className="text-2xs font-semibold tracking-wider text-mut uppercase">
                    Next best action
                  </span>
                </div>
                {nbas.length === 0 ? (
                  <p className="text-2xs text-mut leading-relaxed rounded-lg border border-line bg-panel2 px-4 py-3">
                    Every stage has something to work with — draft, produce, assemble and publish when
                    ready. The palette (Ctrl/Cmd+K) lists pipeline actions too.
                  </p>
                ) : (
                  <div className="grid gap-2">
                    {nbas.slice(0, 1).map((n) => (
                      <button
                        key={n.id}
                        type="button"
                        onClick={n.go}
                        data-testid={`nba-${n.id}`}
                        className="text-left rounded-lg border border-accent/50 bg-accent/10 hover:bg-accent/20 px-4 py-3 flex items-start gap-3 transition-colors"
                      >
                        <n.icon size={15} className="text-accent mt-0.5 shrink-0" />
                        <span className="min-w-0">
                          <span className="block text-[13px] font-medium text-ink">{n.label}</span>
                          <span className="block text-2xs text-mut mt-0.5 leading-relaxed">{n.detail}</span>
                        </span>
                        <RefreshCw size={12} className="ml-auto mt-1 shrink-0 text-mut" aria-hidden />
                      </button>
                    ))}
                    {nbas.slice(1).map((n) => (
                      <button
                        key={n.id}
                        type="button"
                        onClick={n.go}
                        className="text-left rounded-lg border border-line bg-panel2 hover:bg-panel3 px-4 py-2.5 flex items-center gap-3 transition-colors"
                      >
                        <n.icon size={13} className="text-mut shrink-0" />
                        <span className="text-2xs text-mut hover:text-ink min-w-0">
                          <span className="text-ink">{n.label}</span>
                          <span className="text-mut"> — {n.detail}</span>
                        </span>
                      </button>
                    ))}
                  </div>
                )}
                {epubReady && data.kdp?.pass && (
                  <button
                    type="button"
                    onClick={() => goto('publish')}
                    className="mt-2 w-full text-left rounded-lg border border-line bg-panel2 hover:bg-panel3 px-4 py-2.5 flex items-center gap-3 transition-colors"
                  >
                    <PackageCheck size={13} className="text-ok shrink-0" />
                    <span className="text-2xs text-mut">
                      <span className="text-ink">Ship it</span> — audio and storefront checks pass; the
                      publish card has EPUB, M4B and the signed manifest.
                    </span>
                  </button>
                )}
              </div>
            </>
          ) : (
            <p className="text-2xs text-mut py-8 text-center">
              The bundle could not be read — close and reopen the project to refresh.
            </p>
          )}
        </div>

        <div className="flex items-center gap-3 px-5 py-2 border-t border-line bg-panel2/50 text-2xs text-mut shrink-0">
          <span>stats: book/chapter/character/lore list · audio_cache_stats · retention · KDP check</span>
          <button
            type="button"
            onClick={() => void load()}
            className={cn('ml-auto flex items-center gap-1 hover:text-ink', loading && 'opacity-60')}
          >
            <RefreshCw size={11} className={loading ? 'animate-spin' : ''} /> Refresh
          </button>
        </div>
      </div>
    </div>
  );
}
