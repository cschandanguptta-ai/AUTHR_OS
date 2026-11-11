import { Check, Play } from 'lucide-react';
import { cn } from '../../lib/utils';
import { formatMs } from '../../lib/utils';
import type { NarrativeSegmentAst } from '../../types/ast';

/**
 * Audio render state badge — `Unrendered ▶` while audio_hash is null,
 * `Rendered ✓` once the segment carries a synthesized hash (contract §5).
 */
export function AudioStateBadge({
  segment,
  className,
}: {
  segment: Pick<NarrativeSegmentAst, 'audio_hash' | 'audio_duration_ms'>;
  className?: string;
}) {
  const rendered = Boolean(segment.audio_hash);
  return (
    <span
      className={cn(
        'inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-2xs font-mono border',
        rendered
          ? 'border-emerald-500/40 bg-emerald-500/10 text-ok'
          : 'border-zinc-500/40 bg-zinc-500/5 text-mut',
        className,
      )}
      title={
        rendered
          ? `Rendered · hash ${segment.audio_hash?.slice(0, 12)} · ${formatMs(segment.audio_duration_ms)}`
          : 'Unrendered — awaiting synthesis'
      }
    >
      {rendered ? (
        <>
          <Check size={10} className="text-ok" /> Rendered
        </>
      ) : (
        <>
          <Play size={10} /> Unrendered
        </>
      )}
    </span>
  );
}
