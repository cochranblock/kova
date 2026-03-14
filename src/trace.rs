// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! trace — Pipeline trace + LLM call observability.
//! t93=LastTrace (in-memory pipeline trace).
//! LlmTrace: sled-backed per-call telemetry for every LLM invocation.

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

/// t93=LastTrace. Last pipeline run for Explain feature.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LastTrace {
    pub intent: String,
    pub user_msg: String,
    pub stage: String, // "compile" | "clippy" | "tests"
    pub stderr: String,
    pub retry_count: u32,
    pub outcome: String,    // "success" | "failed"
    pub chain: Vec<String>, // "Attempt 1: compile failed" etc.
}

// ── LLM Call Observability ──────────────────────────────────────

/// t109=LlmTrace. Single LLM call trace. Logged to sled after every inference.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LlmTrace {
    /// Unix timestamp (millis).
    pub ts: u64,
    /// Backend: "ollama" or "kalosm".
    pub backend: String,
    /// Model name (e.g. "qwen2.5-coder:1.5b").
    pub model: String,
    /// Node/URL that handled the request.
    pub node: String,
    /// Call type: "generate", "generate_stream", "chat", "code_gen".
    pub call_type: String,
    /// Wall-clock latency in milliseconds.
    pub latency_ms: u64,
    /// Output tokens (from ollama eval_count, or estimated).
    pub tokens_out: Option<u64>,
    /// Tokens per second (from ollama eval_duration, or computed).
    pub tok_per_sec: Option<f64>,
    /// Prompt length in bytes (rough input size proxy).
    pub prompt_bytes: usize,
    /// Response length in bytes.
    pub response_bytes: usize,
    /// "ok" or error message.
    pub status: String,
}

/// Sled tree name for LLM traces.
const LLM_TRACE_TREE: &str = "llm_traces";

/// Global sled db handle (shared with storage module).
static TRACE_DB: OnceLock<Option<sled::Db>> = OnceLock::new();

fn trace_db() -> Option<&'static sled::Db> {
    TRACE_DB
        .get_or_init(|| {
            let path = crate::config::sled_path();
            sled::open(&path).ok()
        })
        .as_ref()
}

/// f161=log_llm. Log an LLM trace to sled. Fire-and-forget.
pub fn log_llm(trace: LlmTrace) {
    if let Some(db) = trace_db() {
        if let Ok(tree) = db.open_tree(LLM_TRACE_TREE) {
            // Key: timestamp_millis + 4 random bytes for uniqueness
            let ts_bytes = trace.ts.to_be_bytes();
            let rand_bytes: [u8; 4] = {
                let mut buf = [0u8; 4];
                let seed = trace.ts ^ (trace.latency_ms << 16) ^ (trace.prompt_bytes as u64);
                buf[0] = seed as u8;
                buf[1] = (seed >> 8) as u8;
                buf[2] = (seed >> 16) as u8;
                buf[3] = (seed >> 24) as u8;
                buf
            };
            let mut key = Vec::with_capacity(12);
            key.extend_from_slice(&ts_bytes);
            key.extend_from_slice(&rand_bytes);

            if let Ok(val) = serde_json::to_vec(&trace) {
                let _ = tree.insert(key, val);
            }
        }
    }
}

/// f162=recent_llm_traces. Query recent LLM traces (most recent first).
pub fn recent_llm_traces(limit: usize) -> Vec<LlmTrace> {
    let Some(db) = trace_db() else {
        return Vec::new();
    };
    let Ok(tree) = db.open_tree(LLM_TRACE_TREE) else {
        return Vec::new();
    };

    let mut traces = Vec::new();
    for entry in tree.iter().rev() {
        if traces.len() >= limit {
            break;
        }
        if let Ok((_, val)) = entry {
            if let Ok(t) = serde_json::from_slice::<LlmTrace>(&val) {
                traces.push(t);
            }
        }
    }
    traces
}

/// t110=LlmStats. Summary stats for LLM traces.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmStats {
    pub total_calls: usize,
    pub total_errors: usize,
    pub total_tokens_out: u64,
    pub avg_latency_ms: f64,
    pub avg_tok_per_sec: f64,
    pub calls_by_model: Vec<(String, usize)>,
    pub calls_by_node: Vec<(String, usize)>,
}

/// f163=llm_stats. Compute aggregate stats from all stored LLM traces.
pub fn llm_stats() -> LlmStats {
    let Some(db) = trace_db() else {
        return empty_stats();
    };
    let Ok(tree) = db.open_tree(LLM_TRACE_TREE) else {
        return empty_stats();
    };

    let mut total = 0usize;
    let mut errors = 0usize;
    let mut tokens = 0u64;
    let mut latency_sum = 0u64;
    let mut tps_sum = 0.0f64;
    let mut tps_count = 0usize;
    let mut by_model: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut by_node: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    for entry in tree.iter() {
        let Ok((_, val)) = entry else { continue };
        let Ok(t) = serde_json::from_slice::<LlmTrace>(&val) else {
            continue;
        };
        total += 1;
        if t.status != "ok" {
            errors += 1;
        }
        tokens += t.tokens_out.unwrap_or(0);
        latency_sum += t.latency_ms;
        if let Some(tps) = t.tok_per_sec {
            tps_sum += tps;
            tps_count += 1;
        }
        *by_model.entry(t.model).or_default() += 1;
        *by_node.entry(t.node).or_default() += 1;
    }

    let mut model_vec: Vec<_> = by_model.into_iter().collect();
    model_vec.sort_by(|a, b| b.1.cmp(&a.1));
    let mut node_vec: Vec<_> = by_node.into_iter().collect();
    node_vec.sort_by(|a, b| b.1.cmp(&a.1));

    LlmStats {
        total_calls: total,
        total_errors: errors,
        total_tokens_out: tokens,
        avg_latency_ms: if total > 0 {
            latency_sum as f64 / total as f64
        } else {
            0.0
        },
        avg_tok_per_sec: if tps_count > 0 {
            tps_sum / tps_count as f64
        } else {
            0.0
        },
        calls_by_model: model_vec,
        calls_by_node: node_vec,
    }
}

fn empty_stats() -> LlmStats {
    LlmStats {
        total_calls: 0,
        total_errors: 0,
        total_tokens_out: 0,
        avg_latency_ms: 0.0,
        avg_tok_per_sec: 0.0,
        calls_by_model: Vec::new(),
        calls_by_node: Vec::new(),
    }
}

/// Get current timestamp in millis.
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// f164=print_llm_stats. Print formatted trace stats to stdout.
pub fn print_llm_stats() {
    let stats = llm_stats();
    println!("LLM Call Traces");
    println!("───────────────────────────────────────");
    println!(
        "Total calls: {} ({} errors, {:.1}% error rate)",
        stats.total_calls,
        stats.total_errors,
        if stats.total_calls > 0 {
            stats.total_errors as f64 / stats.total_calls as f64 * 100.0
        } else {
            0.0
        }
    );
    println!("Total tokens out: {}", stats.total_tokens_out);
    println!("Avg latency: {:.0}ms", stats.avg_latency_ms);
    println!("Avg tok/s: {:.1}", stats.avg_tok_per_sec);
    if !stats.calls_by_model.is_empty() {
        println!("\nBy model:");
        for (model, count) in &stats.calls_by_model {
            println!("  {:<30} {}", model, count);
        }
    }
    if !stats.calls_by_node.is_empty() {
        println!("\nBy node:");
        for (node, count) in &stats.calls_by_node {
            println!("  {:<30} {}", node, count);
        }
    }
}

/// f165=print_recent_traces. Print recent traces as a table.
pub fn print_recent_traces(limit: usize) {
    let traces = recent_llm_traces(limit);
    if traces.is_empty() {
        println!("No LLM traces recorded yet.");
        return;
    }
    println!(
        "{:<12} {:<25} {:<20} {:>8} {:>8} {:>8} {:<6}",
        "Backend", "Model", "Node", "Lat(ms)", "Tokens", "Tok/s", "Status"
    );
    println!("───────────────────────────────────────────────────────────────────────────────────────────");
    for t in &traces {
        let tps = t
            .tok_per_sec
            .map(|v| format!("{:.1}", v))
            .unwrap_or_else(|| "—".into());
        let tok = t
            .tokens_out
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".into());
        let status = if t.status == "ok" { "ok" } else { "ERR" };
        let model_short = if t.model.len() > 24 {
            &t.model[..24]
        } else {
            &t.model
        };
        let node_short = if t.node.len() > 19 {
            &t.node[..19]
        } else {
            &t.node
        };
        println!(
            "{:<12} {:<25} {:<20} {:>8} {:>8} {:>8} {:<6}",
            t.backend, model_short, node_short, t.latency_ms, tok, tps, status
        );
    }
}
