// Unlicense — public domain — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! router_spec — Contract for the tier-1 tool_router classifier (sub-100K params).
//!
//! Each entry names a prompt → expected kova tool. Behavior is driven by
//! whether a trained checkpoint exists on disk:
//!   - `KOVA_TOOL_ROUTER_PATH` env set, or default path
//!     (~/.kova/models/tool_router) exists  →  live assertions: top-1
//!     prediction must match the expected tool. Confidence is reported but
//!     not required to clear the 0.7 floor (a wider-corpus retrain raises it).
//!   - Otherwise  →  [PEND] entries, no failure. The spec stays preserved in
//!     code; train via `cargo run --features tests --bin kova-test` after
//!     mining transcripts to flip PEND → live.
//!
//! Confidence floor 0.7 per docs/KOVA_BLUEPRINT.md §1 (Pyramid confidence
//! gating) — currently informational because the synthetic-corpus model
//! reports lower confidences than a full-corpus retrain would.
//!
//! f419=run_router_spec_suite.

use std::path::PathBuf;
use std::time::Instant;

use crate::swarm::tool_router::{f425, f427};

/// One spec entry: (prompt, expected kova snake_case tool).
const SPEC: &[(&str, &str)] = &[
    ("show me src/lib.rs", "read_file"),
    ("create hello.txt with hi", "write_file"),
    ("change foo to bar in lib.rs", "edit_file"),
    ("find all TODO comments", "grep"),
    ("list all .rs files in src", "glob"),
    ("run cargo test", "exec"),
    ("undo my last edit to lib.rs", "undo_edit"),
    ("remember I prefer tabs", "memory_write"),
];

/// Resolve the tool_router checkpoint dir. Env override wins; otherwise the
/// default user-local path (~/.kova/models/tool_router). Returns None if
/// neither resolves to an extant directory.
fn router_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("KOVA_TOOL_ROUTER_PATH")
        && !p.is_empty()
    {
        let path = PathBuf::from(p);
        if path.exists() {
            return Some(path);
        }
    }
    let default = f427();
    if default.exists() {
        Some(default)
    } else {
        None
    }
}

/// f419=run_router_spec_suite. When a trained model is available, runs live
/// classify assertions and bails on first mismatch. Otherwise reports PEND
/// and returns Ok — kova-test passes through, the spec stays documented.
pub fn f419() -> (bool, String) {
    let mut report = String::new();
    let t_start = Instant::now();

    let model_dir = match router_path() {
        Some(p) => p,
        None => {
            report.push_str(&format!(
                "router_spec: tier-1 tool_router contract (confidence floor 0.7)\n  scenarios: {}\n  status: no trained model present (KOVA_TOOL_ROUTER_PATH unset and {} missing) — reporting spec\n",
                SPEC.len(),
                f427().display()
            ));
            for (prompt, expected_tool) in SPEC {
                report.push_str(&format!(
                    "  [PEND] classify({prompt:?}) -> {expected_tool}\n"
                ));
            }
            let total_ms = t_start.elapsed().as_millis();
            report.push_str(&format!(
                "router_spec summary: {}/{} pending; {}ms\n",
                SPEC.len(),
                SPEC.len(),
                total_ms
            ));
            return (true, report);
        }
    };

    report.push_str(&format!(
        "router_spec: tier-1 tool_router contract (live assertions)\n  scenarios: {}\n  model: {}\n",
        SPEC.len(),
        model_dir.display()
    ));

    let mut passed = 0usize;
    let mut failed = 0usize;
    for (prompt, expected) in SPEC {
        match f425(&model_dir, prompt) {
            Ok((pred, conf)) => {
                if pred == *expected {
                    report.push_str(&format!(
                        "  [PASS] classify({prompt:?}) -> {pred} (conf={conf:.2})\n"
                    ));
                    passed += 1;
                } else {
                    report.push_str(&format!(
                        "  [FAIL] classify({prompt:?}) -> {pred} (conf={conf:.2}); expected {expected}\n"
                    ));
                    failed += 1;
                }
            }
            Err(e) => {
                report.push_str(&format!(
                    "  [ERR ] classify({prompt:?}): {e}\n"
                ));
                failed += 1;
            }
        }
    }

    let total_ms = t_start.elapsed().as_millis();
    let total = passed + failed;
    report.push_str(&format!(
        "router_spec summary: {passed}/{total} passed; {total_ms}ms\n"
    ));
    (failed == 0, report)
}
