import { useRef } from 'react';
import { X } from 'lucide-react';
import type { ReactNode } from 'react';
import { useFocusTrap } from '../../hooks/useFocusTrap';

interface ModalProps {
  open: boolean;
  title: string;
  subtitle?: string;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
  width?: string;
}

/** Professional, minimal modal — no component library. */
export function Modal({ open, title, subtitle, onClose, children, footer, width }: ModalProps) {
  const panelRef = useRef<HTMLDivElement>(null);

  // Focus discipline: trap Tab inside, focus first control on open, restore
  // focus to the opener on close (useFocusTrap also handles Escape).
  useFocusTrap(panelRef, open, onClose);

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 anim-fade"
      role="dialog"
      aria-modal="true"
      aria-label={title}
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        ref={panelRef}
        tabIndex={-1}
        className={`anim-modal w-full ${width ?? 'max-w-lg'} max-h-[86vh] flex flex-col rounded-xl border border-line2 bg-panel shadow-2xl`}
      >
        <header className="flex items-start gap-3 px-5 py-4 border-b border-line">
          <div className="min-w-0 flex-1">
            <h2 className="text-sm font-semibold">{title}</h2>
            {subtitle ? <p className="text-2xs text-mut mt-0.5">{subtitle}</p> : null}
          </div>
          <button
            type="button"
            onClick={onClose}
            className="w-7 h-7 rounded-md border border-line bg-panel2 hover:bg-panel3 flex items-center justify-center text-mut hover:text-ink transition-colors"
            aria-label="Close dialog"
          >
            <X size={13} />
          </button>
        </header>
        <div className="flex-1 min-h-0 overflow-y-auto px-5 py-4">{children}</div>
        {footer ? (
          <footer className="flex items-center justify-end gap-2 px-5 py-3 border-t border-line bg-panel2/50 rounded-b-xl">
            {footer}
          </footer>
        ) : null}
      </div>
    </div>
  );
}

export function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="block">
      <span className="block text-2xs font-medium text-mut mb-1 tracking-wide uppercase">
        {label}
      </span>
      {children}
    </label>
  );
}

export const inputClass =
  'w-full rounded-md border border-line bg-panel2 px-2.5 py-1.5 text-[13px] text-ink placeholder:text-mut/60 focus:border-accent/50';

export const btnGhost =
  'inline-flex items-center gap-1.5 px-3 py-1.5 rounded-md text-[13px] border border-line bg-panel2 text-ink hover:bg-panel3 hover:border-line2 transition-colors disabled:opacity-50';

export const btnPrimary =
  'inline-flex items-center gap-1.5 px-3 py-1.5 rounded-md text-[13px] font-medium bg-accent text-on-accent hover:brightness-110 transition-all disabled:opacity-50';

export const btnDanger =
  'inline-flex items-center gap-1.5 px-3 py-1.5 rounded-md text-[13px] border border-rose-500/40 bg-rose-500/10 text-bad hover:bg-rose-500/20 transition-colors disabled:opacity-50';
