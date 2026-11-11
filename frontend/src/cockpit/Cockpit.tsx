import type { JSX } from 'react';
/**
 * The Author's Cockpit — 3-pane workstation shell (Sprint 6).
 *
 *   [ Project Binder | AST Canvas | Dynamic Inspector ]
 *
 * Pane fractions persist in the uiStore (localStorage-backed) and are
 * resizable with pointer drag handles; double-click a handle to reset.
 */

import { useCallback, useEffect, useRef } from 'react';
import { useUi } from '../stores/uiStore';
import { ProjectBinder } from './ProjectBinder';
import { InspectorPane } from './InspectorPane';
import { LexicalCanvas } from '../editor/LexicalCanvas';

type Edge = 'left' | 'right';

export function Cockpit(): JSX.Element {
  const panes = useUi((s) => s.panes);
  const setPanes = useUi((s) => s.setPanes);
  const containerRef = useRef<HTMLDivElement>(null);
  const dragRef = useRef<{ edge: Edge; startX: number; start: { binder: number; canvas: number; inspector: number } } | null>(null);

  const onPointerMove = useCallback(
    (e: PointerEvent): void => {
      const drag = dragRef.current;
      const el = containerRef.current;
      if (!drag || !el) return;
      const rect = el.getBoundingClientRect();
      const deltaPct = ((e.clientX - drag.startX) / rect.width) * 100;
      const min = 12;
      const max = 55;
      if (drag.edge === 'left') {
        const binder = clamp(drag.start.binder + deltaPct, min, max);
        const canvas = drag.start.canvas + (drag.start.binder - binder);
        setPanes({ ...drag.start, binder, canvas });
      } else {
        const inspector = clamp(drag.start.inspector - deltaPct, min, max);
        const canvas = drag.start.canvas + (drag.start.inspector - inspector);
        setPanes({ ...drag.start, inspector, canvas });
      }
    },
    [setPanes],
  );

  const onPointerUp = useCallback((): void => {
    dragRef.current = null;
    window.removeEventListener('pointermove', onPointerMove);
    window.removeEventListener('pointerup', onPointerUp);
    document.body.classList.remove('dragging-pane');
  }, [onPointerMove]);

  useEffect(() => {
    return () => {
      window.removeEventListener('pointermove', onPointerMove);
      window.removeEventListener('pointerup', onPointerUp);
      document.body.classList.remove('dragging-pane');
    };
  }, [onPointerMove]);

  const startDrag = (edge: Edge) => (e: React.PointerEvent): void => {
    dragRef.current = { edge, startX: e.clientX, start: useUi.getState().panes };
    window.addEventListener('pointermove', onPointerMove);
    window.addEventListener('pointerup', onPointerUp);
    document.body.classList.add('dragging-pane');
  };

  return (
    <div className="cockpit" ref={containerRef}>
      <div className="cockpit-pane pane-binder" style={{ width: `${panes.binder}%` }}>
        <ProjectBinder />
      </div>
      <div
        className="pane-divider"
        role="separator"
        aria-orientation="vertical"
        title="Drag to resize · double-click to reset"
        onPointerDown={startDrag('left')}
        onDoubleClick={() => setPanes({ binder: 20, canvas: 56, inspector: 24 })}
      />
      <div className="cockpit-pane pane-canvas" style={{ width: `${panes.canvas}%` }}>
        <LexicalCanvas />
      </div>
      <div
        className="pane-divider"
        role="separator"
        aria-orientation="vertical"
        title="Drag to resize · double-click to reset"
        onPointerDown={startDrag('right')}
        onDoubleClick={() => setPanes({ binder: 20, canvas: 56, inspector: 24 })}
      />
      <div className="cockpit-pane pane-inspector" style={{ flex: 1 }}>
        <InspectorPane />
      </div>
    </div>
  );
}

function clamp(n: number, lo: number, hi: number): number {
  return Math.min(hi, Math.max(lo, n));
}
