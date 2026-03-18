// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, Mattbusel (circuit breaker pattern)
//! runner — Execute a micro-model template against a cluster node.
//! Includes circuit breaker (Mattbusel/tokio-llm) and budget enforcement.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use super::template::T159;
use crate::cluster::T193;
use crate::providers::{T129, f199, f336};

/// T154=MicroResult
/// Result of running a micro-model.
#[derive(Debug, Clone)]
pub struct T154 {
    /// Template ID that was run.
    pub template_id: String,
    /// Node ID that handled the request.
    pub node_id: String,
    /// Model used.
    pub model: String,
    /// Raw response from the model.
    pub response: String,
    /// Time taken.
    pub duration: Duration,
    /// Tokens generated (if reported).
    pub tokens: Option<u64>,
}

/// T155=CircuitBreaker
/// Circuit breaker state — trips after N consecutive failures.
/// Inspired by Mattbusel/tokio-llm's circuit breaker pattern.
pub struct T155 {
    /// Consecutive failure count.
    failures: AtomicU32,
    /// Threshold before tripping.
    threshold: u32,
    /// Total requests.
    total: AtomicU64,
    /// Total failures.
    total_failures: AtomicU64,
}

impl T155 {
    pub fn new(threshold: u32) -> Self {
        T155 {
            failures: AtomicU32::new(0),
            threshold,
            total: AtomicU64::new(0),
            total_failures: AtomicU64::new(0),
        }
    }

    /// Check if the circuit is open (tripped).
    pub fn is_open(&self) -> bool {
        self.failures.load(Ordering::SeqCst) >= self.threshold
    }

    /// Record a success — resets consecutive failure counter.
    pub fn record_success(&self) {
        self.failures.store(0, Ordering::SeqCst);
        self.total.fetch_add(1, Ordering::SeqCst);
    }

    /// Record a failure — increments consecutive failure counter.
    pub fn record_failure(&self) {
        self.failures.fetch_add(1, Ordering::SeqCst);
        self.total.fetch_add(1, Ordering::SeqCst);
        self.total_failures.fetch_add(1, Ordering::SeqCst);
    }

    /// Reset the breaker.
    pub fn reset(&self) {
        self.failures.store(0, Ordering::SeqCst);
    }

    /// Failure rate (0.0-1.0).
    pub fn failure_rate(&self) -> f64 {
        let total = self.total.load(Ordering::SeqCst);
        if total == 0 {
            return 0.0;
        }
        self.total_failures.load(Ordering::SeqCst) as f64 / total as f64
    }
}

/// T156=Budget
/// Budget tracker — prevents runaway token usage.
/// Inspired by Mattbusel/tokio-llm's budget enforcement.
pub struct T156 {
    /// Max tokens allowed.
    pub limit: u64,
    /// Tokens used so far.
    used: AtomicU64,
}

impl T156 {
    pub fn new(limit: u64) -> Self {
        T156 {
            limit,
            used: AtomicU64::new(0),
        }
    }

    /// Check if budget is exhausted.
    pub fn exhausted(&self) -> bool {
        self.used.load(Ordering::SeqCst) >= self.limit
    }

    /// Record token usage.
    pub fn record(&self, tokens: u64) {
        self.used.fetch_add(tokens, Ordering::SeqCst);
    }

    /// Remaining tokens.
    pub fn remaining(&self) -> u64 {
        let used = self.used.load(Ordering::SeqCst);
        self.limit.saturating_sub(used)
    }

    /// Used tokens.
    pub fn used(&self) -> u64 {
        self.used.load(Ordering::SeqCst)
    }
}

/// f244=run_micro
/// Run a micro-model template against the cluster.
/// Unlike cluster.dispatch(), this does NOT enforce factory-level tier minimums.
/// Micro-models are intentionally small — a 3B fix_compile template should run
/// on any online node, not require a 32B Heavy node.
pub fn f244(
    template: &T159,
    input: &str,
    cluster: &T193,
    breaker: &T155,
    budget: &T156,
) -> Result<T154, String> {
    // Check circuit breaker
    if breaker.is_open() {
        return Err(format!(
            "circuit breaker open for {} (>{} consecutive failures)",
            template.id,
            breaker.failures.load(Ordering::SeqCst)
        ));
    }

    // Check budget
    if budget.exhausted() {
        return Err(format!(
            "budget exhausted ({}/{} tokens used)",
            budget.used(),
            budget.limit
        ));
    }

    let prompt = template.build_prompt(input);
    let start = Instant::now();

    // Find any online, non-busy node — micro-models don't need tier enforcement
    let node = cluster
        .online_nodes()
        .into_iter()
        .find(|n| !n.is_busy())
        .ok_or_else(|| format!("[{}] no available nodes", template.id))?;

    let node_id = node.id.clone();
    let url = node.base_url();

    // Use the template's preferred model. If not available on this node, try to find
    // a compatible model: same family, any size (e.g. qwen2.5-coder:3b → qwen2.5-coder:7b).
    let model = match pick_model(&url, &template.model) {
        Some(m) => m,
        None => template.model.clone(),
    };

    // Remote node → kova serve (OpenAI-compat).
    let provider = T129::OpenAiCompat {
        url: url.clone(),
        api_key: String::new(),
        model: model.clone(),
    };
    let result = f199(&provider, &model, &template.system_prompt, &prompt);

    match result {
        Ok(resp) => {
            let duration = start.elapsed();
            breaker.record_success();
            let est_tokens = resp.tokens_out.unwrap_or((resp.text.len() / 4) as u64);
            budget.record(est_tokens);

            Ok(T154 {
                template_id: template.id.clone(),
                node_id,
                model,
                response: resp.text,
                duration,
                tokens: Some(est_tokens),
            })
        }
        Err(e) => {
            breaker.record_failure();
            Err(format!("[{}] {}", template.id, e))
        }
    }
}

/// Pick the best available model on a node for a template's preferred model.
/// If the exact model exists, use it. Otherwise, find a model from the same family
/// (e.g. qwen2.5-coder:3b → qwen2.5-coder:7b).
fn pick_model(base_url: &str, preferred: &str) -> Option<String> {
    let provider = T129::OpenAiCompat {
        url: base_url.to_string(),
        api_key: String::new(),
        model: preferred.to_string(),
    };
    let models = f336(&provider).ok()?;
    let model_names: Vec<&str> = models.iter().map(|m| m.name.as_str()).collect();

    // Exact match
    if model_names.contains(&preferred) {
        return Some(preferred.to_string());
    }

    // Family match: strip the size tag (e.g. "qwen2.5-coder:3b" → "qwen2.5-coder")
    let family = preferred.split(':').next().unwrap_or(preferred);

    // Find any model from the same family
    model_names
        .iter()
        .find(|m| m.starts_with(family))
        .map(|m| m.to_string())
}

/// f245=run_micro_direct
/// Run a micro-model directly against a specific node URL (bypass cluster routing).
/// Used when you know exactly which node and model to hit.
pub fn f245(
    template: &T159,
    input: &str,
    base_url: &str,
    model_override: Option<&str>,
) -> Result<T154, String> {
    let prompt = template.build_prompt(input);
    let model = model_override.unwrap_or(&template.model);
    let start = Instant::now();

    let provider = T129::OpenAiCompat {
        url: base_url.to_string(),
        api_key: String::new(),
        model: model.to_string(),
    };
    let resp = f199(&provider, model, &template.system_prompt, &prompt)?;

    let duration = start.elapsed();
    let est_tokens = resp.tokens_out.unwrap_or((resp.text.len() / 4) as u64);

    Ok(T154 {
        template_id: template.id.clone(),
        node_id: "direct".into(),
        model: model.to_string(),
        response: resp.text,
        duration,
        tokens: Some(est_tokens),
    })
}
