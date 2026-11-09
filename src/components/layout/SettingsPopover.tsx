// AUTHR_OS — Settings popover: reader-facing preferences (theme, editor type
// size, focus mode) plus the entry to the AI provider endpoints dialog
// (v1.6, ADR-0012 — image generation R2I endpoint + local LLM server, keys in
// the OS keychain). Lives in the workstation header, next to the palette
// trigger. Zero dependencies; dismissal via useDismiss.

import { useRef, useState } from 'react';
import { Check, Settings2, Sparkles, Type, Columns3 } from 'lucide-react';
import { useSettingsStore, type AppTheme } from '../../stores/settingsStore';
import { useDismiss } from '../../hooks/useDismiss';
import { ProviderSettingsModal } from './modals/ProviderSettingsModal';
import { cn } from '../../lib/utils';

const THEMES: Array<{ id: AppTheme; label: string; swatch: string; accentDot: string }> = [
  { id: 'dark', label: 'Dark', swatch: 'bg-[#101013]', accentDot: 'bg-[#f59e0b]' },
  { id: 'midnight', label: 'Midnight', swatch: 'bg-[#080d1c]', accentDot: 'bg-[#7c8cff]' },
  { id: 'sepia', label: 'Sepia', swatch: 'bg-[#fbf7ee]', accentDot: 'bg-[#b45309]' },
];

const FONT_MIN = 14;
const FONT_MAX = 22;

export function SettingsPopover() {
  const [open, setOpen] = useState(false);
  const [providersOpen, setProvidersOpen] = useState(false);
  const wrapRef = useRef<HTMLDivElement>(null);
  useDismiss(wrapRef, () => setOpen(false), open);

  const theme = useSettingsStore((s) => s.theme);
  const setTheme = useSettingsStore((s) => s.setTheme);
  const fontSize = useSettingsStore((s) => s.fontSize);
  const setFontSize = useSettingsStore((s) => s.setFontSize);
  const typewriterMode = useSettingsStore((s) => s.typewriterMode);
  const setTypewriterMode = useSettingsStore((s) => s.setTypewriterMode);

  return (
    <div className="relative" ref={wrapRef}>
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
        className={cn(
          'w-7 h-7 rounded-md border border-line bg-panel2 hover:bg-panel3 flex items-center justify-center transition-colors',
          open ? 'text-ink bg-panel3' : 'text-mut hover:text-ink',
        )}
        aria-label="Reader settings"
        aria-expanded={open}
        title="Theme · type size · focus mode"
      >
        <Settings2 size={14} />
      </button>

      {open && (
        <div className="anim-pop absolute right-0 top-9 z-40 w-72 rounded-lg border border-line2 bg-panel shadow-2xl py-3 px-3.5 grid gap-3.5">
          {/* Theme */}
          <section>
            <div className="text-2xs font-semibold tracking-wider text-mut uppercase mb-2">Theme</div>
            <div className="grid grid-cols-3 gap-1.5" role="radiogroup" aria-label="Theme">
              {THEMES.map((t) => (
                <button
                  key={t.id}
                  type="button"
                  role="radio"
                  aria-checked={theme === t.id}
                  onClick={() => setTheme(t.id)}
                  className={cn(
                    'flex flex-col items-center gap-1.5 rounded-md border px-2 py-2.5 transition-colors',
                    theme === t.id
                      ? 'border-accent/60 bg-panel3'
                      : 'border-line bg-panel2 hover:border-line2',
                  )}
                >
                  <span
                    className={cn(
                      'w-6 h-6 rounded-full border border-line relative flex items-center justify-center',
                      t.swatch,
                    )}
                  >
                    <span className={cn('w-2 h-2 rounded-full', t.accentDot)} />
                    {theme === t.id && (
                      <span className="absolute -top-1 -right-1 text-accent">
                        <Check size={10} strokeWidth={3} />
                      </span>
                    )}
                  </span>
                  <span className={cn('text-2xs', theme === t.id ? 'text-ink font-medium' : 'text-mut')}>
                    {t.label}
                  </span>
                </button>
              ))}
            </div>
          </section>

          {/* Editor type size */}
          <section>
            <div className="text-2xs font-semibold tracking-wider text-mut uppercase mb-2 flex items-center gap-1.5">
              <Type size={11} /> Editor type size
            </div>
            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={() => setFontSize(fontSize - 1)}
                disabled={fontSize <= FONT_MIN}
                className="w-7 h-7 rounded-md border border-line bg-panel2 hover:bg-panel3 text-ink disabled:opacity-40"
                aria-label="Decrease editor type size"
              >
                <span className="text-xs leading-none">A−</span>
              </button>
              <input
                type="range"
                min={FONT_MIN}
                max={FONT_MAX}
                value={fontSize}
                onChange={(e) => setFontSize(Number(e.target.value))}
                className="flex-1"
                aria-label="Editor type size in pixels"
                aria-valuetext={`${fontSize} pixels`}
              />
              <button
                type="button"
                onClick={() => setFontSize(fontSize + 1)}
                disabled={fontSize >= FONT_MAX}
                className="w-7 h-7 rounded-md border border-line bg-panel2 hover:bg-panel3 text-ink disabled:opacity-40"
                aria-label="Increase editor type size"
              >
                <span className="text-sm leading-none">A+</span>
              </button>
              <span className="font-mono text-2xs text-mut w-10 text-right tabular-nums">{fontSize}px</span>
            </div>
          </section>

          {/* AI providers — endpoints, keys, probe */}
          <section>
            <button
              type="button"
              onClick={() => {
                setProvidersOpen(true);
                setOpen(false);
              }}
              className="w-full flex items-center gap-2.5 rounded-md border border-line bg-panel2 hover:bg-panel3 px-3 py-2.5 text-left transition-colors"
            >
              <Sparkles size={14} className={cn('text-accent')} />
              <span className="flex-1 min-w-0">
                <span className="block text-[13px] text-ink">AI providers</span>
                <span className="block text-2xs text-mut">
                  Image endpoint (R2I) · LLM server · API keys
                </span>
              </span>
              <span className="text-2xs text-mut font-mono">v1.6</span>
            </button>
          </section>

          {/* Focus mode */}
          <section>
            <button
              type="button"
              role="switch"
              aria-checked={typewriterMode}
              onClick={() => {
                setTypewriterMode(!typewriterMode);
                setOpen(false);
              }}
              className="w-full flex items-center gap-2.5 rounded-md border border-line bg-panel2 hover:bg-panel3 px-3 py-2.5 text-left transition-colors"
            >
              <Columns3 size={14} className={cn(typewriterMode ? 'text-accent' : 'text-mut')} />
              <span className="flex-1 min-w-0">
                <span className="block text-[13px] text-ink">Focus mode</span>
                <span className="block text-2xs text-mut">Canvas only — Escape always exits</span>
              </span>
              <span
                className={cn(
                  'w-8 h-[18px] rounded-full relative transition-colors shrink-0',
                  typewriterMode ? 'bg-accent' : 'bg-line-2',
                )}
                aria-hidden="true"
              >
                <span
                  className={cn(
                    'absolute top-[2px] w-[14px] h-[14px] rounded-full bg-panel transition-all',
                    typewriterMode ? 'left-[16px]' : 'left-[2px]',
                  )}
                />
              </span>
            </button>
          </section>
        </div>
      )}

      <ProviderSettingsModal open={providersOpen} onClose={() => setProvidersOpen(false)} />
    </div>
  );
}
