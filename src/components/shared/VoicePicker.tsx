// VoicePicker — unified voice selection (D7 §6, R-05): per-engine catalogs
// with search, paste-custom-ID for private voices, and an honest Preview that
// synthesizes a one-line sample through the real audio_synthesize path —
// SCRIPTED previews always work offline; cloud engines preview only when the
// engine is reachable and fail closed with mapped copy. Playback inside the
// window is not possible yet (media_read serves bundle media/ images only —
// frozen contract), so the preview reports the real synthesis result
// (duration, hash, cache path) instead of faking an audio widget.

import { useMemo, useState } from 'react';
import { Loader2, Play, Search } from 'lucide-react';
import { audioSynthesize } from '../../ipc/api';
import { mapError } from '../../ipc/errorCopy';
import { engineCatalog, PREVIEW_LINE, type VoiceDef } from '../../lib/voiceCatalog';
import { cn } from '../../lib/utils';
import { inputClass } from '../layout/Modal';

interface VoicePickerProps {
  engine: string;
  value: string;
  onChange: (voiceId: string) => void;
  /** Compact mode for embedded rows. */
  compact?: boolean;
}

export function VoicePicker({ engine, value, onChange, compact }: VoicePickerProps) {
  const catalog = engineCatalog(engine);
  const [filter, setFilter] = useState('');
  const [showCustom, setShowCustom] = useState(false);
  const [preview, setPreview] = useState<{ busy: boolean; line?: string; ok?: boolean } | null>(null);

  const voices: VoiceDef[] = useMemo(() => {
    const q = filter.trim().toLowerCase();
    if (!q) return catalog.voices;
    return catalog.voices.filter(
      (v) => v.id.toLowerCase().includes(q) || v.label.toLowerCase().includes(q),
    );
  }, [catalog, filter]);

  const runPreview = async () => {
    if (!value.trim() || preview?.busy) return;
    setPreview({ busy: true });
    try {
      const res = await audioSynthesize({
        text: PREVIEW_LINE,
        voice: value.trim(),
        engine,
      });
      if ('path' in res) {
        setPreview({
          busy: false,
          ok: true,
          line: `synthesized ${res.duration_ms != null ? `${(res.duration_ms / 1000).toFixed(1)} s` : ''} · ${res.engine} · cache ${res.path.slice(0, 28)}…`,
        });
      } else {
        setPreview({ busy: false, ok: true, line: 'synthesized — see the synthesis cache' });
      }
    } catch (err) {
      const mapped = mapError(err);
      setPreview({ busy: false, ok: false, line: `${mapped.copy.body} ${mapped.detail}`.trim() });
    }
  };

  return (
    <div className="grid gap-2" data-testid={`voice-picker-${engine}`}>
      <div className="flex items-center gap-2 flex-wrap">
        <span
          className={cn(
            'text-2xs px-1.5 py-0.5 rounded border font-mono shrink-0',
            catalog.offline
              ? 'border-emerald-500/40 bg-emerald-500/10 text-ok'
              : 'border-amber-500/40 bg-amber-500/10 text-warn',
          )}
          title={catalog.offline ? 'works fully offline' : 'requires network + API key at synthesis time'}
        >
          {catalog.offline ? 'offline' : 'network'}
        </span>
        {!compact && catalog.customHint && (
          <span className="text-2xs text-mut leading-tight flex-1 min-w-[20ch]">{catalog.customHint}</span>
        )}
        {catalog.cataloged && (
          <button
            type="button"
            onClick={() => setShowCustom((s) => !s)}
            className="text-2xs text-mut hover:text-ink underline decoration-dotted shrink-0"
          >
            {showCustom ? 'pick from list' : 'paste custom ID'}
          </button>
        )}
      </div>

      {showCustom || !catalog.cataloged ? (
        <input
          className={cn(inputClass, 'font-mono text-xs')}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder={catalog.cataloged ? 'voice id' : 'paste voice UUID…'}
          aria-label="Voice ID"
        />
      ) : (
        <>
          <div className="relative">
            <Search size={11} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-mut" />
            <input
              className={cn(inputClass, 'pl-7 text-2xs py-1')}
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
              placeholder={`Search ${catalog.voices.length} ${engine} voices…`}
              aria-label="Search voices"
            />
          </div>
          <div className={cn('grid gap-1 overflow-y-auto', compact ? 'max-h-36' : 'max-h-48')}>
            {voices.map((v) => (
              <button
                key={v.id}
                type="button"
                onClick={() => onChange(v.id)}
                className={cn(
                  'flex items-center gap-2 rounded-md border px-2 py-1.5 text-left transition-colors',
                  value === v.id
                    ? 'border-accent/60 bg-accent/10'
                    : 'border-line bg-panel2 hover:border-line2',
                )}
                data-testid={`voice-option-${v.id}`}
              >
                <span className={cn('text-xs', value === v.id ? 'text-accent font-semibold' : 'text-ink')}>
                  {v.label}
                </span>
                <span className="font-mono text-2xs text-mut truncate flex-1">{v.id}</span>
                {v.hint && <span className="text-2xs text-mut/70 shrink-0 hidden sm:inline">{v.hint}</span>}
              </button>
            ))}
            {voices.length === 0 && (
              <p className="text-2xs text-mut px-1 py-1">No voice matches “{filter}”.</p>
            )}
          </div>
        </>
      )}

      <div className="flex items-center gap-2 flex-wrap">
        <button
          type="button"
          onClick={() => void runPreview()}
          disabled={!value.trim() || preview?.busy}
          className="text-2xs px-2 py-1 rounded border border-line text-mut hover:text-ink disabled:opacity-50 flex items-center gap-1"
          title="Synthesizes one real line through this engine (SCRIPTED always available; cloud engines fail closed when unreachable)"
          data-testid="voice-preview"
        >
          {preview?.busy ? <Loader2 size={10} className="animate-spin" /> : <Play size={10} />}
          Preview voice
        </button>
        {preview?.line && (
          <span className={cn('text-2xs leading-tight', preview.ok ? 'text-ok' : 'text-bad')} data-testid="voice-preview-result">
            {preview.line}
          </span>
        )}
      </div>
      {preview?.line && preview.ok && (
        <p className="text-2xs text-mut/70 leading-tight">
          Sample renders to the synthesis cache; in-window playback needs a media_read audio
          extension (frozen contract v1.6 — audio files stay CLI/playable-in-folder for now).
        </p>
      )}
    </div>
  );
}
