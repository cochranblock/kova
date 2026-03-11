// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, Mattbusel (circuit breaker pattern)
//! runner — Execute a micro-model template against a cluster node.
//! Includes circuit breaker (Mattbusel/tokio-llm) and budget enforcement.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use super::template::MicroTemplate;
use crate::cluster::{Cluster, TaskKind};
use crate::ollama;

/// Result of running a micro-model.
#[derive(Debug, Clone)]
pub struct MicroResult {
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

/// Circuit breaker state — trips after N consecutive failures.
/// Inspired by Mattbusel/tokio-llm's circuit breaker pattern.
pub struct CircuitBreaker {
    /// Consecutive failure count.
    failures: AtomicU32,
    /// Threshold before tripping.
    threshold: u32,
    /// Total requests.
    total: AtomicU64,
    /// Total failures.
    total_failures: AtomicU64,
}

impl CircuitBreaker {
    pub fn new(threshold: u32) -> Self {
        CircuitBreaker {
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

/// Budget tracker — prevents runaway token usage.
/// Inspired by Mattbusel/tokio-llm's budget enforcement.
pub struct Budget {
    /// Max tokens allowed.
    pub limit: u64,
    /// Tokens used so far.
    used: AtomicU64,
}

impl Budget {
    pub fn new(limit: u64) -> Self {
        Budget {
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

/// Map template name to TaskKind for cluster dispatch.
fn template_to_task(name: &str) -> TaskKind {
    match name {
        "classify_intent" => TaskKind::Classify,
        "fix_compile" => TaskKind::FixCompile,
        "clippy_fix" => TaskKind::ClippyFix,
        "code_review" => TaskKind::CodeReview,
        "test_write" => TaskKind::TestWrite,
        "code_gen" => TaskKind::CodeGen,
        _ => TaskKind::General,
    }
}

/// Run a micro-model template. Dispatches to the cluster, respecting circuit breaker and budget.
pub fn run_micro(
    template: &MicroTemplate,
    input: &str,
    cluster: &Cluster,
    breaker: &CircuitBreaker,
    budget: &Budget,
) -> Result<MicroResult, String> {
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

    // Try cluster dispatch first (routes to best available node)
    let task = template_to_task(&template.name);
    let result = cluster.dispatch(
        task,
        &template.system_prompt,
        &prompt,
        Some(template.num_ctx),
    );

    match result {
        Ok((node_id, response)) => {
            let duration = start.elapsed();
            breaker.record_success();
            // Estimate tokens from response length (rough: 1 token ≈ 4 chars)
            let est_tokens = (response.len() / 4) as u64;
            budget.record(est_tokens);

            Ok(MicroResult {
                template_id: template.id.clone(),
                node_id,
                model: template.model.clone(),
                response,
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

/// Run a micro-model directly against a specific node URL (bypass cluster routing).
/// Used when you know exactly which node and model to hit.
pub fn run_micro_direct(
    template: &MicroTemplate,
    input: &str,
    base_url: &str,
    model_override: Option<&str>,
) -> Result<MicroResult, String> {
    let prompt = template.build_prompt(input);
    let model = model_override.unwrap_or(&template.model);
    let start = Instant::now();

    let response = ollama::generate_with_temp(
        base_url,
        model,
        &template.system_prompt,
        &prompt,
        Some(template.num_ctx),
        Some(template.temperature),
    )?;

    let duration = start.elapsed();
    let est_tokens = (response.len() / 4) as u64;

    Ok(MicroResult {
        template_id: template.id.clone(),
        node_id: "direct".into(),
        model: model.to_string(),
        response,
        duration,
        tokens: Some(est_tokens),
    })
}
