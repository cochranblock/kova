//! providers — Multi-provider LLM client. Local (Kalosm/candle), OpenAI-compatible, Anthropic.
//! Pure Rust local inference is the default. No ollama dependency.
//! f199=f199, f200=f200, f210=f333.
//! f211=f334, f212=f335, f213=f336.
//! f214=f337, f381=anthropic_stream.
//! t129=T129, t130=T130, t131=T131, t134=T188.
// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{mpsc, Arc};

// ── Types ────────────────────────────────────────────────────────

/// t129=T129. Supported LLM providers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum T129 {
    /// Pure Rust local inference via Kalosm/candle. Default provider.
    Local { model_path: PathBuf },
    /// Remote ollama instance (kept for cluster nodes).
    Ollama { url: String },
    /// OpenAI-compatible API (OpenAI, Groq, Together, local vLLM, etc).
    OpenAiCompat {
        url: String,
        api_key: String,
        model: String,
    },
    /// Anthropic Claude API.
    Anthropic { api_key: String, model: String },
}

/// t130=T130. Config for a named provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct T130 {
    pub name: String,
    pub provider: T129,
    pub default_model: Option<String>,
}

/// t131=T131. Unified response from any provider.
#[derive(Debug, Clone)]
pub struct T131 {
    pub text: String,
    pub model: String,
    pub provider_name: String,
    pub latency_ms: u64,
    pub tokens_out: Option<u64>,
}

/// t134=T188. Model available on a provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct T188 {
    pub name: String,
    pub size: u64,
    pub modified_at: String,
}

// ── Generation ───────────────────────────────────────────────────

/// f199=f199. Generate text from any provider.
pub fn f199(
    provider: &T129,
    model: &str,
    system: &str,
    prompt: &str,
) -> Result<T131, String> {
    let t0 = std::time::Instant::now();

    match provider {
        T129::Local { model_path } => {
            let resp_text = local_generate(model_path, system, prompt)?;
            let model_name = model_path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "local".into());
            Ok(T131 {
                text: resp_text,
                model: model_name,
                provider_name: "local".into(),
                latency_ms: t0.elapsed().as_millis() as u64,
                tokens_out: None,
            })
        }
        T129::Ollama { url } => {
            let resp = ollama_generate_raw(url, model, system, prompt, None, Some(0.2))?;
            Ok(T131 {
                text: resp.text,
                model: model.into(),
                provider_name: "local-http".into(),
                latency_ms: t0.elapsed().as_millis() as u64,
                tokens_out: resp.tokens_out,
            })
        }
        T129::OpenAiCompat {
            url,
            api_key,
            model: default_model,
        } => {
            let use_model = if model.is_empty() {
                default_model.as_str()
            } else {
                model
            };
            let resp = openai_compat_generate(url, api_key, use_model, system, prompt)?;
            Ok(T131 {
                text: resp.text,
                model: use_model.into(),
                provider_name: "openai-compat".into(),
                latency_ms: t0.elapsed().as_millis() as u64,
                tokens_out: resp.tokens_out,
            })
        }
        T129::Anthropic { api_key, model: default_model } => {
            let use_model = if model.is_empty() {
                default_model.as_str()
            } else {
                model
            };
            let resp = anthropic_generate(api_key, use_model, system, prompt)?;
            Ok(T131 {
                text: resp.text,
                model: use_model.into(),
                provider_name: "anthropic".into(),
                latency_ms: t0.elapsed().as_millis() as u64,
                tokens_out: resp.tokens_out,
            })
        }
    }
}

// ── Local (Kalosm/candle) ────────────────────────────────────────

/// Pure Rust local inference via Kalosm. Blocks until complete.
fn local_generate(model_path: &std::path::Path, system: &str, prompt: &str) -> Result<String, String> {
    let path = model_path.to_path_buf();
    let sys = system.to_string();
    let inp = prompt.to_string();

    // Spawn a thread with its own tokio runtime (same pattern as inference::f76).
    let handle = std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().map_err(|e| format!("tokio: {}", e))?;
        rt.block_on(async {
            crate::inference::f80(&path, &sys, &inp)
                .await
                .map_err(|e| format!("local inference: {}", e))
        })
    });

    handle.join().map_err(|_| "inference thread panic".to_string())?
}

/// f210=f333. Returns Local provider using config model path.
/// Falls back to Ollama if KOVA_PROVIDER=ollama or model file missing.
pub fn f333() -> T129 {
    // Explicit override: KOVA_PROVIDER=ollama forces remote.
    if std::env::var("KOVA_PROVIDER").as_deref() == Ok("ollama") {
        return T129::Ollama {
            url: crate::config::ollama_url(),
        };
    }

    match crate::config::inference_model_path() {
        Some(path) if path.exists() => T129::Local { model_path: path },
        _ => {
            // No local model found — fall back to ollama.
            T129::Ollama {
                url: crate::config::ollama_url(),
            }
        }
    }
}

// ── T129 Health / Version / Models ───────────────────────────

/// f211=f334. Check if a provider is reachable.
pub fn f334(provider: &T129) -> bool {
    match provider {
        T129::Local { model_path } => model_path.exists(),
        T129::Ollama { url } => {
            let endpoint = format!("{}/api/version", url);
            reqwest::blocking::get(&endpoint)
                .map(|r| r.status().is_success())
                .unwrap_or(false)
        }
        T129::OpenAiCompat { url, .. } => {
            // Use std::net TCP + raw HTTP to avoid reqwest/tokio runtime issues
            use std::io::{Read, Write};
            let url = url.trim_end_matches('/');
            let host_port = url.trim_start_matches("http://").trim_start_matches("https://");
            let addr = if host_port.contains(':') {
                host_port.to_string()
            } else {
                format!("{}:80", host_port)
            };
            let host = host_port.split(':').next().unwrap_or("localhost");
            // Resolve hostname → IP via std::net
            match std::net::ToSocketAddrs::to_socket_addrs(&addr) {
                Ok(mut addrs) => {
                    if let Some(sock_addr) = addrs.next() {
                        match std::net::TcpStream::connect_timeout(&sock_addr, std::time::Duration::from_secs(3)) {
                            Ok(mut stream) => {
                                let req = format!("GET /v1/models HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n", host);
                                let _ = stream.write_all(req.as_bytes());
                                let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(3)));
                                let mut buf = [0u8; 32];
                                stream.read(&mut buf).ok()
                                    .map(|n| {
                                        let resp = String::from_utf8_lossy(&buf[..n]);
                                        resp.contains("200")
                                    })
                                    .unwrap_or(false)
                            }
                            Err(_) => false,
                        }
                    } else { false }
                }
                Err(_) => false,
            }
        }
        T129::Anthropic { .. } => {
            // Anthropic is always "online" if we have a key. Real check would burn tokens.
            true
        }
    }
}

/// f212=f335. Get version string from provider.
pub fn f335(provider: &T129) -> Option<String> {
    match provider {
        T129::Local { .. } => Some(env!("CARGO_PKG_VERSION").to_string()),
        T129::Ollama { url } => {
            #[derive(Deserialize)]
            struct VersionResp {
                version: String,
            }
            let endpoint = format!("{}/api/version", url);
            reqwest::blocking::get(&endpoint)
                .ok()
                .and_then(|r| r.json::<VersionResp>().ok())
                .map(|v| v.version)
        }
        T129::OpenAiCompat { .. } => None,
        T129::Anthropic { .. } => None,
    }
}

/// f213=f336. List available models on a provider.
pub fn f336(provider: &T129) -> Result<Vec<T188>, String> {
    match provider {
        T129::Local { model_path } => {
            // List GGUF files in the model's parent directory.
            let dir = model_path.parent().unwrap_or(model_path.as_path());
            let mut models = Vec::new();
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("gguf") {
                        let name = path
                            .file_stem()
                            .map(|s| s.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                        let modified_at = std::fs::metadata(&path)
                            .and_then(|m| m.modified())
                            .ok()
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| d.as_secs().to_string())
                            .unwrap_or_default();
                        models.push(T188 {
                            name,
                            size,
                            modified_at,
                        });
                    }
                }
            }
            Ok(models)
        }
        T129::Ollama { url } => {
            #[derive(Deserialize)]
            struct TagsResp {
                models: Vec<T188>,
            }
            let endpoint = format!("{}/api/tags", url);
            let resp =
                reqwest::blocking::get(&endpoint).map_err(|e| format!("list models: {}", e))?;
            let tags: TagsResp = resp.json().map_err(|e| format!("parse models: {}", e))?;
            Ok(tags.models)
        }
        T129::OpenAiCompat { url, api_key, .. } => {
            #[derive(Deserialize)]
            struct OaiModelsResp {
                data: Vec<OaiModel>,
            }
            #[derive(Deserialize)]
            struct OaiModel {
                id: String,
            }
            let endpoint = format!("{}/v1/models", url.trim_end_matches('/'));
            let resp = reqwest::blocking::Client::new()
                .get(&endpoint)
                .header("Authorization", format!("Bearer {}", api_key))
                .send()
                .map_err(|e| format!("list models: {}", e))?;
            let models: OaiModelsResp =
                resp.json().map_err(|e| format!("parse models: {}", e))?;
            Ok(models
                .data
                .into_iter()
                .map(|m| T188 {
                    name: m.id,
                    size: 0,
                    modified_at: String::new(),
                })
                .collect())
        }
        T129::Anthropic { .. } => Ok(vec![
            T188 { name: "claude-opus-4-6".into(), size: 0, modified_at: String::new() },
            T188 { name: "claude-sonnet-4-6".into(), size: 0, modified_at: String::new() },
            T188 { name: "claude-haiku-4-5-20251001".into(), size: 0, modified_at: String::new() },
        ]),
    }
}

/// f214=f337. Streaming generation. Returns receiver for token chunks.
pub fn f337(
    provider: &T129,
    model: &str,
    system: &str,
    prompt: &str,
) -> mpsc::Receiver<Arc<str>> {
    match provider {
        T129::Local { model_path } => {
            // Reuse inference::f76 which already returns mpsc::Receiver<Arc<str>>
            crate::inference::f76(model_path, system, &[], prompt)
        }
        T129::Ollama { url } => {
            let (tx, rx) = mpsc::channel();
            let url = format!("{}/api/generate", url);
            let body = serde_json::json!({
                "model": model,
                "prompt": prompt,
                "system": system,
                "stream": true,
                "options": { "num_ctx": 8192, "temperature": 0.2 }
            });
            std::thread::spawn(move || {
                let client = match reqwest::blocking::Client::builder()
                    .timeout(std::time::Duration::from_secs(300))
                    .build()
                {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = tx.send(Arc::from(format!("Error: {}", e)));
                        return;
                    }
                };
                let resp = match client.post(&url).json(&body).send() {
                    Ok(r) => r,
                    Err(e) => {
                        let _ = tx.send(Arc::from(format!("Error: {}", e)));
                        return;
                    }
                };
                let reader = std::io::BufReader::new(resp);
                use std::io::BufRead;
                for line in reader.lines() {
                    let line = match line {
                        Ok(l) => l,
                        Err(_) => break,
                    };
                    if line.is_empty() {
                        continue;
                    }
                    #[derive(Deserialize)]
                    struct StreamChunk {
                        response: String,
                        #[serde(default)]
                        done: bool,
                    }
                    if let Ok(chunk) = serde_json::from_str::<StreamChunk>(&line) {
                        if !chunk.response.is_empty()
                            && tx.send(Arc::from(chunk.response.as_str())).is_err()
                        {
                            break;
                        }
                        if chunk.done {
                            break;
                        }
                    }
                }
            });
            rx
        }
        T129::Anthropic { api_key, model: default_model } => {
            let use_model = if model.is_empty() {
                default_model.clone()
            } else {
                model.to_string()
            };
            f381(api_key, &use_model, system, prompt)
        }
        T129::OpenAiCompat { .. } => {
            // Non-streaming fallback: generate full response and send as one chunk.
            let (tx, rx) = mpsc::channel();
            let provider = provider.clone();
            let model = model.to_string();
            let system = system.to_string();
            let prompt = prompt.to_string();
            std::thread::spawn(move || {
                match f199(&provider, &model, &system, &prompt) {
                    Ok(resp) => {
                        let _ = tx.send(Arc::from(resp.text.as_str()));
                    }
                    Err(e) => {
                        let _ = tx.send(Arc::from(format!("Error: {}", e)));
                    }
                }
            });
            rx
        }
    }
}

// ── Ollama HTTP (self-contained, no external module) ────────────

/// Self-contained ollama /api/generate. No dependency on ollama.rs.
fn ollama_generate_raw(
    base_url: &str,
    model: &str,
    system: &str,
    prompt: &str,
    num_ctx: Option<u32>,
    temperature: Option<f32>,
) -> Result<RawResponse, String> {
    let url = format!("{}/api/generate", base_url);
    let body = serde_json::json!({
        "model": model,
        "prompt": prompt,
        "system": system,
        "stream": false,
        "options": {
            "num_ctx": num_ctx.unwrap_or(8192),
            "temperature": temperature.unwrap_or(0.2),
        }
    });

    let t0 = std::time::Instant::now();
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| format!("http client: {}", e))?;

    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .map_err(|e| {
            let elapsed = t0.elapsed().as_millis() as u64;
            crate::trace::f161(crate::trace::T109 {
                ts: crate::trace::f326(),
                backend: "local-http".into(),
                model: model.into(),
                node: base_url.into(),
                call_type: "generate".into(),
                latency_ms: elapsed,
                tokens_out: None,
                tok_per_sec: None,
                prompt_bytes: prompt.len() + system.len(),
                response_bytes: 0,
                status: format!("send: {}", e),
            });
            format!("generate: {}", e)
        })?;

    if !resp.status().is_success() {
        let status_code = resp.status();
        let body_text = resp.text().unwrap_or_default();
        crate::trace::f161(crate::trace::T109 {
            ts: crate::trace::f326(),
            backend: "local-http".into(),
            model: model.into(),
            node: base_url.into(),
            call_type: "generate".into(),
            latency_ms: t0.elapsed().as_millis() as u64,
            tokens_out: None,
            tok_per_sec: None,
            prompt_bytes: prompt.len() + system.len(),
            response_bytes: 0,
            status: format!("http {}", status_code),
        });
        return Err(format!("http {}: {}", status_code, body_text));
    }

    #[derive(Deserialize)]
    struct GenResp {
        response: String,
        #[serde(default)]
        eval_count: Option<u64>,
        #[serde(default)]
        eval_duration: Option<u64>,
    }

    let gen_resp: GenResp = resp.json().map_err(|e| format!("parse: {}", e))?;

    let elapsed = t0.elapsed().as_millis() as u64;
    let tokens_out = gen_resp.eval_count;
    let tok_per_sec = match (gen_resp.eval_count, gen_resp.eval_duration) {
        (Some(count), Some(dur)) if dur > 0 => {
            Some(count as f64 / (dur as f64 / 1_000_000_000.0))
        }
        _ => None,
    };

    if let (Some(count), Some(tps)) = (tokens_out, tok_per_sec) {
        eprintln!("[provider] {} tokens, {:.1} tok/s", count, tps);
    }

    crate::trace::f161(crate::trace::T109 {
        ts: crate::trace::f326(),
        backend: "local-http".into(),
        model: model.into(),
        node: base_url.into(),
        call_type: "generate".into(),
        latency_ms: elapsed,
        tokens_out,
        tok_per_sec,
        prompt_bytes: prompt.len() + system.len(),
        response_bytes: gen_resp.response.len(),
        status: "ok".into(),
    });

    Ok(RawResponse {
        text: gen_resp.response,
        tokens_out,
    })
}

// ── OpenAI-Compatible ────────────────────────────────────────────

struct RawResponse {
    text: String,
    tokens_out: Option<u64>,
}

fn openai_compat_generate(
    base_url: &str,
    api_key: &str,
    model: &str,
    system: &str,
    prompt: &str,
) -> Result<RawResponse, String> {
    let url = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": prompt}
        ],
        "temperature": 0.2,
        "stream": false
    });

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| format!("http client: {}", e))?;

    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .map_err(|e| format!("openai request: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!(
            "openai http {}: {}",
            resp.status(),
            resp.text().unwrap_or_default()
        ));
    }

    #[derive(Deserialize)]
    struct OaiResponse {
        choices: Vec<OaiChoice>,
        usage: Option<OaiUsage>,
    }
    #[derive(Deserialize)]
    struct OaiChoice {
        message: OaiMessage,
    }
    #[derive(Deserialize)]
    struct OaiMessage {
        content: String,
    }
    #[derive(Deserialize)]
    struct OaiUsage {
        completion_tokens: Option<u64>,
    }

    let oai: OaiResponse = resp.json().map_err(|e| format!("openai parse: {}", e))?;
    let text = oai
        .choices
        .into_iter()
        .next()
        .map(|c| c.message.content)
        .unwrap_or_default();
    let tokens_out = oai.usage.and_then(|u| u.completion_tokens);

    Ok(RawResponse { text, tokens_out })
}

// ── Anthropic ────────────────────────────────────────────────────

fn anthropic_generate(
    api_key: &str,
    model: &str,
    system: &str,
    prompt: &str,
) -> Result<RawResponse, String> {
    let url = "https://api.anthropic.com/v1/messages";
    let body = serde_json::json!({
        "model": model,
        "max_tokens": 4096,
        "system": system,
        "messages": [
            {"role": "user", "content": prompt}
        ]
    });

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| format!("http client: {}", e))?;

    let resp = client
        .post(url)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2024-10-22")
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .map_err(|e| format!("anthropic request: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!(
            "anthropic http {}: {}",
            resp.status(),
            resp.text().unwrap_or_default()
        ));
    }

    #[derive(Deserialize)]
    struct AnthResponse {
        content: Vec<AnthContent>,
        usage: Option<AnthUsage>,
    }
    #[derive(Deserialize)]
    struct AnthContent {
        text: String,
    }
    #[derive(Deserialize)]
    struct AnthUsage {
        output_tokens: Option<u64>,
    }

    let anth: AnthResponse = resp
        .json()
        .map_err(|e| format!("anthropic parse: {}", e))?;
    let text = anth
        .content
        .into_iter()
        .next()
        .map(|c| c.text)
        .unwrap_or_default();
    let tokens_out = anth.usage.and_then(|u| u.output_tokens);

    Ok(RawResponse { text, tokens_out })
}

// ── T129 List ────────────────────────────────────────────────

/// f200=f200. Load configured providers from config.
pub fn f200() -> Vec<T130> {
    let config_path = crate::config::kova_dir().join("providers.toml");
    if !config_path.exists() {
        return default_providers();
    }
    match std::fs::read_to_string(&config_path) {
        Ok(content) => {
            #[derive(Deserialize)]
            struct ProvidersFile {
                #[serde(default)]
                providers: Vec<T130>,
            }
            match toml::from_str::<ProvidersFile>(&content) {
                Ok(f) => f.providers,
                Err(_) => default_providers(),
            }
        }
        Err(_) => default_providers(),
    }
}

fn default_providers() -> Vec<T130> {
    let provider = f333();
    let model_name = match &provider {
        T129::Local { model_path } => model_path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned()),
        _ => Some("qwen2.5-coder:1.5b".into()),
    };
    vec![T130 {
        name: "local".into(),
        provider,
        default_model: model_name,
    }]
}

// ── Anthropic SSE Streaming ──────────────────────────────────────

/// f381=anthropic_stream. Anthropic Messages API with SSE streaming.
/// Returns receiver for token chunks, same pattern as f76.
/// Parses content_block_delta events for incremental text.
pub fn f381(
    api_key: &str,
    model: &str,
    system: &str,
    prompt: &str,
) -> mpsc::Receiver<Arc<str>> {
    let (tx, rx) = mpsc::channel();
    let api_key = api_key.to_string();
    let model = model.to_string();
    let system = system.to_string();
    let prompt = prompt.to_string();

    std::thread::spawn(move || {
        if let Err(e) = anthropic_stream_inner(&api_key, &model, &system, &prompt, &tx) {
            let _ = tx.send(Arc::from(format!("Error: {}", e)));
        }
    });

    rx
}

fn anthropic_stream_inner(
    api_key: &str,
    model: &str,
    system: &str,
    prompt: &str,
    tx: &mpsc::Sender<Arc<str>>,
) -> Result<(), String> {
    let body = serde_json::json!({
        "model": model,
        "max_tokens": 8192,
        "stream": true,
        "system": system,
        "messages": [
            {"role": "user", "content": prompt}
        ]
    });

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|e| format!("http client: {}", e))?;

    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2024-10-22")
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .map_err(|e| format!("anthropic request: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!(
            "anthropic http {}: {}",
            resp.status(),
            resp.text().unwrap_or_default()
        ));
    }

    // Parse SSE stream: lines starting with "data: " contain JSON events.
    let reader = std::io::BufReader::new(resp);
    use std::io::BufRead;

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        // SSE format: "data: {json}" or empty lines or "event: ..." lines.
        let Some(data) = line.strip_prefix("data: ") else {
            continue;
        };

        if data == "[DONE]" {
            break;
        }

        // Parse the SSE event JSON.
        let Ok(event) = serde_json::from_str::<serde_json::Value>(data) else {
            continue;
        };

        let event_type = event.get("type").and_then(|t| t.as_str()).unwrap_or("");

        match event_type {
            "content_block_delta" => {
                // Extract text from delta.text field.
                if let Some(text) = event
                    .get("delta")
                    .and_then(|d| d.get("text"))
                    .and_then(|t| t.as_str())
                    && !text.is_empty()
                    && tx.send(Arc::from(text)).is_err()
                {
                    break; // Receiver dropped.
                }
            }
            "message_stop" => break,
            "error" => {
                let msg = event
                    .get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(|m| m.as_str())
                    .unwrap_or("unknown error");
                let _ = tx.send(Arc::from(format!("Error: {}", msg)));
                break;
            }
            _ => {} // message_start, content_block_start, ping, etc.
        }
    }

    Ok(())
}

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_providers_includes_local() {
        let providers = default_providers();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].name, "local");
        // Default is Local (pure Rust) when model exists, Ollama fallback otherwise.
        assert!(
            matches!(providers[0].provider, T129::Local { .. })
                || matches!(providers[0].provider, T129::Ollama { .. })
        );
    }

    #[test]
    fn provider_config_serde_roundtrip() {
        let config = T130 {
            name: "test".into(),
            provider: T129::OpenAiCompat {
                url: "http://localhost:8080".into(),
                api_key: "sk-test".into(),
                model: "gpt-4".into(),
            },
            default_model: Some("gpt-4".into()),
        };
        let json = serde_json::to_string(&config).unwrap();
        let back: T130 = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "test");
    }

    #[test]
    fn provider_response_fields() {
        let resp = T131 {
            text: "hello".into(),
            model: "test-model".into(),
            provider_name: "test".into(),
            latency_ms: 100,
            tokens_out: Some(5),
        };
        assert_eq!(resp.text, "hello");
        assert_eq!(resp.tokens_out, Some(5));
    }

    #[test]
    fn all_provider_variants_serialize() {
        let providers = vec![
            T129::Local { model_path: PathBuf::from("/models/test.gguf") },
            T129::Ollama { url: "http://localhost:11434".into() },
            T129::OpenAiCompat {
                url: "http://localhost:8080".into(),
                api_key: "sk-test".into(),
                model: "gpt-4".into(),
            },
            T129::Anthropic {
                api_key: "sk-ant-test".into(),
                model: "claude-sonnet-4-6".into(),
            },
        ];
        for p in &providers {
            let json = serde_json::to_string(p).unwrap();
            assert!(!json.is_empty());
            let _back: T129 = serde_json::from_str(&json).unwrap();
        }
    }

    #[test]
    fn provider_response_no_tokens() {
        let resp = T131 {
            text: "output".into(),
            model: "m".into(),
            provider_name: "test".into(),
            latency_ms: 0,
            tokens_out: None,
        };
        assert_eq!(resp.tokens_out, None);
    }

    /// f199 with unreachable URL returns Err (not panic).
    #[test]
    fn provider_generate_unreachable_url_returns_error() {
        let provider = T129::Ollama {
            url: "http://127.0.0.1:1".into(),
        };
        let result = f199(&provider, "test", "system", "prompt");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn default_provider_returns_valid_variant() {
        let provider = f333();
        // Should be Local or Ollama depending on whether model file exists.
        match &provider {
            T129::Local { model_path } => {
                assert!(model_path.to_string_lossy().len() > 0);
            }
            T129::Ollama { url } => {
                assert!(url.starts_with("http"));
            }
            _ => panic!("f333 should return Local or Ollama"),
        }
    }

    #[test]
    fn local_provider_serializes() {
        let p = T129::Local {
            model_path: PathBuf::from("/tmp/test-model.gguf"),
        };
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("Local"));
        assert!(json.contains("test-model.gguf"));
        let back: T129 = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, T129::Local { .. }));
    }
}