// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! ollama HTTP client — remote inference via ollama REST API.
//! Supports streaming and non-streaming generation.

use std::sync::{mpsc, Arc};

/// ollama /api/generate request body.
#[derive(serde::Serialize)]
struct GenerateRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    system: &'a str,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<GenerateOptions>,
}

#[derive(serde::Serialize)]
struct GenerateOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    num_ctx: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

/// ollama /api/generate response (non-streaming).
#[derive(serde::Deserialize)]
struct GenerateResponse {
    response: String,
    #[serde(default)]
    done: bool,
    #[serde(default)]
    eval_count: Option<u64>,
    #[serde(default)]
    eval_duration: Option<u64>,
}

/// ollama /api/tags response.
#[derive(serde::Deserialize)]
struct TagsResponse {
    models: Vec<ModelInfo>,
}

#[derive(serde::Deserialize, Clone, Debug)]
pub struct ModelInfo {
    pub name: String,
    pub size: u64,
    pub modified_at: String,
}

/// ollama /api/version response.
#[derive(serde::Deserialize)]
struct VersionResponse {
    version: String,
}

/// Check if an ollama instance is reachable.
pub fn health(base_url: &str) -> bool {
    let url = format!("{}/api/version", base_url);
    match reqwest::blocking::get(&url) {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}

/// Get ollama version string.
pub fn version(base_url: &str) -> Option<String> {
    let url = format!("{}/api/version", base_url);
    reqwest::blocking::get(&url)
        .ok()
        .and_then(|r| r.json::<VersionResponse>().ok())
        .map(|v| v.version)
}

/// List models on a remote ollama instance.
pub fn list_models(base_url: &str) -> Result<Vec<ModelInfo>, String> {
    let url = format!("{}/api/tags", base_url);
    let resp = reqwest::blocking::get(&url).map_err(|e| format!("ollama list: {}", e))?;
    let tags: TagsResponse = resp
        .json()
        .map_err(|e| format!("ollama list parse: {}", e))?;
    Ok(tags.models)
}

/// Non-streaming generation. Returns full response text.
pub fn generate(
    base_url: &str,
    model: &str,
    system: &str,
    prompt: &str,
    num_ctx: Option<u32>,
) -> Result<String, String> {
    let url = format!("{}/api/generate", base_url);
    let body = GenerateRequest {
        model,
        prompt,
        system,
        stream: false,
        options: Some(GenerateOptions {
            num_ctx,
            temperature: Some(0.2),
        }),
    };

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| format!("http client: {}", e))?;

    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .map_err(|e| format!("ollama generate: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!(
            "ollama http {}: {}",
            resp.status(),
            resp.text().unwrap_or_default()
        ));
    }

    let gen: GenerateResponse = resp
        .json()
        .map_err(|e| format!("ollama response parse: {}", e))?;

    // Log performance if available
    if let (Some(count), Some(dur)) = (gen.eval_count, gen.eval_duration) {
        if dur > 0 {
            let tps = count as f64 / (dur as f64 / 1_000_000_000.0);
            eprintln!("[ollama] {} tokens, {:.1} tok/s", count, tps);
        }
    }

    Ok(gen.response)
}

/// Generate with explicit temperature and num_ctx. For micro-model dispatch.
pub fn generate_with_temp(
    base_url: &str,
    model: &str,
    system: &str,
    prompt: &str,
    num_ctx: Option<u32>,
    temperature: Option<f32>,
) -> Result<String, String> {
    let url = format!("{}/api/generate", base_url);
    let body = GenerateRequest {
        model,
        prompt,
        system,
        stream: false,
        options: Some(GenerateOptions {
            num_ctx,
            temperature,
        }),
    };

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| format!("http client: {}", e))?;

    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .map_err(|e| format!("ollama generate: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!(
            "ollama http {}: {}",
            resp.status(),
            resp.text().unwrap_or_default()
        ));
    }

    let gen: GenerateResponse = resp
        .json()
        .map_err(|e| format!("ollama response parse: {}", e))?;

    if let (Some(count), Some(dur)) = (gen.eval_count, gen.eval_duration) {
        if dur > 0 {
            let tps = count as f64 / (dur as f64 / 1_000_000_000.0);
            eprintln!("[micro] {} tokens, {:.1} tok/s", count, tps);
        }
    }

    Ok(gen.response)
}

/// Streaming generation. Returns receiver for token chunks.
pub fn generate_stream(
    base_url: &str,
    model: &str,
    system: &str,
    prompt: &str,
    num_ctx: Option<u32>,
) -> mpsc::Receiver<Arc<str>> {
    let (tx, rx) = mpsc::channel();
    let url = format!("{}/api/generate", base_url);
    let body = serde_json::json!({
        "model": model,
        "prompt": prompt,
        "system": system,
        "stream": true,
        "options": {
            "num_ctx": num_ctx.unwrap_or(8192),
            "temperature": 0.2,
        }
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

        // Read NDJSON stream line by line
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

            if let Ok(chunk) = serde_json::from_str::<GenerateResponse>(&line) {
                if !chunk.response.is_empty()
                    && tx.send(Arc::from(chunk.response.as_str())).is_err()
                {
                    break; // receiver dropped
                }
                if chunk.done {
                    break;
                }
            }
        }
    });

    rx
}

/// Chat-style generation (using /api/chat endpoint).
pub fn chat(
    base_url: &str,
    model: &str,
    system: &str,
    messages: &[(String, String)],
    user_input: &str,
    num_ctx: Option<u32>,
) -> Result<String, String> {
    let url = format!("{}/api/chat", base_url);

    let mut msgs = Vec::new();
    msgs.push(serde_json::json!({"role": "system", "content": system}));
    for (user, assistant) in messages {
        msgs.push(serde_json::json!({"role": "user", "content": user}));
        msgs.push(serde_json::json!({"role": "assistant", "content": assistant}));
    }
    msgs.push(serde_json::json!({"role": "user", "content": user_input}));

    let body = serde_json::json!({
        "model": model,
        "messages": msgs,
        "stream": false,
        "options": {
            "num_ctx": num_ctx.unwrap_or(8192),
            "temperature": 0.2,
        }
    });

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| format!("http client: {}", e))?;

    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .map_err(|e| format!("ollama chat: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!(
            "ollama http {}: {}",
            resp.status(),
            resp.text().unwrap_or_default()
        ));
    }

    #[derive(serde::Deserialize)]
    struct ChatResponse {
        message: ChatMessage,
    }
    #[derive(serde::Deserialize)]
    struct ChatMessage {
        content: String,
    }

    let chat_resp: ChatResponse = resp
        .json()
        .map_err(|e| format!("ollama chat parse: {}", e))?;

    Ok(chat_resp.message.content)
}
