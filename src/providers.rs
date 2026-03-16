// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! providers — Multi-provider LLM client. Ollama, OpenAI-compatible, Anthropic.
//! Research: rust-genai (multi-provider patterns), reqwest blocking client.
//! f199=provider_generate, f200=provider_list.
//! t129=Provider, t130=ProviderConfig, t131=ProviderResponse.

use serde::{Deserialize, Serialize};

// ── Types ────────────────────────────────────────────────────────

/// t129=Provider. Supported LLM providers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Provider {
    /// Local ollama instance.
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

/// t130=ProviderConfig. Config for a named provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub name: String,
    pub provider: Provider,
    pub default_model: Option<String>,
}

/// t131=ProviderResponse. Unified response from any provider.
#[derive(Debug, Clone)]
pub struct ProviderResponse {
    pub text: String,
    pub model: String,
    pub provider_name: String,
    pub latency_ms: u64,
    pub tokens_out: Option<u64>,
}

// ── Generation ───────────────────────────────────────────────────

/// f199=provider_generate. Generate text from any provider.
pub fn provider_generate(
    provider: &Provider,
    model: &str,
    system: &str,
    prompt: &str,
) -> Result<ProviderResponse, String> {
    let t0 = std::time::Instant::now();

    match provider {
        Provider::Ollama { url } => {
            let resp_text = crate::ollama::generate(url, model, system, prompt, None)?;
            Ok(ProviderResponse {
                text: resp_text,
                model: model.into(),
                provider_name: "ollama".into(),
                latency_ms: t0.elapsed().as_millis() as u64,
                tokens_out: None,
            })
        }
        Provider::OpenAiCompat {
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
            Ok(ProviderResponse {
                text: resp.text,
                model: use_model.into(),
                provider_name: "openai-compat".into(),
                latency_ms: t0.elapsed().as_millis() as u64,
                tokens_out: resp.tokens_out,
            })
        }
        Provider::Anthropic { api_key, model: default_model } => {
            let use_model = if model.is_empty() {
                default_model.as_str()
            } else {
                model
            };
            let resp = anthropic_generate(api_key, use_model, system, prompt)?;
            Ok(ProviderResponse {
                text: resp.text,
                model: use_model.into(),
                provider_name: "anthropic".into(),
                latency_ms: t0.elapsed().as_millis() as u64,
                tokens_out: resp.tokens_out,
            })
        }
    }
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

// ── Provider List ────────────────────────────────────────────────

/// f200=provider_list. Load configured providers from config.
pub fn provider_list() -> Vec<ProviderConfig> {
    let config_path = crate::config::kova_dir().join("providers.toml");
    if !config_path.exists() {
        return default_providers();
    }
    match std::fs::read_to_string(&config_path) {
        Ok(content) => {
            #[derive(Deserialize)]
            struct ProvidersFile {
                #[serde(default)]
                providers: Vec<ProviderConfig>,
            }
            match toml::from_str::<ProvidersFile>(&content) {
                Ok(f) => f.providers,
                Err(_) => default_providers(),
            }
        }
        Err(_) => default_providers(),
    }
}

fn default_providers() -> Vec<ProviderConfig> {
    vec![ProviderConfig {
        name: "local".into(),
        provider: Provider::Ollama {
            url: "http://localhost:11434".into(),
        },
        default_model: Some("qwen2.5-coder:1.5b".into()),
    }]
}

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_providers_includes_ollama() {
        let providers = default_providers();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].name, "local");
        assert!(matches!(providers[0].provider, Provider::Ollama { .. }));
    }

    #[test]
    fn provider_config_serde_roundtrip() {
        let config = ProviderConfig {
            name: "test".into(),
            provider: Provider::OpenAiCompat {
                url: "http://localhost:8080".into(),
                api_key: "sk-test".into(),
                model: "gpt-4".into(),
            },
            default_model: Some("gpt-4".into()),
        };
        let json = serde_json::to_string(&config).unwrap();
        let back: ProviderConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "test");
    }

    #[test]
    fn provider_response_fields() {
        let resp = ProviderResponse {
            text: "hello".into(),
            model: "test-model".into(),
            provider_name: "test".into(),
            latency_ms: 100,
            tokens_out: Some(5),
        };
        assert_eq!(resp.text, "hello");
        assert_eq!(resp.tokens_out, Some(5));
    }
}
