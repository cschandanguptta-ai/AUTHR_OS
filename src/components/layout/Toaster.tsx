import { CheckCircle2, Info, TriangleAlert, XCircle, X } from 'lucide-react';
import { useUiStore, type ToastKind } from '../../stores/uiStore';
import { cn } from '../../lib/utils';

const KIND_ICON: Record<ToastKind, typeof Info> = {
  info: Info,
  success: CheckCircle2,
  error: XCircle,
  warn: TriangleAlert,
};

const KIND_COLOR: Record<ToastKind, string> = {
  info: 'text-accent2',
  success: 'text-emerald-400',
  error: 'text-rose-400',
  warn: 'text-amber-400',
};

export function Toaster() {
  const toasts = useUiStore((s) => s.toasts);
  const dismiss = useUiStore((s) => s.dismiss);

  return (
    <div
      className="fixed bottom-4 right-4 z-[60] flex flex-col gap-2 w-[340px] max-w-[calc(100vw-2rem)]"
      role="status"
      aria-live="polite"
    >
      {toasts.map((t) => {
        const Icon = KIND_ICON[t.kind];
        return (
          <div
            key={t.id}
            className="flex items-start gap-2.5 rounded-lg border border-line2 bg-panel px-3 py-2.5 shadow-xl"
          >
            <Icon size={15} className={cn('mt-0.5 shrink-0', KIND_COLOR[t.kind])} />
            <div className="min-w-0 flex-1">
              <div className="text-[13px] font-medium leading-snug">{t.title}</div>
              {t.message ? (
                <div className="text-2xs text-mut mt-0.5 break-words leading-relaxed">
                  {t.message}
                </div>
              ) : null}
            </div>
            <button
              type="button"
              onClick={() => dismiss(t.id)}
              className="text-mut hover:text-ink shrink-0"
              aria-label="Dismiss notification"
            >
              <X size={12} />
            </button>
          </div>
        );
      })}
    </div>
  );
}
