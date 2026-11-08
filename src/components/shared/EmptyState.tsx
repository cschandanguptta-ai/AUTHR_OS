import type { LucideIcon } from 'lucide-react';
import { cn } from '../../lib/utils';

interface EmptyStateProps {
  icon: LucideIcon;
  title: string;
  hint: string;
  action?: React.ReactNode;
  className?: string;
}

export function EmptyState({ icon: Icon, title, hint, action, className }: EmptyStateProps) {
  return (
    <div
      className={cn(
        'flex flex-col items-center justify-center text-center gap-2 py-10 px-6',
        className,
      )}
    >
      <div className="w-10 h-10 rounded-lg border border-line bg-panel2 flex items-center justify-center text-mut">
        <Icon size={18} strokeWidth={1.6} />
      </div>
      <div className="text-sm font-medium text-ink">{title}</div>
      <div className="text-xs text-mut max-w-[34ch] leading-relaxed">{hint}</div>
      {action ? <div className="mt-2">{action}</div> : null}
    </div>
  );
}
