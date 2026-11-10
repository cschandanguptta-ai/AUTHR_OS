// AUTHR_OS — guard for destructive actions (D7 §7, R-06).
//
// Every delete / sweep / overwrite path must pass through this dialog (or the
// UndoToast). Danger-styled Modal with a typed confirmation for the highest
// stakes action (clean_sweep).

import { useState } from 'react';
import { TriangleAlert } from 'lucide-react';
import { Modal, btnDanger, btnGhost, inputClass } from '../layout/Modal';
import { cn } from '../../lib/utils';

interface ConfirmDialogProps {
  open: boolean;
  title: string;
  body: string;
  confirmLabel?: string;
  /** When set, the user must type this phrase (case-insensitive) to enable the button. */
  typeToConfirm?: string;
  onConfirm: () => void;
  onClose: () => void;
}

export function ConfirmDialog({
  open,
  title,
  body,
  confirmLabel = 'Confirm',
  typeToConfirm,
  onConfirm,
  onClose,
}: ConfirmDialogProps) {
  const [typed, setTyped] = useState('');
  const armed = !typeToConfirm || typed.trim().toLowerCase() === typeToConfirm.toLowerCase();

  return (
    <Modal open={open} title={title} onClose={onClose} width="max-w-md">
      <div className="grid gap-3">
        <div className="flex items-start gap-2.5">
          <span className="w-8 h-8 rounded-lg border border-rose-500/40 bg-rose-500/10 flex items-center justify-center text-bad shrink-0">
            <TriangleAlert size={15} />
          </span>
          <p className="text-[13px] leading-relaxed text-ink">{body}</p>
        </div>
        {typeToConfirm && (
          <label className="grid gap-1">
            <span className="text-2xs text-mut uppercase tracking-wide">
              Type <span className="font-mono text-warn">{typeToConfirm}</span> to confirm
            </span>
            <input
              className={cn(inputClass, 'font-mono')}
              value={typed}
              onChange={(e) => setTyped(e.target.value)}
              autoComplete="off"
              spellCheck={false}
              autoFocus
            />
          </label>
        )}
      </div>
      <footer className="mt-5 flex items-center justify-end gap-2">
        <button type="button" className={btnGhost} onClick={onClose}>
          Cancel
        </button>
        <button
          type="button"
          className={btnDanger}
          disabled={!armed}
          onClick={() => {
            setTyped('');
            onConfirm();
          }}
        >
          {confirmLabel}
        </button>
      </footer>
    </Modal>
  );
}
