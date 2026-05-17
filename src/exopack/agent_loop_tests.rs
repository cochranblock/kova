// Unlicense — public domain — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! agent_loop_tests — Diamond-pattern end-to-end suite for the agent loop.
//!
//! Each scenario writes a mock-responses JSONL file, then spawns `kova chat`
//! with KOVA_INFERENCE=mock + KOVA_INFERENCE_MOCK_FILE pointed at it. The agent
//! loop pulls canned responses in order, parses tool calls, executes them, and
//! loops. Assertions hit filesystem side effects — clean signal, no need to
//! parse the REPL's stdout chrome.
//!
//! f423=run_agent_loop_suite.

use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::json;
use tempfile::TempDir;

/// Hard ceiling on a single `kova chat` subprocess. Prevents a buggy agent
/// loop from hanging the suite — failed scenario instead. Override via env
/// `KOVA_TEST_AGENT_TIMEOUT_SECS` for slow CI machines.
const AGENT_TIMEOUT_DEFAULT_SECS: u64 = 30;

fn agent_timeout() -> Duration {
    let secs = std::env::var("KOVA_TEST_AGENT_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(AGENT_TIMEOUT_DEFAULT_SECS);
    Duration::from_secs(secs)
}

type Scenario = fn(&Path, &Path, &Path) -> Result<(), String>;

const SCENARIOS: &[(&str, Scenario)] = &[
    ("agent_plain_text_no_tool_terminates", s_plain_text),
    ("agent_executes_write_file_then_done", s_write_then_done),
    ("agent_executes_read_file_then_done", s_read_then_done),
    ("agent_chains_write_then_read", s_chain_write_read),
    ("agent_executes_grep_call", s_grep_call),
    ("agent_recovers_from_malformed_then_done", s_malformed_then_done),
    ("agent_handles_overconsumption_gracefully", s_overconsume),
];

/// f423=run_agent_loop_suite. Black-box end-to-end via spawned `kova chat`
/// with mock inference. Returns (all_passed, report).
///
/// Resets the in-process mock cursor at entry as belt-and-braces — the real
/// isolation is per-subprocess (each `kova chat` spawn starts at cursor 0 in
/// its own address space), but if anyone ever calls f422 in-process from
/// kova-test, this prevents cross-suite cursor leak.
pub fn f423(kova_bin: &Path) -> (bool, String) {
    crate::inference::mock::reset_cursor();
    let mut report = String::new();
    let mut passed = 0usize;
    let mut failed = 0usize;
    let t_start = Instant::now();

    report.push_str(&format!(
        "agent_loop_tests: end-to-end via `kova chat` with KOVA_INFERENCE=mock\n  kova bin: {}\n  scenarios: {}\n",
        kova_bin.display(),
        SCENARIOS.len()
    ));

    for (name, scenario) in SCENARIOS {
        let project = match TempDir::new() {
            Ok(t) => t,
            Err(e) => {
                report.push_str(&format!("  [ERR ] {name}: tempdir(project): {e}\n"));
                failed += 1;
                continue;
            }
        };
        let home = match TempDir::new() {
            Ok(t) => t,
            Err(e) => {
                report.push_str(&format!("  [ERR ] {name}: tempdir(home): {e}\n"));
                failed += 1;
                continue;
            }
        };
        let t0 = Instant::now();
        let res = (scenario)(kova_bin, project.path(), home.path());
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
        "agent_loop_tests summary: {passed}/{total} passed in {total_ms}ms\n"
    ));
    (failed == 0, report)
}

// ── Helpers ────────────────────────────────────────────

/// Run `kova chat` with a mock-response sequence. Writes the mock file to
/// home/mock.jsonl, pipes the prompt to stdin, closes stdin (EOF), polls
/// `try_wait` up to agent_timeout(); kills the child on timeout. stdout/stderr
/// are drained on dedicated threads so the pipe buffer can't fill and deadlock
/// the child. Returns (stdout, stderr) or an error message that includes any
/// captured stderr for diagnostics.
fn run_chat(
    kova_bin: &Path,
    project: &Path,
    home: &Path,
    mock_responses: &[&str],
    user_prompt: &str,
) -> Result<(String, String), String> {
    let mock_file = home.join("mock.jsonl");
    let mut body = String::new();
    for r in mock_responses {
        let line = serde_json::to_string(&serde_json::json!({"text": r}))
            .map_err(|e| format!("json encode: {e}"))?;
        body.push_str(&line);
        body.push('\n');
    }
    fs::write(&mock_file, body).map_err(|e| format!("write mock file: {e}"))?;

    let mut child = Command::new(kova_bin)
        .arg("chat")
        .arg("--project")
        .arg(project)
        // env_clear() then explicit set — avoids leaking shell env (KOVA_PERMS,
        // KOVA_MODEL, KOVA_TOOL_ROUTER_PATH, etc.) from the parent process.
        .env_clear()
        .env("HOME", home)
        .env("KOVA_PROJECT", project)
        .env("KOVA_INFERENCE", "mock")
        .env("KOVA_INFERENCE_MOCK_FILE", &mock_file)
        // PATH is required so kova can spawn shell processes for `exec`.
        .env(
            "PATH",
            std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into()),
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn kova chat: {e}"))?;

    let mut stdin = child.stdin.take().ok_or("no stdin handle")?;
    let stdout_raw = child.stdout.take().ok_or("no stdout handle")?;
    let stderr_raw = child.stderr.take().ok_or("no stderr handle")?;

    stdin
        .write_all(format!("{user_prompt}\n").as_bytes())
        .map_err(|e| format!("write prompt: {e}"))?;
    drop(stdin); // EOF → REPL exits its loop after this prompt.

    // Drain stdout/stderr on threads so pipe buffers can't fill. Each thread
    // exits when the child closes its end of the pipe.
    let stdout_thread = thread::spawn(move || {
        let mut buf = String::new();
        let mut r = stdout_raw;
        let _ = r.read_to_string(&mut buf);
        buf
    });
    let stderr_thread = thread::spawn(move || {
        let mut buf = String::new();
        let mut r = stderr_raw;
        let _ = r.read_to_string(&mut buf);
        buf
    });

    // Poll try_wait so we can kill on timeout. Child stays owned by us — no
    // orphan risk: if we time out we kill+wait explicitly.
    let timeout = agent_timeout();
    let deadline = Instant::now() + timeout;
    let mut timed_out = false;
    loop {
        match child.try_wait() {
            Ok(Some(_status)) => break,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    timed_out = true;
                    break;
                }
                thread::sleep(Duration::from_millis(25));
            }
            Err(e) => return Err(format!("try_wait: {e}")),
        }
    }

    let stdout_text = stdout_thread.join().unwrap_or_default();
    let stderr_text = stderr_thread.join().unwrap_or_default();

    if timed_out {
        return Err(format!(
            "kova chat timed out after {:?}\n--- kova chat stderr ---\n{}",
            timeout, stderr_text
        ));
    }
    Ok((stdout_text, stderr_text))
}

/// Build a tool_use response in the format the agent loop's f140 parser
/// expects. Uses serde_json::to_string for the inner JSON so embedded quotes /
/// newlines / unicode in args survive correctly — a raw format!() interpolation
/// would silently produce malformed JSON for any non-trivial input.
fn tool_call_response(tool: &str, args: serde_json::Value) -> String {
    let call = serde_json::json!({"tool": tool, "args": args});
    let body = serde_json::to_string(&call).unwrap_or_else(|_| "{}".into());
    format!("Calling tool.\n```json\n{body}\n```\n")
}

// ── Scenarios ─────────────────────────────────────────

fn s_plain_text(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    // One response, no tool call → agent_loop returns Done immediately.
    let (_stdout, stderr) = run_chat(
        kova_bin,
        project,
        home,
        &["Hello from the mock. No tools needed."],
        "say hi",
    )?;
    // Stderr won't contain "[tool:" because no tool was dispatched.
    if stderr.contains("[tool:") {
        return Err(format!("unexpected tool dispatch; stderr: {stderr}"));
    }
    Ok(())
}

fn s_write_then_done(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let r1 = tool_call_response(
        "write_file",
        json!({"path": "created.txt", "content": "agent-loop-write-marker"}),
    );
    let r2 = "Done. File written.".to_string();
    let (_o, _e) = run_chat(kova_bin, project, home, &[&r1, &r2], "create created.txt")?;
    let on_disk = fs::read_to_string(project.join("created.txt"))
        .map_err(|e| format!("read written file: {e}"))?;
    if !on_disk.contains("agent-loop-write-marker") {
        return Err(format!("file lacks expected marker; got: {on_disk}"));
    }
    Ok(())
}

fn s_read_then_done(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    fs::write(project.join("data.txt"), "read-target-zz\n").map_err(|e| e.to_string())?;
    let r1 = tool_call_response("read_file", json!({"path": "data.txt"}));
    let r2 = "Done. File read.".to_string();
    let (_o, stderr) = run_chat(kova_bin, project, home, &[&r1, &r2], "read data.txt")?;
    // f147 prints "[tool: read_file]" to stderr on each dispatch. Verify it ran.
    if !stderr.contains("[tool: read_file]") {
        return Err(format!(
            "read_file dispatch not visible in stderr: {stderr}"
        ));
    }
    Ok(())
}

fn s_chain_write_read(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let r1 = tool_call_response(
        "write_file",
        json!({"path": "chain.txt", "content": "chained-write"}),
    );
    let r2 = tool_call_response("read_file", json!({"path": "chain.txt"}));
    let r3 = "Done. Chain complete.".to_string();
    let (_o, stderr) = run_chat(
        kova_bin,
        project,
        home,
        &[&r1, &r2, &r3],
        "create and read chain.txt",
    )?;
    // Verify both tools dispatched in order.
    if !stderr.contains("[tool: write_file]") {
        return Err("write_file dispatch missing".into());
    }
    if !stderr.contains("[tool: read_file]") {
        return Err("read_file dispatch missing".into());
    }
    let written = fs::read_to_string(project.join("chain.txt"))
        .map_err(|e| format!("read chain.txt: {e}"))?;
    if !written.contains("chained-write") {
        return Err(format!("write step did not land: {written}"));
    }
    Ok(())
}

fn s_grep_call(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    fs::write(project.join("hit.rs"), "fn grep_marker_qw() {}\n").map_err(|e| e.to_string())?;
    let r1 = tool_call_response("grep", json!({"pattern": "grep_marker_qw"}));
    let r2 = "Done. Grep complete.".to_string();
    let (_o, stderr) = run_chat(
        kova_bin,
        project,
        home,
        &[&r1, &r2],
        "find grep_marker_qw",
    )?;
    if !stderr.contains("[tool: grep]") {
        return Err(format!("grep dispatch missing: {stderr}"));
    }
    Ok(())
}

fn s_malformed_then_done(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    // First response: malformed JSON inside a code block. f140 returns no
    // calls, f147 treats it as Done — agent loop exits gracefully without
    // dispatching anything.
    let r1 = "```json\n{ malformed: not really }\n```";
    let (_o, stderr) = run_chat(kova_bin, project, home, &[r1], "noop")?;
    if stderr.contains("[tool:") {
        return Err(format!(
            "unexpected tool dispatch on malformed response: {stderr}"
        ));
    }
    Ok(())
}

fn s_overconsume(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    // Only one response in the mock file, but the agent calls a tool which
    // forces a second f382 call. The second call walks past EOF and gets the
    // "[mock: end of script]" guard — no tool calls in that text → loop exits.
    let r1 = tool_call_response(
        "write_file",
        json!({"path": "once.txt", "content": "single"}),
    );
    let (_o, _stderr) = run_chat(kova_bin, project, home, &[&r1], "write once.txt")?;
    let on_disk = fs::read_to_string(project.join("once.txt"))
        .map_err(|e| format!("read once.txt: {e}"))?;
    if !on_disk.contains("single") {
        return Err(format!("file content wrong: {on_disk}"));
    }
    Ok(())
}
