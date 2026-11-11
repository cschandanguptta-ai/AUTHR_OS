// UndoToast — 10-second undo window for destructive actions whose inverse is
// cheap and complete (D7 §7, R-06). The action re-executes the inverse; where
// an inverse is NOT cheap (chapter delete cascades segments, threads, shadow
// files) the caller must use ConfirmDialog instead — the rule is grep-able:
// every delete/sweep/overwrite path passes a guard component.

import { useEffect, useRef, useState } from 'react';
import { Undo2, X } from 'lucide-react';
import { create } from 'zustand';

interface UndoState {
  visible: boolean;
  message: string;
  /** Re-executes the inverse of the destructive action. */
  onUndo: (() => void) | null;
  show: (message: string, onUndo: () => void) => void;
  dismiss: () => void;
}

export const useUndoStore = create<UndoState>((set) => ({
  visible: false,
  message: '',
  onUndo: null,
  show: (message, onUndo) => set({ visible: true, message, onUndo }),
  dismiss: () => set({ visible: false, message: '', onUndo: null }),
}));

const WINDOW_MS = 10_000;

export function UndoToast() {
  const visible = useUndoStore((s) => s.visible);
  const message = useUndoStore((s) => s.message);
  const onUndo = useUndoStore((s) => s.onUndo);
  const dismiss = useUndoStore((s) => s.dismiss);
  const timerRef = useRef<number | null>(null);
  const [remaining, setRemaining] = useState(10);

  useEffect(() => {
    if (!visible) return;
    setRemaining(Math.round(WINDOW_MS / 1000));
    if (timerRef.current) window.clearInterval(timerRef.current);
    const tickStarted = Date.now();
    timerRef.current = window.setInterval(() => {
      const left = Math.max(0, Math.round((WINDOW_MS - (Date.now() - tickStarted)) / 1000));
      setRemaining(left);
      if (left <= 0) {
        if (timerRef.current) window.clearInterval(timerRef.current);
        timerRef.current = null;
        dismiss();
      }
    }, 250);
    return () => {
      if (timerRef.current) window.clearInterval(timerRef.current);
      timerRef.current = null;
    };
  }, [visible, dismiss]);

  if (!visible) return null;

  const undo = () => {
    if (timerRef.current) window.clearInterval(timerRef.current);
    timerRef.current = null;
    onUndo?.();
    dismiss();
  };

  return (
    <div
      className="fixed bottom-4 left-1/2 -translate-x-1/2 z-[65] anim-toast flex items-center gap-3 rounded-lg border border-line2 bg-panel px-4 py-2.5 shadow-xl"
      role="status"
      aria-live="polite"
      data-testid="undo-toast"
    >
      <span className="text-[13px] text-ink">{message}</span>
      <button
        type="button"
        onClick={undo}
        className="flex items-center gap-1.5 text-2xs font-medium px-2.5 py-1 rounded bg-accent text-on-accent"
        data-testid="undo-button"
      >
        <Undo2 size={11} /> Undo
      </button>
      <span className="font-mono text-2xs text-mut tabular-nums">{remaining}s</span>
      <button
        type="button"
        onClick={dismiss}
        className="text-mut hover:text-ink"
        aria-label="Dismiss"
      >
        <X size={12} />
      </button>
    </div>
  );
}
