import type { JSX } from 'react';
/**
 * The Dynamic Inspector — right pane of the Author's Cockpit (Sprint 6).
 *
 * Context-aware tab set:
 *   Segment  — speaker / type / emotion / prosody / Foley cue editor for
 *              the segment under the caret (writes segment_update)
 *   Cast     — voice cast rack member (engine + config + IPA)
 *   Thread   — Chekhov ledger entry (status moves)
 *   Envelope — raw envelope of the last invocation (Phase 2 parity)
 *   Events   — live shell event log
 */

import { useEffect, useRef, useState } from 'react';
import type { AppEvent, Envelope } from '../api';
import { useUi } from '../stores/uiStore';
import { useManuscript, speakerLabel } from '../stores/manuscriptStore';
import { useVoiceCast, castOrder, VOICE_ENGINES, type VoiceProfile } from '../stores/voiceCastStore';
import { useLedger, THREAD_STATUSES, type ThreadStatus } from '../stores/ledgerStore';
import { EMOTION_TAGS, AMBIENT_CUES, SFX_CUES } from '../editor/cues';
import { getTransport } from '../transport';
import type { SegmentRow } from '../types';

type Tab = 'segment' | 'cast' | 'thread' | 'envelope' | 'events';

export function InspectorPane(): JSX.Element {
  const selection = useUi((s) => s.selection);
  const events = useUi((s) => s.events);
  const lastEnvelope = useUi((s) => s.lastEnvelope);
  const segments = useManuscript((s) => s.segments);
  const characters = useVoiceCast((s) => s.characters);
  const threads = useLedger((s) => s.threads);

  const selectedSegment = segments.find((s) => s.segment_id === selection.selectedSegmentId) ?? null;
  const selectedCast = characters.find((c) => c.id === selection.selectedCastId) ?? null;
  const selectedThread = threads.find((t) => t.id === selection.selectedThreadId) ?? null;

  const [tab, setTab] = useState<Tab>('envelope');
  const [touched, setTouched] = useState(false);

  // The "dynamic" part: follow the most recent selection once.
  useEffect(() => {
    if (touched) return;
    if (selectedSegment) setTab('segment');
    else if (selectedCast) setTab('cast');
    else if (selectedThread) setTab('thread');
  }, [selectedSegment, selectedCast, selectedThread, touched]);

  const pick = (t: Tab): void => {
    setTouched(true);
    setTab(t);
  };

  return (
    <aside className="inspector-pane" aria-label="Dynamic Inspector">
      <div className="inspector-tabs">
        <button className={tab === 'segment' ? 'active' : ''} onClick={() => pick('segment')}>Segment</button>
        <button className={tab === 'cast' ? 'active' : ''} onClick={() => pick('cast')}>Cast</button>
        <button className={tab === 'thread' ? 'active' : ''} onClick={() => pick('thread')}>Ledger</button>
        <button className={tab === 'envelope' ? 'active' : ''} onClick={() => pick('envelope')}>Envelope</button>
        <button className={tab === 'events' ? 'active' : ''} onClick={() => pick('events')}>
          Events <span className="count">{events.length}</span>
        </button>
      </div>
      <div className="inspector-body-pane">
        {tab === 'segment' && <SegmentInspector segment={selectedSegment} />}
        {tab === 'cast' && <CastInspector castId={selection.selectedCastId} />}
        {tab === 'thread' && <ThreadInspector threadId={selection.selectedThreadId} />}
        {tab === 'envelope' && <EnvelopeView envelope={lastEnvelope} />}
        {tab === 'events' && <EventsView events={events} />}
      </div>
    </aside>
  );
}

// ---------------- Segment inspector ----------------

function SegmentInspector({ segment }: { segment: SegmentRow | null }): JSX.Element {
  const characters = useVoiceCast((s) => s.characters);
  const updateMeta = useManuscript((s) => s.updateMeta);
  const [busy, setBusy] = useState(false);

  if (!segment) {
    return <div className="empty">Click a segment in the canvas to inspect its AST metadata.</div>;
  }

  const patch = async (fields: Partial<SegmentRow>): Promise<void> => {
    setBusy(true);
    await updateMeta(segment.id, fields);
    setBusy(false);
  };

  return (
    <div className="seg-inspector" data-testid="seg-inspector">
      <div className="ins-row mono dim">segment_id: {segment.segment_id} · row #{segment.id} · seq {segment.sequence_order}</div>
      <Field label="Speaker">
        <select
          value={segment.speaker_id}
          disabled={busy}
          onChange={(e) => void patch({ speaker_id: e.target.value })}
        >
          <option value="NARRATOR">NARRATOR — Narrator</option>
          {characters.filter((c) => c.character_key !== 'NARRATOR').map((c) => (
            <option key={c.id} value={c.character_key}>
              {c.character_key} — {c.display_name}
            </option>
          ))}
        </select>
      </Field>
      <Field label="Type">
        <select
          value={segment.segment_type}
          disabled={busy}
          onChange={(e) => void patch({ segment_type: e.target.value as SegmentRow['segment_type'] })}
        >
          {(['NARRATION', 'DIALOGUE', 'INTERNAL_MONOLOGUE'] as const).map((t) => (
            <option key={t} value={t}>{t}</option>
          ))}
        </select>
      </Field>
      <Field label="Emotion">
        <select value={segment.emotion} disabled={busy} onChange={(e) => void patch({ emotion: e.target.value })}>
          {[...new Set<string>([segment.emotion, ...EMOTION_TAGS])].map((e) => (
            <option key={e} value={e}>{e}</option>
          ))}
        </select>
      </Field>
      <Field label={`Prosody rate ×${segment.prosody_rate.toFixed(2)}`}>
        <input
          type="range" min={0.8} max={1.25} step={0.01} value={segment.prosody_rate}
          disabled={busy}
          onChange={(e) => void patch({ prosody_rate: Number(e.target.value) })}
        />
      </Field>
      <Field label={`Prosody pitch ×${segment.prosody_pitch.toFixed(2)}`}>
        <input
          type="range" min={0.85} max={1.15} step={0.01} value={segment.prosody_pitch}
          disabled={busy}
          onChange={(e) => void patch({ prosody_pitch: Number(e.target.value) })}
        />
      </Field>
      <Field label="Ambient bed (∞)">
        <select
          value={segment.ambient_cue ?? ''}
          disabled={busy}
          onChange={(e) => void patch({ ambient_cue: e.target.value || null })}
        >
          <option value="">— none —</option>
          {[...new Set([segment.ambient_cue ?? '', ...AMBIENT_CUES])].filter(Boolean).map((c) => (
            <option key={c} value={c}>{c}</option>
          ))}
        </select>
      </Field>
      <Field label="Foley one-shot (⚡)">
        <select
          value={segment.sfx_cue ?? ''}
          disabled={busy}
          onChange={(e) => void patch({ sfx_cue: e.target.value || null })}
        >
          <option value="">— none —</option>
          {[...new Set([segment.sfx_cue ?? '', ...SFX_CUES])].filter(Boolean).map((c) => (
            <option key={c} value={c}>{c}</option>
          ))}
        </select>
      </Field>
      <div className="ins-row mono dim">
        audio: {segment.audio_hash ? 'rendered' : 'unrendered ▶'}
      </div>
    </div>
  );
}

// ---------------- Cast inspector ----------------

function CastInspector({ castId }: { castId: number | null }): JSX.Element {
  const characters = useVoiceCast((s) => s.characters);
  const castVoice = useVoiceCast((s) => s.castVoice);
  const setPronunciation = useVoiceCast((s) => s.setPronunciation);
  const parseConfig = useVoiceCast((s) => s.parseConfig);
  const member = characters.find((c) => c.id === castId) ?? null;
  const [profile, setProfile] = useState<VoiceProfile>({});
  const [ipa, setIpa] = useState('');
  const [engine, setEngine] = useState<string>('kokoro');
  const [saved, setSaved] = useState<string | null>(null);

  useEffect(() => {
    if (member) {
      setProfile(parseConfig(member));
      setIpa(member.ipa_phonetic ?? '');
      setEngine(member.voice_engine);
      setSaved(null);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [member?.id]);

  if (!member) {
    return <div className="empty">Select a voice in the Cast Rack to profile it.</div>;
  }

  return (
    <div className="cast-inspector">
      <div className="ins-row strong">{member.display_name} <span className="mono dim">{member.character_key}</span></div>
      <Field label="Voice engine">
        <select value={engine} onChange={(e) => setEngine(e.target.value)}>
          {VOICE_ENGINES.map((v) => <option key={v} value={v}>{v}</option>)}
        </select>
      </Field>
      <Field label="Voice id"><input value={profile.voice ?? ''} onChange={(e) => setProfile({ ...profile, voice: e.target.value })} /></Field>
      <Field label="Rate (e.g. -10%)"><input value={profile.rate ?? ''} onChange={(e) => setProfile({ ...profile, rate: e.target.value })} /></Field>
      <Field label="Pitch (e.g. +2st)"><input value={profile.pitch ?? ''} onChange={(e) => setProfile({ ...profile, pitch: e.target.value })} /></Field>
      <Field label="IPA pronunciation"><input value={ipa} placeholder="/ˈmɪərə/" onChange={(e) => setIpa(e.target.value)} /></Field>
      <button
        className="btn primary"
        onClick={async () => {
          await castVoice(member.id, engine, profile);
          await setPronunciation(member.id, ipa.trim() === '' ? null : ipa.trim());
          setSaved('cast saved');
        }}
      >Save casting</button>
      {saved && <span className="pill ok">{saved}</span>}
    </div>
  );
}

// ---------------- Thread inspector ----------------

function ThreadInspector({ threadId }: { threadId: number | null }): JSX.Element {
  const threads = useLedger((s) => s.threads);
  const move = useLedger((s) => s.move);
  const thread = threads.find((t) => t.id === threadId) ?? null;
  if (!thread) return <div className="empty">Select a Chekhov thread to manage its payoff.</div>;
  return (
    <div className="thread-inspector">
      <div className="ins-row strong">{thread.title}</div>
      <div className="ins-row mono dim">planted ch{thread.planted_chapter_index ?? '—'} → payoff ch{thread.payoff_chapter_index ?? '—'}</div>
      {thread.notes && <div className="ins-row">{thread.notes}</div>}
      <Field label="Status">
        <select value={thread.status} onChange={(e) => void move(thread.id, e.target.value as ThreadStatus)}>
          {THREAD_STATUSES.map((s) => <option key={s} value={s}>{s}</option>)}
        </select>
      </Field>
    </div>
  );
}

// ---------------- envelope + events (Phase 2 parity) ----------------

function EnvelopeView({ envelope }: { envelope: { cmd: string; env: Envelope } | null }): JSX.Element {
  if (!envelope) return <div className="empty">Invoke a command to inspect its envelope.</div>;
  return (
    <>
      <div className="cmd-line">
        <span className="mono cmd-chip">{envelope.cmd}</span>
        <span className={`pill ${envelope.env.ok ? 'ok' : 'bad'}`}>{envelope.env.ok ? 'ok' : 'error'}</span>
      </div>
      <pre className="mono envelope-view">{JSON.stringify(envelope.env, null, 2)}</pre>
    </>
  );
}

function EventsView({ events }: { events: AppEvent[] }): JSX.Element {
  const logRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (logRef.current) logRef.current.scrollTop = 0;
  }, [events]);
  return (
    <div className="inspector-body" ref={logRef}>
      {events.length === 0 && <div className="empty">No shell events yet.</div>}
      <ul className="event-log">
        {events.map((e, i) => (
          <li key={i}>
            <span className={`evt ${e.event.replace('authr://', '')}`}>{e.event}</span>
            <span className="mono evt-payload">{JSON.stringify(e.payload)}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}

export function Field({ label, children }: { label: string; children: React.ReactNode }): JSX.Element {
  return (
    <label className="field">
      <span className="field-label">{label}</span>
      {children}
    </label>
  );
}

export { speakerLabel, castOrder, getTransport };
