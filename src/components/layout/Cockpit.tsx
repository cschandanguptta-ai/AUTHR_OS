import { useCallback, useRef } from 'react';
import { BookOpen, Minimize2 } from 'lucide-react';
import { useWorkspaceStore, PANE_DEFAULTS, type PaneKey } from '../../stores/workspaceStore';
import { useSettingsStore } from '../../stores/settingsStore';
import { StudioShell } from '../shared/StudioShell';
import { ProjectBinder } from './ProjectBinder';
import { DynamicInspector } from './DynamicInspector';
import { LexicalCanvas } from '../editor/LexicalCanvas';

function SplitHandle({
  pane,
  side,
  onReset,
}: {
  pane: PaneKey;
  side: 'left' | 'right';
  onReset: () => void;
}) {
  const width = useWorkspaceStore((s) => s.paneWidths[pane]);
  const setPaneWidth = useWorkspaceStore((s) => s.setPaneWidth);
  const activeRef = useRef(false);

  const onPointerDown = useCallback(
    (e: React.PointerEvent) => {
      e.preventDefault();
      const startX = e.clientX;
      const startWidth = width;
      const handleEl = e.currentTarget as HTMLElement;
      activeRef.current = true;
      handleEl.classList.add('active');
      document.body.style.cursor = 'col-resize';
      document.body.style.userSelect = 'none';

      const onMove = (ev: PointerEvent) => {
        const dx = ev.clientX - startX;
        const next = side === 'left' ? startWidth + dx : startWidth - dx;
        setPaneWidth(pane, next);
      };
      const onUp = () => {
        window.removeEventListener('pointermove', onMove);
        window.removeEventListener('pointerup', onUp);
        document.body.style.cursor = '';
        document.body.style.userSelect = '';
        handleEl.classList.remove('active');
        activeRef.current = false;
      };
      window.addEventListener('pointermove', onMove);
      window.addEventListener('pointerup', onUp);
    },
    [pane, side, setPaneWidth, width],
  );

  return (
    <div
      role="separator"
      aria-orientation="vertical"
      aria-label={`Resize ${pane} pane — drag or double-click to reset`}
      className="split-handle"
      onPointerDown={onPointerDown}
      onDoubleClick={onReset}
      title={`Drag to resize · double-click resets to ${PANE_DEFAULTS[pane]}px`}
    />
  );
}

/**
 * The Author's Cockpit — ProjectBinder | CenterCanvas | DynamicInspector,
 * wired with custom fluid split panes (no external library).
 */
export function Cockpit() {
  const binderOpen = useWorkspaceStore((s) => s.binderOpen);
  const inspectorOpen = useWorkspaceStore((s) => s.inspectorOpen);
  const binderWidth = useWorkspaceStore((s) => s.paneWidths.binder);
  const inspectorWidth = useWorkspaceStore((s) => s.paneWidths.inspector);
  const setPaneWidth = useWorkspaceStore((s) => s.setPaneWidth);
  const typewriter = useSettingsStore((s) => s.typewriterMode);
  const setTypewriterMode = useSettingsStore((s) => s.setTypewriterMode);

  const resetBinder = useCallback(() => setPaneWidth('binder', PANE_DEFAULTS.binder), [setPaneWidth]);
  const resetInspector = useCallback(
    () => setPaneWidth('inspector', PANE_DEFAULTS.inspector),
    [setPaneWidth],
  );

  if (typewriter) {
    // Distraction-free mode: the canvas takes over the viewport; same editor
    // instance, so the open chapter and undo history survive. The floating
    // pill (plus the global Escape handler) guarantees a way out — the old
    // build stranded users here with no visible exit.
    return (
      <div className="flex-1 min-h-0 relative anim-fade">
        <LexicalCanvas />
        <button
          type="button"
          onClick={() => setTypewriterMode(false)}
          className="focus-exit-pill anim-fade"
          title="Leave focus mode (Escape)"
          aria-label="Leave focus mode"
        >
          <Minimize2 size={12} />
          <span>Focus mode — press <kbd className="font-mono text-[11px]">Esc</kbd> or click to exit</span>
        </button>
      </div>
    );
  }

  return (
    /* R-17 unified chrome: the Cockpit adopts the StudioShell header family;
       the three-pane geometry and the in-canvas toolbar are untouched — the
       shell only contributes the title row every other studio shares. */
    <StudioShell
      icon={BookOpen}
      title="Novel Cockpit"
      subtitle="Manuscript canvas · project binder · dynamic inspector"
      scroll={false}
    >
    <div className="flex flex-1 min-h-0">
      {binderOpen && (
        <>
          <aside
            style={{ width: binderWidth }}
            className="shrink-0 border-r border-line bg-panel flex flex-col min-h-0"
            aria-label="Project binder"
          >
            <ProjectBinder />
          </aside>
          <SplitHandle pane="binder" side="left" onReset={resetBinder} />
        </>
      )}

      <section className="flex-1 min-w-0 flex flex-col min-h-0 relative" aria-label="Manuscript canvas">
        <LexicalCanvas />
      </section>

      {inspectorOpen && (
        <>
          <SplitHandle pane="inspector" side="right" onReset={resetInspector} />
          <aside
            style={{ width: inspectorWidth }}
            className="shrink-0 border-l border-line bg-panel flex flex-col min-h-0"
            aria-label="Dynamic inspector"
          >
            <DynamicInspector />
          </aside>
        </>
      )}
    </div>
    </StudioShell>
  );
}
