import type { JSX } from 'react';
/**
 * Audio Studio (Sprint 8) — voice profiling, character casting and the
 * pronunciation dictionary, all persisted through the frozen
 * `character_update` / `lore_update` / `segment_update` commands, plus the
 * 3-layer DAW rack view whose DSP engines arrive in Phase 4.
 */

import { useMemo, useState } from 'react';
import { useVoiceCast, castOrder, VOICE_ENGINES, type VoiceProfile } from '../stores/voiceCastStore';
import { useLore } from '../stores/loreStore';
import { useManuscript } from '../stores/manuscriptStore';
import { useBinder } from '../stores/binderStore';
import { AMBIENT_CUES, SFX_CUES } from '../editor/cues';
import { dawLayerStats, pronunciationRows, MASTERING_DEFAULTS } from './audio';
import { Field } from '../cockpit/InspectorPane';

export function AudioStudio(): JSX.Element {
  const characters = useVoiceCast((s) => s.characters);
  const castVoice = useVoiceCast((s) => s.castVoice);
  const setPronunciation = useVoiceCast((s) => s.setPronunciation);
  const parseConfig = useVoiceCast((s) => s.parseConfig);
  const lore = useLore((s) => s.lore);
  const updateLore = useLore((s) => s.update);
  const segments = useManuscript((s) => s.segments);
  const updateMeta = useManuscript((s) => s.updateMeta);
  const books = useBinder((s) => s.books);

  const stats = useMemo(() => dawLayerStats(segments), [segments]);
  const pron = useMemo(() => pronunciationRows(characters, lore), [characters, lore]);

  return (
    <div className="studio">
      <header className="studio-head">
        <h2>Audio Studio</h2>
        <p className="dim">
          Voice profiling · character casting · pronunciation dictionary. Synthesis engines
          mount in <b>Phase 4</b>; every profile you save here is already production state.
        </p>
      </header>

      <section className="studio-grid">
        <div className="studio-card">
          <h3>Voice Cast Rack</h3>
          {castOrder(characters).map((c) => (
            <CastRow key={c.id} row={c} onSave={castVoice} parseConfig={parseConfig} />
          ))}
          {characters.length === 0 && <div className="empty">No cast yet.</div>}
        </div>

        <div className="studio-card">
          <h3>Pronunciation Dictionary</h3>
          <table className="studio-table" data-testid="pronunciation-table">
            <thead>
              <tr><th>Term</th><th>Source</th><th>IPA</th></tr>
            </thead>
            <tbody>
              {pron.map((r) => (
                <tr key={`${r.source}-${r.id}`} className={r.needsPronunciation ? 'warn-row' : ''}>
                  <td>{r.display}</td>
                  <td className="mono dim">{r.source}</td>
                  <td>
                    <input
                      className="ipa-input"
                      defaultValue={r.ipa ?? ''}
                      placeholder="/ˈ…/"
                      onBlur={async (e) => {
                        const v = e.target.value.trim() || null;
                        if (v !== r.ipa) {
                          if (r.source === 'character') await setPronunciation(r.id, v);
                          else await updateLore(r.id, { ipa_phonetic: v });
                        }
                      }}
                    />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {pron.length === 0 && <div className="empty">No proper nouns yet.</div>}
        </div>

        <div className="studio-card">
          <h3>3-Layer DAW Rack <span className="pill">Phase 4 engines</span></h3>
          <div className="daw-stats" data-testid="daw-stats">
            <div className="daw-layer"><span className="mono">SPEECH</span><b>{stats.speech}</b> segments</div>
            <div className="daw-layer"><span className="mono">SPOT SFX</span><b>{stats.sfx}</b> cues</div>
            <div className="daw-layer"><span className="mono">AMBIENT</span><b>{stats.ambient}</b> beds</div>
          </div>
          <h4>Foley assignment (open chapter)</h4>
          <div className="foley-list">
            {segments.map((seg) => (
              <div key={seg.id} className="foley-row">
                <span className="mono dim">{seg.segment_id.slice(0, 13)}</span>
                <span className="foley-text">{seg.text_content.slice(0, 48)}…</span>
                <select
                  value={seg.ambient_cue ?? ''}
                  onChange={(e) => void updateMeta(seg.id, { ambient_cue: e.target.value || null })}
                  title="Ambient bed"
                >
                  <option value="">∞ —</option>
                  {AMBIENT_CUES.map((c) => <option key={c} value={c}>{c}</option>)}
                </select>
                <select
                  value={seg.sfx_cue ?? ''}
                  onChange={(e) => void updateMeta(seg.id, { sfx_cue: e.target.value || null })}
                  title="Foley one-shot"
                >
                  <option value="">⚡ —</option>
                  {SFX_CUES.map((c) => <option key={c} value={c}>{c}</option>)}
                </select>
              </div>
            ))}
            {segments.length === 0 && <div className="empty">Open a chapter in the Cockpit to assign cues.</div>}
          </div>
        </div>

        <div className="studio-card">
          <h3>Mastering Rack <span className="pill">Phase 4</span></h3>
          <div className="mono rack-grid" data-testid="mastering-rack">
            <div>RMS target: <b>{MASTERING_DEFAULTS.targetRmsDb.toFixed(1)} dB</b> (EBU R128)</div>
            <div>Peak ceiling: <b>{MASTERING_DEFAULTS.peakCeilingDbfs.toFixed(1)} dBFS</b> (ACX)</div>
            <div>Ducking: <b>−{MASTERING_DEFAULTS.duckingDb} dB</b>, {MASTERING_DEFAULTS.duckingAttackMs} ms attack / {MASTERING_DEFAULTS.duckingReleaseMs} ms release</div>
            <div className="dim">Two-pass normalize → lookahead limiter → ACX check</div>
          </div>
          <h4>Bundle audio readiness</h4>
          <div className="mono dim" data-testid="audio-readiness">
            {books.map((b) => (
              <div key={b.id}>{b.title}: {b.word_count.toLocaleString()} words in manuscript</div>
            ))}
          </div>
        </div>
      </section>
    </div>
  );
}

function CastRow({
  row,
  onSave,
  parseConfig,
}: {
  row: ReturnType<typeof useVoiceCast.getState>['characters'][number];
  onSave: (id: number, engine: string, profile: VoiceProfile) => Promise<boolean>;
  parseConfig: (row: { voice_config: string | null }) => VoiceProfile;
}): JSX.Element {
  const [engine, setEngine] = useState(row.voice_engine);
  const [profile, setProfile] = useState<VoiceProfile>(parseConfig(row));
  const [dirty, setDirty] = useState(false);

  return (
    <div className="cast-row" data-testid={`cast-row-${row.character_key}`}>
      <div className="cast-row-head">
        <b>{row.display_name}</b>
        <span className="mono dim">{row.character_key} · {row.role}</span>
      </div>
      <div className="cast-row-grid">
        <Field label="engine">
          <select
            value={engine}
            onChange={(e) => {
              setEngine(e.target.value);
              setDirty(true);
            }}
          >
            {VOICE_ENGINES.map((v) => <option key={v} value={v}>{v}</option>)}
          </select>
        </Field>
        <Field label="voice id">
          <input value={profile.voice ?? ''} onChange={(e) => { setProfile({ ...profile, voice: e.target.value }); setDirty(true); }} />
        </Field>
        <Field label="rate">
          <input value={profile.rate ?? ''} onChange={(e) => { setProfile({ ...profile, rate: e.target.value }); setDirty(true); }} />
        </Field>
        <Field label="pitch">
          <input value={profile.pitch ?? ''} onChange={(e) => { setProfile({ ...profile, pitch: e.target.value }); setDirty(true); }} />
        </Field>
      </div>
      {dirty && (
        <button
          className="btn primary sm"
          onClick={async () => {
            await onSave(row.id, engine, profile);
            setDirty(false);
          }}
        >Save profile</button>
      )}
    </div>
  );
}

// (end of Audio Studio)
