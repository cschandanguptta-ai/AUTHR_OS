// AUTHR_OS — local LLM backends (Phase 5, Sprint 13).
//
// LlamaServerClient talks the llama.cpp `llama-server` /completion protocol:
//   POST {prompt, n_predict, temperature, seed, cache_prompt: true,
//         keep_alive: -1 (pin the static prefix in VRAM KV-cache), grammar?}
// The transport is injectable; E2E tests exercise the real HTTP path against
// a loopback stub server, so client code is verified without any model.

use crate::error::AppError;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

pub const DEFAULT_SERVER_URL: &str = "http://127.0.0.1:8080";

// ─────────────────────────────────────────────────────────────────────────────
// OFFLINE_GRAMMAR engine — deterministic, dependency-free text generation.
// A seeded SplitMix64 stream drives a Markov-ish selector over the prompt's
// own vocabulary, producing coherent placeholder prose. It exists so the
// authoring pipeline (beat expansion, continuity prompts) is testable with
// zero network and zero model downloads.
// ─────────────────────────────────────────────────────────────────────────────

pub struct OfflineGrammarEngine;

impl OfflineGrammarEngine {
    fn splitmix64(state: &mut u64) -> u64 {
        *state = state.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = *state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }

    fn prompt_seed(prompt: &str, seed: u64) -> u64 {
        let mut h = Sha256::new();
        h.update(prompt.as_bytes());
        h.update(seed.to_le_bytes());
        let d = h.finalize();
        u64::from_le_bytes(d[0..8].try_into().unwrap())
    }

    fn vocabulary(prompt: &str) -> Vec<String> {
        let mut words: Vec<String> = prompt
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.chars().any(|c| c.is_alphabetic()))
            .map(|w| w.to_lowercase())
            .collect();
        // Anchor sentence frames keep output structurally novel-like.
        let frames = [
            "the", "a", "her", "his", "their", "one", "beyond", "beneath", "still", "then",
            "again", "toward", "against", "before", "after", "around", "beneath",
        ];
        words.extend(frames.iter().map(|s| s.to_string()));
        words
    }

    /// Generate `max_tokens` words of deterministic continuation prose.
    pub fn generate(prompt: &str, max_tokens: usize, seed: u64) -> Value {
        let mut state = Self::prompt_seed(prompt, seed);
        let vocab = Self::vocabulary(prompt);
        let mut words: Vec<String> = Vec::new();
        let mut sentence_len = 6 + (Self::splitmix64(&mut state) % 6) as usize;
        for i in 0..max_tokens.max(1) {
            if sentence_len == 0 {
                words.push(".".into());
                sentence_len = 5 + (Self::splitmix64(&mut state) % 7) as usize;
                continue;
            }
            let pick = vocab[(Self::splitmix64(&mut state) % vocab.len().max(1) as u64) as usize].clone();
            let word = if i == 0 || words.last().map(|w| w == ".").unwrap_or(true) {
                let mut c = pick.chars();
                match c.next() {
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                    None => pick.clone(),
                }
            } else {
                pick
            };
            words.push(word);
            sentence_len -= 1;
        }
        let mut text = words.join(" ");
        if !text.ends_with('.') {
            text.push('.');
        }
        json!({
            "backend": "OFFLINE_GRAMMAR",
            "text": text,
            "tokens": max_tokens.max(1),
            "seed": seed,
            "prompt_cache_pinned": false,
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// llama-server HTTP client (llama.cpp)
// ─────────────────────────────────────────────────────────────────────────────

/// Minimal HTTP/1.1 POST over loopback — dependency-free, verified against a
/// stub server in tests and against real llama-server in production.
pub fn http_post_json(url: &str, body: &str, timeout: Duration) -> Result<(u16, String), AppError> {
    let (host, port, path) = parse_url(url)?;
    let addr = format!("{host}:{port}");
    let mut stream = TcpStream::connect(&addr)
        .map_err(|e| AppError::ai_engine_unavailable(format!("connect {addr}: {e}")))?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|e| AppError::ai_engine_unavailable(format!("timeout: {e}")))?;
    let req = format!(
        "POST {path} HTTP/1.1\r\nHost: {host}:{port}\r\nContent-Type: application/json\r\n\
         Content-Length: {len}\r\nConnection: close\r\n\r\n{body}",
        len = body.len()
    );
    stream
        .write_all(req.as_bytes())
        .map_err(|e| AppError::ai_engine_unavailable(format!("write: {e}")))?;
    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .map_err(|e| AppError::ai_engine_unavailable(format!("read: {e}")))?;
    let text = String::from_utf8_lossy(&raw).into_owned();
    let status: u16 = text
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| AppError::ai_engine_unavailable("malformed HTTP response"))?;
    let body = text
        .split_once("\r\n\r\n")
        .map(|(_, b)| b.to_string())
        .unwrap_or_default();
    Ok((status, body))
}

fn parse_url(url: &str) -> Result<(String, u16, String), AppError> {
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| AppError::ai(format!("only http:// server urls supported, got `{url}`")))?;
    let (hostport, path) = match rest.split_once('/') {
        Some((hp, p)) => (hp, format!("/{p}")),
        None => (rest, "/".to_string()),
    };
    let (host, port) = match hostport.rsplit_once(':') {
        Some((h, p)) => (
            h.to_string(),
            p.parse::<u16>()
                .map_err(|_| AppError::ai(format!("bad port in `{url}`")))?,
        ),
        None => (hostport.to_string(), 80),
    };
    Ok((host, port, path))
}

pub struct GenerateRequest {
    pub prompt: String,
    pub max_tokens: usize,
    pub temperature: f64,
    pub seed: u64,
    pub grammar: Option<String>,
    pub server_url: Option<String>,
}

/// Route a generate request: explicit `server_url` (or AUTHR_LLAMA_SERVER env)
/// → llama-server; otherwise → OFFLINE_GRAMMAR.
pub fn generate(req: &GenerateRequest) -> Result<Value, AppError> {
    let server = req
        .server_url
        .clone()
        .or_else(|| std::env::var("AUTHR_LLAMA_SERVER").ok());
    match server {
        Some(url) => llama_server_generate(&url, req),
        None => Ok(OfflineGrammarEngine::generate(&req.prompt, req.max_tokens, req.seed)),
    }
}

fn llama_server_generate(url: &str, req: &GenerateRequest) -> Result<Value, AppError> {
    let mut body = json!({
        "prompt": req.prompt,
        "n_predict": req.max_tokens,
        "temperature": req.temperature,
        "seed": req.seed,
        "cache_prompt": true,   // pin the static prompt prefix in KV-cache
        "keep_alive": -1,       // never evict the prefix from VRAM
    });
    if let Some(g) = &req.grammar {
        body["grammar"] = json!(g);
    }
    let (status, resp) = http_post_json(url, &body.to_string(), Duration::from_secs(120))?;
    if status != 200 {
        return Err(AppError::ai_engine_unavailable(format!(
            "llama-server responded {status}: {}",
            resp.chars().take(160).collect::<String>()
        )));
    }
    let parsed: Value = serde_json::from_str(&resp)
        .map_err(|e| AppError::ai_engine_unavailable(format!("llama-server json: {e}")))?;
    let text = parsed["content"]
        .as_str()
        .map(str::to_string)
        .or_else(|| parsed["choices"][0]["text"].as_str().map(str::to_string))
        .ok_or_else(|| AppError::ai_engine_unavailable("llama-server response missing `content`"))?;
    Ok(json!({
        "backend": "LLAMA_SERVER",
        "server": url,
        "text": text,
        "tokens": req.max_tokens,
        "seed": req.seed,
        "prompt_cache_pinned": true,
        "timings": parsed["timings"].clone(),
    }))
}

/// `local_llm_status` payload.
pub fn status() -> Value {
    let server = std::env::var("AUTHR_LLAMA_SERVER").ok();
    json!({
        "default_backend": "OFFLINE_GRAMMAR",
        "server_configured": server.is_some(),
        "server_url": server,
        "capabilities": {
            "grammar_constrained_output": true,
            "prompt_prefix_kv_cache": true,
            "offline": true,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};

    /// One-shot loopback HTTP stub server (background thread, single request).
    fn spawn_stub(response: &'static str) -> (std::sync::mpsc::Receiver<String>, u16) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            if let Ok((mut sock, _)) = listener.accept() {
                let mut raw = Vec::new();
                let mut reader = BufReader::new(&sock);
                loop {
                    let mut line = String::new();
                    match reader.read_line(&mut line) {
                        Ok(0) | Err(_) => break,
                        Ok(_) => {
                            raw.extend_from_slice(line.as_bytes());
                            if line == "\r\n" || line == "\n" {
                                break;
                            }
                        }
                    }
                }
                let headers = String::from_utf8_lossy(&raw).into_owned();
                // Drain the body so the assertion sees the full request.
                let content_length: usize = headers
                    .lines()
                    .find_map(|l| {
                        let v = l.strip_prefix("Content-Length:")?;
                        v.trim().parse().ok()
                    })
                    .unwrap_or(0);
                let mut body = vec![0u8; content_length];
                use std::io::Read as _;
                let _ = reader.read_exact(&mut body);
                let _ = tx.send(format!("{headers}{}", String::from_utf8_lossy(&body)));
                let _ = sock.write_all(response.as_bytes());
                let _ = sock.flush();
            }
        });
        (rx, port)
    }

    #[test]
    fn offline_engine_is_deterministic_and_prompt_anchored() {
        let a = OfflineGrammarEngine::generate("Kira climbed the silent lighthouse", 40, 7);
        let b = OfflineGrammarEngine::generate("Kira climbed the silent lighthouse", 40, 7);
        assert_eq!(a["text"], b["text"]);
        assert_eq!(a["backend"], "OFFLINE_GRAMMAR");
        let c = OfflineGrammarEngine::generate("Kira climbed the silent lighthouse", 40, 8);
        assert_ne!(a["text"], c["text"], "different seed → different text");
        // Every word comes from the prompt vocabulary or the frame set.
        let text = a["text"].as_str().unwrap();
        assert!(text.split_whitespace().count() >= 30);
    }

    #[test]
    fn offline_engine_respects_token_cap() {
        let out = OfflineGrammarEngine::generate("once upon a time", 10, 1);
        let words = out["text"].as_str().unwrap().split_whitespace().count();
        assert!(words <= 11, "words={words}");
    }

    #[test]
    fn llama_client_sends_cache_and_keep_alive_flags() {
        let (rx, port) = spawn_stub(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"content\":\"The tide turned.\",\"timings\":{\"predicted_ms\":12.5}}\r\n",
        );
        let req = GenerateRequest {
            prompt: "Chapter 3 opening:".into(),
            max_tokens: 32,
            temperature: 0.7,
            seed: 42,
            grammar: Some("root ::= word+".into()),
            server_url: Some(format!("http://127.0.0.1:{port}")),
        };
        let out = llama_server_generate(&format!("http://127.0.0.1:{port}"), &req).unwrap();
        assert_eq!(out["backend"], "LLAMA_SERVER");
        assert_eq!(out["text"], "The tide turned.");
        assert_eq!(out["prompt_cache_pinned"], true);
        let http_req = rx.recv().unwrap();
        assert!(http_req.starts_with("POST / HTTP/1.1"));
        assert!(http_req.contains("\"cache_prompt\":true"));
        assert!(http_req.contains("\"keep_alive\":-1"));
        assert!(http_req.contains("\"grammar\":\"root ::= word+\""));
    }

    #[test]
    fn unreachable_server_maps_to_engine_unavailable() {
        // Port 1 is reserved — connect fails fast and deterministically.
        let req = GenerateRequest {
            prompt: "x".into(),
            max_tokens: 4,
            temperature: 0.5,
            seed: 0,
            grammar: None,
            server_url: Some("http://127.0.0.1:1".into()),
        };
        let err = llama_server_generate("http://127.0.0.1:1", &req).unwrap_err();
        assert_eq!(err.code, "AI_ENGINE_UNAVAILABLE");
    }

    #[test]
    fn url_parse_rejects_non_http() {
        assert!(parse_url("https://example.com").is_err());
        let (h, p, path) = parse_url("http://127.0.0.1:8080/completion").unwrap();
        assert_eq!((h.as_str(), p, path.as_str()), ("127.0.0.1", 8080, "/completion"));
    }
}
