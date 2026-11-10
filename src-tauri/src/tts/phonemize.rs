// AUTHR_OS — Local G2P phonemization (build plan §6.5, Gap 5):
//  1. Proper-noun substitution against `character_profiles` and `lorebook`
//     (any token/phrase with a non-empty `ipa_phonetic` wins verbatim).
//  2. Rule-based English G2P fallback (digraph + single-letter IPA rules).
//  3. Fallback mode: an unresolvable word passes through raw without halting
//     synthesis.

use crate::error::AppError;
use crate::ipc::{Ctx, args};
use serde_json::{Value, json};

/// Ordered digraph rules (first match wins), then single letters.
const DIGRAPHS: &[(&str, &str)] = &[
    ("tch", "tʃ"), ("igh", "aɪ"), ("ing", "ɪŋ"),
    ("ch", "tʃ"), ("sh", "ʃ"), ("th", "θ"), ("ph", "f"), ("wh", "w"),
    ("ck", "k"), ("ng", "ŋ"), ("qu", "kw"),
    ("ee", "iː"), ("ea", "iː"), ("oo", "uː"), ("ou", "aʊ"), ("ow", "aʊ"),
    ("ai", "eɪ"), ("ay", "eɪ"), ("oa", "oʊ"), ("oi", "ɔɪ"), ("oy", "ɔɪ"),
    ("au", "ɔː"), ("aw", "ɔː"), ("ie", "aɪ"), ("ei", "eɪ"), ("ey", "eɪ"),
    ("ar", "ɑːr"), ("or", "ɔːr"), ("ir", "ɜːr"), ("ur", "ɜːr"), ("er", "ɜːr"),
];

const LETTERS: &[(char, &str)] = &[
    ('a', "æ"), ('b', "b"), ('c', "k"), ('d', "d"), ('e', "ɛ"), ('f', "f"),
    ('g', "g"), ('h', "h"), ('i', "ɪ"), ('j', "dʒ"), ('k', "k"), ('l', "l"),
    ('m', "m"), ('n', "n"), ('o', "ɒ"), ('p', "p"), ('q', "k"), ('r', "r"),
    ('s', "s"), ('t', "t"), ('u', "ʌ"), ('v', "v"), ('w', "w"), ('x', "ks"),
    ('y', "i"), ('z', "z"),
];

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub word: String,
    pub ipa: String,
    pub source: String, // profile | lore | rules | raw
}

/// Rule-based G2P for one lowercase word (digraph table first, then letters).
pub fn rules_g2p(word: &str) -> String {
    let lower: String = word
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .collect::<String>()
        .to_lowercase();
    let mut chars: Vec<char> = lower.chars().collect();
    // trailing silent 'e': "gate" → g·æt·(silent); keep it after a vowel
    // ("see", "die") and in two-letter words ("be", "he")
    if chars.len() > 2 && chars.last() == Some(&'e') {
        let prev = chars[chars.len() - 2];
        let is_vowel = "aeiouy".contains(prev);
        if !is_vowel {
            chars.pop();
        }
    }
    let mut out = String::new();
    let mut i = 0usize;
    while i < chars.len() {
        let rest: String = chars[i..].iter().collect();
        if let Some((g, ipa)) = DIGRAPHS.iter().find(|(g, _)| rest.starts_with(g)) {
            out.push_str(ipa);
            i += g.chars().count();
            continue;
        }
        let c = chars[i];
        if let Some((_, ipa)) = LETTERS.iter().find(|(l, _)| *l == c) {
            out.push_str(ipa);
        }
        i += 1;
    }
    if out.is_empty() {
        word.to_string()
    } else {
        out
    }
}

/// Proper-noun dictionaries from the bundle DB: single tokens and phrases.
fn load_dictionary(conn: &rusqlite::Connection) -> Vec<(String, String, String)> {
    // (match_text_lower, ipa, source)
    let mut dict = Vec::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT character_key, display_name, ipa_phonetic FROM character_profiles
         WHERE ipa_phonetic != ''",
    ) {
        if let Ok(rows) = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        }) {
            for row in rows.flatten() {
                let (key, display, ipa) = row;
                dict.push((key.to_lowercase(), ipa.clone(), "profile".to_string()));
                for w in display.split_whitespace() {
                    dict.push((w.trim_matches(|c: char| !c.is_ascii_alphanumeric()).to_lowercase(),
                               ipa.clone(), "profile".to_string()));
                }
            }
        }
    }
    if let Ok(mut stmt) = conn.prepare("SELECT key, ipa_phonetic FROM lorebook WHERE ipa_phonetic != ''") {
        if let Ok(rows) = stmt.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        }) {
            for (key, ipa) in rows.flatten() {
                dict.push((key.to_lowercase(), ipa, "lore".to_string()));
            }
        }
    }
    dict.sort_by(|a, b| b.0.len().cmp(&a.0.len())); // longest match first
    dict
}

pub fn phonemize(conn: &rusqlite::Connection, text: &str) -> Vec<Token> {
    let dict = load_dictionary(conn);
    let lower = text.to_lowercase();
    let mut spans: Vec<(usize, usize, String, String, String)> = Vec::new(); // (start,end,ipa,matched,source)

    // 1. phrase/token dictionary matches (longest first, non-overlapping)
    for (match_text, ipa, source) in &dict {
        if match_text.is_empty() {
            continue;
        }
        let mut from = 0usize;
        while let Some(pos) = lower[from..].find(match_text.as_str()) {
            let start = from + pos;
            let end = start + match_text.len();
            let boundary_ok = |start: usize, end: usize| {
                let before_ok = start == 0 || !lower.as_bytes()[start - 1].is_ascii_alphanumeric();
                let after_ok = end >= lower.len() || !lower.as_bytes()[end].is_ascii_alphanumeric();
                before_ok && after_ok
            };
            let overlapped = spans.iter().any(|(s, e, _, _, _)| start < *e && *s < end);
            if boundary_ok(start, end) && !overlapped {
                spans.push((start, end, ipa.clone(), text[start..end].to_string(), source.clone()));
            }
            from = end.max(from + 1);
        }
    }
    spans.sort_by_key(|(s, _, _, _, _)| *s);

    // 2. fill the gaps with rule-based G2P / raw passthrough
    let mut tokens: Vec<Token> = Vec::new();
    let mut cursor = 0usize;
    for (start, end, ipa, matched, source) in spans {
        for raw in text[cursor..start].split_whitespace() {
            tokens.push(tokenize_word(raw));
        }
        tokens.push(Token { word: matched, ipa, source });
        cursor = end;
    }
    for raw in text[cursor..].split_whitespace() {
        tokens.push(tokenize_word(raw));
    }
    tokens
}

fn tokenize_word(raw: &str) -> Token {
    let core = raw.trim_matches(|c: char| !c.is_ascii_alphanumeric());
    if core.is_empty() || !core.chars().any(|c| c.is_ascii_alphabetic()) {
        // digits / pure punctuation → pass through raw (fallback mode §6.5.4)
        return Token { word: raw.to_string(), ipa: raw.to_string(), source: "raw".to_string() };
    }
    let ipa = rules_g2p(core);
    let source = if ipa == core { "raw" } else { "rules" };
    Token { word: raw.to_string(), ipa, source: source.to_string() }
}

/// IPC #42 `phonemize_text` — proper-noun IPA substitution preview.
pub fn phonemize_text(ctx: &Ctx, a: &Value) -> Result<Value, AppError> {
    let text = args::req_str(a, "text")?;
    let tokens = phonemize(&ctx.conn, &text);
    let phonemized: String = tokens.iter().map(|t| t.ipa.clone()).collect::<Vec<_>>().join(" ");
    Ok(json!({
        "text": text,
        "tokens": tokens.iter().map(|t| json!({
            "word": t.word, "ipa": t.ipa, "source": t.source,
        })).collect::<Vec<_>>(),
        "phonemized": phonemized,
        "proper_noun_hits": tokens.iter().filter(|t| t.source == "profile" || t.source == "lore").count(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digraph_rules() {
        assert_eq!(rules_g2p("ship"), "ʃɪp");
        assert_eq!(rules_g2p("chat"), "tʃæt");
        assert_eq!(rules_g2p("thin"), "θɪn");
        assert_eq!(rules_g2p("moon"), "muːn");
        assert_eq!(rules_g2p("see"), "siː");
        assert_eq!(rules_g2p("gate"), "gæt"); // silent trailing e dropped
        assert!(!rules_g2p("day").is_empty());
    }

    #[test]
    fn unresolvable_letters_pass_through() {
        let t = tokenize_word("XÆ'");
        assert_eq!(t.source, "rules");
    }

    #[test]
    fn syllable_guard_word_passthrough() {
        // pure punctuation word → raw
        let t = tokenize_word("—");
        assert_eq!(t.source, "raw");
        assert_eq!(t.ipa, "—");
    }
}
