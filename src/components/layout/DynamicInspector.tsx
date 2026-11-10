import { useMemo, useState } from 'react';
import { Activity, CheckCheck, ChevronDown, Gauge, ListChecks, ScrollText, SlidersHorizontal, Volume2, AudioLines, Clapperboard, X } from 'lucide-react';
import { useManuscriptStore } from '../../stores/manuscriptStore';
import { useVoiceCastStore } from '../../stores/voiceCastStore';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { useProjectStore } from '../../stores/projectStore';
import { useChapterStats } from '../../hooks/useChapterStats';
import { toast } from '../../stores/uiStore';
import {
  CHAPTER_STATUSES,
  CHAPTER_STATUS_DOTS,
  SEGMENT_TYPES,
  SEGMENT_TYPE_LABELS,
  emotionLabel,
} from '../../lib/constants';
import { cn, formatNumber, formatMs } from '../../lib/utils';
import { AudioStateBadge } from '../shared/AudioStateBadge';
import { Select } from '../shared/Select';
import { inputClass } from './Modal';
import type { SegmentType } from '../../types/ast';

const EMOTIONS = [
  'neutral', 'tense', 'whisper', 'cold_anger', 'joyful',
  'sorrowful', 'defiant', 'serene', 'urgent', 'tender',
] as const;


function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center gap-2">
      <span className="text-2xs text-mut w-16 shrink-0 uppercase tracking-wide">{label}</span>
      <div className="flex-1 min-w-0">{children}</div>
    </div>
  );
}

function SectionTitle({ icon: Icon, children }: { icon: typeof Gauge; children: React.ReactNode }) {
  return (
    <div className="flex items-center gap-1.5 px-4 pt-4 pb-1.5">
      <Icon size={12} className="text-accent" />
      <span className="text-2xs font-semibold tracking-wider text-mut uppercase">{children}</span>
    </div>
  );
}

/** Contextual inspector: segment prosody chrome + chapter meta, live from the stores. */
export function DynamicInspector() {
  const segments = useManuscriptStore((s) => s.segments);
  const selectedSegmentId = useManuscriptStore((s) => s.selectedSegmentId);
  const updateSegment = useManuscriptStore((s) => s.updateSegment);
  const bulkSelectedIds = useManuscriptStore((s) => s.bulkSelectedIds);
  const chapters = useManuscriptStore((s) => s.chapters);
  const activeChapterId = useManuscriptStore((s) => s.activeChapterId);
  const updateChapter = useManuscriptStore((s) => s.updateChapter);
  const characters = useVoiceCastStore((s) => s.characters);
  const readonly = useProjectStore((s) => s.readonly);

  // R-11: pending bulk tag choice — applied on Apply, one transaction per segment.
  const [bulkSpeaker, setBulkSpeaker] = useState('');
  const [bulkEmotion, setBulkEmotion] = useState('');

  const segment = useMemo(
    () => segments.find((s) => s.segment_id === selectedSegmentId) ?? null,
    [segments, selectedSegmentId],
  );
  const chapter = useMemo(
    () => chapters.find((c) => c.id === activeChapterId) ?? null,
    [chapters, activeChapterId],
  );

  const applyBulk = () => {
    const patch: { speaker_id?: string; emotion?: string } = {};
    if (bulkSpeaker) patch.speaker_id = bulkSpeaker;
    if (bulkEmotion) patch.emotion = bulkEmotion;
    if (Object.keys(patch).length === 0) return;
    const n = useManuscriptStore.getState().applyBulkTag(patch);
    if (n > 0) {
      toast(
        'success',
        `Tagged ${n} segment${n === 1 ? '' : 's'}`,
        `${bulkSpeaker ? `speaker → ${bulkSpeaker}` : ''}${bulkSpeaker && bulkEmotion ? ' · ' : ''}${bulkEmotion ? `emotion → ${bulkEmotion}` : ''} — flows through the regular autosave.`,
      );
      setBulkSpeaker('');
      setBulkEmotion('');
    }
  };

  // Worker-backed aggregates (ADR-0008): the progress bar reads worker output
  // instead of re-counting every segment on the main thread.
  const stats = useChapterStats(segments);
  const actualWords = stats.words;
  const target = chapter?.target_word_count ?? 0;
  const pct = target > 0 ? Math.min(100, Math.round((actualWords / target) * 100)) : 0;

  return (
    <div className="flex-1 min-h-0 overflow-y-auto pb-6">
      {/* ── Bulk tagging (R-11) ──────────────────────────────────────────── */}
      <SectionTitle icon={ListChecks}>Bulk tagging</SectionTitle>
      <div className="px-4 pb-2 grid gap-2" data-testid="bulk-tagging">
        <div className="flex items-center gap-1.5 flex-wrap" role="group" aria-label="Quick select segments">
          <span className="text-2xs text-mut mr-0.5">Select:</span>
          {(
            [
              ['all', 'All'],
              ['dialogue', 'Dialogue'],
              ['dialogue_untagged', 'Dialogue w/o speaker'],
              ['narration', 'Narration'],
            ] as const
          ).map(([filter, label]) => (
            <button
              key={filter}
              type="button"
              disabled={readonly || segments.length === 0}
              onClick={() => useManuscriptStore.getState().bulkSelect(filter)}
              className="text-2xs px-2 py-0.5 rounded border border-line bg-panel2 hover:bg-panel3 text-mut hover:text-ink disabled:opacity-40 transition-colors"
            >
              {label}
            </button>
          ))}
        </div>
        {bulkSelectedIds.length > 0 ? (
          <div className="rounded-md border border-accent2/40 bg-panel2 p-2.5 grid gap-2" data-testid="bulk-bar">
            <div className="flex items-center gap-1.5 text-2xs">
              <span className="font-mono font-semibold text-ink">{bulkSelectedIds.length}</span>
              <span className="text-mut">
                segment{bulkSelectedIds.length === 1 ? '' : 's'} selected — Ctrl/Cmd+click paragraphs to adjust
              </span>
              <button
                type="button"
                onClick={() => useManuscriptStore.getState().clearBulk()}
                className="ml-auto text-mut hover:text-ink"
                aria-label="Clear bulk selection"
              >
                <X size={12} />
              </button>
            </div>
            <div className="grid grid-cols-2 gap-2">
              <Select
                value={bulkSpeaker}
                onChange={setBulkSpeaker}
                ariaLabel="Bulk speaker"
                options={[
                  { value: '', label: 'Speaker — keep' },
                  { value: 'NARRATOR', label: 'NARRATOR' },
                  ...characters.map((c) => ({ value: c.character_key, label: c.display_name })),
                ]}
              />
              <Select
                value={bulkEmotion}
                onChange={setBulkEmotion}
                ariaLabel="Bulk emotion"
                options={[
                  { value: '', label: 'Emotion — keep' },
                  ...EMOTIONS.map((emo) => ({ value: emo, label: emotionLabel(emo) })),
                ]}
              />
            </div>
            <button
              type="button"
              disabled={readonly || (!bulkSpeaker && !bulkEmotion)}
              onClick={applyBulk}
              className="flex items-center justify-center gap-1.5 text-2xs px-2.5 py-1.5 rounded-md border border-accent2/50 bg-accent2/10 text-accent2 hover:bg-accent2/20 disabled:opacity-40 transition-colors"
              data-testid="bulk-apply"
            >
              <CheckCheck size={12} /> Apply to {bulkSelectedIds.length} selected
            </button>
          </div>
        ) : (
          <p className="text-2xs text-mut leading-relaxed">
            Ctrl/Cmd+click paragraphs in read-along mode — or a quick select above — then apply one
            speaker/emotion to the whole set. Untagged narration already defaults to NARRATOR.
          </p>
        )}
      </div>

      {/* ── Segment inspector ─────────────────────────────────────────────── */}
      <SectionTitle icon={SlidersHorizontal}>Segment inspector</SectionTitle>
      {segment ? (
        <div className="px-4 grid gap-2.5">
          <div className="flex items-center gap-2 flex-wrap">
            <span
              className={cn(
                'text-2xs font-mono px-1.5 py-0.5 rounded border',
                segment.segment_type === 'DIALOGUE'
                  ? 'border-amber-500/40 bg-amber-500/10 text-warn'
                  : segment.segment_type === 'INTERNAL_MONOLOGUE'
                    ? 'border-violet-500/40 bg-violet-500/10 text-accent2'
                    : 'border-zinc-500/40 bg-zinc-500/10 text-mut',
              )}
            >
              {SEGMENT_TYPE_LABELS[segment.segment_type]}
            </span>
            <span className="font-mono text-2xs text-mut truncate" title={segment.segment_id}>
              #{segment.segment_id.slice(0, 8)}
            </span>
            <AudioStateBadge segment={segment} className="ml-auto" />
          </div>

          <Row label="Type">
            <Select
              value={segment.segment_type}
              onChange={(v) =>
                updateSegment(segment.segment_id, {
                  segment_type: v as SegmentType,
                })
              }
              options={SEGMENT_TYPES.map((t) => ({ value: t, label: SEGMENT_TYPE_LABELS[t] }))}
            />
          </Row>

          <Row label="Speaker">
            <Select
              value={segment.speaker_id}
              onChange={(v) => updateSegment(segment.segment_id, { speaker_id: v })}
              options={[
                { value: 'NARRATOR', label: 'NARRATOR' },
                ...characters.map((c) => ({ value: c.character_key, label: c.display_name })),
              ]}
            />
          </Row>

          <Row label="Emotion">
            <Select
              value={segment.emotion}
              onChange={(v) => updateSegment(segment.segment_id, { emotion: v })}
              options={EMOTIONS.map((emo) => ({ value: emo, label: emotionLabel(emo) }))}
            />
          </Row>

          <div className="mt-1">
            <div className="flex items-center justify-between text-2xs text-mut mb-1">
              <span>PROSODY · RATE</span>
              <span className="font-mono text-ink">×{segment.prosody_rate.toFixed(2)}</span>
            </div>
            <input
              type="range"
              min={0.8}
              max={1.25}
              step={0.01}
              value={segment.prosody_rate}
              onChange={(e) =>
                updateSegment(segment.segment_id, {
                  prosody_rate: Number(e.target.value),
                })
              }
              className="w-full"
              aria-label="Prosody rate"
            />
          </div>
          <div>
            <div className="flex items-center justify-between text-2xs text-mut mb-1">
              <span>PROSODY · PITCH</span>
              <span className="font-mono text-ink">×{segment.prosody_pitch.toFixed(2)}</span>
            </div>
            <input
              type="range"
              min={0.85}
              max={1.15}
              step={0.01}
              value={segment.prosody_pitch}
              onChange={(e) =>
                updateSegment(segment.segment_id, {
                  prosody_pitch: Number(e.target.value),
                })
              }
              className="w-full"
              aria-label="Prosody pitch"
            />
          </div>

          <Row label="Ambient">
            <input
              className={`${inputClass} py-1`}
              value={segment.ambient_cue ?? ''}
              placeholder="rain_on_glass · looping bed"
              onChange={(e) =>
                updateSegment(segment.segment_id, {
                  ambient_cue: e.target.value || null,
                })
              }
            />
          </Row>
          <Row label="SFX">
            <input
              className={`${inputClass} py-1`}
              value={segment.sfx_cue ?? ''}
              placeholder="vault_door · one-shot"
              onChange={(e) =>
                updateSegment(segment.segment_id, { sfx_cue: e.target.value || null })
              }
            />
          </Row>

          {segment.audio_hash && (
            <div className="text-2xs font-mono text-mut flex items-center gap-2 pt-1">
              <Volume2 size={12} className="text-ok" />
              {formatMs(segment.audio_duration_ms)} · hash {segment.audio_hash.slice(0, 10)}…
            </div>
          )}
        </div>
      ) : (
        <p className="px-4 pb-2 text-2xs text-mut leading-relaxed">
          Click a paragraph in the canvas to inspect its speaker, emotion and prosody. New
          paragraphs default to NARRATOR / neutral at ×1.00.
        </p>
      )}

      {/* ── Chapter meta ──────────────────────────────────────────────────── */}
      <SectionTitle icon={ScrollText}>Chapter meta</SectionTitle>
      {chapter ? (
        <div className="px-4 grid gap-2.5 pb-4">
          <Row label="Title">
            <input
              className={`${inputClass} py-1`}
              defaultValue={chapter.title}
              key={chapter.id}
              onBlur={(e) => {
                if (e.target.value.trim() && e.target.value !== chapter.title) {
                  void updateChapter(chapter.id, { title: e.target.value.trim() });
                }
              }}
            />
          </Row>
          <Row label="Status">
            <div className="flex items-center gap-2">
              <span
                className={cn(
                  'w-2 h-2 rounded-full shrink-0',
                  CHAPTER_STATUS_DOTS[chapter.status],
                )}
              />
              <select
                className={`${inputClass} py-1`}
                value={chapter.status}
                onChange={(e) =>
                  void updateChapter(chapter.id, {
                    status: e.target.value as typeof chapter.status,
                  })
                }
              >
                {CHAPTER_STATUSES.map((s) => (
                  <option key={s} value={s}>
                    {s}
                  </option>
                ))}
              </select>
            </div>
          </Row>
          <Row label="Target">
            <input
              type="number"
              className={`${inputClass} py-1 font-mono`}
              defaultValue={chapter.target_word_count}
              key={`t-${chapter.id}`}
              onBlur={(e) => {
                const n = Number(e.target.value);
                if (Number.isFinite(n) && n !== chapter.target_word_count) {
                  void updateChapter(chapter.id, { target_word_count: n });
                }
              }}
            />
          </Row>

          <div className="mt-1.5 rounded-md border border-line bg-panel2 p-3">
            <div className="flex items-center justify-between text-2xs mb-1.5">
              <span className="text-mut uppercase tracking-wide flex items-center gap-1.5">
                <Activity size={11} /> Progress
              </span>
              <span className="font-mono text-ink">
                {formatNumber(actualWords)} / {formatNumber(target)} words
              </span>
            </div>
            <div className="h-1.5 rounded-full bg-panel3 overflow-hidden">
              <div
                className="h-full rounded-full bg-accent transition-all"
                style={{ width: `${pct}%` }}
              />
            </div>
            <div className="text-2xs text-mut mt-1.5 font-mono">
              {pct}% of target ·{' '}
              {useManuscriptStore.getState().dirty ? 'unsaved changes' : 'in sync with bundle'}
            </div>
          </div>

          {/* Next-step handoff (R-07): route to audio, then scene, as the
              chapter becomes eligible — the silent-film trap gets an exit. */}
          {segments.length > 0 && (
            <div className="rounded-md border border-line bg-panel2 p-3 grid gap-1.5" data-testid="inspector-handoff">
              <div className="flex items-center gap-1.5 text-2xs">
                <AudioLines size={11} className="text-accent" />
                <span className="text-mut uppercase tracking-wide">Next step</span>
                {segments.every((s) => s.audio_hash) ? (
                  <span className="ml-auto text-ok font-medium">audio ready</span>
                ) : (
                  <span className="ml-auto text-warn font-medium">
                    {segments.filter((s) => !s.audio_hash).length} segments without audio
                  </span>
                )}
              </div>
              <div className="flex items-center gap-2 flex-wrap">
                <button
                  type="button"
                  onClick={() =>
                    useWorkspaceStore.getState().switchStudio('audio', {
                      chapterId: activeChapterId ?? undefined,
                      arm: 'produce',
                    })
                  }
                  className="text-2xs text-accent hover:underline flex items-center gap-1"
                  data-testid="handoff-produce-audio"
                >
                  <AudioLines size={10} /> Produce audio for ch {chapter?.chapter_number ?? '—'}
                </button>
                {segments.every((s) => s.audio_hash) && (
                  <button
                    type="button"
                    onClick={() =>
                      useWorkspaceStore.getState().switchStudio('video', {
                        chapterId: activeChapterId ?? undefined,
                        arm: 'assemble',
                      })
                    }
                    className="text-2xs text-mut hover:text-ink flex items-center gap-1"
                    data-testid="handoff-assemble"
                  >
                    <Clapperboard size={10} /> Assemble the video novel
                  </button>
                )}
              </div>
            </div>
          )}
        </div>
      ) : (
        <p className="px-4 text-2xs text-mut">No chapter selected.</p>
      )}

      <div className="mx-4 mt-2 border-t border-line" />
      <p className="px-4 py-2.5 text-2xs text-mut leading-relaxed flex items-start gap-1.5">
        <ChevronDown size={11} className="mt-0.5 shrink-0 text-mut/70" />
        Prosody edits flow through the same 400 ms autosave as prose — one transaction per sync,
        audio hashes invalidated only on changed rows.
      </p>
    </div>
  );
}
