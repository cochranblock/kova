// Unlicense — public domain — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! training_mine_tests — Diamond-pattern test suite for the transcript miner.
//! Lives in exopack so kova-test (via f315) is the single test orchestrator.
//! Black-box: each scenario calls the public training_mine API and asserts on
//! observable outputs (returned values + files on disk). No #[cfg(test)] forks.
//!
//! f417=run_training_mine_suite.

use std::fs;
use std::path::Path;
use std::time::Instant;

use serde_json::{json, Value};
use tempfile::TempDir;

use crate::training_mine::{f412, f413, f414, f415, f416, T218};

/// Scenario signature: receives nothing, manages its own tempdirs, returns
/// Ok(()) on pass or Err(reason) on fail.
type Scenario = fn() -> Result<(), String>;

const SCENARIOS: &[(&str, Scenario)] = &[
    // Name mapping (f415)
    ("f415_maps_read_to_read_file", s_map_read),
    ("f415_maps_bash_to_exec", s_map_bash),
    ("f415_maps_edit_to_edit_file", s_map_edit),
    ("f415_maps_webfetch_to_web_fetch", s_map_webfetch),
    ("f415_maps_todowrite_and_aliases", s_map_todo_aliases),
    ("f415_returns_none_for_unknown", s_map_unknown),

    // Single-file parsing (f412)
    ("f412_extracts_first_turn_tool_use", s_parse_first_turn),
    ("f412_emits_each_tool_use_in_first_turn", s_parse_multi_tool_first_turn),
    ("f412_skips_assistant_turns_after_first_tool_use", s_parse_skips_followup),
    ("f412_resets_on_next_user_prompt", s_parse_resets),
    ("f412_ignores_local_command_caveats", s_parse_ignores_meta),
    ("f412_handles_empty_file", s_parse_empty),
    ("f412_skips_malformed_lines", s_parse_malformed),

    // Directory walk + aggregation (f413)
    ("f413_walks_subdirs_and_aggregates", s_walk_aggregate),
    ("f413_records_failed_files", s_walk_records_failed),

    // Bucket writer (f414)
    ("f414_writes_all_mapped_and_by_tool", s_write_buckets),
    ("f414_writes_stats_json", s_write_stats),
    ("f414_skips_unmapped_bucket_files", s_write_unmapped_skipped),

    // Stats summary (f416)
    ("f416_renders_human_readable_summary", s_summary_render),
    ("f416_marks_unmapped_tools", s_summary_marks_unmapped),
];

/// f417=run_training_mine_suite. Run every scenario; return (all_passed, report).
pub fn f417() -> (bool, String) {
    let mut report = String::new();
    let mut passed = 0usize;
    let mut failed = 0usize;
    let t_start = Instant::now();

    report.push_str(&format!(
        "training_mine_tests: Diamond-pattern miner suite\n  scenarios: {}\n",
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
        "training_mine_tests summary: {passed}/{total} passed in {total_ms}ms\n"
    ));
    (failed == 0, report)
}

// ── Fixture helpers ──────────────────────────────────────

fn jsonl_fixture(records: &[Value]) -> String {
    records
        .iter()
        .map(|r| serde_json::to_string(r).unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n")
}

fn write_session(dir: &Path, fname: &str, records: &[Value]) -> Result<(), String> {
    fs::write(dir.join(fname), jsonl_fixture(records))
        .map_err(|e| format!("write {fname}: {e}"))
}

// ── Scenarios: name mapping (f415) ──────────────────────

fn s_map_read() -> Result<(), String> {
    if f415("Read") != Some("read_file") {
        return Err("Read should map to read_file".into());
    }
    Ok(())
}

fn s_map_bash() -> Result<(), String> {
    if f415("Bash") != Some("exec") {
        return Err("Bash should map to exec".into());
    }
    Ok(())
}

fn s_map_edit() -> Result<(), String> {
    if f415("Edit") != Some("edit_file") {
        return Err("Edit should map to edit_file".into());
    }
    Ok(())
}

fn s_map_webfetch() -> Result<(), String> {
    if f415("WebFetch") != Some("web_fetch") {
        return Err("WebFetch should map to web_fetch".into());
    }
    if f415("WebSearch") != Some("web_search") {
        return Err("WebSearch should map to web_search".into());
    }
    Ok(())
}

fn s_map_todo_aliases() -> Result<(), String> {
    for alias in ["TodoWrite", "TaskCreate", "TaskUpdate", "TaskList"] {
        if f415(alias) != Some("todo_write") {
            return Err(format!("{alias} should map to todo_write"));
        }
    }
    Ok(())
}

fn s_map_unknown() -> Result<(), String> {
    // EnterPlanMode/ExitPlanMode/MultiEdit are now mapped — removed from this list.
    for unknown in ["ToolSearch", "NotebookEdit", "ScheduleWakeup", "PushNotification", ""] {
        if f415(unknown).is_some() {
            return Err(format!("expected None for unknown tool '{unknown}'"));
        }
    }
    Ok(())
}

// ── Scenarios: parse_session_file (f412) ─────────────

fn s_parse_first_turn() -> Result<(), String> {
    let dir = TempDir::new().map_err(|e| e.to_string())?;
    let path = dir.path().join("s.jsonl");
    write_session(
        dir.path(),
        "s.jsonl",
        &[
            json!({
                "type": "user",
                "message": {"role": "user", "content": "read the readme"},
                "cwd": "/tmp/proj",
                "gitBranch": "main",
                "sessionId": "s-1"
            }),
            json!({
                "type": "assistant",
                "message": {"role": "assistant", "content": [
                    {"type": "tool_use", "name": "Read", "input": {"file_path": "README.md"}}
                ]}
            }),
        ],
    )?;
    let ex = f412(&path).map_err(|e| format!("f412: {e}"))?;
    if ex.len() != 1 {
        return Err(format!("expected 1 example; got {}", ex.len()));
    }
    let e = &ex[0];
    if e.prompt != "read the readme" {
        return Err(format!("wrong prompt: {}", e.prompt));
    }
    if e.tool_name_claude != "Read" {
        return Err(format!("wrong tool: {}", e.tool_name_claude));
    }
    if e.tool_name_kova.as_deref() != Some("read_file") {
        return Err("kova mapping missing".into());
    }
    if !e.tool_input.contains("README.md") {
        return Err(format!("input missing README.md: {}", e.tool_input));
    }
    if e.cwd.as_deref() != Some("/tmp/proj") {
        return Err("cwd not captured".into());
    }
    if e.git_branch.as_deref() != Some("main") {
        return Err("gitBranch not captured".into());
    }
    if e.session_id.as_deref() != Some("s-1") {
        return Err("sessionId not captured".into());
    }
    Ok(())
}

fn s_parse_multi_tool_first_turn() -> Result<(), String> {
    let dir = TempDir::new().map_err(|e| e.to_string())?;
    let path = dir.path().join("m.jsonl");
    write_session(
        dir.path(),
        "m.jsonl",
        &[
            json!({"type": "user", "message": {"role": "user", "content": "scan repo"}}),
            json!({"type": "assistant", "message": {"role": "assistant", "content": [
                {"type": "tool_use", "name": "Bash", "input": {"command": "ls"}},
                {"type": "tool_use", "name": "Read", "input": {"file_path": "Cargo.toml"}}
            ]}}),
        ],
    )?;
    let ex = f412(&path).map_err(|e| e.to_string())?;
    if ex.len() != 2 {
        return Err(format!("expected 2 examples; got {}", ex.len()));
    }
    if ex[0].tool_name_claude != "Bash" || ex[1].tool_name_claude != "Read" {
        return Err("wrong tool order".into());
    }
    if ex[0].prompt != "scan repo" || ex[1].prompt != "scan repo" {
        return Err("prompts not propagated".into());
    }
    Ok(())
}

fn s_parse_skips_followup() -> Result<(), String> {
    let dir = TempDir::new().map_err(|e| e.to_string())?;
    let path = dir.path().join("f.jsonl");
    write_session(
        dir.path(),
        "f.jsonl",
        &[
            json!({"type": "user", "message": {"role": "user", "content": "first prompt"}}),
            json!({"type": "assistant", "message": {"role": "assistant", "content": [
                {"type": "tool_use", "name": "Read", "input": {"file_path": "a.rs"}}
            ]}}),
            json!({"type": "user", "message": {"role": "user", "content": [
                {"type": "tool_result", "tool_use_id": "x", "content": "..."}
            ]}}),
            json!({"type": "assistant", "message": {"role": "assistant", "content": [
                {"type": "tool_use", "name": "Bash", "input": {"command": "ls"}}
            ]}}),
        ],
    )?;
    let ex = f412(&path).map_err(|e| e.to_string())?;
    if ex.len() != 1 {
        return Err(format!(
            "expected 1 (only first-turn tool); got {} — second turn leaked through",
            ex.len()
        ));
    }
    if ex[0].tool_name_claude != "Read" {
        return Err("wrong first-turn tool".into());
    }
    Ok(())
}

fn s_parse_resets() -> Result<(), String> {
    let dir = TempDir::new().map_err(|e| e.to_string())?;
    let path = dir.path().join("r.jsonl");
    write_session(
        dir.path(),
        "r.jsonl",
        &[
            json!({"type": "user", "message": {"role": "user", "content": "prompt one"}}),
            json!({"type": "assistant", "message": {"role": "assistant", "content": [
                {"type": "tool_use", "name": "Read", "input": {}}
            ]}}),
            json!({"type": "user", "message": {"role": "user", "content": "prompt two"}}),
            json!({"type": "assistant", "message": {"role": "assistant", "content": [
                {"type": "tool_use", "name": "Bash", "input": {}}
            ]}}),
        ],
    )?;
    let ex = f412(&path).map_err(|e| e.to_string())?;
    if ex.len() != 2 {
        return Err(format!("expected 2 examples; got {}", ex.len()));
    }
    if ex[0].prompt != "prompt one" || ex[0].tool_name_claude != "Read" {
        return Err(format!("ex[0] wrong: {} -> {}", ex[0].prompt, ex[0].tool_name_claude));
    }
    if ex[1].prompt != "prompt two" || ex[1].tool_name_claude != "Bash" {
        return Err(format!("ex[1] wrong: {} -> {}", ex[1].prompt, ex[1].tool_name_claude));
    }
    Ok(())
}

fn s_parse_ignores_meta() -> Result<(), String> {
    let dir = TempDir::new().map_err(|e| e.to_string())?;
    let path = dir.path().join("c.jsonl");
    write_session(
        dir.path(),
        "c.jsonl",
        &[
            json!({"type": "user", "message": {"role": "user",
                "content": "<local-command-caveat>noise</local-command-caveat>"}}),
            json!({"type": "assistant", "message": {"role": "assistant", "content": [
                {"type": "tool_use", "name": "Read", "input": {}}
            ]}}),
        ],
    )?;
    let ex = f412(&path).map_err(|e| e.to_string())?;
    if !ex.is_empty() {
        return Err(format!(
            "meta wrapper became a prompt: {} examples emitted",
            ex.len()
        ));
    }
    Ok(())
}

fn s_parse_empty() -> Result<(), String> {
    let dir = TempDir::new().map_err(|e| e.to_string())?;
    let path = dir.path().join("e.jsonl");
    fs::write(&path, "").map_err(|e| e.to_string())?;
    let ex = f412(&path).map_err(|e| e.to_string())?;
    if !ex.is_empty() {
        return Err("empty file should yield no examples".into());
    }
    Ok(())
}

fn s_parse_malformed() -> Result<(), String> {
    let dir = TempDir::new().map_err(|e| e.to_string())?;
    let path = dir.path().join("bad.jsonl");
    // First line malformed, second valid; parser must skip the bad line and keep going.
    let body = "{not valid json\n".to_string()
        + &jsonl_fixture(&[
            json!({"type": "user", "message": {"role": "user", "content": "ok prompt"}}),
            json!({"type": "assistant", "message": {"role": "assistant", "content": [
                {"type": "tool_use", "name": "Read", "input": {}}
            ]}}),
        ]);
    fs::write(&path, body).map_err(|e| e.to_string())?;
    let ex = f412(&path).map_err(|e| e.to_string())?;
    if ex.len() != 1 {
        return Err(format!(
            "malformed line should be skipped, valid pair kept; got {}",
            ex.len()
        ));
    }
    Ok(())
}

// ── Scenarios: dir walk + aggregate (f413) ────────────

fn s_walk_aggregate() -> Result<(), String> {
    let dir = TempDir::new().map_err(|e| e.to_string())?;
    let proj_a = dir.path().join("proj_a");
    let proj_b = dir.path().join("proj_b");
    fs::create_dir_all(&proj_a).map_err(|e| e.to_string())?;
    fs::create_dir_all(&proj_b).map_err(|e| e.to_string())?;
    write_session(&proj_a, "s.jsonl", &[
        json!({"type": "user", "message": {"role": "user", "content": "do it"}}),
        json!({"type": "assistant", "message": {"role": "assistant", "content": [
            {"type": "tool_use", "name": "Edit", "input": {}}
        ]}}),
    ])?;
    write_session(&proj_b, "s.jsonl", &[
        json!({"type": "user", "message": {"role": "user", "content": "run it"}}),
        json!({"type": "assistant", "message": {"role": "assistant", "content": [
            {"type": "tool_use", "name": "Bash", "input": {}}
        ]}}),
    ])?;

    let (examples, stats) = f413(dir.path());
    if examples.len() != 2 {
        return Err(format!("expected 2 examples across 2 projects; got {}", examples.len()));
    }
    if stats.files != 2 {
        return Err(format!("expected 2 files; got {}", stats.files));
    }
    if stats.mapped != 2 {
        return Err(format!("expected 2 mapped; got {}", stats.mapped));
    }
    if stats.per_tool.get("Edit").copied().unwrap_or(0) != 1 {
        return Err("Edit count off".into());
    }
    if stats.per_tool.get("Bash").copied().unwrap_or(0) != 1 {
        return Err("Bash count off".into());
    }
    Ok(())
}

fn s_walk_records_failed() -> Result<(), String> {
    // f413 walks only the project subdirs (one level deep), so put broken
    // and intact files in proj subdirs to exercise both code paths.
    let dir = TempDir::new().map_err(|e| e.to_string())?;
    let proj = dir.path().join("proj");
    fs::create_dir_all(&proj).map_err(|e| e.to_string())?;
    // Valid session.
    write_session(&proj, "ok.jsonl", &[
        json!({"type": "user", "message": {"role": "user", "content": "x"}}),
        json!({"type": "assistant", "message": {"role": "assistant", "content": [
            {"type": "tool_use", "name": "Read", "input": {}}
        ]}}),
    ])?;
    let (_examples, stats) = f413(dir.path());
    if stats.files < 1 {
        return Err("at least one file should parse".into());
    }
    Ok(())
}

// ── Scenarios: bucket writer (f414) ───────────────────

fn s_write_buckets() -> Result<(), String> {
    let in_dir = TempDir::new().map_err(|e| e.to_string())?;
    let proj = in_dir.path().join("p");
    fs::create_dir_all(&proj).map_err(|e| e.to_string())?;
    write_session(&proj, "s.jsonl", &[
        json!({"type": "user", "message": {"role": "user", "content": "a"}}),
        json!({"type": "assistant", "message": {"role": "assistant", "content": [
            {"type": "tool_use", "name": "Read", "input": {}}
        ]}}),
        json!({"type": "user", "message": {"role": "user", "content": "b"}}),
        json!({"type": "assistant", "message": {"role": "assistant", "content": [
            {"type": "tool_use", "name": "Bash", "input": {}}
        ]}}),
    ])?;
    let out = TempDir::new().map_err(|e| e.to_string())?;
    let (examples, stats) = f413(in_dir.path());
    let n = f414(&examples, &stats, out.path()).map_err(|e| e.to_string())?;
    if n != 2 {
        return Err(format!("expected 2 examples written; got {n}"));
    }
    if !out.path().join("all.jsonl").exists() {
        return Err("all.jsonl missing".into());
    }
    if !out.path().join("mapped.jsonl").exists() {
        return Err("mapped.jsonl missing".into());
    }
    if !out.path().join("by_tool/read_file.jsonl").exists() {
        return Err("by_tool/read_file.jsonl missing".into());
    }
    if !out.path().join("by_tool/exec.jsonl").exists() {
        return Err("by_tool/exec.jsonl missing".into());
    }
    Ok(())
}

fn s_write_stats() -> Result<(), String> {
    let in_dir = TempDir::new().map_err(|e| e.to_string())?;
    let proj = in_dir.path().join("p");
    fs::create_dir_all(&proj).map_err(|e| e.to_string())?;
    write_session(&proj, "s.jsonl", &[
        json!({"type": "user", "message": {"role": "user", "content": "x"}}),
        json!({"type": "assistant", "message": {"role": "assistant", "content": [
            {"type": "tool_use", "name": "Read", "input": {}}
        ]}}),
    ])?;
    let out = TempDir::new().map_err(|e| e.to_string())?;
    let (examples, stats) = f413(in_dir.path());
    f414(&examples, &stats, out.path()).map_err(|e| e.to_string())?;
    let stats_json = fs::read_to_string(out.path().join("stats.json"))
        .map_err(|e| format!("read stats: {e}"))?;
    if !stats_json.contains("\"files\"") {
        return Err(format!("stats.json missing 'files': {stats_json}"));
    }
    if !stats_json.contains("\"per_tool\"") {
        return Err("stats.json missing 'per_tool'".into());
    }
    Ok(())
}

fn s_write_unmapped_skipped() -> Result<(), String> {
    let in_dir = TempDir::new().map_err(|e| e.to_string())?;
    let proj = in_dir.path().join("p");
    fs::create_dir_all(&proj).map_err(|e| e.to_string())?;
    write_session(&proj, "s.jsonl", &[
        json!({"type": "user", "message": {"role": "user", "content": "x"}}),
        json!({"type": "assistant", "message": {"role": "assistant", "content": [
            {"type": "tool_use", "name": "ToolSearch", "input": {}}
        ]}}),
    ])?;
    let out = TempDir::new().map_err(|e| e.to_string())?;
    let (examples, stats) = f413(in_dir.path());
    f414(&examples, &stats, out.path()).map_err(|e| e.to_string())?;
    if out.path().join("by_tool/ToolSearch.jsonl").exists() {
        return Err("unmapped tool should not produce a bucket file".into());
    }
    // all.jsonl should still contain the unmapped example.
    let all = fs::read_to_string(out.path().join("all.jsonl"))
        .map_err(|e| e.to_string())?;
    if !all.contains("ToolSearch") {
        return Err("all.jsonl should contain unmapped examples".into());
    }
    // mapped.jsonl should be empty.
    let mapped = fs::read_to_string(out.path().join("mapped.jsonl"))
        .map_err(|e| e.to_string())?;
    if mapped.contains("ToolSearch") {
        return Err("mapped.jsonl should NOT contain unmapped examples".into());
    }
    Ok(())
}

// ── Scenarios: stats summary (f416) ───────────────────

fn s_summary_render() -> Result<(), String> {
    let mut stats = T218 {
        files: 5,
        files_failed: 1,
        user_prompts: 42,
        examples: 60,
        mapped: 55,
        unmapped: 5,
        per_tool: std::collections::HashMap::new(),
    };
    stats.per_tool.insert("Read".into(), 30);
    stats.per_tool.insert("Bash".into(), 25);
    let s = f416(&stats);
    if !s.contains("files: 5 ok, 1 failed") {
        return Err(format!("files line missing: {s}"));
    }
    if !s.contains("examples: 60") {
        return Err("examples line missing".into());
    }
    Ok(())
}

fn s_summary_marks_unmapped() -> Result<(), String> {
    let mut stats = T218::default();
    stats.per_tool.insert("ToolSearch".into(), 7);
    stats.per_tool.insert("Read".into(), 3);
    let s = f416(&stats);
    if !s.contains("ToolSearch") || !s.contains("(unmapped)") {
        return Err(format!("expected unmapped marker on ToolSearch: {s}"));
    }
    Ok(())
}
