import { useMemo, useState } from 'react';
import { Loader2 } from 'lucide-react';
import { Modal, Field, inputClass, btnGhost, btnPrimary } from '../Modal';
import { useManuscriptStore } from '../../../stores/manuscriptStore';
import { CHAPTER_STATUSES, SEGMENT_TYPES } from '../../../lib/constants';
import { nextChapterNumber } from '../../../lib/utils';

interface Props {
  open: boolean;
  onClose: () => void;
}

export function NewChapterModal({ open, onClose }: Props) {
  const books = useManuscriptStore((s) => s.books);
  const chapters = useManuscriptStore((s) => s.chapters);
  const activeBookId = useManuscriptStore((s) => s.activeBookId);
  const createChapter = useManuscriptStore((s) => s.createChapter);

  const defaultBookId = activeBookId ?? books[0]?.id ?? null;
  const [bookId, setBookId] = useState<string>('');
  const [chapterNumber, setChapterNumber] = useState<string>('');
  const [title, setTitle] = useState('');
  const [actNumber, setActNumber] = useState('1');
  const [synopsis, setSynopsis] = useState('');
  const [targetWordCount, setTargetWordCount] = useState('2200');
  const [status, setStatus] = useState('PLANNED');
  const [busy, setBusy] = useState(false);

  const effectiveBookId = bookId ? Number(bookId) : defaultBookId;
  const autoNumber = useMemo(
    () => (effectiveBookId ? nextChapterNumber(chapters, effectiveBookId) : 1),
    [chapters, effectiveBookId],
  );

  const submit = async () => {
    if (!effectiveBookId || !title.trim() || busy) return;
    setBusy(true);
    try {
      const row = await createChapter({
        book_id: effectiveBookId,
        chapter_number: Number(chapterNumber) || autoNumber,
        title: title.trim(),
        act_number: Number(actNumber) || 1,
        synopsis: synopsis.trim() || undefined,
        target_word_count: Number(targetWordCount) || undefined,
        status,
      });
      if (row) {
        setTitle('');
        setSynopsis('');
        setChapterNumber('');
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
      title="New chapter"
      subtitle="chapter_create · act structure drives the binder grouping"
      footer={
        <>
          <button type="button" className={btnGhost} onClick={onClose}>
            Cancel
          </button>
          <button
            type="button"
            className={btnPrimary}
            onClick={() => void submit()}
            disabled={busy || !title.trim()}
          >
            {busy && <Loader2 size={13} className="animate-spin" />}
            Create chapter
          </button>
        </>
      }
    >
      <div className="grid gap-3.5">
        <div className="grid grid-cols-2 gap-3">
          <Field label="Book">
            <select
              className={inputClass}
              value={String(effectiveBookId ?? '')}
              onChange={(e) => setBookId(e.target.value)}
            >
              {books.map((b) => (
                <option key={b.id} value={b.id}>
                  {b.title}
                </option>
              ))}
            </select>
          </Field>
          <Field label="Chapter number">
            <input
              className={inputClass}
              type="number"
              min={1}
              value={chapterNumber}
              placeholder={String(autoNumber)}
              onChange={(e) => setChapterNumber(e.target.value)}
            />
          </Field>
        </div>
        <Field label="Title">
          <input
            className={inputClass}
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder="The Ledger of Small Debts"
            autoFocus
          />
        </Field>
        <div className="grid grid-cols-3 gap-3">
          <Field label="Act">
            <select className={inputClass} value={actNumber} onChange={(e) => setActNumber(e.target.value)}>
              <option value="1">Act I</option>
              <option value="2">Act II</option>
              <option value="3">Act III</option>
            </select>
          </Field>
          <Field label="Status">
            <select className={inputClass} value={status} onChange={(e) => setStatus(e.target.value)}>
              {CHAPTER_STATUSES.map((s) => (
                <option key={s} value={s}>
                  {s}
                </option>
              ))}
            </select>
          </Field>
          <Field label="Target words">
            <input
              className={inputClass}
              type="number"
              min={0}
              value={targetWordCount}
              onChange={(e) => setTargetWordCount(e.target.value)}
            />
          </Field>
        </div>
        <Field label="Synopsis">
          <textarea
            className={`${inputClass} min-h-[72px] resize-y`}
            value={synopsis}
            onChange={(e) => setSynopsis(e.target.value)}
            placeholder="What promise does this chapter make, and which Chekhov thread does it load?"
          />
        </Field>
        <p className="text-2xs text-mut leading-relaxed">
          Segments are authored as {SEGMENT_TYPES.join(' / ')} rows in the AST canvas and stored
          with speaker, emotion and prosody per the frozen contract.
        </p>
      </div>
    </Modal>
  );
}
