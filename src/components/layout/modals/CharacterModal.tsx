import { useState } from 'react';
import { Loader2, Trash2 } from 'lucide-react';
import { Modal, Field, inputClass, btnGhost, btnPrimary, btnDanger } from '../Modal';
import { useVoiceCastStore } from '../../../stores/voiceCastStore';
import { CHARACTER_ROLES, VOICE_ENGINES } from '../../../lib/constants';
import { slugify } from '../../../lib/utils';
import type { CharacterRow } from '../../../types/ast';

interface Props {
  open: boolean;
  onClose: () => void;
  /** When provided, the modal edits this profile instead of creating one. */
  edit?: CharacterRow;
}

/** CharacterModal — character_upsert with all profile fields incl. the 5-engine picker. */
export function CharacterModal({ open, onClose, edit }: Props) {
  const upsertCharacter = useVoiceCastStore((s) => s.upsertCharacter);
  const deleteCharacter = useVoiceCastStore((s) => s.deleteCharacter);

  const [keyEdited, setKeyEdited] = useState(false);
  const [characterKey, setCharacterKey] = useState(edit?.character_key ?? '');
  const [displayName, setDisplayName] = useState(edit?.display_name ?? '');
  const [role, setRole] = useState(edit?.role ?? 'PROTAGONIST');
  const [bio, setBio] = useState(edit?.bio ?? '');
  const [ipa, setIpa] = useState(edit?.ipa_phonetic ?? '');
  const [voiceEngine, setVoiceEngine] = useState(edit?.voice_engine ?? 'elevenlabs_v2');
  const [voiceId, setVoiceId] = useState(edit?.voice_id ?? '');
  const [baseRate, setBaseRate] = useState(String(edit?.base_rate ?? 1.0));
  const [basePitch, setBasePitch] = useState(String(edit?.base_pitch ?? 1.0));
  const [cadence, setCadence] = useState(edit?.cadence_notes ?? '');
  const [busy, setBusy] = useState(false);

  const effectiveKey = keyEdited || edit ? characterKey : slugify(displayName);

  const submit = async () => {
    if (!effectiveKey.trim() || !displayName.trim() || busy) return;
    setBusy(true);
    try {
      const row = await upsertCharacter({
        character_key: effectiveKey.trim(),
        display_name: displayName.trim(),
        role,
        bio: bio.trim() || undefined,
        ipa_phonetic: ipa.trim() || undefined,
        voice_engine: voiceEngine,
        voice_id: voiceId.trim() || undefined,
        base_rate: Number(baseRate) || 1.0,
        base_pitch: Number(basePitch) || 1.0,
        cadence_notes: cadence.trim() || undefined,
      });
      if (row) onClose();
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    if (!edit) return;
    const ok = await deleteCharacter(edit.character_key);
    if (ok) onClose();
  };

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={edit ? `Edit ${edit.display_name}` : 'New character'}
      subtitle="character_upsert · voice profiles feed the Phase 5 synthesis pipeline"
      width="max-w-xl"
      footer={
        <>
          {edit && (
            <button type="button" className={`${btnDanger} mr-auto`} onClick={() => void remove()}>
              <Trash2 size={13} /> Remove
            </button>
          )}
          <button type="button" className={btnGhost} onClick={onClose}>
            Cancel
          </button>
          <button
            type="button"
            className={btnPrimary}
            onClick={() => void submit()}
            disabled={busy || !displayName.trim() || !effectiveKey.trim()}
          >
            {busy && <Loader2 size={13} className="animate-spin" />}
            {edit ? 'Save profile' : 'Add to rack'}
          </button>
        </>
      }
    >
      <div className="grid gap-3.5">
        <div className="grid grid-cols-2 gap-3">
          <Field label="Display name">
            <input
              className={inputClass}
              value={displayName}
              onChange={(e) => setDisplayName(e.target.value)}
              placeholder="Sera Vantine"
              autoFocus
            />
          </Field>
          <Field label="Character key (speaker_id)">
            <input
              className={`${inputClass} font-mono`}
              value={effectiveKey}
              onChange={(e) => {
                setKeyEdited(true);
                setCharacterKey(e.target.value);
              }}
              placeholder="sera_vantine"
            />
          </Field>
        </div>
        <div className="grid grid-cols-2 gap-3">
          <Field label="Role">
            <select className={inputClass} value={role} onChange={(e) => setRole(e.target.value)}>
              {CHARACTER_ROLES.map((r) => (
                <option key={r} value={r}>
                  {r}
                </option>
              ))}
            </select>
          </Field>
          <Field label="Voice engine">
            <select
              className={inputClass}
              value={voiceEngine}
              onChange={(e) => setVoiceEngine(e.target.value)}
            >
              {VOICE_ENGINES.map((v) => (
                <option key={v} value={v}>
                  {v}
                </option>
              ))}
            </select>
          </Field>
        </div>
        <div className="grid grid-cols-2 gap-3">
          <Field label="Voice id (engine-side)">
            <input
              className={`${inputClass} font-mono`}
              value={voiceId}
              onChange={(e) => setVoiceId(e.target.value)}
              placeholder="EXAVITQu4vr4xnSDjMa5"
            />
          </Field>
          <Field label="IPA phonetic">
            <input
              className={`${inputClass} font-mono`}
              value={ipa}
              onChange={(e) => setIpa(e.target.value)}
              placeholder="ˈsɛrə ˈvæntaɪn"
            />
          </Field>
        </div>
        <div className="grid grid-cols-2 gap-3">
          <Field label={`Base rate × ${Number(baseRate) || 1.0}`}>
            <input
              type="range"
              className="w-full mt-2"
              min={0.8}
              max={1.25}
              step={0.01}
              value={baseRate}
              onChange={(e) => setBaseRate(e.target.value)}
            />
          </Field>
          <Field label={`Base pitch × ${Number(basePitch) || 1.0}`}>
            <input
              type="range"
              className="w-full mt-2"
              min={0.85}
              max={1.15}
              step={0.01}
              value={basePitch}
              onChange={(e) => setBasePitch(e.target.value)}
            />
          </Field>
        </div>
        <Field label="Bio">
          <textarea
            className={`${inputClass} min-h-[60px] resize-y`}
            value={bio}
            onChange={(e) => setBio(e.target.value)}
            placeholder="Cartographer of dead cities; speaks like she is rationing air."
          />
        </Field>
        <Field label="Cadence notes">
          <textarea
            className={`${inputClass} min-h-[52px] resize-y`}
            value={cadence}
            onChange={(e) => setCadence(e.target.value)}
            placeholder="Clipped clauses; drops final consonants when lying."
          />
        </Field>
      </div>
    </Modal>
  );
}
