// AUTHR_OS — textStats worker: chapter-level text analysis off the main
// thread (ADR-0008 workerisation, frontend side). Every keystroke batch used
// to re-walk every segment's text synchronously in render paths (LexicalCanvas
// live counter + DynamicInspector progress); now the store serialises the
// light rows once and this worker returns the aggregates without touching the
// paint path. Pure function, no imports — safe as a module worker.

export interface WorkerSegment {
  id: string;
  text: string;
  type: string;
}

export interface TextStatsRequest {
  kind: 'stats';
  segments: WorkerSegment[];
}

export interface TextStatsResult {
  words: number;
  chars: number;
  narrationWords: number;
  dialogueWords: number;
  monologueWords: number;
  segments: number;
  longest: { id: string; words: number } | null;
}

function countWords(text: string): number {
  const t = text.trim();
  if (!t) return 0;
  let n = 1;
  for (let i = 0; i < t.length; i++) {
    if (t[i] === ' ' || t[i] === '\n' || t[i] === '\t') {
      // count runs of whitespace once
      while (i + 1 < t.length && (t[i + 1] === ' ' || t[i + 1] === '\n' || t[i + 1] === '\t')) i++;
      n++;
    }
  }
  return n;
}

self.onmessage = (e: MessageEvent<TextStatsRequest>) => {
  if (e.data?.kind !== 'stats') return;
  const out: TextStatsResult = {
    words: 0,
    chars: 0,
    narrationWords: 0,
    dialogueWords: 0,
    monologueWords: 0,
    segments: 0,
    longest: null,
  };
  let longestWords = -1;
  let longestId = '';
  for (const s of e.data.segments) {
    const w = countWords(s.text);
    out.words += w;
    out.chars += s.text.length;
    out.segments += 1;
    if (s.type === 'DIALOGUE') out.dialogueWords += w;
    else if (s.type === 'INTERNAL_MONOLOGUE') out.monologueWords += w;
    else out.narrationWords += w;
    if (w > longestWords) {
      longestWords = w;
      longestId = s.id;
    }
  }
  out.longest = longestWords > 0 ? { id: longestId, words: longestWords } : null;
  (self as unknown as Worker).postMessage(out);
};
