// AUTHR_OS — palette fuzzy matcher (R-15). Zero dependencies, deterministic:
//   1. exact substring beats everything (earlier hit = higher score)
//   2. in-order subsequence with consecutive-run bonuses ("pcp" → "Produce Card P…")
//   3. typo tolerance: each query token must edit-distance-match some target
//      token within a small budget (≤1 for short words, ≤2 for ≥5 letters),
//      so "prodice chapter audio" and "audoi" still find their commands.
// Returns null when the target does not match at all.

/** Plain Levenshtein distance, or null when it exceeds `max` (small strings). */
function boundedLevenshtein(a: string, b: string, max: number): number | null {
  if (Math.abs(a.length - b.length) > max) return null;
  const dp: number[][] = Array.from({ length: a.length + 1 }, () => new Array(b.length + 1).fill(0));
  for (let i = 0; i <= a.length; i++) dp[i][0] = i;
  for (let j = 0; j <= b.length; j++) dp[0][j] = j;
  for (let i = 1; i <= a.length; i++) {
    for (let j = 1; j <= b.length; j++) {
      dp[i][j] = Math.min(
        dp[i - 1][j] + 1,
        dp[i][j - 1] + 1,
        dp[i - 1][j - 1] + (a[i - 1] === b[j - 1] ? 0 : 1),
      );
    }
  }
  const d = dp[a.length][b.length];
  return d <= max ? d : null;
}

const TOKEN_SPLIT = /[\s\-_/.:,·'']+/;
const TOKEN_BUDGET = (len: number): number => (len >= 5 ? 2 : 1);

export function fuzzyScore(query: string, target: string): number | null {
  const q = query.trim().toLowerCase();
  if (!q) return 0;
  const t = target.toLowerCase();

  // 1 — exact substring, earlier is better.
  const idx = t.indexOf(q);
  if (idx >= 0) return 1000 - idx;

  // 2 — in-order subsequence with bonuses for consecutive runs.
  let qi = 0;
  let subScore = 500;
  let prev = -2;
  for (let ti = 0; ti < t.length && qi < q.length; ti++) {
    if (t[ti] === q[qi]) {
      subScore += prev === ti - 1 ? 5 : 2;
      prev = ti;
      qi++;
    }
  }
  if (qi === q.length) return subScore - Math.max(0, t.length - q.length) * 0.5;

  // 3 — token-wise typo tolerance.
  const tokens = t.split(TOKEN_SPLIT).filter(Boolean);
  if (tokens.length === 0) return null;
  const qTokens = q.split(/\s+/).filter(Boolean);
  let worst: number | null = null;
  for (const qt of qTokens) {
    const budget = TOKEN_BUDGET(qt.length);
    let best: number | null = null;
    for (const tt of tokens) {
      const d = boundedLevenshtein(qt, tt, budget);
      if (d != null && (best == null || d < best)) best = d;
    }
    if (best == null) return null;
    worst = worst == null ? best : Math.max(worst, best);
  }
  return 200 - (worst ?? 0) * 40;
}
