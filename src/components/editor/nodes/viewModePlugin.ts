// viewModePlugin — applies the triple-view contract to the Lexical root:
//   clean     → prose only (gutter, chips, foley hidden by CSS)
//   script    → Speaker Gutter Badges + Emotion Chips + Foley pills visible
//   readalong → chips/foley visible, gutter hidden, selected segment
//               highlighted as the read-along cursor (karaoke timeline lands
//               with the audio produce pipeline; per-segment spans are
//               already reserved).
//
// Also owns click-to-select in read-along mode: clicking a paragraph selects
// its segment (identity resolved from the paragraph's `data-segment-id`).

import { useEffect } from 'react';
import type { MouseEvent as DomMouseEvent } from 'react';
import { useLexicalComposerContext } from '@lexical/react/LexicalComposerContext';
import type { ViewMode } from '../../../stores/workspaceStore';
import { useManuscriptStore } from '../../../stores/manuscriptStore';

function applyRootClass(root: HTMLElement, mode: ViewMode) {
  root.classList.remove('editor-clean', 'editor-script', 'editor-readalong');
  root.classList.add(`editor-${mode}`);
}

function highlightSelected(root: HTMLElement, selectedSegmentId: string | null) {
  root.querySelectorAll('.canvas-paragraph.seg-active').forEach((el) => {
    el.classList.remove('seg-active');
  });
  if (!selectedSegmentId) return;
  const target = root.querySelector(
    `.canvas-paragraph[data-segment-id="${CSS.escape(selectedSegmentId)}"]`,
  );
  if (target) target.classList.add('seg-active');
}

export function useViewModeSync(viewMode: ViewMode): void {
  const [editor] = useLexicalComposerContext();

  // Root class tracking (root element may be replaced on remount).
  useEffect(() => {
    const update = () => {
      const root = editor.getRootElement();
      if (root) applyRootClass(root, viewMode);
    };
    update();
    return editor.registerRootListener(
      (root: HTMLElement | null, prev: HTMLElement | null) => {
        if (prev) applyRootClass(prev, 'clean'); // remove marker classes
        if (prev) prev.classList.remove('editor-clean', 'editor-script', 'editor-readalong');
        if (root) applyRootClass(root, viewMode);
      },
    );
  }, [editor, viewMode]);

  // Read-along cursor highlight follows the selected segment.
  useEffect(() => {
    const paint = () => {
      const root = editor.getRootElement();
      if (root) highlightSelected(root, useManuscriptStore.getState().selectedSegmentId);
    };
    paint();
    const unsub = useManuscriptStore.subscribe((s, prev) => {
      if (s.selectedSegmentId !== prev.selectedSegmentId) paint();
    });
    return unsub;
  }, [editor, viewMode]);

  // Click-to-select inside read-along mode.
  useEffect(() => {
    if (viewMode !== 'readalong') return;
    const onClick = (evt: Event) => {
      const e = evt as unknown as DomMouseEvent<HTMLElement>;
      const target = e.target as HTMLElement | null;
      const paragraph = target?.closest?.('.canvas-paragraph');
      if (!paragraph) return;
      const segmentId = paragraph.getAttribute('data-segment-id');
      if (segmentId) {
        e.preventDefault();
        useManuscriptStore.getState().selectSegment(segmentId);
      }
    };
    const rootEl = editor.getRootElement()?.parentElement ?? document;
    rootEl.addEventListener('click', onClick);
    return () => rootEl.removeEventListener('click', onClick);
  }, [editor, viewMode]);
}
