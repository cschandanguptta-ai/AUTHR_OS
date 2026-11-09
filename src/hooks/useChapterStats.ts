// AUTHR_OS — useChapterStats: debounced, worker-backed chapter text stats.
//
// Workerisation boundary (ADR-0008, frontend side): the word-count walk over
// every segment previously ran synchronously in LexicalCanvas's render body
// (every store tick) and DynamicInspector's memo; long chapters re-counted
// tens of thousands of words per keystroke batch. The rows are serialised to
// a shared module worker (debounced 200 ms) and aggregates come back without
// blocking paint. One singleton worker, reference-counted by consumers.

import { useEffect, useRef, useState } from 'react';
import type { TextStatsRequest, TextStatsResult } from '../workers/textStats.worker';

const EMPTY: TextStatsResult = {
  words: 0,
  chars: 0,
  narrationWords: 0,
  dialogueWords: 0,
  monologueWords: 0,
  segments: 0,
  longest: null,
};

interface StatsSegment {
  segment_id: string;
  text_content: string;
  segment_type: string;
}

let shared: Worker | null = null;
let listeners = 0;
const results = new Map<number, (r: TextStatsResult) => void>();
let nextListenerId = 1;

function ensureWorker(): Worker | null {
  if (shared) return shared;
  if (typeof Worker === 'undefined') return null; // non-browser guard
  try {
    shared = new Worker(new URL('../workers/textStats.worker.ts', import.meta.url), {
      type: 'module',
    });
    shared.onmessage = (e: MessageEvent<TextStatsResult>) => {
      for (const cb of results.values()) cb(e.data);
    };
    shared.onerror = () => {
      // A dead worker must never wedge the UI — drop it; the next consumer
      // re-creates it and callers keep their last good stats.
      shared?.terminate();
      shared = null;
    };
  } catch {
    shared = null;
  }
  return shared;
}

function releaseWorker() {
  listeners -= 1;
  if (listeners <= 0 && shared) {
    shared.terminate();
    shared = null;
    results.clear();
  }
}

export function useChapterStats(segments: StatsSegment[], debounceMs = 200): TextStatsResult {
  const [stats, setStats] = useState<TextStatsResult>(EMPTY);
  const idRef = useRef(0);

  useEffect(() => {
    idRef.current = nextListenerId++;
    results.set(idRef.current, setStats);
    const worker = ensureWorker();
    listeners += 1;
    let timer: number | null = null;
    let lastPayload = '';

    const post = () => {
      if (!worker) return;
      const payload: TextStatsRequest = {
        kind: 'stats',
        segments: segments.map((s) => ({
          id: s.segment_id,
          text: s.text_content,
          type: s.segment_type,
        })),
      };
      const json = JSON.stringify(payload);
      if (json === lastPayload) return; // identical rows — skip the round-trip
      lastPayload = json;
      worker.postMessage(payload);
    };

    // Initial run + debounced updates as the segment rows change.
    post();
    if (segments.length > 0) {
      timer = window.setTimeout(post, debounceMs);
    }

    return () => {
      if (timer !== null) window.clearTimeout(timer);
      results.delete(idRef.current);
      releaseWorker();
    };
    // JSON-stable identity: recompute only when row contents actually change.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [segments.map((s) => `${s.segment_id}:${s.text_content.length}:${s.segment_type}`).join('|')]);

  return stats;
}
