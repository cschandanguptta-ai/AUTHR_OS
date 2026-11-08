// AUTHR_OS — proper-noun IPA phonetics (Phase 4, Sprint 10).
//
// Two-tier lookup:
//   1. curated lexicon of frequent fantasy/sci-fi character names,
//   2. a rule-based English G2P fallback (digraphs, vowel sets, silent-e,
//      soft c/g) producing approximate IPA.
// The result feeds TTS prosody hints and the `audio_pronounce` IPC command.

use serde_json::{json, Value};

/// Curated lexicon: `lower-case name → IPA`.
pub const LEXICON: &[(&str, &str)] = &[
    ("kira", "ˈkɪərə"),
    ("mira", "ˈmɪərə"),
    ("aria", "ˈɑːriə"),
    ("liara", "liˈɑːrə"),
    ("nyx", "nɪks"),
    ("orin", "ˈɒrɪn"),
    ("thorne", "θɔːrn"),
    ("aelia", "ˈiːliə"),
    ("kael", "keɪl"),
    ("sylas", "ˈsaɪləs"),
    ("vex", "vɛks"),
    ("juno", "ˈdʒuːnoʊ"),
    ("atlas", "ˈætləs"),
    ("echo", "ˈɛkoʊ"),
    ("iris", "ˈaɪrɪs"),
    ("zara", "ˈzɑːrə"),
    ("fenrir", "ˈfɛnrɪr"),
    ("aurelia", "ɔːˈriːliə"),
    ("cassius", "ˈkæʃəs"),
    ("draven", "ˈdreɪvən"),
    ("elara", "ɪˈlɑːrə"),
    ("luna", "ˈluːnə"),
    ("orpheus", "ˈɔːrfiːəs"),
    ("seraphine", "ˈsɛrəfiːn"),
    ("xander", "ˈzændər"),
    ("yara", "ˈjɑːrə"),
    ("narrator", "ˈnærreɪtər"),
];

fn is_vowel(c: char) -> bool {
    matches!(c, 'a' | 'e' | 'i' | 'o' | 'u')
}

/// Rule-based fallback G2P for unknown proper nouns. Deliberately simple and
/// deterministic; documented as heuristic (not a neural phonemizer).
pub fn rule_g2p(word: &str) -> String {
    let w: Vec<char> = word.to_lowercase().chars().collect();
    let n = w.len();
    let mut out = String::new();
    let mut i = 0usize;
    let has_final_e = n >= 2 && w[n - 1] == 'e' && !is_vowel(w[n - 2]);
    let end = if has_final_e { n - 1 } else { n };
    while i < end {
        let two: String = w[i..(i + 2).min(end)].iter().collect();
        let c = w[i];
        // Digraphs first.
        match two.as_str() {
            "th" => { out.push('θ'); i += 2; continue; }
            "sh" => { out.push('ʃ'); i += 2; continue; }
            "ch" => { out.push_str("tʃ"); i += 2; continue; }
            "ph" => { out.push('f'); i += 2; continue; }
            "wh" => { out.push('w'); i += 2; continue; }
            "ck" => { out.push('k'); i += 2; continue; }
            "ng" => { out.push('ŋ'); i += 2; continue; }
            "qu" => { out.push_str("kw"); i += 2; continue; }
            "gh" => { out.push('g'); i += 2; continue; }
            _ => {}
        }
        if i + 1 < end && is_vowel(c) && w[i + 1] == c {
            // doubled vowel → long sound
            let ipa = match c {
                'e' => "iː",
                'o' | 'u' => "uː",
                'a' => "ɑː",
                'i' => "aɪ",
                _ => "",
            };
            out.push_str(ipa);
            i += 2;
            continue;
        }
        if i + 1 < end && is_vowel(c) && is_vowel(w[i + 1]) {
            let ipa = match two.as_str() {
                "ee" | "ea" => "iː",
                "oo" => "uː",
                "ou" | "ow" => "aʊ",
                "oi" | "oy" => "ɔɪ",
                "ai" | "ay" | "ei" | "ey" => "eɪ",
                "au" | "aw" => "ɔː",
                "ie" => "iː",
                "eu" => "juː",
                _ => "",
            };
            if !ipa.is_empty() {
                out.push_str(ipa);
                i += 2;
                continue;
            }
        }
        // Single characters.
        let one: String = match c {
            'a' => {
                if has_final_e && i + 2 == end { "eɪ" } else { "æ" }.to_string()
            }
            'e' => {
                if has_final_e && i + 2 == end { "iː" } else { "ɛ" }.to_string()
            }
            'i' => {
                if has_final_e && i + 2 == end { "aɪ" } else { "ɪ" }.to_string()
            }
            'o' => {
                if has_final_e && i + 2 == end { "oʊ" } else { "ɑ" }.to_string()
            }
            'u' => {
                if has_final_e && i + 2 == end { "juː" } else { "ʌ" }.to_string()
            }
            'y' => {
                if i == 0 {
                    "j"
                } else if i + 1 == end {
                    "i"
                } else {
                    "ɪ"
                }
                .to_string()
            }
            'c' => {
                if i + 1 < end && matches!(w[i + 1], 'e' | 'i' | 'y') {
                    "s"
                } else {
                    "k"
                }
                .to_string()
            }
            'g' => {
                if i + 1 < end && matches!(w[i + 1], 'e' | 'i' | 'y') {
                    "dʒ"
                } else {
                    "ɡ"
                }
                .to_string()
            }
            's' => {
                if i + 1 == end && i > 0 && is_vowel(w[i - 1]) {
                    "z"
                } else {
                    "s"
                }
                .to_string()
            }
            'x' => "ks".to_string(),
            'j' => "dʒ".to_string(),
            other => other.to_string(),
        };
        out.push_str(&one);
        i += 1;
    }
    if out.is_empty() {
        out.push_str(word);
    }
    out
}

/// Look up the IPA rendering of a proper noun.
pub fn to_ipa(name: &str) -> (String, &'static str) {
    let lower = name.to_lowercase();
    for (k, v) in LEXICON {
        if *k == lower {
            return (v.to_string(), "lexicon");
        }
    }
    (rule_g2p(name), "rules")
}

/// `audio_pronounce` payload — resolve a batch of names.
pub fn pronounce(names: &[String]) -> Value {
    let entries: Vec<Value> = names
        .iter()
        .map(|name| {
            let (ipa, source) = to_ipa(name);
            json!({ "name": name, "ipa": ipa, "source": source })
        })
        .collect();
    json!({ "entries": entries, "count": entries.len() })
}

/// Substitute known proper nouns inside a synthesis string with
/// `<phoneme>`-ready slash notation: `Kira → /ˈkɪərə/`.
pub fn substitute_text(text: &str, names: &[String]) -> (String, usize) {
    let mut out = text.to_string();
    let mut applied = 0usize;
    for name in names {
        if name.is_empty() {
            continue;
        }
        let (ipa, _) = to_ipa(name);
        let replaced = out.replace(name, &format!("/{ipa}/"));
        if replaced != out {
            applied += replaced.matches(&format!("/{ipa}/")).count();
            out = replaced;
        }
    }
    (out, applied)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexicon_hits() {
        let (ipa, src) = to_ipa("Kira");
        assert_eq!(ipa, "ˈkɪərə");
        assert_eq!(src, "lexicon");
    }

    #[test]
    fn rules_fallback_digraphs() {
        let (ipa, src) = to_ipa("Thax");
        assert_eq!(src, "rules");
        assert!(ipa.starts_with('θ'), "ipa={ipa}");
        assert!(ipa.ends_with("ks"), "ipa={ipa}");
    }

    #[test]
    fn silent_e_and_soft_c() {
        assert_eq!(rule_g2p("Cade"), "keɪd");
        assert_eq!(rule_g2p("Cila"), "sɪlæ");
    }

    #[test]
    fn substitute_counts_occurrences() {
        let (text, n) = substitute_text("Kira met Kira.", &["Kira".into()]);
        assert_eq!(n, 2);
        assert!(text.contains("/ˈkɪərə/"));
    }
}
