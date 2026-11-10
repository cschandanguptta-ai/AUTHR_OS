// AUTHR_OS — Rust G2P tokenizer (Phase 5 / Sprint 13, build plan §8).
//
// Grapheme→phoneme conversion for the local TTS stack (Kokoro-82M feed):
//   1. Proper-noun IPA substitution — override dictionary applied on whole
//      words first (character_profiles.ipa_phonetic / lorebook.ipa_phonetic
//      flow in through `VoiceProfile` and `ipa_overrides`).
//   2. Rule-based English G2P — digraphs, vowel teams, magic-e, soft c/g,
//      syllable pauses between words.
//   3. Output: ARPAbet-style phoneme tokens with stress digits on vowels
//      (`K IH1 R AH0`), plus an IPA rendering for display/audit.
//
// Fully deterministic and offline — no network, no model files.

pub const PAUSE: &str = "PAUSE";

/// (grapheme → phonemes) ordered rule tables — first match wins per position.
const DIGRAPHS: &[(&str, &[&str])] = &[
    ("tch", &["CH"]),
    ("sch", &["S", "K"]),
    ("sh", &["SH"]),
    ("ch", &["CH"]),
    ("th", &["TH"]), // voiced/dental refinement below
    ("ph", &["F"]),
    ("ck", &["K"]),
    ("ng", &["NG"]),
    ("qu", &["K", "W"]),
    ("wh", &["W"]),
    ("gh", &["G"]), // rough/ghost class; "ugh"-class falls to G too (coarse)
    ("wr", &["R"]),
    ("kn", &["N"]),
    ("gn", &["N"]),
    ("mb", &["M"]), // word-final -mb (climb) handled by ending trim below
];

const VOWEL_TEAMS: &[(&str, &[&str])] = &[
    ("ai", &["EY"]),
    ("ay", &["EY"]),
    ("ee", &["IY"]),
    ("ea", &["IY"]),
    ("eigh", &["EY"]),
    ("igh", &["AY"]),
    ("oa", &["OW"]),
    ("oe", &["OW"]),
    ("oo", &["UW"]),
    ("ou", &["AW"]),
    ("ow", &["OW"]),
    ("oi", &["OY"]),
    ("oy", &["OY"]),
    ("au", &["AO"]),
    ("aw", &["AO"]),
    ("ei", &["EY"]),
    ("ey", &["IY"]),
    ("ie", &["IY"]),
    ("ue", &["UW"]),
    ("ui", &["UW"]),
    ("eu", &["UW"]),
    ("ew", &["UW"]),
];

// Function words where <th> is voiced (ð) rather than voiceless (θ).
const TH_VOICED: &[&str] = &[
    "the", "this", "that", "these", "those", "them", "then", "there", "they", "their", "though",
    "than", "thus", "either", "neither", "other", "another", "mother", "father", "brother",
    "weather", "whether", "together", "rather", "with",
];

const MAGIC_E: &[(char, &str)] = &[
    ('a', "EY"), // cake
    ('e', "IY"), // these
    ('i', "AY"), // time
    ('o', "OW"), // home
    ('u', "UW"), // flute
];

fn is_vowel_char(c: char) -> bool {
    matches!(c, 'a' | 'e' | 'i' | 'o' | 'u' | 'y')
}

/// Phonemize a whole phrase. `ipa_overrides` map whole words (case-insensitive)
/// to IPA pronunciations — proper-noun substitution (Sprint 13).
pub fn phonemize(text: &str, ipa_overrides: &[(String, String)]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (wi, word_raw) in text.split(|c: char| !c.is_alphanumeric() && c != '\'').enumerate() {
        let word = word_raw.trim_matches('\'');
        if word.is_empty() {
            continue;
        }
        if wi > 0 && !out.is_empty() {
            out.push(PAUSE.to_string());
        }
        let lower = word.to_lowercase();
        if let Some((_, ipa)) = ipa_overrides
            .iter()
            .find(|(term, _)| term.to_lowercase() == lower)
        {
            out.extend(ipa_to_phonemes(ipa));
            continue;
        }
        out.extend(phonemize_word(&lower, text));
    }
    // Collapse duplicate pauses.
    out.dedup();
    out
}

/// G2P for one lowercase word. `orig` retains case for magic-e detection aid.
fn phonemize_word(word: &str, _orig: &str) -> Vec<String> {
    let chars: Vec<char> = word.chars().collect();
    let n = chars.len();
    let mut idx = 0usize;
    let mut out: Vec<String> = Vec::new();
    let mut vowel_seen = false;

    while idx < n {
        let rest: String = chars[idx..].iter().collect();

        // th voicing refinement for known function words / VO+C classes.
        if rest.starts_with("th") {
            let phone = if TH_VOICED.contains(&word) { "DH" } else { "TH" };
            out.push(phone.to_string());
            idx += 2;
            continue;
        }

        // Word-final silent -mb / -mn.
        if idx + 2 == n && rest == "mb" {
            out.push("M".to_string());
            break;
        }
        if idx + 2 == n && rest == "mn" {
            out.push("M".to_string());
            break;
        }

        // Magic-e: V + (single consonant) + e + end, e.g. "time", "home".
        // The final <e> is silent — the whole triple is consumed at once.
        if is_vowel_char(chars[idx]) && n >= 3 && chars[n - 1] == 'e' && idx + 2 == n - 1 {
            let c1 = chars[idx + 1];
            if !is_vowel_char(c1) && chars[idx + 2] == 'e' {
                let phone = MAGIC_E
                    .iter()
                    .find(|(v, _)| *v == chars[idx])
                    .map(|(_, p)| *p)
                    .unwrap_or("AH");
                push_vowel(&mut out, phone, vowel_seen);
                vowel_seen = true;
                // Emit the intervening consonant (soft c/g rules apply).
                match c1 {
                    'c' => out.push("K".to_string()), // c before e/i/y would be
                    // soft, but here the next letter is the silent <e>; the
                    // common pattern (picnic-e is rare) hard-codes hard c.
                    'g' => out.push("G".to_string()),
                    'x' => {
                        out.push("K".to_string());
                        out.push("S".to_string());
                    }
                    'j' => out.push("JH".to_string()),
                    'y' => out.push("Y".to_string()),
                    other => {
                        let single = match other {
                            'b' => "B", 'd' => "D", 'f' => "F", 'h' => "HH", 'k' => "K",
                            'l' => "L", 'm' => "M", 'n' => "N", 'p' => "P", 'r' => "R",
                            's' => "S", 't' => "T", 'v' => "V", 'w' => "W", 'z' => "Z",
                            _ => "",
                        };
                        if !single.is_empty() {
                            out.push(single.to_string());
                        }
                    }
                }
                idx += 3;
                continue;
            }
        }

        // Vowel teams.
        if is_vowel_char(chars[idx]) {
            let mut matched = false;
            for (g, phones) in VOWEL_TEAMS {
                if rest.starts_with(g) {
                    for (i, p) in phones.iter().enumerate() {
                        push_vowel(&mut out, p, vowel_seen && i == 0);
                    }
                    vowel_seen = true;
                    idx += g.len();
                    matched = true;
                    break;
                }
            }
            if matched {
                continue;
            }
            // Single vowel.
            let single = match chars[idx] {
                'a' => "AE",
                'e' => "EH",
                'i' => "IH",
                'o' => "AA",
                'u' => "AH",
                'y' => {
                    if vowel_seen || idx == n - 1 {
                        "IY"
                    } else {
                        "IH"
                    }
                }
                _ => "AH",
            };
            // Soft c/g rule interacts with the FOLLOWING vowel — handled below
            // at the consonant branch; here only vowels are consumed.
            push_vowel(&mut out, single, vowel_seen);
            vowel_seen = true;
            idx += 1;
            continue;
        }

        // Consonants (digraphs FIRST so ch/th/ck/ng/ph win over the soft
        // c/g single-letter rules).
        let mut matched = false;
        for (g, phones) in DIGRAPHS {
            if rest.starts_with(g) {
                for p in phones.iter() {
                    out.push(p.to_string());
                }
                idx += g.len();
                matched = true;
                break;
            }
        }
        if matched {
            continue;
        }

        if chars[idx] == 'c' {
            let soft = chars
                .get(idx + 1)
                .map(|c| matches!(*c, 'e' | 'i' | 'y'))
                .unwrap_or(false);
            out.push(if soft { "S".to_string() } else { "K".to_string() });
            idx += 1;
            continue;
        }
        if chars[idx] == 'g' {
            let soft = chars
                .get(idx + 1)
                .map(|c| matches!(*c, 'e' | 'i' | 'y'))
                .unwrap_or(false);
            out.push(if soft { "JH".to_string() } else { "G".to_string() });
            idx += 1;
            continue;
        }
        if chars[idx] == 'x' {
            out.push("K".to_string());
            out.push("S".to_string());
            idx += 1;
            continue;
        }
        if chars[idx] == 'j' {
            out.push("JH".to_string());
            idx += 1;
            continue;
        }
        if chars[idx] == 'y' {
            out.push("Y".to_string());
            idx += 1;
            continue;
        }

        // Single consonant map.
        let single = match chars[idx] {
            'b' => "B",
            'd' => "D",
            'f' => "F",
            'h' => "HH",
            'k' => "K",
            'l' => "L",
            'm' => "M",
            'n' => "N",
            'p' => "P",
            'r' => "R",
            's' => "S",
            't' => "T",
            'v' => "V",
            'w' => "W",
            'z' => "Z",
            _ => {
                idx += 1;
                continue;
            }
        };
        out.push(single.to_string());
        idx += 1;
    }

    if out.is_empty() {
        out.push("AH".to_string());
    }
    out
}

fn push_vowel(out: &mut Vec<String>, phone: &str, already_stressed: bool) {
    if already_stressed {
        out.push(format!("{phone}0"));
    } else {
        out.push(format!("{phone}1"));
    }
}

// ── IPA ⇄ phoneme bridge ─────────────────────────────────────────────────────

/// Best-effort IPA string → ARPAbet-style phonemes (proper-noun substitution).
/// Stress marks (ˈ / ˌ) stress the following vowel; length/other diacritics
/// are ignored.
pub fn ipa_to_phonemes(ipa: &str) -> Vec<String> {
    let src: Vec<char> = ipa.chars().collect();
    let mut out: Vec<String> = Vec::new();
    let mut stress_next = false;
    let mut i = 0usize;
    while i < src.len() {
        // Stress marks.
        if src[i] == 'ˈ' || src[i] == 'ˌ' {
            stress_next = true;
            i += 1;
            continue;
        }
        // Multi-char IPA segments first.
        let two: String = src[i..(i + 2).min(src.len())].iter().collect();
        let two_match = match two.as_str() {
            "tʃ" => Some("CH"),
            "dʒ" => Some("JH"),
            "ɪə" => Some("IH0R"),
            "eə" | "ɛə" => Some("EH0R"),
            "ʊə" => Some("UH0R"),
            "aɪ" => Some("AY"),
            "aʊ" => Some("AW"),
            "ɔɪ" => Some("OY"),
            "eɪ" => Some("EY"),
            "oʊ" | "əʊ" => Some("OW"),
            _ => None,
        };
        if let Some(p) = two_match {
            push_ipa_vowel(&mut out, p, stress_next);
            stress_next = false;
            i += 2;
            continue;
        }
        let single = match src[i] {
            'p' => "P",
            'b' => "B",
            't' => "T",
            'd' => "D",
            'k' => "K",
            'g' => "G",
            'f' => "F",
            'v' => "V",
            'θ' => "TH",
            'ð' => "DH",
            's' => "S",
            'z' => "Z",
            'ʃ' => "SH",
            'ʒ' => "ZH",
            'h' => "HH",
            'm' => "M",
            'n' => "N",
            'ŋ' => "NG",
            'l' => "L",
            'r' | 'ɹ' | 'ʁ' => "R",
            'w' => "W",
            'j' => "Y",
            'ɪ' => "IH",
            'i' => "IY",
            'e' | 'ɛ' => "EH",
            'æ' => "AE",
            'a' | 'ɑ' => "AA",
            'ɒ' => "AA",
            'ɔ' => "AO",
            'u' => "UW",
            'ʊ' => "UH",
            'ʌ' | 'ə' => "AH",
            'ɜ' | 'ɝ' | 'ɚ' => "ER",
            _ => {
                i += 1;
                continue;
            }
        };
        let is_vowel = matches!(
            single,
            "IH" | "IY" | "EH" | "AE" | "AA" | "AO" | "UW" | "UH" | "AH" | "ER" | "AY" | "AW"
                | "OY" | "EY" | "OW"
        );
        push_ipa_vowel(&mut out, single, stress_next && is_vowel);
        if is_vowel {
            stress_next = false;
        }
        i += 1;
    }
    if out.is_empty() {
        out.push("AH1".to_string());
    }
    out
}

fn push_ipa_vowel(out: &mut Vec<String>, token: &str, stress: bool) {
    if token.ends_with('R') && token.contains('0') {
        out.push(token.to_string());
        return;
    }
    if matches!(token, "AY" | "AW" | "OY" | "EY" | "OW") && stress {
        out.push(format!("{token}1"));
        return;
    }
    let is_vowel = matches!(
        token,
        "IH" | "IY" | "EH" | "AE" | "AA" | "AO" | "UW" | "UH" | "AH" | "ER"
    );
    if is_vowel {
        out.push(format!("{token}{}", if stress { 1 } else { 0 }));
    } else {
        out.push(token.to_string());
    }
}

/// ARPAbet-style phonemes → IPA string (display/audit field). Stress lives
/// in the phoneme stress digits and is not re-marked here; monosyllables
/// therefore render without a stress mark, matching convention.
pub fn phonemes_to_ipa(phonemes: &[String]) -> String {
    const TABLE: &[(&str, &str)] = &[
        ("P", "p"), ("B", "b"), ("T", "t"), ("D", "d"), ("K", "k"), ("G", "g"),
        ("F", "f"), ("V", "v"), ("TH", "θ"), ("DH", "ð"), ("S", "s"), ("Z", "z"),
        ("SH", "ʃ"), ("ZH", "ʒ"), ("CH", "tʃ"), ("JH", "dʒ"), ("HH", "h"),
        ("M", "m"), ("N", "n"), ("NG", "ŋ"), ("L", "l"), ("R", "r"), ("W", "w"), ("Y", "j"),
        ("IY", "i"), ("IH", "ɪ"), ("EY", "eɪ"), ("EH", "ɛ"), ("AE", "æ"),
        ("AA", "ɑ"), ("AO", "ɔ"), ("OW", "oʊ"), ("UH", "ʊ"), ("UW", "u"),
        ("AH", "ə"), ("ER", "ɜ"), ("AY", "aɪ"), ("AW", "aʊ"), ("OY", "ɔɪ"),
        ("PAUSE", " | "),
    ];
    let mut out = String::new();
    for p in phonemes {
        let base = p.trim_end_matches(|c: char| c.is_ascii_digit());
        if let Some((_, ipa)) = TABLE.iter().find(|(k, _)| *k == base) {
            out.push_str(ipa);
        }
    }
    out.trim().to_string()
}

/// Count syllable-ish units (vowel tokens) — used by the offline TTS engine.
pub fn vowel_count(phonemes: &[String]) -> usize {
    phonemes
        .iter()
        .filter(|p| {
            let base = p.trim_end_matches(|c: char| c.is_ascii_digit());
            matches!(
                base,
                "IY" | "IH" | "EY" | "EH" | "AE" | "AA" | "AO" | "OW" | "UH" | "UW" | "AH" | "ER"
                    | "AY" | "AW" | "OY"
            )
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digraphs_and_teams() {
        assert_eq!(phonemize("ship", &[]), vec!["SH", "IH1", "P"]);
        assert_eq!(phonemize("chain", &[]), vec!["CH", "EY1", "N"]);
    }

    #[test]
    fn magic_e() {
        assert_eq!(phonemize("time", &[]), vec!["T", "AY1", "M"]);
        assert_eq!(phonemize("home", &[]), vec!["HH", "OW1", "M"]);
    }

    #[test]
    fn soft_c_g() {
        assert_eq!(phonemize("city", &[]), vec!["S", "IH1", "T", "IY0"]);
        assert_eq!(phonemize("gem", &[]), vec!["JH", "EH1", "M"]);
    }

    #[test]
    fn ipa_override() {
        let out = phonemize("Kira sails", &[("Kira".into(), "ˈkɪərə".into())]);
        assert!(out.contains(&"R".to_string()));
        assert!(out.contains(&"PAUSE".to_string()));
    }

    #[test]
    fn ipa_roundtrip() {
        let ipa = phonemes_to_ipa(&["SH".into(), "IH1".into(), "P".into()]);
        assert_eq!(ipa, "ʃɪp");
    }
}
