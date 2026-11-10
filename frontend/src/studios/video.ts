/**
 * Video Studio logic (Sprint 8) — slide pacing timeline from real segment
 * data. Durations reuse cached `audio_duration_ms` when present, else the
 * 150 wpm reading estimate; a per-chapter pacing multiplier lets the
 * author stretch or compress slides (session-local until the Phase 6
 * FFmpeg render pipeline lands).
 */

import type { ChapterRow, SegmentRow } from '../types';
import { computeTimings } from '../editor/astBridge';

export interface ChapterSlide {
  chapterId: number;
  index: number;
  title: string;
  segmentCount: number;
  durationMs: number;
  startMs: number;
}

export interface VideoTimeline {
  slides: ChapterSlide[];
  totalMs: number;
}

export function buildTimeline(
  chapters: ChapterRow[],
  segments: SegmentRow[],
  pacing: Record<number, number> = {},
): VideoTimeline {
  const ordered = [...chapters].sort((a, b) => a.chapter_index - b.chapter_index);
  const slides: ChapterSlide[] = [];
  let cursor = 0;
  for (const ch of ordered) {
    const rows = segments
      .filter((s) => s.chapter_id === ch.id)
      .sort((a, b) => a.sequence_order - b.sequence_order);
    const raw = computeTimings(rows).reduce((acc, t) => acc + t.durationMs, 0);
    const factor = pacing[ch.id] ?? 1.0;
    const durationMs = Math.max(1000, Math.round(raw * factor));
    slides.push({
      chapterId: ch.id,
      index: ch.chapter_index,
      title: ch.title,
      segmentCount: rows.length,
      durationMs,
      startMs: cursor,
    });
    cursor += durationMs;
  }
  return { slides, totalMs: cursor };
}

export function formatMs(ms: number): string {
  const total = Math.round(ms / 1000);
  const m = Math.floor(total / 60);
  const s = total % 60;
  return `${m}:${String(s).padStart(2, '0')}`;
}

/**
 * The FFmpeg concat script the Phase 6 renderer will execute. Surfaced as
 * copyable text so the pipeline is inspectable before it is automated.
 */
export function buildRenderScript(timeline: VideoTimeline, projectTitle: string): string {
  const lines: string[] = [
    `# AUTHR_OS Video Studio render plan — ${projectTitle}`,
    `# total runtime ${formatMs(timeline.totalMs)} · ${timeline.slides.length} slides`,
    `# (FFmpeg execution ships with the Phase 6 publishing pipeline)`,
    '',
  ];
  for (const slide of timeline.slides) {
    lines.push(`# slide ch${String(slide.index).padStart(2, '0')} — ${slide.title}`);
    lines.push(
      `ffmpeg -loop 1 -i "slides/ch${String(slide.index).padStart(2, '0')}.png" ` +
        `-i "audio/ch${String(slide.index).padStart(2, '0')}.m4a" ` +
        `-c:v libx264 -tune stillimage -c:a aac -shortest ` +
        `-t ${(slide.durationMs / 1000).toFixed(3)} "out/ch${String(slide.index).padStart(2, '0')}.mp4"`,
    );
  }
  lines.push('', `# concat:`, `printf "file 'out/ch%s.mp4'\\n" 01..${timeline.slides.length} > concat.txt`);
  return lines.join('\n');
}
