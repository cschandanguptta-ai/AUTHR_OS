import { useMemo } from 'react';
import { Activity, ChevronDown, Gauge, ScrollText, SlidersHorizontal, Volume2 } from 'lucide-react';
import { useManuscriptStore } from '../../stores/manuscriptStore';
import { useVoiceCastStore } from '../../stores/voiceCastStore';
import {
  CHAPTER_STATUSES,
  CHAPTER_STATUS_DOTS,
  SEGMENT_TYPES,
  SEGMENT_TYPE_LABELS,
  emotionLabel,
} from '../../lib/constants';
import { cn, formatNumber, formatMs, wordCount } from '../../lib/utils';
import { AudioStateBadge } from '../shared/AudioStateBadge';
import { inputClass } from './Modal';
import type { SegmentType } from '../../types/ast';

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
  const chapters = useManuscriptStore((s) => s.chapters);
  const activeChapterId = useManuscriptStore((s) => s.activeChapterId);
  const updateChapter = useManuscriptStore((s) => s.updateChapter);
  const characters = useVoiceCastStore((s) => s.characters);

  const segment = useMemo(
    () => segments.find((s) => s.segment_id === selectedSegmentId) ?? null,
    [segments, selectedSegmentId],
  );
  const chapter = useMemo(
    () => chapters.find((c) => c.id === activeChapterId) ?? null,
    [chapters, activeChapterId],
  );

  const actualWords = useMemo(
    () => segments.reduce((acc, s) => acc + wordCount(s.text_content), 0),
    [segments],
  );
  const target = chapter?.target_word_count ?? 0;
  const pct = target > 0 ? Math.min(100, Math.round((actualWords / target) * 100)) : 0;

  return (
    <div className="flex-1 min-h-0 overflow-y-auto pb-6">
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
            <select
              className={`${inputClass} py-1`}
              value={segment.segment_type}
              onChange={(e) =>
                updateSegment(segment.segment_id, {
                  segment_type: e.target.value as SegmentType,
                })
              }
            >
              {SEGMENT_TYPES.map((t) => (
                <option key={t} value={t}>
                  {SEGMENT_TYPE_LABELS[t]}
                </option>
              ))}
            </select>
          </Row>

          <Row label="Speaker">
            <select
              className={`${inputClass} py-1`}
              value={segment.speaker_id}
              onChange={(e) =>
                updateSegment(segment.segment_id, { speaker_id: e.target.value })
              }
            >
              <option value="NARRATOR">NARRATOR</option>
              {characters.map((c) => (
                <option key={c.character_key} value={c.character_key}>
                  {c.display_name}
                </option>
              ))}
            </select>
          </Row>

          <Row label="Emotion">
            <select
              className={`${inputClass} py-1`}
              value={segment.emotion}
              onChange={(e) => updateSegment(segment.segment_id, { emotion: e.target.value })}
            >
              {[
                'neutral',
                'tense',
                'whisper',
                'cold_anger',
                'joyful',
                'sorrowful',
                'defiant',
                'serene',
                'urgent',
                'tender',
              ].map((emo) => (
                <option key={emo} value={emo}>
                  {emotionLabel(emo)}
                </option>
              ))}
            </select>
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
