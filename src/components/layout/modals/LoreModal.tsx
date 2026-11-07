import { useState } from 'react';
import { Loader2 } from 'lucide-react';
import { Modal, Field, inputClass, btnGhost, btnPrimary } from '../Modal';
import { useLoreStore } from '../../../stores/loreStore';
import { LORE_CATEGORIES } from '../../../lib/constants';
import { slugify } from '../../../lib/utils';

interface Props {
  open: boolean;
  onClose: () => void;
}

export function LoreModal({ open, onClose }: Props) {
  const upsertLore = useLoreStore((s) => s.upsertLore);

  const [keyEdited, setKeyEdited] = useState(false);
  const [key, setKey] = useState('');
  const [category, setCategory] = useState('GENERAL');
  const [content, setContent] = useState('');
  const [ipa, setIpa] = useState('');
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    const effectiveKey = keyEdited ? key.trim() : slugify(key);
    if (!effectiveKey || !content.trim() || busy) return;
    setBusy(true);
    try {
      const row = await upsertLore({
        key: effectiveKey,
        content: content.trim(),
        category,
        ipa_phonetic: ipa.trim() || undefined,
      });
      if (row) {
        setKey('');
        setContent('');
        setIpa('');
        setKeyEdited(false);
        onClose();
      }
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal
      open={open}
      onClose={onClose}
      title="New lore entry"
      subtitle="lore_upsert · entries power consistency checks and pronunciation rules"
      footer={
        <>
          <button type="button" className={btnGhost} onClick={onClose}>
            Cancel
          </button>
          <button
            type="button"
            className={btnPrimary}
            onClick={() => void submit()}
            disabled={busy || !key.trim() || !content.trim()}
          >
            {busy && <Loader2 size={13} className="animate-spin" />}
            Save entry
          </button>
        </>
      }
    >
      <div className="grid gap-3.5">
        <div className="grid grid-cols-2 gap-3">
          <Field label="Key">
            <input
              className={`${inputClass} font-mono`}
              value={key}
              onChange={(e) => {
                setKey(e.target.value);
                setKeyEdited(true);
              }}
              placeholder="the_ninefold_vault"
              autoFocus
            />
          </Field>
          <Field label="Category">
            <select className={inputClass} value={category} onChange={(e) => setCategory(e.target.value)}>
              {LORE_CATEGORIES.map((c) => (
                <option key={c} value={c}>
                  {c.replace('_', ' ')}
                </option>
              ))}
            </select>
          </Field>
        </div>
        <Field label="IPA phonetic">
          <input
            className={`${inputClass} font-mono`}
            value={ipa}
            onChange={(e) => setIpa(e.target.value)}
            placeholder="nəˈvɛrn"
          />
        </Field>
        <Field label="Content">
          <textarea
            className={`${inputClass} min-h-[110px] resize-y`}
            value={content}
            onChange={(e) => setContent(e.target.value)}
            placeholder="What the reader must be able to infer about this place, faction or rule — and what must never contradict it."
          />
        </Field>
      </div>
    </Modal>
  );
}
