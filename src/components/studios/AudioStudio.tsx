// AudioStudio — Restored Studio 1 (Sprint 8): voice profiling, character
// casting, pronunciation dictionary and the 3-layer DSP rack visualization.
// Synthesis itself arrives with Phase 4 (Lookahead TTS + DSP rack); everything
// editable here is real contract-backed data.

import { useEffect, useMemo, useState } from 'react';
import { AudioWaveform, BookA, Loader2, Mic2, Save, Sliders, Sparkles, Trash2 } from 'lucide-react';
import { StudioShell } from '../shared/StudioShell';
import { EmptyState } from '../shared/EmptyState';
import { AudioStateBadge } from '../shared/AudioStateBadge';
import { useVoiceCastStore } from '../../stores/voiceCastStore';
import { useLoreStore } from '../../stores/loreStore';
import { useManuscriptStore } from '../../stores/manuscriptStore';
import { ROLE_BADGES, VOICE_ENGINES, EMOTION_PRESETS } from '../../lib/constants';
import { cn } from '../../lib/utils';
import { inputClass, btnPrimary } from '../layout/Modal';
import { toast } from '../../stores/uiStore';
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
          {busy && <Loader2 size={13} className="animate-spin text-accent" />}
          {draft.role !== 'NARRATOR' && (
            <button
              type="button"
              title="Remove from cast (blocked while segments reference this speaker)"
              onClick={async () => {
                const ok = await deleteCharacter(draft.character_key);
                if (!ok) toast('warn', 'Cannot delete', 'Segments still reference this speaker.');
              }}
              className="text-mut hover:text-bad transition-colors"
              aria-label={`Delete ${draft.display_name}`}
            >
              <Trash2 size={13} />
            </button>
          )}
        </div>
      </div>
      <div className="grid grid-cols-[1fr_1fr] gap-3">
        <label className="grid gap-1">
          <span className="text-2xs text-mut uppercase tracking-wide">Voice engine</span>
          <select
            className={inputClass}
            value={draft.voice_engine}
            onChange={(e) => setDraft({ ...draft, voice_engine: e.target.value })}
          >
            {VOICE_ENGINES.map((e) => <option key={e} value={e}>{e}</option>)}
          </select>
        </label>
        <label className="grid gap-1">
          <span className="text-2xs text-mut uppercase tracking-wide">Voice ID</span>
          <input
            className={inputClass}
            value={draft.voice_id}
            onChange={(e) => setDraft({ ...draft, voice_id: e.target.value })}
            placeholder="af_bella"
          />
        </label>
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

  return (
    <div className="px-6 grid gap-2" data-testid="pronunciation-dictionary">
      <p className="text-2xs text-mut">
        Proper-noun IPA substitution (§6.5 G2P): any token with a non-empty phonetic is
        injected verbatim before synthesis.
      </p>
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
    </div>
  );
}

export function AudioStudio() {
  const characters = useVoiceCastStore((s) => s.characters);
  const loadCharacters = useVoiceCastStore((s) => s.loadCharacters);
  const loadLore = useLoreStore((s) => s.loadLore);
  const segments = useManuscriptStore((s) => s.segments);
  const [layerState, setLayerState] = useState<Record<string, { muted: boolean; solo: boolean }>>({
    speech: { muted: false, solo: false },
    sfx: { muted: false, solo: false },
    ambient: { muted: false, solo: false },
  });

  useEffect(() => {
    void loadCharacters();
    void loadLore();
  }, [loadCharacters, loadLore]);

  const unrendered = useMemo(() => segments.filter((s) => !s.audio_hash).length, [segments]);
  const castable = characters.filter((c) => c.role !== 'NARRATOR');

  return (
    <StudioShell
      icon={AudioWaveform}
      title="Audio Studio"
      subtitle="Voice profiling · casting · pronunciation · 3-layer DSP rack preview"
    >
      <div className="pb-10">
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

        <SectionTitle icon={Sparkles}>Emotion presets</SectionTitle>
        <div className="px-6 grid grid-cols-5 gap-2">
          {EMOTION_PRESETS.map((e) => (
            <div key={e.id} className="border border-line rounded-lg bg-panel2 px-3 py-2.5" title={e.hint}>
              <div className="text-xs font-semibold">{e.label}</div>
              <div className="text-2xs text-mut mt-0.5 leading-tight">{e.hint}</div>
            </div>
          ))}
        </div>

        <SectionTitle
          icon={Sliders}
          aside={<span className="text-2xs text-mut font-mono">sidechain −18 dB · attack 120 ms · release 750 ms</span>}
        >
          3-layer mastering rack (visual preview — DSP ships in Phase 4)
        </SectionTitle>
        <div className="px-6 grid gap-2">
          {[
            { id: 'speech', label: 'Layer 1 — Multi-Voice Speech', engine: 'per-character voice profiles' },
            { id: 'sfx', label: 'Layer 2 — Spot-SFX Foley', engine: '40-cue bundled Foley library' },
            { id: 'ambient', label: 'Layer 3 — Ambient Bed', engine: 'ducked under speech · procedural loops' },
          ].map((layer) => {
            const st = layerState[layer.id];
            return (
              <div key={layer.id} className="flex items-center gap-3 border border-line rounded-xl bg-panel2 px-4 py-3">
                <AudioWaveform size={15} className="text-accent shrink-0" />
                <div className="min-w-0 flex-1">
                  <div className="text-xs font-semibold">{layer.label}</div>
                  <div className="text-2xs text-mut">{layer.engine}</div>
                </div>
                <button
                  type="button"
                  onClick={() => setLayerState((s) => ({ ...s, [layer.id]: { ...st, muted: !st.muted } }))}
                  className={cn(
                    'text-2xs px-2 py-1 rounded border font-mono',
                    st.muted ? 'border-red-500/40 bg-red-500/10 text-bad' : 'border-line text-mut hover:text-ink',
                  )}
                >
                  {st.muted ? 'MUTED' : 'LIVE'}
                </button>
                <button
                  type="button"
                  onClick={() => setLayerState((s) => ({ ...s, [layer.id]: { ...st, solo: !st.solo } }))}
                  className={cn(
                    'text-2xs px-2 py-1 rounded border font-mono',
                    st.solo ? 'border-amber-500/40 bg-amber-500/10 text-warn' : 'border-line text-mut hover:text-ink',
                  )}
                >
                  {st.solo ? 'SOLO' : 'SOLO'}
                </button>
              </div>
            );
          })}
        </div>

        <SectionTitle
          icon={AudioWaveform}
          aside={<span className="text-2xs text-mut font-mono">{unrendered} unrendered in open chapter</span>}
        >
          Render queue (open chapter)
        </SectionTitle>
        <div className="px-6 grid gap-1">
          {segments.length === 0 ? (
            <p className="text-2xs text-mut">Open a chapter in the cockpit to queue its segments.</p>
          ) : (
            segments.map((s) => (
              <div key={s.segment_id} className="flex items-center gap-3 border-b border-line/60 py-1.5">
                <span className="font-mono text-2xs text-mut w-8">#{s.sequence_order}</span>
                <span className="text-xs truncate flex-1" title={s.text_content}>{s.text_content}</span>
                <span className="text-2xs text-mut">{s.emotion}</span>
                <AudioStateBadge segment={{ audio_hash: s.audio_hash, audio_duration_ms: s.audio_duration_ms }} />
              </div>
            ))
          )}
        </div>
      </div>
    </StudioShell>
  );
}
