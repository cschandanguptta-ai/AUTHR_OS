import { useState } from 'react';
import { Loader2 } from 'lucide-react';
import { Modal, Field, inputClass, btnGhost, btnPrimary } from '../Modal';
import { useThreadStore } from '../../../stores/threadStore';
import { useManuscriptStore } from '../../../stores/manuscriptStore';
import { THREAD_TYPE_OPTIONS } from '../../../lib/constants';

interface Props {
  open: boolean;
  onClose: () => void;
}

export function ThreadModal({ open, onClose }: Props) {
  const openThread = useThreadStore((s) => s.openThread);
  const chapters = useManuscriptStore((s) => s.chapters);
  const activeChapter = useManuscriptStore((s) => s.activeChapterId);

  const [threadTitle, setThreadTitle] = useState('');
  const [threadType, setThreadType] = useState('CHEKHOV_GUN');
  const [introChapter, setIntroChapter] = useState<string>(
    activeChapter ? String(activeChapter) : '',
  );
  const [payoff, setPayoff] = useState('');
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    if (!threadTitle.trim() || !introChapter || !payoff.trim() || busy) return;
    setBusy(true);
    try {
      const row = await openThread({
        thread_title: threadTitle.trim(),
        thread_type: threadType,
        introduced_in_chapter: Number(introChapter),
        payoff_requirement: payoff.trim(),
      });
      if (row) {
        setThreadTitle('');
        setPayoff('');
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
      title="New thread"
      subtitle="thread_open · the ledger tracks every promise until its payoff"
      footer={
        <>
          <button type="button" className={btnGhost} onClick={onClose}>
            Cancel
          </button>
          <button
            type="button"
            className={btnPrimary}
            onClick={() => void submit()}
            disabled={busy || !threadTitle.trim() || !introChapter || !payoff.trim()}
          >
            {busy && <Loader2 size={13} className="animate-spin" />}
            Introduce thread
          </button>
        </>
      }
    >
      <div className="grid gap-3.5">
        <Field label="Thread title">
          <input
            className={inputClass}
            value={threadTitle}
            onChange={(e) => setThreadTitle(e.target.value)}
            placeholder="The unanswered ledger page"
            autoFocus
          />
        </Field>
        <div className="grid grid-cols-2 gap-3">
          <Field label="Type">
            <select className={inputClass} value={threadType} onChange={(e) => setThreadType(e.target.value)}>
              {THREAD_TYPE_OPTIONS.map((t) => (
                <option key={t} value={t}>
                  {t.replace('_', ' ')}
                </option>
              ))}
            </select>
          </Field>
          <Field label="Introduced in chapter">
            <select
              className={inputClass}
              value={introChapter}
              onChange={(e) => setIntroChapter(e.target.value)}
            >
              <option value="">Select chapter…</option>
              {chapters.map((c) => (
                <option key={c.id} value={c.id}>
                  Ch {String(c.chapter_number).padStart(2, '0')} — {c.title}
                </option>
              ))}
            </select>
          </Field>
        </div>
        <Field label="Payoff requirement">
          <textarea
            className={`${inputClass} min-h-[90px] resize-y`}
            value={payoff}
            onChange={(e) => setPayoff(e.target.value)}
            placeholder="What must visibly happen before this thread can be marked RESOLVED — the scene, revelation or reversal the reader is owed."
          />
        </Field>
      </div>
    </Modal>
  );
}
