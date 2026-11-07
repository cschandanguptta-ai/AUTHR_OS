// AUTHR_OS — 400 ms debounced autosave hook driving the Lexical → AST bridge.

import { useCallback, useEffect, useRef } from 'react';

export interface AutosaveController {
  schedule: () => void;
  flush: () => Promise<void>;
  cancel: () => void;
}

export function useAutosave(onSave: () => Promise<void> | void, delay = 400): AutosaveController {
  const fnRef = useRef(onSave);
  fnRef.current = onSave;
  const timerRef = useRef<number | null>(null);
  const pendingRef = useRef(false);

  const cancel = useCallback(() => {
    if (timerRef.current !== null) {
      window.clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  const flush = useCallback(async () => {
    cancel();
    if (!pendingRef.current) return;
    pendingRef.current = false;
    await fnRef.current();
  }, [cancel]);

  const schedule = useCallback(() => {
    pendingRef.current = true;
    if (timerRef.current !== null) window.clearTimeout(timerRef.current);
    timerRef.current = window.setTimeout(() => {
      timerRef.current = null;
      pendingRef.current = false;
      void fnRef.current();
    }, delay);
  }, [delay]);

  useEffect(() => () => cancel(), [cancel]);

  return { schedule, flush, cancel };
}
