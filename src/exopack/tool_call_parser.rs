// Unlicense — public domain — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! tool_call_parser — Diamond-pattern test suite for f140 (parse_tool_calls).
//!
//! cc_features exercises tools via MCP, which delivers already-parsed JSON-RPC
//! args. f140 is what parses *free-form LLM output* in the agent loop — it has
//! a separate failure surface (malformed JSON, missing tool key, code blocks
//! vs bare JSON, etc.) that MCP can't reach. This suite covers it directly via
//! the public lib API, no #[cfg(test)] forks.
//!
//! f418=run_tool_call_parser_suite.

use std::time::Instant;

use crate::tools::f140;

type Scenario = fn() -> Result<(), String>;

const SCENARIOS: &[(&str, Scenario)] = &[
    ("f140_parses_json_code_block", s_json_block),
    ("f140_parses_bare_json_with_tool_key", s_bare_json),
    ("f140_ignores_malformed_json", s_malformed),
    ("f140_yields_empty_on_no_tool_calls", s_no_calls),
    ("f140_extracts_args_correctly", s_args_extracted),
    ("f140_handles_multiple_calls_in_one_response", s_multiple_calls),
];

/// f418=run_tool_call_parser_suite. Black-box parser tests via the public f140
/// API. Returns (all_passed, report).
pub fn f418() -> (bool, String) {
    let mut report = String::new();
    let mut passed = 0usize;
    let mut failed = 0usize;
    let t_start = Instant::now();

    report.push_str(&format!(
        "tool_call_parser: f140 (parse_tool_calls) suite\n  scenarios: {}\n",
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
        "tool_call_parser summary: {passed}/{total} passed in {total_ms}ms\n"
    ));
    (failed == 0, report)
}

// ── Scenarios ────────────────────────────────────────────

fn s_json_block() -> Result<(), String> {
    let text = "Let me read it.\n```json\n{\"tool\": \"read_file\", \"args\": {\"path\": \"src/lib.rs\"}}\n```\n";
    let calls = f140(text);
    if calls.len() != 1 {
        return Err(format!("expected 1 call from code block; got {}", calls.len()));
    }
    if calls[0].tool != "read_file" {
        return Err(format!("wrong tool: {}", calls[0].tool));
    }
    if calls[0].args.get("path").map(String::as_str) != Some("src/lib.rs") {
        return Err(format!("wrong path arg: {:?}", calls[0].args.get("path")));
    }
    Ok(())
}

fn s_bare_json() -> Result<(), String> {
    let text = r#"Calling: {"tool": "grep", "args": {"pattern": "TODO"}}"#;
    let calls = f140(text);
    if calls.len() != 1 {
        return Err(format!("expected 1 call from bare JSON; got {}", calls.len()));
    }
    if calls[0].tool != "grep" {
        return Err(format!("wrong tool: {}", calls[0].tool));
    }
    if calls[0].args.get("pattern").map(String::as_str) != Some("TODO") {
        return Err("wrong pattern arg".into());
    }
    Ok(())
}

fn s_malformed() -> Result<(), String> {
    let text = "```json\n{\"tool\": \"read_file\" \"args\": broken}\n```";
    let calls = f140(text);
    if !calls.is_empty() {
        return Err(format!(
            "malformed JSON should yield no calls; got {}",
            calls.len()
        ));
    }
    Ok(())
}

fn s_no_calls() -> Result<(), String> {
    let calls = f140("Just prose, no JSON, no tool calls here.");
    if !calls.is_empty() {
        return Err(format!("expected empty; got {} calls", calls.len()));
    }
    Ok(())
}

fn s_args_extracted() -> Result<(), String> {
    let text = "```json\n{\"tool\": \"edit_file\", \"args\": {\"path\": \"x.rs\", \"old_text\": \"foo\", \"new_text\": \"bar\"}}\n```";
    let calls = f140(text);
    if calls.len() != 1 {
        return Err(format!("expected 1 call; got {}", calls.len()));
    }
    let args = &calls[0].args;
    if args.get("path").map(String::as_str) != Some("x.rs") {
        return Err("missing/wrong path".into());
    }
    if args.get("old_text").map(String::as_str) != Some("foo") {
        return Err("missing/wrong old_text".into());
    }
    if args.get("new_text").map(String::as_str) != Some("bar") {
        return Err("missing/wrong new_text".into());
    }
    Ok(())
}

fn s_multiple_calls() -> Result<(), String> {
    // Two separate code blocks in one response — the parser should yield both.
    let text = "\
First step:\n\
```json\n\
{\"tool\": \"read_file\", \"args\": {\"path\": \"a.rs\"}}\n\
```\n\
Then:\n\
```json\n\
{\"tool\": \"grep\", \"args\": {\"pattern\": \"main\"}}\n\
```\n";
    let calls = f140(text);
    if calls.len() != 2 {
        return Err(format!("expected 2 calls; got {}", calls.len()));
    }
    if calls[0].tool != "read_file" || calls[1].tool != "grep" {
        return Err(format!(
            "wrong tool order: {} then {}",
            calls[0].tool, calls[1].tool
        ));
    }
    Ok(())
}
