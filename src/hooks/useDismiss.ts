import { useEffect, type RefObject } from 'react';

/**
 * Dismiss a floating surface (menu, popover, palette) on outside pointerdown
 * or Escape. Attach the returned ref to the surface's container element —
 * interactions that start inside it never count as "outside".
 */
export function useDismiss(
  ref: RefObject<HTMLElement | null>,
  onDismiss: () => void,
  active: boolean,
) {
  useEffect(() => {
    if (!active) return;

    const onPointerDown = (e: PointerEvent) => {
      const el = ref.current;
      if (el && !el.contains(e.target as Node)) onDismiss();
    };
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation();
        onDismiss();
      }
    };

    // pointerdown (not click) so drag-starts outside also dismiss.
    document.addEventListener('pointerdown', onPointerDown, true);
    document.addEventListener('keydown', onKeyDown, true);
    return () => {
      document.removeEventListener('pointerdown', onPointerDown, true);
      document.removeEventListener('keydown', onKeyDown, true);
    };
  }, [ref, onDismiss, active]);
}
