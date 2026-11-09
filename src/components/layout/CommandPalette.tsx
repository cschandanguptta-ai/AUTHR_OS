// AUTHR_OS — Command palette (Ctrl/Cmd+K): every cockpit action reachable
// from one fuzzy-filterable surface. Zero dependencies — a flat command list,
// a substring filter, group headers, and arrow-key navigation.

import { useEffect, useMemo, useRef, useState } from 'react';
import {
  AudioLines,
  BookOpen,
  Clapperboard,
  Columns3,
  Eye,
  FileText,
  GitCommitHorizontal,
  Languages,
  Layers,
  Lock,
  LogOut,
  Megaphone,
  Moon,
  PenTool,
  Radio,
  Sparkles,
  Sunrise,
  type LucideIcon,
} from 'lucide-react';
import { useWorkspaceStore, type StudioId, type ViewMode } from '../../stores/workspaceStore';
import { useSettingsStore } from '../../stores/settingsStore';
import { useProjectStore } from '../../stores/projectStore';
import * as api from '../../ipc/api';
import { toast, toastError } from '../../stores/uiStore';
import { useFocusTrap } from '../../hooks/useFocusTrap';
import { IS_MAC, MOD_ALT_KEY, MOD_KEY, cn } from '../../lib/utils';

interface Command {
  id: string;
  group: 'Navigate' | 'Canvas' | 'Project' | 'Theme';
  label: string;
  hint?: string;
  keys?: string;
  icon: LucideIcon;
  danger?: boolean;
  run: () => void;
}

function buildCommands(): Command[] {
  const ws = useWorkspaceStore.getState();
  const settings = useSettingsStore.getState();
  const project = useProjectStore.getState();
  const close = useWorkspaceStore.getState().setPaletteOpen.bind(null, false);

  const studio = (id: StudioId, label: string, icon: LucideIcon, keys: string, hint: string): Command => ({
    id: `studio-${id}`,
    group: 'Navigate',
    label,
    hint,
    keys,
    icon,
    run: () => {
      ws.setActiveStudio(id);
      close();
    },
  });

  const viewMode = (mode: ViewMode, label: string, icon: LucideIcon, hint: string): Command => ({
    id: `view-${mode}`,
    group: 'Canvas',
    label,
    hint,
    icon,
    run: () => {
      ws.setViewMode(mode);
      close();
    },
  });

  return [
    studio('novel', 'Novel', BookOpen, `${MOD_ALT}1`, 'Manuscript AST canvas, binder, inspector'),
    studio('audio', 'Audio', AudioLines, `${MOD_ALT}2`, 'Voice cast, synthesis and mastering'),
    studio('video', 'Scene Studio', Clapperboard, `${MOD_ALT}3`, 'Storyboard cards, art styles, repaint and MP4'),
    studio('localization', 'Localization', Languages, `${MOD_ALT}4`, 'Translation passes and locale QA'),
    studio('marketing', 'Marketing', Megaphone, `${MOD_ALT}5`, 'Blurb, hooks and platform copy'),
    studio('craft', 'Craft', PenTool, `${MOD_ALT}6`, 'Formats, personas, arcs, retention, KDP'),
    viewMode('clean', 'Canvas: clean prose', FileText, 'Chrome hidden, literary serif'),
    viewMode('script', 'Canvas: production script', Layers, 'Speaker gutters, chips, foley'),
    viewMode('readalong', 'Canvas: read-along', Radio, 'Active segment highlight, no gutter'),
    {
      id: 'toggle-binder',
      group: 'Canvas',
      label: 'Toggle project binder',
      keys: `${MOD_ALT}B`,
      icon: Columns3,
      run: () => {
        ws.toggleBinder();
        close();
      },
    },
    {
      id: 'toggle-inspector',
      group: 'Canvas',
      label: 'Toggle dynamic inspector',
      keys: `${MOD_ALT}I`,
      icon: Columns3,
      run: () => {
        ws.toggleInspector();
        close();
      },
    },
    {
      id: 'focus-mode',
      group: 'Canvas',
      label: settings.typewriterMode ? 'Leave focus mode' : 'Enter focus mode',
      hint: 'Canvas only · Escape exits',
      icon: Eye,
      run: () => {
        settings.setTypewriterMode(!settings.typewriterMode);
        close();
      },
    },
    {
      id: 'git-snapshot',
      group: 'Project',
      label: 'Git snapshot',
      hint: 'Commit the manuscript shadow state',
      keys: 'git_snapshot',
      icon: GitCommitHorizontal,
      run: async () => {
        close();
        try {
          const res = await api.gitSnapshot();
          toast('success', 'Snapshot committed', res.commit.slice(0, 12));
        } catch (err) {
          toastError('git_snapshot failed', err);
        }
      },
    },
    {
      id: 'clean-sweep',
      group: 'Project',
      label: 'Clean sweep + FTS rebuild',
      hint: 'Purge purgatory segments and threads',
      keys: 'clean_sweep',
      icon: Sparkles,
      run: async () => {
        close();
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
      },
    },
    {
      id: 'lock-status',
      group: 'Project',
      label: 'Re-probe lock status',
      keys: 'lock_status',
      icon: Lock,
      run: () => {
        close();
        void useProjectStore.getState().refreshLockStatus();
      },
    },
    {
      id: 'close-project',
      group: 'Project',
      label: 'Close project',
      hint: 'Back to the welcome screen',
      icon: LogOut,
      danger: true,
      run: () => {
        close();
        void project.closeProject();
      },
    },
    {
      id: 'theme-dark',
      group: 'Theme',
      label: 'Theme: dark',
      icon: Moon,
      run: () => {
        useSettingsStore.getState().setTheme('dark');
        close();
      },
    },
    {
      id: 'theme-midnight',
      group: 'Theme',
      label: 'Theme: midnight',
      icon: Moon,
      run: () => {
        useSettingsStore.getState().setTheme('midnight');
        close();
      },
    },
    {
      id: 'theme-sepia',
      group: 'Theme',
      label: 'Theme: sepia',
      icon: Sunrise,
      run: () => {
        useSettingsStore.getState().setTheme('sepia');
        close();
      },
    },
  ];
}

const MOD = MOD_KEY;
const MOD_ALT = MOD_ALT_KEY;

export function CommandPalette() {
  const open = useWorkspaceStore((s) => s.paletteOpen);
  const setPaletteOpen = useWorkspaceStore((s) => s.setPaletteOpen);
  const [query, setQuery] = useState('');
  const [index, setIndex] = useState(0);
  const panelRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  useFocusTrap(panelRef, open, () => setPaletteOpen(false));

  // Commands read store state at open time, so entering focus mode flips the
  // label next time the palette opens.
  const commands = useMemo(() => (open ? buildCommands() : []), [open]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return commands;
    return commands.filter((c) =>
      `${c.group} ${c.label} ${c.hint ?? ''} ${c.keys ?? ''}`.toLowerCase().includes(q),
    );
  }, [commands, query]);

  useEffect(() => setIndex(0), [query]);

  // Reset transient state on every open.
  useEffect(() => {
    if (open) {
      setQuery('');
      setIndex(0);
      inputRef.current?.focus();
    }
  }, [open]);

  // Keep the highlighted row in view while arrowing.
  useEffect(() => {
    listRef.current
      ?.querySelector('[data-active="true"]')
      ?.scrollIntoView({ block: 'nearest' });
  }, [index, filtered]);

  if (!open) return null;

  const runCommand = (cmd: Command) => void cmd.run();

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setIndex((i) => Math.min(i + 1, filtered.length - 1));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setIndex((i) => Math.max(i - 1, 0));
    } else if (e.key === 'Enter' && filtered[index]) {
      e.preventDefault();
      runCommand(filtered[index]);
    }
  };

  let lastGroup = '';

  return (
    <div
      className="fixed inset-0 z-[70] flex items-start justify-center pt-[12vh] px-4 bg-black/60 anim-fade"
      role="dialog"
      aria-modal="true"
      aria-label="Command palette"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) setPaletteOpen(false);
      }}
    >
      <div
        ref={panelRef}
        onKeyDown={onKeyDown}
        className="anim-modal w-full max-w-xl rounded-xl border border-line2 bg-panel shadow-2xl overflow-hidden flex flex-col max-h-[62vh]"
      >
        <div className="flex items-center gap-2.5 px-4 py-3 border-b border-line shrink-0">
          <span className="text-2xs font-mono text-accent border border-accent/40 bg-accent/10 rounded px-1.5 py-0.5">
            {MOD}K
          </span>
          <input
            ref={inputRef}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search commands — studios, canvas, project, themes…"
            className="flex-1 bg-transparent border-none outline-none text-sm text-ink placeholder:text-mut/70"
            aria-label="Search commands"
          />
          <kbd className="text-2xs text-mut font-mono border border-line rounded px-1.5 py-0.5">
            {filtered.length} cmd{filtered.length === 1 ? '' : 's'}
          </kbd>
        </div>

        <div ref={listRef} className="flex-1 min-h-0 overflow-y-auto py-1.5" role="listbox" aria-label="Commands">
          {filtered.length === 0 ? (
            <div className="px-4 py-8 text-center text-2xs text-mut">
              No command matches “{query}”.
            </div>
          ) : (
            filtered.map((cmd, i) => {
              const header = cmd.group !== lastGroup ? cmd.group : null;
              lastGroup = cmd.group;
              const Icon = cmd.icon;
              return (
                <div key={cmd.id}>
                  {header && (
                    <div className="px-4 pt-2.5 pb-1 text-2xs font-semibold tracking-wider text-mut uppercase select-none">
                      {header}
                    </div>
                  )}
                  <button
                    type="button"
                    role="option"
                    aria-selected={i === index}
                    data-active={i === index || undefined}
                    onMouseEnter={() => setIndex(i)}
                    onClick={() => runCommand(cmd)}
                    className={cn(
                      'w-full flex items-center gap-3 px-4 py-2 text-left text-[13px] transition-colors',
                      i === index ? 'bg-panel3' : 'hover:bg-panel2/60',
                    )}
                  >
                    <Icon
                      size={14}
                      strokeWidth={1.8}
                      className={cn('shrink-0', cmd.danger ? 'text-bad' : i === index ? 'text-accent' : 'text-mut')}
                    />
                    <span className={cn('min-w-0 flex-1', cmd.danger ? 'text-bad' : 'text-ink')}>
                      {cmd.label}
                      {cmd.hint && <span className="text-mut"> — {cmd.hint}</span>}
                    </span>
                    {cmd.keys && (
                      <kbd className="shrink-0 text-2xs font-mono text-mut border border-line rounded px-1.5 py-0.5">
                        {cmd.keys}
                      </kbd>
                    )}
                  </button>
                </div>
              );
            })
          )}
        </div>

        <div className="flex items-center gap-3 px-4 py-2 border-t border-line bg-panel2/50 text-2xs text-mut shrink-0">
          <span><kbd className="font-mono">↑↓</kbd> navigate</span>
          <span><kbd className="font-mono">↵</kbd> run</span>
          <span><kbd className="font-mono">esc</kbd> close</span>
          {!IS_MAC && <span className="ml-auto text-mut/70">⌘⌥ = Ctrl+Alt here</span>}
        </div>
      </div>
    </div>
  );
}
