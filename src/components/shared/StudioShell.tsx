import type { LucideIcon } from 'lucide-react';
import type { ReactNode } from 'react';

interface StudioShellProps {
  icon: LucideIcon;
  title: string;
  subtitle: string;
  actions?: ReactNode;
  children: ReactNode;
  /**
   * When false the body does not own the scroll (R-17 unified chrome): panels
   * that manage their own geometry (the Cockpit's three flex panes) mount
   * under the same header without getting wrapped in an overflow container.
   */
  scroll?: boolean;
}

/**
 * Consistent chrome for the restored studio workstations (Sprint 8):
 * header + icon, scrollable body, shared rhythm.
 */
export function StudioShell({ icon: Icon, title, subtitle, actions, children, scroll = true }: StudioShellProps) {
  return (
    <div className="flex-1 min-h-0 flex flex-col">
      <header className="flex items-center gap-3 px-5 py-3 border-b border-line bg-panel shrink-0">
        <div className="w-8 h-8 rounded-lg border border-line bg-panel2 flex items-center justify-center text-accent">
          <Icon size={16} strokeWidth={1.7} />
        </div>
        <div className="min-w-0">
          <h1 className="text-sm font-semibold tracking-wide">{title}</h1>
          <p className="text-2xs text-mut truncate">{subtitle}</p>
        </div>
        <div className="ml-auto flex items-center gap-2">{actions}</div>
      </header>
      <div className={scroll ? 'flex-1 min-h-0 overflow-y-auto' : 'flex-1 min-h-0 flex flex-col'}>{children}</div>
    </div>
  );
}
