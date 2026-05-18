#![allow(non_camel_case_types)]
//! trace — Pipeline trace + LLM call observability.
//! T93=LastTrace (in-memory pipeline trace).
//! T109=LlmTrace: redb-backed per-call telemetry for every LLM invocation.
// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use redb::{ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Pipeline execution stage — typed replacement for the old `stage: String`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Stage {
    #[default]
    Inference,
    Compile,
    Clippy,
    Tests,
}

impl std::fmt::Display for Stage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Inference => "inference",
            Self::Compile => "compile",
            Self::Clippy => "clippy",
            Self::Tests => "tests",
        })
    }
}

/// Pipeline execution outcome — typed replacement for the old `outcome: String`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    #[default]
    Pending,
    Success,
    Failed,
}

impl std::fmt::Display for Outcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Pending => "pending",
            Self::Success => "success",
            Self::Failed => "failed",
        })
    }
}

/// t93=T93. Last pipeline run for Explain feature.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct T93 {
    pub intent: String,
    pub user_msg: String,
    pub stage: Stage,
    pub stderr: String,
    pub retry_count: u32,
    pub outcome: Outcome,
    pub chain: Vec<String>, // "Attempt 1: compile failed" etc.
}

// ── LLM Call Observability ──────────────────────────────────────

/// t109=T109. Single LLM call trace. Logged to redb after every inference.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct T109 {
    pub ts: u64,
    pub backend: String,
    pub model: String,
    pub node: String,
    pub call_type: String,
    pub latency_ms: u64,
    pub tokens_out: Option<u64>,
    pub tok_per_sec: Option<f64>,
    pub prompt_bytes: usize,
    pub response_bytes: usize,
    pub status: String,
}

const LLM_TRACE_TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("llm_traces");

static TRACE_SEQ: AtomicU32 = AtomicU32::new(0);

/// f161=f161. Log an LLM trace. Fire-and-forget.
pub fn f161(trace: T109) {
    let Some(db) = crate::storage::global_db() else { return };
    let ts_bytes = trace.ts.to_be_bytes();
    let discriminant = TRACE_SEQ.fetch_add(1, Ordering::Relaxed).to_be_bytes();
    let mut key = Vec::with_capacity(12);
    key.extend_from_slice(&ts_bytes);
    key.extend_from_slice(&discriminant);

    let Ok(val) = serde_json::to_vec(&trace) else { return };
    let Ok(txn) = db.begin_write() else { return };
    {
        let Ok(mut table) = txn.open_table(LLM_TRACE_TABLE) else { return };
        let _ = table.insert(key.as_slice(), val.as_slice());
    }
    let _ = txn.commit();
}

/// f162=f162. Query recent LLM traces (most recent first).
pub fn f162(limit: usize) -> Vec<T109> {
    let Some(db) = crate::storage::global_db() else { return Vec::new() };
    let Ok(txn) = db.begin_read() else { return Vec::new() };
    let table = match txn.open_table(LLM_TRACE_TABLE) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };
    let Ok(iter) = table.iter() else { return Vec::new() };
    let mut traces = Vec::new();
    for entry in iter.rev() {
        if traces.len() >= limit { break; }
        if let Ok((_, val)) = entry
            && let Ok(t) = serde_json::from_slice::<T109>(val.value())
        {
            traces.push(t);
        }
    }
    traces
}

/// t110=T110. Summary stats for LLM traces.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct T110 {
    pub total_calls: usize,
    pub total_errors: usize,
    pub total_tokens_out: u64,
    pub avg_latency_ms: f64,
    pub avg_tok_per_sec: f64,
    pub calls_by_model: Vec<(String, usize)>,
    pub calls_by_node: Vec<(String, usize)>,
}

/// f163=f163. Compute aggregate stats from all stored LLM traces.
pub fn f163() -> T110 {
    let Some(db) = crate::storage::global_db() else { return empty_stats() };
    let Ok(txn) = db.begin_read() else { return empty_stats() };
    let table = match txn.open_table(LLM_TRACE_TABLE) {
        Ok(t) => t,
        Err(_) => return empty_stats(),
    };
    let Ok(iter) = table.iter() else { return empty_stats() };

    let mut total = 0usize;
    let mut errors = 0usize;
    let mut tokens = 0u64;
    let mut latency_sum = 0u64;
    let mut tps_sum = 0.0f64;
    let mut tps_count = 0usize;
    let mut by_model: std::collections::HashMap<String, usize> = Default::default();
    let mut by_node: std::collections::HashMap<String, usize> = Default::default();

    for entry in iter {
        let Ok((_, val)) = entry else { continue };
        let Ok(t) = serde_json::from_slice::<T109>(val.value()) else { continue };
        total += 1;
        if t.status != "ok" { errors += 1; }
        tokens += t.tokens_out.unwrap_or(0);
        latency_sum += t.latency_ms;
        if let Some(tps) = t.tok_per_sec { tps_sum += tps; tps_count += 1; }
        *by_model.entry(t.model).or_default() += 1;
        *by_node.entry(t.node).or_default() += 1;
    }

    let mut model_vec: Vec<_> = by_model.into_iter().collect();
    model_vec.sort_by(|a, b| b.1.cmp(&a.1));
    let mut node_vec: Vec<_> = by_node.into_iter().collect();
    node_vec.sort_by(|a, b| b.1.cmp(&a.1));

    T110 {
        total_calls: total,
        total_errors: errors,
        total_tokens_out: tokens,
        avg_latency_ms: if total > 0 { latency_sum as f64 / total as f64 } else { 0.0 },
        avg_tok_per_sec: if tps_count > 0 { tps_sum / tps_count as f64 } else { 0.0 },
        calls_by_model: model_vec,
        calls_by_node: node_vec,
    }
}

fn empty_stats() -> T110 {
    T110 {
        total_calls: 0,
        total_errors: 0,
        total_tokens_out: 0,
        avg_latency_ms: 0.0,
        avg_tok_per_sec: 0.0,
        calls_by_model: Vec::new(),
        calls_by_node: Vec::new(),
    }
}

/// f326=now_ms. Get current timestamp in millis.
pub fn f326() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// f164=f164. Print formatted trace stats to stdout.
pub fn f164() {
    let stats = f163();
    println!("LLM Call Traces");
    println!("───────────────────────────────────────");
    println!(
        "Total calls: {} ({} errors, {:.1}% error rate)",
        stats.total_calls,
        stats.total_errors,
        if stats.total_calls > 0 { stats.total_errors as f64 / stats.total_calls as f64 * 100.0 } else { 0.0 }
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

/// f165=f165. Print recent traces as a table.
pub fn f165(limit: usize) {
    let traces = f162(limit);
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
        let tps = t.tok_per_sec.map(|v| format!("{:.1}", v)).unwrap_or_else(|| "—".into());
        let tok = t.tokens_out.map(|v| v.to_string()).unwrap_or_else(|| "—".into());
        let status = if t.status == "ok" { "ok" } else { "ERR" };
        println!(
            "{:<12} {:<25} {:<20} {:>8} {:>8} {:>8} {:<6}",
            t.backend,
            t.model.get(..24.min(t.model.len())).unwrap_or(&t.model),
            t.node.get(..19.min(t.node.len())).unwrap_or(&t.node),
            t.latency_ms, tok, tps, status
        );
    }
}
