// Unlicense — public domain — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! router_training_tests — Diamond-pattern suite for the tier-1 tool_router.
//!
//! Each scenario trains a fresh tool_router on synthetic examples in a tempdir
//! and asserts the classifier behaves correctly. After this module passes, the
//! router_spec module's PEND entries can flip to live assertions (it picks up
//! the trained checkpoint via `KOVA_TOOL_ROUTER_PATH` or the default path).
//!
//! f428=run_router_training_suite.

use std::time::Instant;

use tempfile::TempDir;

use crate::swarm::tool_router::{
    class_to_tool, f424, f425, tool_to_class, RouterTrainConfig, KOVA_ROUTER_TOOLS,
};
use crate::swarm::train::Example;

type Scenario = fn() -> Result<(), String>;

const SCENARIOS: &[(&str, Scenario)] = &[
    ("kova_router_tools_stable_ordering", s_class_ordering),
    ("tool_to_class_roundtrip", s_class_roundtrip),
    ("tool_to_class_unknown_returns_none", s_class_unknown),
    ("router_trains_on_synthetic_corpus", s_train_synth),
    ("router_predicts_read_for_show_me_prompts", s_pred_read),
    ("router_predicts_write_for_create_prompts", s_pred_write),
    ("router_predicts_edit_for_change_prompts", s_pred_edit),
    ("router_predicts_grep_for_find_prompts", s_pred_grep),
    ("router_predicts_glob_for_list_prompts", s_pred_glob),
    ("router_predicts_exec_for_run_prompts", s_pred_exec),
    ("router_predicts_undo_for_revert_prompts", s_pred_undo),
    ("router_predicts_memory_for_remember_prompts", s_pred_memory),
    ("router_top1_accuracy_above_threshold_on_train", s_train_accuracy),
    ("router_class_names_persisted_in_config", s_config_class_names),
];

/// f428=run_router_training_suite. Returns (all_passed, report).
pub fn f428() -> (bool, String) {
    let mut report = String::new();
    let mut passed = 0usize;
    let mut failed = 0usize;
    let t_start = Instant::now();

    report.push_str(&format!(
        "router_training_tests: tier-1 tool_router classifier suite\n  scenarios: {}\n",
        SCENARIOS.len()
    ));

    for (name, scenario) in SCENARIOS {
        let t0 = Instant::now();
        let res = (scenario)();
        let ms = t0.elapsed().as_millis();
        match res {
            Ok(()) => {
                report.push_str(&format!("  [PASS] {name} ({ms}ms)\n"));
                passed += 1;
            }
            Err(e) => {
                report.push_str(&format!("  [FAIL] {name} ({ms}ms): {e}\n"));
                failed += 1;
            }
        }
    }

    let total_ms = t_start.elapsed().as_millis();
    let total = passed + failed;
    report.push_str(&format!(
        "router_training_tests summary: {passed}/{total} passed in {total_ms}ms\n"
    ));
    (failed == 0, report)
}

// ── Fixture: synthetic training corpus ─────────────────

/// Reuse the canonical synthetic pairs from swarm::tool_router so test fixture
/// and CLI (`kova train-router --mix-synthetic`) share one source of truth.
fn synth_examples() -> Vec<Example> {
    crate::swarm::tool_router::synth_examples()
}

fn fast_config() -> RouterTrainConfig {
    // Smaller feature dim + fewer epochs keep the synthetic suite fast (sub-2s
    // per scenario). Full real-data training in `kova train-router` would use
    // the Default (feature_dim=8192, epochs=30).
    RouterTrainConfig {
        feature_dim: 2048,
        epochs: 40,
        lr: 0.02,
    }
}

/// Train a fresh model into a tempdir, return the model directory path. The
/// TempDir is leaked into the scenario so the model files outlive the call —
/// callers should keep the returned TempDir alive for the lifetime of their
/// predictions.
fn train_synth() -> Result<(TempDir, std::path::PathBuf), String> {
    let dir = TempDir::new().map_err(|e| format!("tempdir: {e}"))?;
    let examples = synth_examples();
    if examples.is_empty() {
        return Err("synth_examples produced 0 examples — class names drift?".into());
    }
    let cfg = fast_config();
    let model_dir = f424(&examples, dir.path(), &cfg)?;
    Ok((dir, model_dir))
}

fn predict_one(model_dir: &std::path::Path, prompt: &str) -> Result<(String, f32), String> {
    f425(model_dir, prompt)
}

// ── Scenarios: name mapping ────────────────────────────

fn s_class_ordering() -> Result<(), String> {
    // KOVA_ROUTER_TOOLS must contain at least the 8 router_spec tools.
    for required in [
        "read_file",
        "write_file",
        "edit_file",
        "grep",
        "glob",
        "exec",
        "undo_edit",
        "memory_write",
    ] {
        if !KOVA_ROUTER_TOOLS.iter().any(|t| *t == required) {
            return Err(format!("KOVA_ROUTER_TOOLS missing required tool: {required}"));
        }
    }
    Ok(())
}

fn s_class_roundtrip() -> Result<(), String> {
    for (idx, name) in KOVA_ROUTER_TOOLS.iter().enumerate() {
        let class = tool_to_class(name)
            .ok_or_else(|| format!("tool_to_class('{name}') returned None"))?;
        if class != idx {
            return Err(format!(
                "tool_to_class('{name}') = {class}; expected {idx}"
            ));
        }
        let back = class_to_tool(idx).ok_or_else(|| format!("class_to_tool({idx}) None"))?;
        if back != *name {
            return Err(format!("class_to_tool({idx}) = {back}; expected {name}"));
        }
    }
    Ok(())
}

fn s_class_unknown() -> Result<(), String> {
    for unknown in ["NotebookEdit", "ScheduleWakeup", "ToolSearch", ""] {
        if tool_to_class(unknown).is_some() {
            return Err(format!("tool_to_class('{unknown}') should be None"));
        }
    }
    Ok(())
}

// ── Scenarios: training + prediction ────────────────────

fn s_train_synth() -> Result<(), String> {
    let (_dir, model_dir) = train_synth()?;
    if !model_dir.exists() {
        return Err(format!("model dir not created at {}", model_dir.display()));
    }
    if !model_dir.join("weights.bin").exists() {
        return Err("weights.bin not written".into());
    }
    if !model_dir.join("bias.bin").exists() {
        return Err("bias.bin not written".into());
    }
    if !model_dir.join("config.json").exists() {
        return Err("config.json not written".into());
    }
    Ok(())
}

fn assert_pred(
    model_dir: &std::path::Path,
    prompt: &str,
    expected: &str,
) -> Result<(), String> {
    let (pred, conf) = predict_one(model_dir, prompt)?;
    if pred != expected {
        return Err(format!(
            "classify({prompt:?}) = ({pred}, {conf:.2}); expected {expected}"
        ));
    }
    Ok(())
}

fn s_pred_read() -> Result<(), String> {
    let (_d, m) = train_synth()?;
    assert_pred(&m, "show me src/lib.rs", "read_file")
}

fn s_pred_write() -> Result<(), String> {
    let (_d, m) = train_synth()?;
    assert_pred(&m, "create hello.txt with hi", "write_file")
}

fn s_pred_edit() -> Result<(), String> {
    let (_d, m) = train_synth()?;
    assert_pred(&m, "change foo to bar in lib.rs", "edit_file")
}

fn s_pred_grep() -> Result<(), String> {
    let (_d, m) = train_synth()?;
    assert_pred(&m, "find all TODO comments", "grep")
}

fn s_pred_glob() -> Result<(), String> {
    let (_d, m) = train_synth()?;
    assert_pred(&m, "list all .rs files in src", "glob")
}

fn s_pred_exec() -> Result<(), String> {
    let (_d, m) = train_synth()?;
    assert_pred(&m, "run cargo test", "exec")
}

fn s_pred_undo() -> Result<(), String> {
    let (_d, m) = train_synth()?;
    assert_pred(&m, "undo my last edit to lib.rs", "undo_edit")
}

fn s_pred_memory() -> Result<(), String> {
    let (_d, m) = train_synth()?;
    assert_pred(&m, "remember I prefer tabs", "memory_write")
}

fn s_train_accuracy() -> Result<(), String> {
    let (_d, m) = train_synth()?;
    let examples = synth_examples();
    let mut correct = 0usize;
    for ex in &examples {
        let (pred, _conf) = predict_one(&m, &ex.text)?;
        let expected = class_to_tool(ex.label).unwrap_or("?");
        if pred == expected {
            correct += 1;
        }
    }
    let acc = correct as f32 / examples.len() as f32;
    let floor = 0.80;
    if acc < floor {
        return Err(format!(
            "train accuracy {:.1}% below floor {:.0}%",
            acc * 100.0,
            floor * 100.0
        ));
    }
    Ok(())
}

fn s_config_class_names() -> Result<(), String> {
    let (_d, m) = train_synth()?;
    let config_str = std::fs::read_to_string(m.join("config.json"))
        .map_err(|e| format!("read config: {e}"))?;
    let v: serde_json::Value =
        serde_json::from_str(&config_str).map_err(|e| format!("parse config: {e}"))?;
    let names = v
        .get("class_names")
        .and_then(|n| n.as_array())
        .ok_or("class_names missing")?;
    if names.len() != KOVA_ROUTER_TOOLS.len() {
        return Err(format!(
            "class_names length {} != KOVA_ROUTER_TOOLS length {}",
            names.len(),
            KOVA_ROUTER_TOOLS.len()
        ));
    }
    for (i, expected) in KOVA_ROUTER_TOOLS.iter().enumerate() {
        let got = names[i].as_str().unwrap_or("");
        if got != *expected {
            return Err(format!(
                "class_names[{i}] = {got}; expected {expected}"
            ));
        }
    }
    Ok(())
}
