import type { JSX } from 'react';
/**
 * Video Studio (Sprint 8) — slide pacing timeline built from the real
 * segment AST. Durations come from cached audio timings when present,
 * else the 150 wpm estimate; per-chapter pacing multipliers adjust the
 * timeline in-session. The FFmpeg MP4 render itself ships with the
 * Phase 6 publishing pipeline; the exact render plan is copyable now.
 */

import { useMemo, useState } from 'react';
import { useBinder } from '../stores/binderStore';
import { useManuscript } from '../stores/manuscriptStore';
import { useSession } from '../stores/sessionStore';
import { buildTimeline, buildRenderScript, formatMs } from './video';

export function VideoStudio(): JSX.Element {
  const books = useBinder((s) => s.books);
  const chapters = useBinder((s) => s.chapters);
  const segments = useManuscript((s) => s.segments);
  const mountedRoot = useSession((s) => s.mountedRoot);
  const [pacing, setPacing] = useState<Record<number, number>>({});
  const [copied, setCopied] = useState(false);

  const timeline = useMemo(() => buildTimeline(chapters, segments, pacing), [chapters, segments, pacing]);
  const script = useMemo(
    () => buildRenderScript(timeline, mountedRoot?.split(/[\\/]/).pop() ?? 'project'),
    [timeline, mountedRoot],
  );

  return (
    <div className="studio">
      <header className="studio-head">
        <h2>Video Studio</h2>
        <p className="dim">
          Slide pacing timeline over the chapter AST. MP4 rendering mounts in{' '}
          <b>Phase 6</b> — the render plan below is the exact FFmpeg script it will run.
        </p>
      </header>

      <section className="studio-grid">
        <div className="studio-card wide">
          <h3>Slide Timeline <span className="mono dim">total {formatMs(timeline.totalMs)}</span></h3>
          <div className="timeline" data-testid="video-timeline">
            {timeline.slides.map((slide) => {
              const widthPct = timeline.totalMs > 0 ? (slide.durationMs / timeline.totalMs) * 100 : 0;
              return (
                <div key={slide.chapterId} className="timeline-slide" style={{ width: `${widthPct}%` }}>
                  <div className="timeline-slide-fill" />
                  <div className="timeline-slide-label">
                    <span className="mono">ch{String(slide.index).padStart(2, '0')}</span>
                    <span title={slide.title}>{slide.title}</span>
                    <span className="mono dim">{formatMs(slide.durationMs)} · {slide.segmentCount} seg</span>
                  </div>
                  <input
                    className="pacing-slider"
                    type="range"
                    min={0.5}
                    max={2}
                    step={0.05}
                    value={pacing[slide.chapterId] ?? 1}
                    title={`Pacing ×${(pacing[slide.chapterId] ?? 1).toFixed(2)}`}
                    onChange={(e) =>
                      setPacing((p) => ({ ...p, [slide.chapterId]: Number(e.target.value) }))
                    }
                  />
                </div>
              );
            })}
            {timeline.slides.length === 0 && <div className="empty">No chapters yet.</div>}
          </div>
        </div>

        <div className="studio-card wide">
          <h3>FFmpeg Render Plan <span className="pill">Phase 6</span></h3>
          <pre className="mono render-script" data-testid="render-script">{script}</pre>
          <button
            className="btn"
            onClick={async () => {
              try {
                await navigator.clipboard.writeText(script);
                setCopied(true);
                window.setTimeout(() => setCopied(false), 1500);
              } catch {
                setCopied(false);
              }
            }}
          >Copy render plan</button>
          {copied && <span className="pill ok">copied</span>}
        </div>
      </section>

      {books.length > 0 && (
        <footer className="studio-foot mono dim">
          bundle: {books[0].title} · target {books[0].target_word_count?.toLocaleString() ?? '—'} words
        </footer>
      )}
    </div>
  );
}
