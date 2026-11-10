// AudioStudio — voice profiling, casting, pronunciation, and the in-window
// produce pipeline (D7 §2, R-01): one Run action takes a chapter from text to
// an ACX-checked, captioned audiobook. Everything here is real contract-backed
// data — the old decorative DSP rack is gone; the mastering knobs on the
// produce card are the actual audio_master parameters.

import { useEffect, useMemo, useState } from 'react';
import { AudioWaveform, BookA, Mic2, Save, Sparkles, Trash2 } from 'lucide-react';
import { StudioShell } from '../shared/StudioShell';
import { EmptyState } from '../shared/EmptyState';
import { useVoiceCastStore } from '../../stores/voiceCastStore';
import { useLoreStore } from '../../stores/loreStore';
import { useManuscriptStore } from '../../stores/manuscriptStore';
import { useAudioRunStore, type AudioRunRecord } from '../../stores/audioRunStore';
import { audioCacheStats, audioPronounce, type AudioCacheStats, type PronounceEntry } from '../../ipc/api';
import { ROLE_BADGES, EMOTION_PRESETS, emotionChipClass } from '../../lib/constants';
import { VOICE_ENGINE_OPTIONS } from '../../lib/voiceCatalog';
import { VoicePicker } from '../shared/VoicePicker';
import { cn, formatBytes } from '../../lib/utils';
import { inputClass, btnPrimary } from '../layout/Modal';
import { toast } from '../../stores/uiStore';
import { ProduceRunCard } from './audio/ProduceRunCard';
import type { CharacterRow } from '../../types/ast';

function SectionTitle({ icon: Icon, children, aside }: {
  icon: typeof Mic2;
  children: React.ReactNode;
  aside?: React.ReactNode;
}) {
  return (
    <div className="flex items-center gap-2 px-6 pt-6 pb-2">
      <Icon size={13} className="text-accent" />
      <h2 className="text-xs font-semibold tracking-wider text-mut uppercase">{children}</h2>
      <div className="ml-auto">{aside}</div>
    </div>
  );
}

function VoiceRow({ character }: { character: CharacterRow }) {
  const upsertCharacter = useVoiceCastStore((s) => s.upsertCharacter);
  const deleteCharacter = useVoiceCastStore((s) => s.deleteCharacter);
  const [draft, setDraft] = useState(character);
  const [busy, setBusy] = useState(false);

  useEffect(() => setDraft(character), [character]);

  const changed =
    draft.voice_engine !== character.voice_engine ||
    draft.voice_id !== character.voice_id ||
    Math.abs(draft.base_rate - character.base_rate) > 1e-6 ||
    Math.abs(draft.base_pitch - character.base_pitch) > 1e-6 ||
    draft.cadence_notes !== character.cadence_notes;

  const save = async () => {
    setBusy(true);
    await upsertCharacter({
      character_key: draft.character_key,
      display_name: draft.display_name,
      voice_engine: draft.voice_engine,
      voice_id: draft.voice_id,
      base_rate: draft.base_rate,
      base_pitch: draft.base_pitch,
      cadence_notes: draft.cadence_notes,
    });
    setBusy(false);
    toast('success', 'Voice profile saved', `${draft.display_name} → ${draft.voice_engine}`);
  };

  return (
    <div className="border border-line rounded-xl bg-panel2 px-4 py-3 grid gap-2.5">
      <div className="flex items-center gap-2 flex-wrap">
        <span className={cn('text-2xs px-2 py-0.5 rounded-full border', ROLE_BADGES[draft.role])}>
          {draft.role}
        </span>
        <span className="text-sm font-semibold">{draft.display_name}</span>
        <span className="font-mono text-2xs text-mut" title={draft.character_key}>{draft.character_key}</span>
        <div className="ml-auto flex items-center gap-2">
          {changed && !busy && (
            <button type="button" onClick={save} className={cn(btnPrimary, 'text-2xs px-2.5 py-1 flex items-center gap-1')}>
              <Save size={11} /> Save voice
            </button>
          )}
          {busy && <span className="text-2xs text-mut">saving…</span>}
          {draft.role !== 'NARRATOR' && (
            <button
              type="button"
              title="Remove from cast (blocked while segments reference this speaker)"
              onClick={async () => {
                const ok = await deleteCharacter(draft.character_key);
                if (!ok) toast('warn', 'Cannot remove', 'Segments still reference this speaker.');
              }}
              className="text-mut hover:text-bad transition-colors"
              aria-label={`Delete ${draft.display_name}`}
            >
              <Trash2 size={13} />
            </button>
          )}
        </div>
      </div>
      <div className="grid grid-cols-[auto_1fr] gap-3">
        <label className="grid gap-1 w-48">
          <span className="text-2xs text-mut uppercase tracking-wide">Voice engine</span>
          <select
            className={inputClass}
            value={draft.voice_engine}
            onChange={(e) => setDraft({ ...draft, voice_engine: e.target.value })}
          >
            {VOICE_ENGINE_OPTIONS.map((e) => <option key={e} value={e}>{e}</option>)}
          </select>
        </label>
        <div className="grid gap-1">
          <span className="text-2xs text-mut uppercase tracking-wide">Voice</span>
          <VoicePicker engine={draft.voice_engine} value={draft.voice_id} onChange={(voiceId) => setDraft({ ...draft, voice_id: voiceId })} compact />
        </div>
      </div>
      <div className="grid grid-cols-2 gap-4">
        <label className="grid gap-1">
          <span className="text-2xs text-mut uppercase tracking-wide flex justify-between">
            Base rate <span className="font-mono text-ink">{draft.base_rate.toFixed(2)}×</span>
          </span>
          <input
            type="range" min={0.8} max={1.25} step={0.01}
            value={draft.base_rate}
            onChange={(e) => setDraft({ ...draft, base_rate: Number(e.target.value) })}
          />
        </label>
        <label className="grid gap-1">
          <span className="text-2xs text-mut uppercase tracking-wide flex justify-between">
            Base pitch <span className="font-mono text-ink">{draft.base_pitch.toFixed(2)}×</span>
          </span>
          <input
            type="range" min={0.85} max={1.15} step={0.01}
            value={draft.base_pitch}
            onChange={(e) => setDraft({ ...draft, base_pitch: Number(e.target.value) })}
          />
        </label>
      </div>
      <label className="grid gap-1">
        <span className="text-2xs text-mut uppercase tracking-wide">Cadence notes</span>
        <input
          className={inputClass}
          value={draft.cadence_notes}
          onChange={(e) => setDraft({ ...draft, cadence_notes: e.target.value })}
          placeholder="Clipped consonants, breathy starts…"
        />
      </label>
    </div>
  );
}

function PronunciationEditor() {
  const characters = useVoiceCastStore((s) => s.characters);
  const upsertCharacter = useVoiceCastStore((s) => s.upsertCharacter);
  const lore = useLoreStore((s) => s.lore);
  const upsertLore = useLoreStore((s) => s.upsertLore);
  const [check, setCheck] = useState<PronounceEntry[] | null>(null);
  const [checking, setChecking] = useState(false);

  const runCheck = async () => {
    const names = [
      ...characters.map((c) => c.display_name),
      ...lore.map((l) => l.key),
    ].filter(Boolean);
    if (names.length === 0) return;
    setChecking(true);
    try {
      const res = await audioPronounce({ names });
      setCheck(res.entries);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      toast('error', 'Pronunciation check failed', msg);
    } finally {
      setChecking(false);
    }
  };

  return (
    <div className="px-6 grid gap-2" data-testid="pronunciation-dictionary">
      <div className="flex items-center gap-2 flex-wrap">
        <p className="text-2xs text-mut flex-1 min-w-[24ch]">
          Proper-noun IPA substitution (§6.5 G2P): any token with a non-empty phonetic is
          injected verbatim before synthesis.
        </p>
        <button
          type="button"
          onClick={() => void runCheck()}
          disabled={checking}
          className="text-2xs px-2 py-1 rounded border border-line text-mut hover:text-ink"
        >
          {checking ? 'Checking…' : 'Verify names through the G2P engine'}
        </button>
      </div>
      {characters.filter((c) => c.role !== 'NARRATOR').map((c) => (
        <div key={c.character_key} className="flex items-center gap-2">
          <span className="text-xs w-28 truncate" title={c.display_name}>{c.display_name}</span>
          <input
            className={cn(inputClass, 'font-mono text-xs')}
            defaultValue={c.ipa_phonetic}
            placeholder="ˈkɪərə"
            onBlur={(e) => {
              if (e.target.value !== c.ipa_phonetic) {
                void upsertCharacter({ character_key: c.character_key, display_name: c.display_name, ipa_phonetic: e.target.value });
                toast('success', 'Pronunciation saved', `${c.display_name}: ${e.target.value}`);
              }
            }}
          />
        </div>
      ))}
      {lore.map((l) => (
        <div key={l.key} className="flex items-center gap-2">
          <span className="text-xs w-28 truncate" title={l.key}>{l.key}</span>
          <input
            className={cn(inputClass, 'font-mono text-xs')}
            defaultValue={l.ipa_phonetic}
            placeholder="IPA"
            onBlur={(e) => {
              if (e.target.value !== l.ipa_phonetic) {
                void upsertLore({ key: l.key, content: l.content, category: l.category, ipa_phonetic: e.target.value });
              }
            }}
          />
        </div>
      ))}
      {characters.length === 0 && lore.length === 0 && (
        <p className="text-2xs text-mut">No cast or lore entries yet.</p>
      )}
      {check && check.length > 0 && (
        <div className="grid gap-0.5 border border-line rounded-lg bg-panel px-3 py-2" data-testid="pronounce-check">
          {check.map((e) => (
            <div key={e.name} className="flex items-center gap-2 text-2xs font-mono">
              <span className="text-ink w-32 truncate">{e.name}</span>
              <span className="text-accent">/{e.ipa}/</span>
              <span className="text-mut ml-auto">{e.source}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function RunHistory() {
  const history = useAudioRunStore((s) => s.history);
  const clearHistory = useAudioRunStore((s) => s.clearHistory);

  const verdict = (r: AudioRunRecord) => {
    if (r.status === 'done') {
      return r.acx?.pass
        ? <span className="text-2xs px-1.5 py-0.5 rounded border border-emerald-500/40 bg-emerald-500/10 text-ok">ACX PASS</span>
        : <span className="text-2xs px-1.5 py-0.5 rounded border border-amber-500/40 bg-amber-500/10 text-warn">ACX CHECK</span>;
    }
    if (r.status === 'failed') {
      return <span className="text-2xs px-1.5 py-0.5 rounded border border-rose-500/40 bg-rose-500/10 text-bad">FAILED</span>;
    }
    if (r.status === 'canceled') {
      return <span className="text-2xs px-1.5 py-0.5 rounded border border-amber-500/40 bg-amber-500/10 text-warn">CANCELED</span>;
    }
    return null;
  };

  return (
    <div className="px-6 grid gap-1.5">
      {history.length === 0 ? (
        <p className="text-2xs text-mut">
          No runs yet — the last 12 produce runs land here with their ACX verdict and artifacts.
        </p>
      ) : (
        <>
          {history.map((r) => (
            <div key={r.id} className="border border-line rounded-lg bg-panel2 px-3 py-2 grid gap-1" data-testid="run-history-row">
              <div className="flex items-center gap-2 flex-wrap text-2xs">
                <span className="font-mono text-mut">
                  {new Date(r.finishedAt ?? r.startedAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
                </span>
                <span className="text-ink font-medium">{r.chapterLabel}</span>
                <span className="font-mono text-mut">{r.engine}</span>
                <span className="text-mut">{r.segments} segments</span>
                {verdict(r)}
                {r.acx && (
                  <span className="font-mono text-mut/80">
                    RMS {r.acx.rms_db.toFixed(1)} · Peak {r.acx.peak_db.toFixed(1)} · Noise {r.acx.noise_floor_db.toFixed(1)}
                  </span>
                )}
              </div>
              <div className="flex items-center gap-2 flex-wrap text-2xs font-mono text-mut/80">
                {[r.artifacts.masterWav, r.artifacts.vtt, r.artifacts.lrc, r.artifacts.m4b]
                  .filter(Boolean)
                  .map((a) => <span key={a}>✓ {a}</span>)}
                {r.status === 'failed' && <span className="text-bad/80">{r.error?.title}</span>}
              </div>
            </div>
          ))}
          <div>
            <button type="button" onClick={clearHistory} className="text-2xs text-mut hover:text-ink underline decoration-dotted">
              Clear history
            </button>
          </div>
        </>
      )}
    </div>
  );
}

export function AudioStudio() {
  const characters = useVoiceCastStore((s) => s.characters);
  const loadCharacters = useVoiceCastStore((s) => s.loadCharacters);
  const loadLore = useLoreStore((s) => s.loadLore);
  const segments = useManuscriptStore((s) => s.segments);
  const selectedSegmentId = useManuscriptStore((s) => s.selectedSegmentId);
  const updateSegment = useManuscriptStore((s) => s.updateSegment);
  const [cache, setCache] = useState<AudioCacheStats | null>(null);

  useEffect(() => {
    void loadCharacters();
    void loadLore();
    audioCacheStats().then(setCache).catch(() => setCache(null));
  }, [loadCharacters, loadLore]);

  const castable = useMemo(() => characters.filter((c) => c.role !== 'NARRATOR'), [characters]);
  const selectedSegment = segments.find((s) => s.segment_id === selectedSegmentId) ?? null;

  const applyEmotion = (id: string, label: string) => {
    if (!selectedSegment) {
      toast('warn', 'No segment selected', 'Click a paragraph in the editor, then apply an emotion here.');
      return;
    }
    updateSegment(selectedSegment.segment_id, { emotion: id });
    toast('info', `Emotion: ${label}`, `Applied to the selected segment — saved with the next autosave.`);
  };

  return (
    <StudioShell
      icon={AudioWaveform}
      title="Audio Studio"
      subtitle="Voice profiling · casting · pronunciation · produce chapter audio"
    >
      <div className="pb-10">
        <SectionTitle icon={AudioWaveform}>Produce run — chapter audiobook</SectionTitle>
        <div className="px-6">
          <ProduceRunCard />
        </div>

        <SectionTitle icon={Mic2}>Voice Cast Rack — profiling</SectionTitle>
        <div className="px-6 grid gap-3">
          {castable.length === 0 ? (
            <EmptyState
              icon={Mic2}
              title="No cast yet"
              hint="Create characters in the Voice Cast Rack binder section to profile their voices."
            />
          ) : (
            castable.map((c) => <VoiceRow key={c.character_key} character={c} />)
          )}
        </div>

        <SectionTitle icon={BookA}>Pronunciation dictionary</SectionTitle>
        <PronunciationEditor />

        <SectionTitle
          icon={Sparkles}
          aside={selectedSegment
            ? <span className="text-2xs text-mut font-mono">applies to selected segment #{selectedSegment.sequence_order}</span>
            : undefined}
        >
          Emotion presets — one-click on the selected segment
        </SectionTitle>
        <div className="px-6 grid grid-cols-5 gap-2">
          {EMOTION_PRESETS.map((e) => (
            <button
              key={e.id}
              type="button"
              onClick={() => applyEmotion(e.id, e.label)}
              title={selectedSegment ? `${e.hint} — apply to the selected segment` : e.hint}
              className={cn(
                'border rounded-lg px-3 py-2.5 text-left transition-colors hover:border-accent/50',
                selectedSegment?.emotion === e.id ? e.chip : 'border-line bg-panel2',
              )}
            >
              <div className="text-xs font-semibold">{e.label}</div>
              <div className="text-2xs text-mut mt-0.5 leading-tight">{e.hint}</div>
            </button>
          ))}
        </div>
        {selectedSegment && (
          <p className="px-6 pt-2 text-2xs text-mut">
            Selected segment emotion: <span className={cn('text-2xs px-1.5 py-0.5 rounded border ml-1', emotionChipClass(selectedSegment.emotion))}>
              {selectedSegment.emotion}
            </span>
          </p>
        )}

        <SectionTitle
          icon={AudioWaveform}
          aside={cache && (
            <span className="text-2xs text-mut font-mono">
              {cache.chunks} chunks · {formatBytes(cache.bytes)} · hit rate {(cache.hit_rate * 100).toFixed(0)}%
            </span>
          )}
        >
          Run history
        </SectionTitle>
        <RunHistory />
      </div>
    </StudioShell>
  );
}
