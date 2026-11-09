// AUTHR_OS — Retention & serialization kit (contract v1.4).
//
// Folds the "Serialized Fiction Blueprint 2.0" (SFB-2.0) — a reader-retention
// framework for chapter pacing and serial drafting — into the offline craft
// layer, complementing the macro plot arcs of §4.14 (which shape a whole
// book) with chapter-level machinery (which shape every single chapter):
//
//   · seven-beat chapter model     — the within-chapter micro-structure
//   · six chapter-ending types     — taxonomy + selection rules (no rotation
//                                    by rote; endings follow the drama)
//   · micro-arc cycle (10–15 ch)   — 4 phases bridging chapter and book
//   · QC rubric (7 × 0–2)          — editorial bands + zero-flag rule
//   · drafting worksheet (A–E)     — pre-draft checklist fused with book state
//   · serialization cadence        — 2–3 chapters/week, 4–8 chapter buffer
//   · opening-hook strategy        — 200–400 words, seven hook types
//   · information management       — immediate / progressive / long-term
//
// Storage is purely ADDITIVE: `chapter_retention(book_id, chapter_number, …)`
// via an eager idempotent migration (same pattern as `craft_state`). No §5
// row shape is touched, no new error code (reuses `CRAFT_ERROR`).

use crate::error::AppError;
use crate::ipc::args;
use rusqlite::Connection;
use serde_json::{json, Value};

/// Blueprint identity stamped on every payload.
pub const BLUEPRINT_ID: &str = "SFB-2.0";

/// QC rubric: the seven 0–2 criteria, in scoring order.
pub const QC_CRITERIA: [&str; 7] = [
    "opening_attention",
    "narrative_objective",
    "conflict_progression",
    "character_development",
    "plot_advancement",
    "chapter_payoff",
    "ending_pull",
];

/// Zero-flag criteria: a 0 here forces revision regardless of the total
/// (blueprint Part 9: clarity and meaningful payoff are non-negotiable).
const ZERO_FLAG_CRITERIA: [&str; 2] = ["narrative_objective", "chapter_payoff"];

// ─────────────────────────────────────────────────────────────────────────────
// Static blueprint data
// ─────────────────────────────────────────────────────────────────────────────

/// Part 2.1 — the seven-beat chapter model (functions of a chapter, not
/// mandatory scene divisions).
pub fn chapter_beats() -> Vec<Value> {
    vec![
        json!({"id":"opening_continuity","label":"Opening Continuity","share":"10–15% (with forward pull)","purpose":"Reconnect the reader with the immediate situation; continue the preceding chapter's tension without repeating information."}),
        json!({"id":"objective","label":"Immediate Objective","share":"objective+resistance+escalation ≈ 50–60%","purpose":"Establish what the viewpoint character wants this chapter — physical, emotional, relational or informational."}),
        json!({"id":"resistance","label":"Resistance","share":"objective+resistance+escalation ≈ 50–60%","purpose":"Introduce an obstacle, conflicting intention, unexpected development or internal struggle."}),
        json!({"id":"escalation","label":"Escalation","share":"objective+resistance+escalation ≈ 50–60%","purpose":"Increase difficulty or consequences; complicate the situation rather than repeat the original problem."}),
        json!({"id":"turning_point","label":"Turning Point","share":"turning point+payoff ≈ 20–25%","purpose":"An important discovery, decision, confrontation, reversal or change of understanding."}),
        json!({"id":"payoff","label":"Chapter Payoff","share":"turning point+payoff ≈ 20–25%","purpose":"Deliver a meaningful result — success, failure, compromise, discovery or consequence. Something must change."}),
        json!({"id":"forward_pull","label":"Forward Pull","share":"10–15% (with opening continuity)","purpose":"End with a new question, looming consequence, decision, revelation or unresolved emotional situation that makes the next chapter hard to resist."}),
    ]
}

/// Part 3.1 — six chapter-ending types with worked examples.
pub fn ending_types() -> Vec<Value> {
    vec![
        json!({"id":"danger","label":"Danger","example":"The door burst open. Three armed men entered, and one of them was carrying her missing brother's identification card.","when":"Physical danger, immediate threat or an escalating tactical situation."}),
        json!({"id":"emotional_revelation","label":"Emotional Revelation","example":"He had spent ten years searching for the woman who saved his life. Now she stood before him, asking him to forget that she had ever existed.","when":"A relationship, loyalty or personal belief is suddenly challenged."}),
        json!({"id":"mystery_discovery","label":"Mystery or Discovery","example":"The signature on the original agreement was not the chairman's. It belonged to the person who had supposedly died twenty years earlier.","when":"An unexpected piece of information changes the reader's understanding."}),
        json!({"id":"irreversible_choice","label":"Irreversible Choice","example":"She could expose the truth and destroy her family, or remain silent and allow an innocent man to take the blame.","when":"A character must decide between options with serious consequences."}),
        json!({"id":"reversal","label":"Reversal","example":"The witnesses had all confirmed his alibi. Then the prosecutor played the recording.","when":"A seemingly successful plan, victory or understanding collapses."}),
        json!({"id":"anticipation","label":"Anticipation","example":"At midnight, the palace gates would open. For the first time in fifteen years, the two brothers would stand on opposite sides of the same battlefield.","when":"The reader knows an important confrontation, reunion or event is approaching."}),
    ]
}

/// Part 3.2 — ending selection rules (never rotate mechanically).
pub fn ending_rules() -> Vec<String> {
    vec![
        "Do not use the same type of high-intensity ending repeatedly.".into(),
        "Ensure the ending arises naturally from the preceding events.".into(),
        "Make the stakes understandable.".into(),
        "Ensure the unresolved question matters to the protagonist or central plot.".into(),
        "Do not repeatedly promise answers that the following chapters fail to deliver.".into(),
        "Allow occasional quiet, reflective or emotionally satisfying endings.".into(),
    ]
}

/// Part 4.1 — the 10–15 chapter micro-arc cycle.
pub fn micro_arc_phases() -> Vec<Value> {
    vec![
        json!({"id":"setup","label":"Objective and Setup","chapters":"1–3","offset":0,"span":3,"goals":[
            "Establish a localized conflict.",
            "Introduce the immediate objective.",
            "Establish relevant characters and stakes.",
            "Create an important question."]}),
        json!({"id":"escalation","label":"Escalation","chapters":"4–7","offset":3,"span":4,"goals":[
            "Introduce obstacles.",
            "Develop rivalries and relationships.",
            "Reveal partial information.",
            "Create setbacks and raise the cost of failure."]}),
        json!({"id":"reversal_pivot","label":"Reversal and Pivot","chapters":"8–11","offset":7,"span":4,"goals":[
            "Reveal information that changes the situation.",
            "Challenge the protagonist's assumptions.",
            "Introduce a new tactical or emotional direction.",
            "Force an important decision."]}),
        json!({"id":"climax_resolution","label":"Climax and Resolution","chapters":"12–15","offset":11,"span":4,"goals":[
            "Bring the localized conflict to its decisive confrontation.",
            "Deliver a meaningful outcome and show the consequences.",
            "Reward the reader with an important payoff.",
            "Introduce the next narrative objective."]}),
    ]
}

/// Part 5 — character-attachment levers (retention beyond plot).
pub fn attachment_levers() -> Vec<Value> {
    vec![
        json!({"id":"desire","text":"What does the character genuinely want?"}),
        json!({"id":"vulnerability","text":"What makes the character emotionally or psychologically exposed?"}),
        json!({"id":"contradiction","text":"What is the difference between how the character appears and how they actually behave?"}),
        json!({"id":"relationships","text":"What does the character stand to lose through love, loyalty, ambition, betrayal or friendship?"}),
        json!({"id":"consequences","text":"How do important events permanently change the character?"}),
        json!({"id":"agency","text":"Does the protagonist make consequential decisions rather than simply react?"}),
        json!({"id":"growth","text":"Does the character change, deteriorate or discover something meaningful?"}),
    ]
}

/// Part 1.1 — opening-hook types and the 200–400 word benchmark.
pub fn opening_strategy() -> Value {
    json!({
        "benchmark_words": [200, 400],
        "hook_types": [
            {"id":"disturbing_event","text":"A disturbing or unusual event."},
            {"id":"urgent_problem","text":"An urgent problem."},
            {"id":"pursuit","text":"A character pursuing something important."},
            {"id":"confrontation","text":"An unexpected confrontation."},
            {"id":"assumption_breaking_mystery","text":"A mystery that challenges the reader's assumptions."},
            {"id":"emotional_situation","text":"A powerful emotional situation."},
            {"id":"intriguing_voice","text":"An intriguing voice or unusual narrative perspective."}
        ],
        "avoid": [
            "Extended descriptions of geography or history.",
            "Family-background info dumps.",
            "Fictional-system lectures unless they serve the immediate scene."
        ],
        "rule": "Begin with a character experiencing, pursuing or confronting something that matters; world-building emerges through action, observation and decision."
    })
}

/// Part 6 — information and revelation management.
pub fn information_levels() -> Vec<Value> {
    vec![
        json!({"id":"immediate","text":"What the reader needs to understand the current scene."}),
        json!({"id":"progressive","text":"What the reader should gradually discover over the next few chapters."}),
        json!({"id":"long_term","text":"What stays unresolved until a major turning point or climax."}),
    ]
}

/// Part 7 — serialization cadence and buffer guidance.
pub fn cadence() -> Value {
    json!({
        "chapters_per_week": [2, 3],
        "buffer_chapters": [4, 8],
        "engagement": [
            "Ask readers which character they currently trust.",
            "Invite predictions after an important revelation.",
            "Conduct polls after a major story development.",
            "Encourage discussion about a moral dilemma.",
            "Ask readers which relationship they find most compelling."
        ],
        "rule": "Consistency beats intensity: keep the schedule you communicated; never let polls dictate plot, character consistency or long-term direction."
    })
}

/// Part 8 — the drafting worksheet sections A–E.
pub fn worksheet_sections() -> Vec<Value> {
    vec![
        json!({"letter":"A","id":"narrative_objective","title":"Narrative Objective","prompts":[
            "What is the main purpose of this chapter?",
            "What does the protagonist want?",
            "What prevents the protagonist from achieving it?"]}),
        json!({"letter":"B","id":"character_development","title":"Character Development","prompts":[
            "Which character receives the most meaningful development?",
            "What emotional conflict is present?",
            "Does a relationship change?"]}),
        json!({"letter":"C","id":"plot_development","title":"Plot Development","prompts":[
            "What new information is revealed?",
            "What complication is introduced?",
            "What changes by the end?"]}),
        json!({"letter":"D","id":"reader_experience","title":"Reader Experience","prompts":[
            "Why should the reader care about this chapter?",
            "What question is the reader expecting to be answered?",
            "What satisfying payoff will the chapter deliver?",
            "What will make the reader want the next chapter?"]}),
        json!({"letter":"E","id":"chapter_ending","title":"Chapter Ending","prompts":[
            "Is the ending consistent with the preceding events?",
            "Is it sufficiently compelling?",
            "Does it create anticipation without feeling artificially interrupted?"]}),
    ]
}

/// Part 9 — editorial bands for the 14-point QC total.
pub fn qc_bands() -> Vec<Value> {
    vec![
        json!({"min":12,"max":14,"band":"STRONG","verdict":"Strong chapter; proceed to final editing."}),
        json!({"min":9,"max":11,"band":"EXAMINE","verdict":"Reasonably effective; examine the weaker areas."}),
        json!({"min":6,"max":8,"band":"STRUCTURAL_REVISION","verdict":"Requires structural revision."}),
        json!({"min":0,"max":5,"band":"RECONSIDER","verdict":"Reconsider the chapter's purpose and narrative construction."}),
    ]
}

/// Part 10 — final drafting principles + the ultimate rule.
pub fn principles() -> Value {
    json!({
        "final_checks": [
            "The chapter has a clear narrative purpose.",
            "The protagonist or viewpoint character has an identifiable objective.",
            "The chapter contains meaningful resistance or conflict.",
            "Events produce consequences.",
            "The plot or character situation changes.",
            "Exposition is integrated naturally.",
            "The reader receives some form of narrative gratification.",
            "The ending creates an appropriate forward pull.",
            "The chapter does not repeat previously established information unnecessarily.",
            "The chapter contributes to the larger story."
        ],
        "ultimate_rule": [
            "Never sacrifice narrative credibility merely to create a cliffhanger.",
            "Never sacrifice character development merely to accelerate the plot.",
            "Never withhold every important answer merely to generate curiosity.",
            "Never make the reader feel that their time has been wasted."
        ]
    })
}

/// Full blueprint payload (also served by `craft_catalog kind=retention`).
pub fn blueprint_json() -> Value {
    json!({
        "blueprint": BLUEPRINT_ID,
        "title": "Serialized Fiction Blueprint 2.0",
        "subtitle": "A practical framework for reader retention, chapter pacing and novel drafting",
        "core_principle": [
            "Give the reader something worthwhile for the time invested.",
            "Change the situation, understanding, relationships or stakes.",
            "Create a compelling reason to continue reading."
        ],
        "chapter_beats": chapter_beats(),
        "ending_types": ending_types(),
        "ending_rules": ending_rules(),
        "micro_arc": micro_arc_phases(),
        "qc_criteria": QC_CRITERIA,
        "qc_bands": qc_bands(),
        "zero_flag_criteria": ZERO_FLAG_CRITERIA,
        "worksheet_sections": worksheet_sections(),
        "opening": opening_strategy(),
        "information_levels": information_levels(),
        "attachment_levers": attachment_levers(),
        "cadence": cadence(),
        "principles": principles(),
        "counts": {
            "chapter_beats": chapter_beats().len(),
            "ending_types": ending_types().len(),
            "micro_arc_phases": micro_arc_phases().len(),
            "qc_criteria": QC_CRITERIA.len(),
            "worksheet_sections": worksheet_sections().len()
        }
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Pure scoring / planning functions
// ─────────────────────────────────────────────────────────────────────────────

/// Band lookup for a QC total.
pub fn band_for(total: i64) -> Value {
    qc_bands()
        .into_iter()
        .find(|b| total >= b["min"].as_i64().unwrap_or(0) && total <= b["max"].as_i64().unwrap_or(-1))
        .unwrap_or_else(|| json!({"min":0,"max":0,"band":"RECONSIDER","verdict":"Out of range."}))
}

/// Score a chapter from its seven 0–2 ratings. Returns the total, band,
/// zero-flags and whether revision is mandatory.
pub fn score_chapter(scores: &[i64]) -> Result<Value, String> {
    if scores.len() != QC_CRITERIA.len() {
        return Err(format!(
            "expected {} scores (one per criterion {:?}), got {}",
            QC_CRITERIA.len(),
            QC_CRITERIA,
            scores.len()
        ));
    }
    for (i, s) in scores.iter().enumerate() {
        if !(0..=2).contains(s) {
            return Err(format!(
                "score for `{}` must be 0–2, got {s}",
                QC_CRITERIA[i]
            ));
        }
    }
    let total: i64 = scores.iter().sum();
    let band = band_for(total);
    let zero_flags: Vec<&str> = scores
        .iter()
        .zip(QC_CRITERIA.iter())
        .filter(|(s, c)| **s == 0 && ZERO_FLAG_CRITERIA.contains(&**c))
        .map(|(_, c)| *c)
        .collect();
    Ok(json!({
        "scores": scores,
        "total": total,
        "max": 14,
        "band": band["band"],
        "verdict": band["verdict"],
        "zero_flags": zero_flags,
        "revision_required": !zero_flags.is_empty(),
    }))
}

/// Micro-arc phase for a 1-based chapter number (the 15-chapter cycle wraps:
/// chapter 16 re-enters Phase 1 of cycle 2).
pub fn phase_for(chapter_number: i64) -> Value {
    let n = chapter_number.max(1);
    let pos = (n - 1).rem_euclid(15) as usize;
    let cycle = (n - 1) / 15 + 1;
    let phases = micro_arc_phases();
    let idx = if pos <= 2 {
        0
    } else if pos <= 6 {
        1
    } else if pos <= 10 {
        2
    } else {
        3
    };
    let phase = &phases[idx];
    let offset = phase["offset"].as_u64().unwrap_or(0) as usize;
    let span = phase["span"].as_u64().unwrap_or(1) as usize;
    let step_in_phase = pos - offset + 1;
    json!({
        "chapter_number": n,
        "cycle": cycle,
        "phase_index": idx,
        "phase_id": phase["id"],
        "phase_label": phase["label"],
        "phase_chapters": phase["chapters"],
        "step_in_phase": step_in_phase,
        "phase_span": span,
        "goals": phase["goals"],
    })
}

/// Ending suggestions for the next chapter: the six types minus the ones used
/// in the recent window (blueprint rule 3.2.1 — never the same high-intensity
/// ending repeatedly). `recent` is ordered oldest → newest.
pub fn suggest_endings(recent: &[String]) -> Vec<Value> {
    let window = recent.iter().rev().take(3);
    let blocked: Vec<&str> = window.map(|s| s.as_str()).collect();
    ending_types()
        .into_iter()
        .filter(|e| !blocked.contains(&e["id"].as_str().unwrap_or("")))
        .collect()
}

/// Additive storage: per-chapter QC scores, ending type and notes.
pub fn ensure_retention_schema(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS chapter_retention (
            book_id INTEGER NOT NULL,
            chapter_number INTEGER NOT NULL,
            ending_type TEXT NOT NULL DEFAULT '',
            scores_json TEXT NOT NULL DEFAULT '[]',
            total INTEGER NOT NULL DEFAULT -1,
            band TEXT NOT NULL DEFAULT '',
            notes TEXT NOT NULL DEFAULT '',
            updated_at INTEGER,
            PRIMARY KEY (book_id, chapter_number)
        );
        CREATE INDEX IF NOT EXISTS idx_chapter_retention_book
            ON chapter_retention(book_id);",
    )
    .map_err(|e| AppError::storage(format!("retention schema: {e}")))?;
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Command handlers (§4.15)
// ─────────────────────────────────────────────────────────────────────────────

fn book_exists(conn: &Connection, book_id: i64) -> Result<Option<String>, AppError> {
    conn.query_row(
        "SELECT title FROM books WHERE id = ?1",
        [book_id],
        |r| r.get::<_, String>(0),
    )
    .map(Some)
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        other => Err(AppError::from(other)),
    })
}

fn require_book(conn: &Connection, book_id: i64) -> Result<String, AppError> {
    book_exists(conn, book_id)?.ok_or_else(|| {
        AppError::not_found(format!("book {book_id} does not exist in this bundle"))
    })
}

/// Ending types used by chapters < the given one, oldest → newest.
fn ending_history(conn: &Connection, book_id: i64, before: i64) -> Result<Vec<String>, AppError> {
    let mut stmt = conn
        .prepare(
            "SELECT ending_type FROM chapter_retention
             WHERE book_id = ?1 AND chapter_number < ?2 AND ending_type <> ''
             ORDER BY chapter_number",
        )
        .map_err(AppError::from)?;
    let rows = stmt
        .query_map(rusqlite::params![book_id, before], |r| r.get::<_, String>(0))
        .map_err(AppError::from)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(AppError::from)?);
    }
    Ok(out)
}

/// #63 craft_retention_check — QC a drafted chapter against the SFB-2.0
/// rubric (7 × 0–2), persist the verdict and steer the next chapter.
/// Writer (lease enforced). Storage: upsert into `chapter_retention`.
pub fn craft_retention_check(ctx: &mut crate::ipc::Ctx, a: &Value) -> crate::error::IpcResult {
    let book_id = args::req_i64(a, "book_id")?;
    let chapter_number = args::req_i64(a, "chapter_number")?;
    if chapter_number < 1 {
        return Err(AppError::craft(format!(
            "chapter_number must be ≥ 1, got {chapter_number}"
        )));
    }
    let title = require_book(&ctx.conn, book_id)?;
    let chapter_id: i64 = ctx
        .conn
        .query_row(
            "SELECT id FROM chapters WHERE book_id = ?1 AND chapter_number = ?2",
            rusqlite::params![book_id, chapter_number],
            |r| r.get(0),
        )
        .map_err(|_| {
            AppError::not_found(format!(
                "chapter {chapter_number} does not exist in book {book_id} — QC applies to drafted chapters"
            ))
        })?;

    let scores_v = a
        .get("scores")
        .and_then(|v| v.as_array())
        .ok_or_else(|| AppError::craft("missing required array arg `scores` (7 entries, 0–2 each)"))?;
    let mut scores: Vec<i64> = Vec::with_capacity(scores_v.len());
    for v in scores_v {
        let n = v.as_i64().ok_or_else(|| {
            AppError::craft("scores entries must be integers 0–2")
        })?;
        scores.push(n);
    }
    let verdict = score_chapter(&scores).map_err(AppError::craft)?;

    let ending_type = match args::get_str(a, "ending_type") {
        Some(t) if !t.is_empty() => {
            if !ending_types().iter().any(|e| e["id"] == t.as_str()) {
                return Err(AppError::craft(format!(
                    "unknown ending_type `{t}` — valid: danger, emotional_revelation, mystery_discovery, irreversible_choice, reversal, anticipation"
                )));
            }
            t
        }
        _ => String::new(),
    };
    let notes = args::get_str(a, "notes").unwrap_or_default();
    if notes.chars().count() > 2000 {
        return Err(AppError::craft("notes limited to 2000 characters"));
    }

    ensure_retention_schema(&ctx.conn)?;
    let scores_json = serde_json::to_string(&verdict["scores"]).unwrap_or_else(|_| "[]".into());
    ctx.conn.execute(
        "INSERT INTO chapter_retention(book_id, chapter_number, ending_type, scores_json,
                                      total, band, notes, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, strftime('%s','now'))
         ON CONFLICT(book_id, chapter_number) DO UPDATE SET
           ending_type = excluded.ending_type,
           scores_json = excluded.scores_json,
           total = excluded.total,
           band = excluded.band,
           notes = excluded.notes,
           updated_at = excluded.updated_at",
        rusqlite::params![
            book_id,
            chapter_number,
            ending_type,
            scores_json,
            verdict["total"].as_i64().unwrap_or(-1),
            verdict["band"].as_str().unwrap_or(""),
            notes
        ],
    )
    .map_err(AppError::from)?;

    // Suggestions for the NEXT chapter see the ending just saved.
    let mut history = ending_history(&ctx.conn, book_id, chapter_number + 1)?;
    history.truncate(10);
    let suggestions = suggest_endings(&history);
    let next_phase = phase_for(chapter_number + 1);

    ctx.after_commit()?;
    Ok(json!({
        "book_id": book_id,
        "book_title": title,
        "chapter_number": chapter_number,
        "chapter_id": chapter_id,
        "qc": verdict,
        "ending_type": ending_type,
        "next_chapter": {
            "number": chapter_number + 1,
            "phase": next_phase,
            "suggested_endings": suggestions,
        },
        "blueprint": BLUEPRINT_ID,
    }))
}

/// #64 craft_retention_report — serialization status: per-chapter QC verdicts,
/// ending rotation, micro-arc phase map, word-count conformance and the
/// cadence/buffer arithmetic. Read-only.
pub fn craft_retention_report(ctx: &mut crate::ipc::Ctx, a: &Value) -> crate::error::IpcResult {
    let book_id = args::req_i64(a, "book_id")?;
    let title = require_book(&ctx.conn, book_id)?;
    let cpw = args::get_f64(a, "chapters_per_week").unwrap_or(2.5).clamp(0.5, 7.0);

    ensure_retention_schema(&ctx.conn)?;
    let mut stmt = ctx
        .conn
        .prepare(
            "SELECT c.chapter_number, c.title, c.status, c.actual_word_count,
                    r.total, r.band, r.ending_type, r.notes, r.updated_at
             FROM chapters c
             LEFT JOIN chapter_retention r
               ON r.book_id = c.book_id AND r.chapter_number = c.chapter_number
             WHERE c.book_id = ?1
             ORDER BY c.chapter_number",
        )
        .map_err(AppError::from)?;
    let rows = stmt
        .query_map([book_id], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, Option<i64>>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, Option<String>>(7)?,
                r.get::<_, Option<i64>>(8)?,
            ))
        })
        .map_err(AppError::from)?;

    let mut chapters: Vec<Value> = Vec::new();
    let mut bands = json!({"STRONG":0,"EXAMINE":0,"STRUCTURAL_REVISION":0,"RECONSIDER":0});
    let mut scored = 0i64;
    let mut revision_required = 0i64;
    let mut endings_histogram = serde_json::Map::new();
    for row in rows {
        let (num, ch_title, status, words, total, band, ending, notes, updated) = row.map_err(AppError::from)?;
        let length_guidance = if words <= 0 {
            "unwritten"
        } else if (1000..=2000).contains(&words) {
            "in_range"
        } else if words < 1000 {
            "short"
        } else {
            "long"
        };
        let mut entry = json!({
            "chapter_number": num,
            "title": ch_title,
            "status": status,
            "word_count": words,
            "word_length_guidance": length_guidance,
            "phase": phase_for(num),
        });
        if let Some(t) = total {
            scored += 1;
            if let Some(b) = band.as_deref() {
                if let Some(c) = bands[b].as_i64() {
                    bands[b] = json!(c + 1);
                }
            }
            let flags = serde_json::from_str::<Vec<i64>>(
                &ctx.conn
                    .query_row(
                        "SELECT scores_json FROM chapter_retention WHERE book_id=?1 AND chapter_number=?2",
                        rusqlite::params![book_id, num],
                        |r| r.get::<_, String>(0),
                    )
                    .unwrap_or_else(|_| "[]".into()),
            )
            .unwrap_or_default();
            let zero_flagged = flags
                .iter()
                .zip(QC_CRITERIA.iter())
                .any(|(s, c)| *s == 0 && ZERO_FLAG_CRITERIA.contains(&*c));
            if zero_flagged {
                revision_required += 1;
            }
            entry["qc"] = json!({
                "total": t, "band": band, "revision_required": zero_flagged,
                "ending_type": ending, "notes": notes, "updated_at": updated,
            });
            if let Some(e) = ending.as_deref().filter(|e| !e.is_empty()) {
                let count = endings_histogram.get(e).and_then(|v| v.as_i64()).unwrap_or(0);
                endings_histogram.insert(e.to_string(), json!(count + 1));
            }
        }
        chapters.push(entry);
    }

    // Cadence / buffer arithmetic (status pipeline is the source of truth).
    let (mut reserve, mut published, mut planned) = (0i64, 0i64, 0i64);
    for c in &chapters {
        match c["status"].as_str().unwrap_or("") {
            "PUBLISHED" => published += 1,
            "DRAFTING" | "POLISHED" | "AUDIO_READY" => reserve += 1,
            _ => planned += 1,
        }
    }
    let buffer_weeks = if cpw > 0.0 {
        (reserve as f64 / cpw * 10.0).round() / 10.0
    } else {
        0.0
    };
    let max_chapter = chapters.last().map(|c| c["chapter_number"].as_i64().unwrap_or(0)).unwrap_or(0);

    // Ending rotation over the recent window.
    let history = ending_history(&ctx.conn, book_id, i64::MAX)?;
    let recent: Vec<String> = history.iter().rev().take(3).rev().cloned().collect();
    let repeated: Vec<String> = {
        let mut seen: Vec<String> = Vec::new();
        let mut dup: Vec<String> = Vec::new();
        for e in &recent {
            if seen.contains(e) && !dup.contains(e) {
                dup.push(e.clone());
            }
            seen.push(e.clone());
        }
        dup
    };

    Ok(json!({
        "book_id": book_id,
        "book_title": title,
        "chapters_per_week": cpw,
        "chapters": chapters,
        "cadence": {
            "reserve_chapters": reserve,
            "published_chapters": published,
            "planned_chapters": planned,
            "buffer_weeks": buffer_weeks,
            "buffer_healthy": reserve >= 4,
            "guidance": cadence(),
        },
        "summary": {
            "chapters_total": chapters.len(),
            "qc_scored": scored,
            "bands": bands,
            "revision_required": revision_required,
            "endings_used": endings_histogram,
            "recent_endings": recent,
            "ending_repetition_in_window": repeated,
            "max_chapter": max_chapter,
            "next_phase": phase_for(max_chapter + 1),
        },
        "blueprint": BLUEPRINT_ID,
    }))
}

/// #65 craft_retention_worksheet — pre-draft checklist for a chapter (which
/// need not exist yet), fused with book state: craft preset, arc beat stamp,
/// micro-arc phase, ending rotation and previous QC. Read-only.
pub fn craft_retention_worksheet(ctx: &mut crate::ipc::Ctx, a: &Value) -> crate::error::IpcResult {
    let book_id = args::req_i64(a, "book_id")?;
    let chapter_number = args::req_i64(a, "chapter_number")?;
    if chapter_number < 1 {
        return Err(AppError::craft(format!(
            "chapter_number must be ≥ 1, got {chapter_number}"
        )));
    }
    let title = require_book(&ctx.conn, book_id)?;
    ensure_retention_schema(&ctx.conn)?;

    // Craft context: preset + arc beat stamp + persona directive.
    let state = crate::craft::engine::get_state(&ctx.conn, book_id)?;
    let arc = crate::craft::catalog::arc_by_id(state["plot_arc_id"].as_str().unwrap_or("SAVE_THE_CAT"));
    let beat_stamp: Option<(String, String)> = ctx
        .conn
        .query_row(
            "SELECT chapter_beats.beat_id, chapter_beats.beat_label FROM chapter_beats
             JOIN chapters ON chapters.id = chapter_beats.chapter_id
             WHERE chapters.book_id = ?1 AND chapters.chapter_number = ?2
             ORDER BY chapter_beats.beat_index LIMIT 1",
            rusqlite::params![book_id, chapter_number],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .ok();
    let persona = crate::craft::catalog::persona_by_id(
        state["style_persona_id"].as_str().unwrap_or("EDGE_THRILLER"),
    );

    // Chapter row (may not exist yet — planning ahead is the point).
    let chapter: Option<(String, i64, i64, String)> = ctx
        .conn
        .query_row(
            "SELECT title, COALESCE(target_word_count,0), actual_word_count, status
             FROM chapters WHERE book_id = ?1 AND chapter_number = ?2",
            rusqlite::params![book_id, chapter_number],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, String>(3)?,
                ))
            },
        )
        .ok();

    // Ending rotation + previous QC.
    let history = ending_history(&ctx.conn, book_id, chapter_number)?;
    let recent: Vec<String> = history.iter().rev().take(3).rev().cloned().collect();
    let suggestions = suggest_endings(&recent);
    let previous_qc: Option<(i64, String, bool)> = ctx
        .conn
        .query_row(
            "SELECT total, band, scores_json FROM chapter_retention
             WHERE book_id = ?1 AND chapter_number = ?2",
            rusqlite::params![book_id, chapter_number],
            |r| {
                let total: i64 = r.get(0)?;
                let band: String = r.get(1)?;
                let flags: String = r.get(2)?;
                let scores: Vec<i64> = serde_json::from_str(&flags).unwrap_or_default();
                let zero = scores
                    .iter()
                    .zip(QC_CRITERIA.iter())
                    .any(|(s, c)| *s == 0 && ZERO_FLAG_CRITERIA.contains(&*c));
                Ok((total, band, zero))
            },
        )
        .ok();

    let phase = phase_for(chapter_number);
    let beat_context = match &beat_stamp {
        Some((id, label)) => format!("Arc beat stamp: `{label}` ({id})."),
        None => "No arc beat stamp — run craft_scaffold or set one via the arc plan.".into(),
    };

    let mut sections = worksheet_sections();
    // Fuse live context into each section (additive `context` field).
    for s in sections.iter_mut() {
        let ctx_line = match s["id"].as_str().unwrap_or("") {
            "narrative_objective" => format!(
                "{beat_context} Micro-arc phase: {} (step {} of {}, chapter {} of cycle {}).",
                phase["phase_label"].as_str().unwrap_or(""),
                phase["step_in_phase"].as_i64().unwrap_or(0),
                phase["phase_span"].as_i64().unwrap_or(0),
                phase["chapter_number"].as_i64().unwrap_or(0),
                phase["cycle"].as_i64().unwrap_or(0)
            ),
            "character_development" => format!(
                "Style persona: {} — {}",
                persona.as_ref().map(|p| p["label"].as_str().unwrap_or("")).unwrap_or(""),
                persona.as_ref().map(|p| p["directive"].as_str().unwrap_or("")).unwrap_or("")
            ),
            "plot_development" => {
                let goals: Vec<String> = phase["goals"]
                    .as_array()
                    .map(|g| g.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
                    .unwrap_or_default();
                format!("Phase goals: {}", goals.join(" "))
            }
            "reader_experience" => format!(
                "Blueprint word-range for serial chapters: 1000–2000 (project target may differ). Cadence: 2–3 chapters/week with a 4–8 chapter buffer."
            ),
            "chapter_ending" => format!(
                "Selection rules: {}. Suggested types now: {}.",
                ending_rules().join(" "),
                suggestions
                    .iter()
                    .map(|e| e["id"].as_str().unwrap_or(""))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            _ => String::new(),
        };
        s["context"] = json!(ctx_line);
    }

    Ok(json!({
        "book_id": book_id,
        "book_title": title,
        "chapter_number": chapter_number,
        "chapter_exists": chapter.is_some(),
        "chapter": chapter.map(|(t, target, words, status)| json!({
            "title": t, "target_word_count": target,
            "actual_word_count": words, "status": status,
        })),
        "phase": phase,
        "arc": arc.as_ref().map(|a| json!({"id": a["id"], "label": a["label"]})),
        "beat_stamp": beat_stamp.map(|(id, label)| json!({"beat_id": id, "beat_label": label})),
        "previous_qc": previous_qc.map(|(total, band, zero)| json!({
            "total": total, "band": band, "revision_required": zero,
        })),
        "ending_rotation": {
            "recent": recent,
            "suggested": suggestions,
            "blocked_window": 3,
        },
        "sections": sections,
        "qc_criteria": QC_CRITERIA,
        "blueprint": BLUEPRINT_ID,
    }))
}

// ─────────────────────────────────────────────────────────────────────────────
// tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoring_bands_and_totals() {
        let perfect = score_chapter(&[2, 2, 2, 2, 2, 2, 2]).unwrap();
        assert_eq!(perfect["total"], 14);
        assert_eq!(perfect["band"], "STRONG");
        assert_eq!(perfect["revision_required"], false);

        let mid = score_chapter(&[1, 1, 1, 2, 1, 1, 2]).unwrap();
        assert_eq!(mid["total"], 9);
        assert_eq!(mid["band"], "EXAMINE");

        let low = score_chapter(&[1, 0, 1, 1, 0, 1, 1]).unwrap();
        assert_eq!(low["total"], 5);
        assert_eq!(low["band"], "RECONSIDER");

        let structural = score_chapter(&[1, 1, 1, 0, 1, 1, 1]).unwrap();
        assert_eq!(structural["total"], 6);
        assert_eq!(structural["band"], "STRUCTURAL_REVISION");
    }

    #[test]
    fn zero_flags_force_revision_even_when_total_is_strong() {
        // 12/14 total (STRONG band) but narrative_objective = 0.
        let v = score_chapter(&[2, 0, 2, 2, 2, 2, 2]).unwrap();
        assert_eq!(v["total"], 12);
        assert_eq!(v["band"], "STRONG");
        assert_eq!(v["revision_required"], true);
        assert_eq!(v["zero_flags"], json!(["narrative_objective"]));

        // Both zero-flag criteria at once.
        let v2 = score_chapter(&[2, 0, 2, 2, 2, 0, 2]).unwrap();
        assert_eq!(v2["zero_flags"], json!(["narrative_objective", "chapter_payoff"]));

        // A zero on a NON-flag criterion never forces revision.
        let v3 = score_chapter(&[0, 2, 2, 2, 2, 2, 2]).unwrap();
        assert_eq!(v3["revision_required"], false);
    }

    #[test]
    fn scoring_rejects_bad_input() {
        assert!(score_chapter(&[2, 2, 2]).is_err()); // wrong length
        assert!(score_chapter(&[2, 2, 2, 2, 2, 2, 3]).is_err()); // out of range
        assert!(score_chapter(&[2, 2, 2, 2, 2, 2, -1]).is_err());
        // All zeros is legal input (total 0 → RECONSIDER band).
        assert_eq!(score_chapter(&[0, 0, 0, 0, 0, 0, 0]).unwrap()["band"], "RECONSIDER");
    }

    #[test]
    fn micro_arc_phase_mapping_wraps_every_15() {
        let p = |n: i64| phase_for(n)["phase_id"].take().as_str().unwrap_or("").to_string();
        assert_eq!(p(1), "setup");
        assert_eq!(p(3), "setup");
        assert_eq!(p(4), "escalation");
        assert_eq!(p(7), "escalation");
        assert_eq!(p(8), "reversal_pivot");
        assert_eq!(p(11), "reversal_pivot");
        assert_eq!(p(12), "climax_resolution");
        assert_eq!(p(15), "climax_resolution");
        // Cycle 2 wraps back to setup.
        assert_eq!(p(16), "setup");
        assert_eq!(p(30), "climax_resolution");
        let detail = phase_for(16);
        assert_eq!(detail["cycle"], 2);
        assert_eq!(detail["step_in_phase"], 1);
        // Non-positive chapter numbers clamp to 1.
        assert_eq!(phase_for(0)["chapter_number"], 1);
    }

    #[test]
    fn ending_suggestions_block_recent_window() {
        let all = suggest_endings(&[]);
        assert_eq!(all.len(), 6);
        let one = suggest_endings(&["danger".into()]);
        assert_eq!(one.len(), 5);
        assert!(all.iter().any(|e| e["id"] == "danger"));
        assert!(!one.iter().any(|e| e["id"] == "danger"));
        // The window is the last three (ordered oldest → newest).
        let three = suggest_endings(&[
            "danger".into(),
            "reversal".into(),
            "anticipation".into(),
            "danger".into(),
        ]);
        // Window = last 3 = [reversal, anticipation, danger] → 3 remain.
        assert_eq!(three.len(), 3);
        assert!(!three.iter().any(|e| e["id"] == "danger"));
        assert!(!three.iter().any(|e| e["id"] == "reversal"));
        assert!(three.iter().any(|e| e["id"] == "irreversible_choice"));
    }

    #[test]
    fn blueprint_payload_is_complete() {
        let bp = blueprint_json();
        assert_eq!(bp["blueprint"], BLUEPRINT_ID);
        assert_eq!(bp["counts"]["chapter_beats"], 7);
        assert_eq!(bp["counts"]["ending_types"], 6);
        assert_eq!(bp["counts"]["micro_arc_phases"], 4);
        assert_eq!(bp["counts"]["qc_criteria"], 7);
        assert_eq!(bp["counts"]["worksheet_sections"], 5);
        for e in bp["ending_types"].as_array().unwrap() {
            assert!(!e["example"].as_str().unwrap_or("").is_empty());
            assert!(!e["when"].as_str().unwrap_or("").is_empty());
        }
        for b in bp["chapter_beats"].as_array().unwrap() {
            assert!(!b["purpose"].as_str().unwrap_or("").is_empty());
        }
        // Zero-flag criteria must be real QC criteria.
        for f in bp["zero_flag_criteria"].as_array().unwrap() {
            assert!(QC_CRITERIA.contains(&f.as_str().unwrap()));
        }
    }

    #[test]
    fn retention_schema_is_idempotent_and_keyed() {
        let conn = Connection::open_in_memory().unwrap();
        ensure_retention_schema(&conn).unwrap();
        ensure_retention_schema(&conn).unwrap(); // second run stays green
        conn.execute(
            "INSERT INTO chapter_retention(book_id, chapter_number, ending_type,
                                           scores_json, total, band)
             VALUES (1, 1, 'danger', '[2,2,2,2,2,2,2]', 14, 'STRONG')",
            [],
        )
        .unwrap();
        // Primary key (book_id, chapter_number) upserts cleanly; the partial
        // DO UPDATE only moves `band`, so the original total (14) survives.
        conn.execute(
            "INSERT INTO chapter_retention(book_id, chapter_number, ending_type,
                                           scores_json, total, band)
             VALUES (1, 1, 'reversal', '[1,2,2,2,2,2,2]', 13, 'EXAMINE')
             ON CONFLICT(book_id, chapter_number) DO UPDATE SET band = excluded.band",
            [],
        )
        .unwrap();
        let (band, total): (String, i64) = conn
            .query_row(
                "SELECT band, total FROM chapter_retention WHERE book_id = 1 AND chapter_number = 1",
                [],
                |r| Ok((r.get(0).unwrap(), r.get(1).unwrap())),
            )
            .unwrap();
        assert_eq!(band, "EXAMINE");
        assert_eq!(total, 14);
    }
}
