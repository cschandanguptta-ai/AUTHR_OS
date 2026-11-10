import { useRef, useState } from 'react';
import {
  AudioLines,
  BookOpen,
  Clapperboard,
  Command,
  Eye,
  GitCommitHorizontal,
  Languages,
  Lock,
  LogOut,
  Megaphone,
  MoreHorizontal,
  PenTool,
  Rocket,
  Sparkles,
  Trash2,
} from 'lucide-react';
import { useWorkspaceStore, type StudioId } from '../../stores/workspaceStore';
import { useProjectStore } from '../../stores/projectStore';
import * as api from '../../ipc/api';
import { toast, toastError } from '../../stores/uiStore';
import { cn, IS_MAC, MOD_KEY } from '../../lib/utils';
import { SettingsPopover } from './SettingsPopover';
import { useDismiss } from '../../hooks/useDismiss';
import { useConfirmStore } from '../../stores/confirmStore';

const STUDIOS: Array<{ id: StudioId; label: string; icon: typeof BookOpen }> = [
  { id: 'novel', label: 'Novel', icon: BookOpen },
  { id: 'audio', label: 'Audio', icon: AudioLines },
  { id: 'video', label: 'Video', icon: Clapperboard },
  { id: 'localization', label: 'Localization', icon: Languages },
  { id: 'marketing', label: 'Marketing', icon: Megaphone },
  { id: 'craft', label: 'Craft', icon: PenTool },
  { id: 'publish', label: 'Publish', icon: Rocket },
];

/** Lock dot: green = live lease held by this session; amber = read-only foreign lease. */
function LockDot() {
  const lockInfo = useProjectStore((s) => s.lockInfo);
  const refresh = useProjectStore((s) => s.refreshLockStatus);
  const readonly = useProjectStore((s) => s.readonly);

  const tone = readonly
    ? 'bg-amber-400'
    : lockInfo?.locked
      ? 'bg-emerald-400'
      : 'bg-zinc-500';
  const label = readonly
    ? 'Read-only — fresh foreign lease honored'
    : lockInfo?.locked
      ? `Lease held · pid ${lockInfo.pid ?? '?'} @ ${lockInfo.hostname ?? 'local'}`
      : 'No lease recorded';

  return (
    <button
      type="button"
      onClick={() => void refresh()}
      title={`${label} — click to re-probe project_lock_status`}
      className="flex items-center gap-1.5 text-2xs text-mut hover:text-ink transition-colors"
      aria-label={label}
    >
      <span className={cn('w-2 h-2 rounded-full', tone)} />
      <span className="hidden md:inline">{readonly ? 'READ-ONLY' : 'LEASED'}</span>
    </button>
  );
}

export function WorkstationSwitcher() {
  const activeStudio = useWorkspaceStore((s) => s.activeStudio);
  const setActiveStudio = useWorkspaceStore((s) => s.setActiveStudio);
  const manifest = useProjectStore((s) => s.manifest);
  const readonly = useProjectStore((s) => s.readonly);
  const bundlePath = useProjectStore((s) => s.bundlePath);
  const closeProject = useProjectStore((s) => s.closeProject);
  const setPaletteOpen = useWorkspaceStore((s) => s.setPaletteOpen);
  const [menuOpen, setMenuOpen] = useState(false);
  const kebabRef = useRef<HTMLDivElement>(null);

  // The kebab menu now closes on outside pointerdown and Escape.
  useDismiss(kebabRef, () => setMenuOpen(false), menuOpen);

  const snapshot = async () => {
    try {
      const res = await api.gitSnapshot();
      toast('success', 'Snapshot committed', res.commit.slice(0, 12));
    } catch (err) {
      toastError('git_snapshot failed', err);
    }
    setMenuOpen(false);
  };

  const cleanSweep = async () => {
    setMenuOpen(false);
    useConfirmStore.getState().ask({
      title: 'Run clean sweep?',
      body: 'Purges orphaned segment purgatory rows and threads, then rebuilds the full-text index. Attached chapters keep their text — this only removes orphans. It cannot be undone.',
      confirmLabel: 'Clean sweep',
      typeToConfirm: 'sweep',
      onConfirm: () => {
        void (async () => {
          try {
            const res = await api.cleanSweep();
            toast(
              'success',
              'Clean sweep complete',
              `${res.purged_segments} segments + ${res.purged_threads} threads purged · FTS rebuilt`,
            );
          } catch (err) {
            toastError('clean_sweep failed', err);
          }
        })();
      },
    });
  };

  return (
    <header
      className="flex items-center gap-2 px-3 py-2 border-b border-line bg-panel shrink-0"
      aria-label="Workstation switcher"
    >
      {/* Wordmark */}
      <div className="flex items-baseline gap-1 px-2 select-none">
        <span className="text-[13px] font-semibold tracking-[0.16em]">AUTHR</span>
        <span className="text-[13px] font-mono font-semibold text-accent">_OS</span>
      </div>

      {/* Studio tabs */}
      <nav className="flex items-center ml-2" aria-label="Studios">
        {STUDIOS.map((s, i) => (
          <div key={s.id} className="flex items-center">
            {i > 0 && (
              <span className="text-mut/50 text-2xs px-1 select-none" aria-hidden="true">
                ✦
              </span>
            )}
            <button
              type="button"
              onClick={() => setActiveStudio(s.id)}
              aria-current={activeStudio === s.id ? 'page' : undefined}
              title={`${s.label} studio — ${IS_MAC ? '⌘⌥' : 'Ctrl+Alt+'}${STUDIOS.indexOf(s) + 1}`}
              className={cn(
                'flex items-center gap-1.5 px-2.5 py-1.5 rounded-md text-[13px] transition-colors relative',
                activeStudio === s.id
                  ? 'bg-panel3 text-ink font-medium'
                  : 'text-mut hover:text-ink hover:bg-panel2',
              )}
            >
              <s.icon size={14} strokeWidth={1.8} className={activeStudio === s.id ? 'text-accent' : ''} />
              <span className="hidden lg:inline">{s.label}</span>
              {activeStudio === s.id && (
                <span
                  aria-hidden="true"
                  className="absolute -bottom-[7px] left-2.5 right-2.5 h-[2px] rounded-full bg-accent/80"
                />
              )}
            </button>
          </div>
        ))}
      </nav>

      {/* Right cluster */}
      <div className="ml-auto flex items-center gap-2.5">
        {readonly && (
          <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded border border-amber-500/40 bg-amber-500/10 text-warn text-2xs font-medium">
            <Eye size={11} /> READ-ONLY
          </span>
        )}

        {/* Command palette trigger */}
        <button
          type="button"
          onClick={() => setPaletteOpen(true)}
          className="flex items-center gap-1.5 px-2 py-1 rounded-md border border-line bg-panel2 hover:bg-panel3 text-mut hover:text-ink text-2xs transition-colors"
          aria-label="Open command palette"
          title="Command palette — search every action"
        >
          <Command size={12} />
          <kbd className="font-mono">{MOD_KEY}K</kbd>
        </button>

        <SettingsPopover />

        <span className="text-[13px] text-ink font-medium truncate max-w-[220px]">
          {manifest?.title ?? 'Untitled bundle'}
        </span>
        <LockDot />
        {bundlePath && (
          <span className="hidden xl:inline font-mono text-2xs text-mut truncate max-w-[220px]">
            {bundlePath}
          </span>
        )}

        {/* Maintenance kebab */}
        <div className="relative" ref={kebabRef}>
          <button
            type="button"
            onClick={() => setMenuOpen((o) => !o)}
            className="w-7 h-7 rounded-md border border-line bg-panel2 hover:bg-panel3 flex items-center justify-center text-mut hover:text-ink transition-colors"
            aria-label="Project maintenance menu"
            aria-expanded={menuOpen}
          >
            <MoreHorizontal size={14} />
          </button>
          {menuOpen && (
            <div className="anim-pop absolute right-0 top-9 z-40 w-60 rounded-lg border border-line2 bg-panel shadow-2xl py-1.5">
              <button
                type="button"
                onClick={() => void snapshot()}
                className="w-full flex items-center gap-2.5 px-3 py-2 text-[13px] hover:bg-panel2 text-ink text-left"
              >
                <GitCommitHorizontal size={14} className="text-mut" />
                Git snapshot
                <span className="ml-auto text-2xs text-mut font-mono">git_snapshot</span>
              </button>
              <button
                type="button"
                onClick={() => void cleanSweep()}
                className="w-full flex items-center gap-2.5 px-3 py-2 text-[13px] hover:bg-panel2 text-ink text-left"
              >
                <Sparkles size={14} className="text-mut" />
                Clean sweep + FTS rebuild
                <span className="ml-auto text-2xs text-mut font-mono">clean_sweep</span>
              </button>
              <button
                type="button"
                onClick={() => {
                  void useProjectStore.getState().refreshLockStatus();
                  setMenuOpen(false);
                }}
                className="w-full flex items-center gap-2.5 px-3 py-2 text-[13px] hover:bg-panel2 text-ink text-left"
              >
                <Lock size={14} className="text-mut" />
                Re-probe lock status
                <span className="ml-auto text-2xs text-mut font-mono">lock_status</span>
              </button>
              <div className="my-1 border-t border-line" />
              <button
                type="button"
                onClick={() => {
                  setMenuOpen(false);
                  void closeProject();
                }}
                className="w-full flex items-center gap-2.5 px-3 py-2 text-[13px] hover:bg-panel2 text-bad text-left"
              >
                <LogOut size={14} />
                Close project
                <span className="ml-auto text-2xs text-mut font-mono">
                  <Trash2 size={11} className="inline" />
                </span>
              </button>
            </div>
          )}
        </div>
      </div>
    </header>
  );
}
