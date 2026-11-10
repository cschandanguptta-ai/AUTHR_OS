// AUTHR_OS — topological lookahead TTS scheduler (Phase 4, Sprint 10).
//
// The chapter's dialogue turns form a DAG. Dependency rule: consecutive turns
// by the SAME speaker are chained (turn k must complete before turn k+1) so
// the engine can thread `previous_request_id` for acoustic continuity; turns
// by different speakers are independent and synthesize in parallel.
//
// `plan()` returns topological waves (Kahn layering — every turn in a wave
// has all predecessors in earlier waves) plus a longest-first execution
// order per wave, the dependency list, and each turn's continuity pointer.

use crate::error::AppError;
use serde_json::{json, Value};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Turn {
    pub idx: usize,
    pub speaker: String,
    pub est_ms: u64,
}

#[derive(Debug, Clone)]
pub struct Plan {
    /// Topological waves of turn indices (parallelizable within a wave).
    pub waves: Vec<Vec<usize>>,
    /// Global execution order (waves concatenated, longest-first inside).
    pub order: Vec<usize>,
    /// (turn, predecessor) edges actually derived from speaker chains.
    pub deps: Vec<(usize, usize)>,
    /// For each turn index: the turn whose `request_id` becomes its
    /// `previous_request_id` (the speaker's immediately preceding turn).
    pub prev_request_of: HashMap<usize, usize>,
}

fn validate(turns: &[Turn]) -> Result<(), AppError> {
    for (i, t) in turns.iter().enumerate() {
        if t.idx != i {
            return Err(AppError::audio(format!(
                "turn list must be dense and ordered: position {i} carries idx {}",
                t.idx
            )));
        }
        if t.speaker.trim().is_empty() {
            return Err(AppError::audio(format!("turn {i} has an empty speaker")));
        }
    }
    Ok(())
}

/// Build the lookahead plan. Cycles are impossible with single-speaker
/// chaining, but the Kahn loop still guards (defensive) and reports a
/// scheduling failure instead of looping forever.
pub fn plan(turns: &[Turn]) -> Result<Plan, AppError> {
    validate(turns)?;

    // Per-speaker chains in encounter order.
    let mut last_of: HashMap<&str, usize> = HashMap::new();
    let mut succ: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut indegree = vec![0usize; turns.len()];
    let mut deps: Vec<(usize, usize)> = Vec::new();
    let mut prev_request_of: HashMap<usize, usize> = HashMap::new();
    for t in turns {
        if let Some(&prev) = last_of.get(t.speaker.as_str()) {
            succ.entry(prev).or_default().push(t.idx);
            indegree[t.idx] += 1;
            deps.push((t.idx, prev));
            prev_request_of.insert(t.idx, prev);
        }
        last_of.insert(t.speaker.as_str(), t.idx);
    }

    // Kahn layering.
    let mut waves: Vec<Vec<usize>> = Vec::new();
    let mut ready: Vec<usize> = (0..turns.len()).filter(|&i| indegree[i] == 0).collect();
    let mut placed = 0usize;
    while !ready.is_empty() {
        // Longest-first heuristic inside a wave (maximize parallel throughput).
        ready.sort_by(|a, b| turns[*b].est_ms.cmp(&turns[*a].est_ms).then(a.cmp(b)));
        let wave = ready.clone();
        let mut next: Vec<usize> = Vec::new();
        for &i in &wave {
            placed += 1;
            for &s in succ.get(&i).into_iter().flatten() {
                indegree[s] -= 1;
                if indegree[s] == 0 {
                    next.push(s);
                }
            }
        }
        waves.push(wave);
        ready = next;
    }
    if placed != turns.len() {
        return Err(AppError::audio(format!(
            "scheduling deadlock: {placed}/{} turns placed (dependency cycle)",
            turns.len()
        )));
    }

    let order = waves.iter().flatten().copied().collect();
    Ok(Plan { waves, order, deps, prev_request_of })
}

/// `audio_plan` payload from a list of (speaker, est_ms) pairs.
pub fn plan_json(pairs: Vec<(String, u64)>) -> Result<Value, AppError> {
    let turns: Vec<Turn> = pairs
        .into_iter()
        .enumerate()
        .map(|(idx, (speaker, est_ms))| Turn { idx, speaker, est_ms })
        .collect();
    let plan = plan(&turns)?;
    let waves: Vec<Value> = plan
        .waves
        .iter()
        .map(|w| json!(w.iter().map(|i| json!({
            "idx": i,
            "speaker": turns[*i].speaker,
            "est_ms": turns[*i].est_ms,
        })).collect::<Vec<_>>()))
        .collect();
    let deps: Vec<Value> = plan
        .deps
        .iter()
        .map(|(t, p)| json!({ "turn": t, "depends_on": p }))
        .collect();
    let continuity: Vec<Value> = plan
        .order
        .iter()
        .filter_map(|i| plan.prev_request_of.get(i).map(|p| json!({
            "turn": i,
            "previous_request_from": p,
            "previous_request_id": crate::audio::cache::AudioCache::key(
                "continuity", &turns[*p].speaker, 0, "", &turns[*p].speaker
            )[..16].to_string(),
        })))
        .collect();
    Ok(json!({
        "turns": turns.len(),
        "speakers": last_speaker_count(&turns),
        "waves": waves,
        "wave_count": waves.len(),
        "deps": deps,
        "continuity": continuity,
        "order": plan.order,
    }))
}

fn last_speaker_count(turns: &[Turn]) -> usize {
    let mut seen: Vec<&str> = Vec::new();
    for t in turns {
        if !seen.contains(&t.speaker.as_str()) {
            seen.push(&t.speaker);
        }
    }
    seen.len()
}

/// Canonical duration estimator (ms) for unsynthesized text — `chars×55+400`.
/// Single source of truth (tech-debt T-3): `audio_plan`, the synthesis
/// scheduler, captions, EPUB SMIL overlays and M4B chapter marks all derive
/// from this one function. Do NOT copy the formula — call this.
pub fn estimate_duration_ms(text: &str) -> u64 {
    (text.chars().count() as f64 * 55.0 + 400.0) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turns(spec: &[(&str, u64)]) -> Vec<Turn> {
        spec.iter()
            .enumerate()
            .map(|(i, (s, ms))| Turn { idx: i, speaker: s.to_string(), est_ms: *ms })
            .collect()
    }

    #[test]
    fn independent_speakers_share_wave() {
        let p = plan(&turns(&[("KIRA", 100), ("NARRATOR", 400), ("ORIN", 200)])).unwrap();
        assert_eq!(p.waves.len(), 1);
        assert_eq!(p.waves[0].len(), 3);
        // Longest first inside the wave.
        assert_eq!(p.waves[0][0], 1);
        assert!(p.deps.is_empty());
    }

    #[test]
    fn same_speaker_is_chained() {
        let p = plan(&turns(&[
            ("KIRA", 100),
            ("NARRATOR", 100),
            ("KIRA", 100),
            ("KIRA", 100),
        ]))
        .unwrap();
        // KIRA chain: 0→2→3. NARRATOR free. Equal est_ms → stable idx order.
        assert_eq!(p.waves.len(), 3);
        assert_eq!(p.waves[0], vec![0, 1]);
        assert_eq!(p.waves[1], vec![2]);
        assert_eq!(p.waves[2], vec![3]);
        assert_eq!(p.prev_request_of[&2], 0);
        assert_eq!(p.prev_request_of[&3], 2);
    }

    #[test]
    fn rejects_unordered_or_empty_speakers() {
        let bad = vec![Turn { idx: 1, speaker: "A".into(), est_ms: 1 }, Turn { idx: 0, speaker: "B".into(), est_ms: 1 }];
        assert!(plan(&bad).is_err());
        let empty_speaker = vec![Turn { idx: 0, speaker: "  ".into(), est_ms: 1 }];
        assert!(plan(&empty_speaker).is_err());
    }

    #[test]
    fn stress_many_speakers_fully_parallel() {
        // 50 turns, 10 speakers, interleaved — wave count == chain depth == 5.
        let spec: Vec<(&str, u64)> = (0..50)
            .map(|i| (["S0", "S1", "S2", "S3", "S4", "S5", "S6", "S7", "S8", "S9"][i % 10], 50))
            .collect();
        let p = plan(&turns(&spec)).unwrap();
        assert_eq!(p.waves.len(), 5);
        assert_eq!(p.order.len(), 50);
        assert_eq!(p.deps.len(), 40);
        // Every wave has exactly one turn per speaker (no serialization holes).
        for w in &p.waves {
            assert_eq!(w.len(), 10);
        }
    }

    #[test]
    fn estimator_matches_locked_formula() {
        // Locked formula (invariants §3, tech-debt T-3): chars×55+400 ms.
        // "one two three" = 13 chars → 1115 ms (mirrors publish/mod.rs test).
        assert_eq!(estimate_duration_ms("one two three"), 1115);
        assert_eq!(estimate_duration_ms(""), 400);
        assert_eq!(estimate_duration_ms("four"), 620);
    }
}
