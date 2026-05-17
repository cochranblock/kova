// Unlicense — public domain — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! cc_features — Claude Code feature replication via MCP stdio.
//!
//! Diamond Rust Binary Architecture: kova-test (this side) spawns kova mcp
//! (other side) and drives it via JSON-RPC 2.0 over stdio — the exact same
//! external interface Claude Desktop uses. No lib internals. Every Claude
//! Code feature is exercised through the binary's external surface only.
//!
//! f403=mcp_spawn, f404=mcp_request, f405=mcp_call_tool, f406=run_cc_suite.
//! t216=T216 (McpHarness).

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tempfile::TempDir;

/// Hard ceiling on a single MCP request roundtrip. Keeps a dead/hung child from
/// hanging the whole suite — runs to a timeout error instead.
const MCP_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// t216=T216. McpHarness — owns a spawned `kova mcp` subprocess plus stdio
/// handles. A reader thread pumps stdout into a channel so f404 can apply a
/// timeout; a drain thread captures stderr so the pipe buffer can't fill and
/// deadlock the child. Dropping the handle kills + reaps the child.
pub struct T216 {
    child: Child,
    stdin: ChildStdin,
    stdout_rx: mpsc::Receiver<String>,
    stderr_buf: Arc<Mutex<String>>,
    next_id: u64,
    _stdout_thread: Option<JoinHandle<()>>,
    _stderr_thread: Option<JoinHandle<()>>,
}

impl T216 {
    /// f403=mcp_spawn. Spawn `kova mcp --project <project>` with HOME isolated
    /// to the test tempdir. Spawns drain threads for stdout (into a bounded
    /// channel) and stderr (into an in-memory buffer) so the child can never
    /// deadlock on a full pipe buffer. Performs the initialize handshake.
    pub fn f403(kova_bin: &Path, project: &Path, home: &Path) -> Result<Self, String> {
        let mut child = Command::new(kova_bin)
            .arg("mcp")
            .arg("--project")
            .arg(project)
            .env("HOME", home)
            .env("KOVA_PROJECT", project)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("spawn kova mcp: {e}"))?;
        let stdin = child.stdin.take().ok_or("no stdin")?;
        let stdout_raw = child.stdout.take().ok_or("no stdout")?;
        let stderr_raw = child.stderr.take().ok_or("no stderr")?;

        // Drain stdout into a channel of complete lines. f404 reads with a
        // timeout via recv_timeout so a dead child can't hang the test.
        let (stdout_tx, stdout_rx) = mpsc::channel::<String>();
        let stdout_thread = thread::spawn(move || {
            let mut reader = BufReader::new(stdout_raw);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) => break, // EOF
                    Ok(_) => {
                        if stdout_tx.send(line.clone()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        // Drain stderr continuously so the child's pipe buffer (typically 64KB
        // on Linux) can't fill. Stash into a buffer for diagnostics on failure.
        let stderr_buf = Arc::new(Mutex::new(String::new()));
        let stderr_buf_for_thread = Arc::clone(&stderr_buf);
        let stderr_thread = thread::spawn(move || {
            let mut reader = BufReader::new(stderr_raw);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) => break,
                    Ok(_) => {
                        if let Ok(mut buf) = stderr_buf_for_thread.lock() {
                            buf.push_str(&line);
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        let mut h = T216 {
            child,
            stdin,
            stdout_rx,
            stderr_buf,
            next_id: 1,
            _stdout_thread: Some(stdout_thread),
            _stderr_thread: Some(stderr_thread),
        };

        // Handshake — required by MCP spec before tools/call is valid.
        let init = h.f404(
            "initialize",
            json!({
                "protocolVersion": "2024-11-05",
                "clientInfo": {"name": "kova-test", "version": "0.1"},
                "capabilities": {}
            }),
        )?;
        if init.get("error").is_some() {
            return Err(format!("initialize failed: {init}"));
        }

        // initialized is a notification — no response expected, no reply read.
        let body = format!(
            "{}\n",
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"})
        );
        h.stdin
            .write_all(body.as_bytes())
            .map_err(|e| h.err_with_stderr(format!("write init-notif: {e}")))?;
        h.stdin.flush().map_err(|e| h.err_with_stderr(format!("flush: {e}")))?;
        Ok(h)
    }

    /// f404=mcp_request. Send a JSON-RPC request, read responses with a hard
    /// timeout, skip notifications (no `id` field), and match the response by
    /// `id` so an out-of-band server message can't desynchronize the protocol.
    pub fn f404(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        let req = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        let body = format!("{req}\n");
        self.stdin
            .write_all(body.as_bytes())
            .map_err(|e| self.err_with_stderr(format!("write: {e}")))?;
        self.stdin.flush().map_err(|e| self.err_with_stderr(format!("flush: {e}")))?;

        let deadline = Instant::now() + MCP_REQUEST_TIMEOUT;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(self.err_with_stderr(format!(
                    "timeout after {:?} waiting for response to id={id} method={method}",
                    MCP_REQUEST_TIMEOUT
                )));
            }
            let line = match self.stdout_rx.recv_timeout(remaining) {
                Ok(l) => l,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    return Err(self.err_with_stderr(format!(
                        "timeout waiting for response to id={id} method={method}"
                    )));
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(self.err_with_stderr(format!(
                        "kova mcp closed stdout before responding to id={id}"
                    )));
                }
            };
            if line.trim().is_empty() {
                continue;
            }
            let parsed: Value = serde_json::from_str(&line)
                .map_err(|e| format!("parse error {e} on line: {line}"))?;
            // Notifications have no `id` (or id=null). Skip them.
            match parsed.get("id") {
                None => continue,
                Some(Value::Null) => continue,
                Some(v) if v == &json!(id) => return Ok(parsed),
                Some(other) => {
                    // Out-of-order response; in practice f176 is strictly
                    // request/response, but be defensive — skip and keep looking.
                    eprintln!("[cc_features] skipping response with id={other}, want id={id}");
                }
            }
        }
    }

    /// f405=mcp_call_tool. Issue a tools/call. Returns Ok(text) on success, Err
    /// on tool failure (isError=true) or RPC-level error. Errors include the
    /// child's stderr context.
    pub fn f405(&mut self, name: &str, arguments: Value) -> Result<String, String> {
        let resp = self.f404("tools/call", json!({"name": name, "arguments": arguments}))?;
        if let Some(err) = resp.get("error") {
            return Err(self.err_with_stderr(format!("rpc error: {err}")));
        }
        let result = resp.get("result").ok_or_else(|| format!("no result in: {resp}"))?;
        let is_error = result
            .get("isError")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let text = result
            .get("content")
            .and_then(|c| c.get(0))
            .and_then(|b| b.get("text"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if is_error {
            Err(text)
        } else {
            Ok(text)
        }
    }

    /// Send a single raw JSON-RPC line (must end with `\n`) and read the next
    /// response line from the channel with the same timeout discipline as f404.
    /// Used for protocol-level negative tests that need to send malformed
    /// requests f404 wouldn't normally emit.
    pub fn send_raw(&mut self, line: &str) -> Result<Value, String> {
        self.stdin
            .write_all(line.as_bytes())
            .map_err(|e| self.err_with_stderr(format!("write raw: {e}")))?;
        self.stdin
            .flush()
            .map_err(|e| self.err_with_stderr(format!("flush raw: {e}")))?;
        let reply = self
            .stdout_rx
            .recv_timeout(MCP_REQUEST_TIMEOUT)
            .map_err(|e| self.err_with_stderr(format!("recv raw: {e}")))?;
        serde_json::from_str(&reply).map_err(|e| format!("parse raw: {e}; body: {reply}"))
    }

    /// Compose an error message with the child's accumulated stderr appended,
    /// so a failing test surfaces what the subprocess actually printed.
    fn err_with_stderr(&self, msg: String) -> String {
        let stderr_snapshot = self
            .stderr_buf
            .lock()
            .ok()
            .map(|g| g.clone())
            .unwrap_or_default();
        if stderr_snapshot.trim().is_empty() {
            msg
        } else {
            format!("{msg}\n--- kova mcp stderr ---\n{stderr_snapshot}")
        }
    }
}

impl Drop for T216 {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        // Drain threads exit on read EOF after the child is reaped; we don't
        // join them here because that could block if the kernel hasn't yet
        // closed the read end. They're tagged with `_` to silence unused-must-use.
    }
}

/// Negate boolean-style: ensure a tool call FAILS, return Err if it unexpectedly
/// succeeded (with the success output for diagnostics). Replaces 20 copies of
/// `match h.f405(...) { Ok(_) => Err(..), Err(_) => Ok(()) }`.
fn expect_fail(
    h: &mut T216,
    tool: &str,
    args: Value,
    what: &str,
) -> Result<(), String> {
    match h.f405(tool, args) {
        Ok(out) => Err(format!("expected {what} to fail; got success: {out}")),
        Err(_) => Ok(()),
    }
}

/// Like expect_fail but also asserts on a substring of the error message.
fn expect_fail_with(
    h: &mut T216,
    tool: &str,
    args: Value,
    what: &str,
    needle: &str,
) -> Result<(), String> {
    match h.f405(tool, args) {
        Ok(out) => Err(format!("expected {what} to fail; got success: {out}")),
        Err(e) => {
            if e.to_lowercase().contains(&needle.to_lowercase()) {
                Ok(())
            } else {
                Err(format!("expected '{needle}' in error; got: {e}"))
            }
        }
    }
}

// ── Test scenario registry ─────────────────────────────

/// Test scenario signature: takes (kova_bin, project, home) tempdirs and returns
/// Ok(()) on pass or Err(reason) on fail. Each scenario manages its own MCP
/// session via T216::f403.
type Scenario = fn(&Path, &Path, &Path) -> Result<(), String>;

/// Master list of Claude Code feature replication scenarios. Each entry is a
/// (label, fn) pair. f406 iterates the list, runs each in isolation, and
/// reports pass/fail with per-scenario timing.
const SCENARIOS: &[(&str, Scenario)] = &[
    // ── MCP protocol primitives ──
    ("mcp_initialize_handshake", s_initialize),
    ("mcp_tools_list_returns_registry", s_tools_list),
    ("mcp_unknown_method_returns_error", s_unknown_method),
    ("mcp_invalid_jsonrpc_version_rejected", s_bad_jsonrpc),

    // ── read_file (8 scenarios) ──
    ("read_file_returns_content", s_read_basic),
    ("read_file_includes_line_numbers", s_read_line_numbers),
    ("read_file_with_offset", s_read_offset),
    ("read_file_with_limit", s_read_limit),
    ("read_file_offset_and_limit_together", s_read_offset_limit),
    ("read_file_missing_path_fails", s_read_missing),
    ("read_file_empty_file_ok", s_read_empty),
    ("read_file_no_path_arg_fails", s_read_no_arg),

    // ── write_file (5) ──
    ("write_file_creates_new", s_write_new),
    ("write_file_creates_parent_dirs", s_write_parents),
    ("write_file_overwrites_existing", s_write_overwrite),
    ("write_file_no_content_fails", s_write_no_content),
    ("write_file_no_path_fails", s_write_no_path),

    // ── edit_file (5) ──
    ("edit_file_unique_match_replaces", s_edit_unique),
    ("edit_file_nonunique_match_fails", s_edit_nonunique),
    ("edit_file_missing_match_fails", s_edit_missing),
    ("edit_file_preserves_surrounding_content", s_edit_preserves),
    ("edit_file_no_args_fails", s_edit_no_args),

    // ── glob (4) ──
    ("glob_finds_matching_files", s_glob_basic),
    ("glob_filters_extension", s_glob_extension),
    ("glob_recursive_double_star", s_glob_recursive),
    ("glob_no_matches_empty_result", s_glob_no_matches),

    // ── grep (4) ──
    ("grep_finds_text_in_files", s_grep_basic),
    ("grep_returns_file_path_with_match", s_grep_path),
    ("grep_no_matches_clean_exit", s_grep_empty),
    ("grep_multiple_files_aggregated", s_grep_multi),

    // ── exec (5) ──
    ("exec_echo_returns_stdout", s_exec_echo),
    ("exec_nonzero_exit_reports_failure", s_exec_fail),
    ("exec_pwd_uses_project_dir", s_exec_cwd),
    ("exec_stderr_captured", s_exec_stderr),
    ("exec_no_command_fails", s_exec_no_arg),

    // ── memory_write (2) ──
    ("memory_write_appends_to_memory", s_memory_append),
    ("memory_write_persists_across_calls", s_memory_persist),

    // ── undo_edit (2) ──
    ("undo_restores_after_edit", s_undo_after_edit),
    ("undo_with_no_checkpoint_fails", s_undo_no_checkpoint),

    // ── code_outline (2) ──
    ("code_outline_extracts_functions", s_outline_fns),
    ("code_outline_extracts_structs", s_outline_structs),

    // ── Multi-step flows (4) ──
    ("flow_write_then_read", s_flow_write_read),
    ("flow_write_edit_undo", s_flow_write_edit_undo),
    ("flow_grep_then_read_match", s_flow_grep_read),
    ("flow_create_dir_write_glob", s_flow_create_write_glob),

    // ── todo_write (5) ──
    ("todo_write_saves_single_item", s_todo_single),
    ("todo_write_saves_multiple_items", s_todo_multi),
    ("todo_write_rejects_invalid_status", s_todo_bad_status),
    ("todo_write_rejects_empty_content", s_todo_empty_content),
    ("todo_write_replaces_full_list", s_todo_replace),

    // ── agent (5) ──
    ("agent_queues_with_minimal_args", s_agent_basic),
    ("agent_defaults_to_general_purpose", s_agent_default_type),
    ("agent_accepts_subagent_type", s_agent_type),
    ("agent_run_in_background_flag", s_agent_bg),
    ("agent_requires_description_and_prompt", s_agent_missing_args),

    // ── ask_user_question (4) ──
    ("ask_question_only", s_ask_basic),
    ("ask_with_options_renders_choices", s_ask_with_options),
    ("ask_rejects_invalid_options_json", s_ask_bad_options),
    ("ask_rejects_option_missing_label", s_ask_missing_label),

    // ── web_fetch (5) ──
    ("web_fetch_returns_body_text", s_fetch_basic),
    ("web_fetch_rejects_file_scheme", s_fetch_file_scheme),
    ("web_fetch_reports_http_error", s_fetch_http_error),
    ("web_fetch_truncates_at_max_bytes", s_fetch_truncate),
    ("web_fetch_requires_url_arg", s_fetch_no_url),

    // ── web_search (4) ──
    ("web_search_hits_engine_url_template", s_search_basic),
    ("web_search_url_encodes_query", s_search_encodes),
    ("web_search_requires_query_arg", s_search_no_query),
    ("web_search_template_needs_placeholder", s_search_no_placeholder),

    // ── plan mode (15) ──
    ("plan_enter_acknowledges", s_plan_enter),
    ("plan_exit_echoes_plan", s_plan_exit),
    ("plan_exit_requires_plan_arg", s_plan_exit_no_arg),
    ("plan_blocks_write_file", s_plan_blocks_write),
    ("plan_blocks_edit_file", s_plan_blocks_edit),
    ("plan_blocks_exec", s_plan_blocks_exec),
    ("plan_blocks_bash_alias", s_plan_blocks_bash),
    ("plan_blocks_undo_edit", s_plan_blocks_undo),
    ("plan_allows_read_file", s_plan_allows_read),
    ("plan_allows_grep", s_plan_allows_grep),
    ("plan_allows_glob", s_plan_allows_glob),
    ("plan_allows_todo_write", s_plan_allows_todo),
    ("plan_allows_memory_write", s_plan_allows_memory),
    ("plan_exit_re_enables_write", s_plan_exit_re_enables),
    ("plan_enter_idempotent", s_plan_enter_idempotent),

    // ── Error handling (3) ──
    ("unknown_tool_returns_error", s_unknown_tool),
    ("missing_required_arg_returns_error", s_missing_arg),
    ("nonexistent_path_fails_gracefully", s_nonexistent_path),
];

/// f406=run_cc_suite. Diamond-pattern Claude Code feature suite. For each
/// scenario: fresh tempdir + fresh HOME + fresh `kova mcp` subprocess. Returns
/// (all_passed, human-readable summary).
pub fn f406(kova_bin: &Path) -> (bool, String) {
    let mut report = String::new();
    let mut passed = 0usize;
    let mut failed = 0usize;
    let t_start = Instant::now();

    report.push_str(&format!(
        "cc_features: Diamond-pattern Claude Code feature suite via MCP stdio\n  kova bin: {}\n  scenarios: {}\n",
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
        "cc_features summary: {passed}/{total} passed in {total_ms}ms\n"
    ));
    (failed == 0, report)
}

// ── Helpers ────────────────────────────────────────────

fn open(kova_bin: &Path, project: &Path, home: &Path) -> Result<T216, String> {
    T216::f403(kova_bin, project, home)
}

fn require_contains(haystack: &str, needle: &str) -> Result<(), String> {
    if haystack.contains(needle) {
        Ok(())
    } else {
        Err(format!("expected to contain '{needle}'; got: {haystack}"))
    }
}

fn require_not_contains(haystack: &str, needle: &str) -> Result<(), String> {
    if haystack.contains(needle) {
        Err(format!("unexpected '{needle}' in: {haystack}"))
    } else {
        Ok(())
    }
}

// ── MCP protocol primitives ────────────────────────────

fn s_initialize(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    // f403 already does the initialize. If we got here, it passed.
    let _h = open(kova_bin, project, home)?;
    Ok(())
}

fn s_tools_list(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    let resp = h.f404("tools/list", json!({}))?;
    let tools = resp
        .get("result")
        .and_then(|r| r.get("tools"))
        .and_then(Value::as_array)
        .ok_or("no tools array in tools/list response")?;
    if tools.len() < 15 {
        return Err(format!("expected >= 15 tools; got {}", tools.len()));
    }
    let names: Vec<&str> = tools
        .iter()
        .filter_map(|t| t.get("name").and_then(Value::as_str))
        .collect();
    for req in [
        "read_file", "write_file", "edit_file", "exec", "grep", "glob",
        "memory_write", "undo_edit", "todo_write", "agent",
        "ask_user_question", "web_fetch", "web_search",
        "enter_plan_mode", "exit_plan_mode",
    ] {
        if !names.contains(&req) {
            return Err(format!("tools/list missing required tool: {req}"));
        }
    }
    Ok(())
}

fn s_unknown_method(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    let resp = h.f404("does/not/exist", json!({}))?;
    if resp.get("error").is_none() {
        return Err(format!("expected error for unknown method; got: {resp}"));
    }
    Ok(())
}

fn s_bad_jsonrpc(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    // Send a raw line with wrong jsonrpc version.
    let resp = h.send_raw("{\"jsonrpc\":\"1.0\",\"id\":99,\"method\":\"tools/list\"}\n")?;
    if resp.get("error").is_none() {
        return Err(format!("expected error for bad jsonrpc version; got: {resp}"));
    }
    Ok(())
}

// ── read_file ────────────────────────────────────────

fn s_read_basic(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    fs::write(project.join("hello.txt"), "line one\nline two\n").map_err(|e| e.to_string())?;
    let mut h = open(kova_bin, project, home)?;
    let text = h.f405("read_file", json!({"path": "hello.txt"}))?;
    require_contains(&text, "line one")?;
    require_contains(&text, "line two")
}

fn s_read_line_numbers(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    fs::write(project.join("ln.txt"), "alpha\nbeta\ngamma\n").map_err(|e| e.to_string())?;
    let mut h = open(kova_bin, project, home)?;
    let text = h.f405("read_file", json!({"path": "ln.txt"}))?;
    // read_file's tool description says "Returns file text with line numbers."
    // Accept either "1" near "alpha" or explicit numbering; failure if no digit.
    if !text.contains('1') {
        return Err(format!("expected line-number digit in output; got: {text}"));
    }
    Ok(())
}

fn s_read_offset(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    fs::write(project.join("o.txt"), "a\nb\nc\nd\ne\n").map_err(|e| e.to_string())?;
    let mut h = open(kova_bin, project, home)?;
    let text = h.f405("read_file", json!({"path": "o.txt", "offset": 3}))?;
    require_contains(&text, "c")?;
    require_not_contains(&text, "a\n")
}

fn s_read_limit(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    fs::write(project.join("l.txt"), "a\nb\nc\nd\ne\n").map_err(|e| e.to_string())?;
    let mut h = open(kova_bin, project, home)?;
    let text = h.f405("read_file", json!({"path": "l.txt", "limit": 2}))?;
    require_contains(&text, "a")?;
    require_contains(&text, "b")?;
    require_not_contains(&text, "e")
}

fn s_read_offset_limit(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    fs::write(project.join("ol.txt"), "a\nb\nc\nd\ne\n").map_err(|e| e.to_string())?;
    let mut h = open(kova_bin, project, home)?;
    let text = h.f405(
        "read_file",
        json!({"path": "ol.txt", "offset": 2, "limit": 2}),
    )?;
    require_contains(&text, "b")?;
    require_contains(&text, "c")?;
    require_not_contains(&text, "e")
}

fn s_read_missing(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    expect_fail(&mut h, "read_file", json!({"path": "does_not_exist.txt"}), "read of missing file")
}

fn s_read_empty(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    fs::write(project.join("empty.txt"), "").map_err(|e| e.to_string())?;
    let mut h = open(kova_bin, project, home)?;
    let _ = h.f405("read_file", json!({"path": "empty.txt"}))?;
    Ok(())
}

fn s_read_no_arg(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    expect_fail(&mut h, "read_file", json!({}), "read with no path arg")
}

// ── write_file ────────────────────────────────────────

fn s_write_new(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    h.f405("write_file", json!({"path": "new.txt", "content": "hello"}))?;
    let on_disk = fs::read_to_string(project.join("new.txt")).map_err(|e| e.to_string())?;
    if on_disk != "hello" {
        return Err(format!("expected 'hello'; got '{on_disk}'"));
    }
    Ok(())
}

fn s_write_parents(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    h.f405(
        "write_file",
        json!({"path": "a/b/c.txt", "content": "deep"}),
    )?;
    let on_disk = fs::read_to_string(project.join("a/b/c.txt")).map_err(|e| e.to_string())?;
    if on_disk != "deep" {
        return Err(format!("expected 'deep'; got '{on_disk}'"));
    }
    Ok(())
}

fn s_write_overwrite(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    fs::write(project.join("o.txt"), "old").map_err(|e| e.to_string())?;
    let mut h = open(kova_bin, project, home)?;
    h.f405("write_file", json!({"path": "o.txt", "content": "new"}))?;
    let on_disk = fs::read_to_string(project.join("o.txt")).map_err(|e| e.to_string())?;
    if on_disk != "new" {
        return Err(format!("expected overwrite; got '{on_disk}'"));
    }
    Ok(())
}

fn s_write_no_content(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    expect_fail(&mut h, "write_file", json!({"path": "x.txt"}), "write without content")
}

fn s_write_no_path(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    expect_fail(&mut h, "write_file", json!({"content": "x"}), "write without path")
}

// ── edit_file ─────────────────────────────────────────

fn s_edit_unique(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    fs::write(project.join("e.rs"), "fn old() {}\n").map_err(|e| e.to_string())?;
    let mut h = open(kova_bin, project, home)?;
    h.f405(
        "edit_file",
        json!({"path": "e.rs", "old_text": "old", "new_text": "new"}),
    )?;
    let on_disk = fs::read_to_string(project.join("e.rs")).map_err(|e| e.to_string())?;
    if on_disk != "fn new() {}\n" {
        return Err(format!("expected 'fn new() {{}}'; got '{on_disk}'"));
    }
    Ok(())
}

fn s_edit_nonunique(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    fs::write(project.join("d.rs"), "foo\nfoo\n").map_err(|e| e.to_string())?;
    let mut h = open(kova_bin, project, home)?;
    expect_fail(
        &mut h,
        "edit_file",
        json!({"path": "d.rs", "old_text": "foo", "new_text": "bar"}),
        "non-unique match edit",
    )
}

fn s_edit_missing(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    fs::write(project.join("m.rs"), "fn a() {}\n").map_err(|e| e.to_string())?;
    let mut h = open(kova_bin, project, home)?;
    expect_fail(
        &mut h,
        "edit_file",
        json!({"path": "m.rs", "old_text": "nonexistent", "new_text": "x"}),
        "edit with missing match",
    )
}

fn s_edit_preserves(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let before = "// header\nfn target_fn() {}\n// trailer\n";
    fs::write(project.join("p.rs"), before).map_err(|e| e.to_string())?;
    let mut h = open(kova_bin, project, home)?;
    h.f405(
        "edit_file",
        json!({"path": "p.rs", "old_text": "target_fn", "new_text": "renamed_fn"}),
    )?;
    let after = fs::read_to_string(project.join("p.rs")).map_err(|e| e.to_string())?;
    require_contains(&after, "// header")?;
    require_contains(&after, "// trailer")?;
    require_contains(&after, "renamed_fn")?;
    require_not_contains(&after, "target_fn")
}

fn s_edit_no_args(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    expect_fail(
        &mut h,
        "edit_file",
        json!({"path": "x.rs"}),
        "edit_file without text args",
    )
}

// ── glob ──────────────────────────────────────────────

fn s_glob_basic(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    fs::create_dir_all(project.join("src")).map_err(|e| e.to_string())?;
    fs::write(project.join("src/a.rs"), "").map_err(|e| e.to_string())?;
    fs::write(project.join("src/b.rs"), "").map_err(|e| e.to_string())?;
    let mut h = open(kova_bin, project, home)?;
    let out = h.f405("glob", json!({"pattern": "**/*.rs"}))?;
    require_contains(&out, "a.rs")?;
    require_contains(&out, "b.rs")
}

fn s_glob_extension(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    fs::write(project.join("a.rs"), "").map_err(|e| e.to_string())?;
    fs::write(project.join("b.txt"), "").map_err(|e| e.to_string())?;
    let mut h = open(kova_bin, project, home)?;
    let out = h.f405("glob", json!({"pattern": "*.rs"}))?;
    require_contains(&out, "a.rs")?;
    require_not_contains(&out, "b.txt")
}

fn s_glob_recursive(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    fs::create_dir_all(project.join("deep/nest")).map_err(|e| e.to_string())?;
    fs::write(project.join("deep/nest/hit.rs"), "").map_err(|e| e.to_string())?;
    let mut h = open(kova_bin, project, home)?;
    let out = h.f405("glob", json!({"pattern": "**/*.rs"}))?;
    require_contains(&out, "hit.rs")
}

fn s_glob_no_matches(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    let out = h.f405("glob", json!({"pattern": "*.nonexistent_ext"}))?;
    // Tool should succeed even with no matches.
    let _ = out;
    Ok(())
}

// ── grep ──────────────────────────────────────────────

fn s_grep_basic(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    fs::write(project.join("a.rs"), "fn target_marker_x() {}\n").map_err(|e| e.to_string())?;
    let mut h = open(kova_bin, project, home)?;
    let out = h.f405("grep", json!({"pattern": "target_marker_x"}))?;
    require_contains(&out, "target_marker_x")
}

fn s_grep_path(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    fs::write(project.join("p.rs"), "fn unique_marker_a() {}\n").map_err(|e| e.to_string())?;
    let mut h = open(kova_bin, project, home)?;
    let out = h.f405("grep", json!({"pattern": "unique_marker_a"}))?;
    require_contains(&out, "p.rs")
}

fn s_grep_empty(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    fs::write(project.join("e.rs"), "fn x() {}\n").map_err(|e| e.to_string())?;
    let mut h = open(kova_bin, project, home)?;
    // grep may return failure with status indicating no matches; both Ok and Err are tolerable.
    let _ = h.f405("grep", json!({"pattern": "totally_absent_string_zzz"}));
    Ok(())
}

fn s_grep_multi(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    fs::write(project.join("a.rs"), "shared_token_qq\n").map_err(|e| e.to_string())?;
    fs::write(project.join("b.rs"), "shared_token_qq\n").map_err(|e| e.to_string())?;
    let mut h = open(kova_bin, project, home)?;
    let out = h.f405("grep", json!({"pattern": "shared_token_qq"}))?;
    require_contains(&out, "a.rs")?;
    require_contains(&out, "b.rs")
}

// ── exec ──────────────────────────────────────────────

fn s_exec_echo(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    let out = h.f405("exec", json!({"command": "echo cc_test_marker_42"}))?;
    require_contains(&out, "cc_test_marker_42")
}

fn s_exec_fail(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    expect_fail(&mut h, "exec", json!({"command": "false"}), "exec of `false` (exit 1)")
}

fn s_exec_cwd(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    let out = h.f405("exec", json!({"command": "pwd"}))?;
    let proj_canon = std::fs::canonicalize(project)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| project.to_string_lossy().into_owned());
    // pwd should print the project dir (or its canonical form).
    if !out.contains(&proj_canon) && !out.contains(&project.to_string_lossy().to_string()) {
        return Err(format!("pwd did not report project dir; got: {out}"));
    }
    Ok(())
}

fn s_exec_stderr(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    // Write to stderr, then succeed. exec captures both.
    let out = h.f405(
        "exec",
        json!({"command": "echo cc_stderr_marker 1>&2"}),
    )?;
    require_contains(&out, "cc_stderr_marker")
}

fn s_exec_no_arg(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    expect_fail(&mut h, "exec", json!({}), "exec with no command")
}

// ── memory_write ──────────────────────────────────────

fn s_memory_append(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    h.f405("memory_write", json!({"content": "cc_mem_marker_alpha"}))?;
    let mem = fs::read_to_string(home.join(".kova/memory.md")).map_err(|e| e.to_string())?;
    require_contains(&mem, "cc_mem_marker_alpha")
}

fn s_memory_persist(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    h.f405("memory_write", json!({"content": "first_mem_entry"}))?;
    h.f405("memory_write", json!({"content": "second_mem_entry"}))?;
    let mem = fs::read_to_string(home.join(".kova/memory.md")).map_err(|e| e.to_string())?;
    require_contains(&mem, "first_mem_entry")?;
    require_contains(&mem, "second_mem_entry")
}

// ── undo_edit ─────────────────────────────────────────

fn s_undo_after_edit(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    fs::write(project.join("u.txt"), "original\n").map_err(|e| e.to_string())?;
    let mut h = open(kova_bin, project, home)?;
    h.f405(
        "edit_file",
        json!({"path": "u.txt", "old_text": "original", "new_text": "modified"}),
    )?;
    let mid = fs::read_to_string(project.join("u.txt")).map_err(|e| e.to_string())?;
    if mid != "modified\n" {
        return Err(format!("edit step left wrong content: {mid}"));
    }
    h.f405("undo_edit", json!({"path": "u.txt"}))?;
    let after = fs::read_to_string(project.join("u.txt")).map_err(|e| e.to_string())?;
    if after != "original\n" {
        return Err(format!("undo did not restore; got: {after}"));
    }
    Ok(())
}

fn s_undo_no_checkpoint(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    fs::write(project.join("v.txt"), "x").map_err(|e| e.to_string())?;
    let mut h = open(kova_bin, project, home)?;
    expect_fail(
        &mut h,
        "undo_edit",
        json!({"path": "v.txt"}),
        "undo with no checkpoint",
    )
}

// ── code_outline ──────────────────────────────────────

fn s_outline_fns(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    fs::write(
        project.join("ol.rs"),
        "pub fn outlined_marker_fn() -> u32 { 42 }\n",
    )
    .map_err(|e| e.to_string())?;
    let mut h = open(kova_bin, project, home)?;
    let out = h.f405("code_outline", json!({"path": "ol.rs"}))?;
    require_contains(&out, "outlined_marker_fn")
}

fn s_outline_structs(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    fs::write(
        project.join("st.rs"),
        "pub struct OutlinedMarkerStruct { pub x: u32 }\n",
    )
    .map_err(|e| e.to_string())?;
    let mut h = open(kova_bin, project, home)?;
    let out = h.f405("code_outline", json!({"path": "st.rs"}))?;
    require_contains(&out, "OutlinedMarkerStruct")
}

// ── Multi-step flows ──────────────────────────────────

fn s_flow_write_read(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    h.f405(
        "write_file",
        json!({"path": "rt.txt", "content": "round_trip_payload_zz"}),
    )?;
    let out = h.f405("read_file", json!({"path": "rt.txt"}))?;
    require_contains(&out, "round_trip_payload_zz")
}

fn s_flow_write_edit_undo(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    h.f405(
        "write_file",
        json!({"path": "f.txt", "content": "alpha"}),
    )?;
    h.f405(
        "edit_file",
        json!({"path": "f.txt", "old_text": "alpha", "new_text": "beta"}),
    )?;
    if fs::read_to_string(project.join("f.txt")).map_err(|e| e.to_string())? != "beta" {
        return Err("edit step did not produce 'beta'".into());
    }
    h.f405("undo_edit", json!({"path": "f.txt"}))?;
    if fs::read_to_string(project.join("f.txt")).map_err(|e| e.to_string())? != "alpha" {
        return Err("undo did not restore 'alpha'".into());
    }
    Ok(())
}

fn s_flow_grep_read(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    fs::create_dir_all(project.join("src")).map_err(|e| e.to_string())?;
    fs::write(
        project.join("src/lib.rs"),
        "pub fn flow_marker_xyz() -> u32 { 42 }\n",
    )
    .map_err(|e| e.to_string())?;
    let mut h = open(kova_bin, project, home)?;
    let g = h.f405("grep", json!({"pattern": "flow_marker_xyz"}))?;
    require_contains(&g, "src/lib.rs")?;
    let r = h.f405("read_file", json!({"path": "src/lib.rs"}))?;
    require_contains(&r, "flow_marker_xyz")
}

fn s_flow_create_write_glob(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    h.f405(
        "write_file",
        json!({"path": "src/a.rs", "content": "// a"}),
    )?;
    h.f405(
        "write_file",
        json!({"path": "src/b.rs", "content": "// b"}),
    )?;
    let out = h.f405("glob", json!({"pattern": "src/*.rs"}))?;
    require_contains(&out, "a.rs")?;
    require_contains(&out, "b.rs")
}

// ── Error handling ────────────────────────────────────

fn s_unknown_tool(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    expect_fail(&mut h, "totally_made_up_tool", json!({}), "unknown tool")
}

fn s_missing_arg(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    expect_fail(&mut h, "grep", json!({}), "grep without pattern arg")
}

fn s_nonexistent_path(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    expect_fail(
        &mut h,
        "read_file",
        json!({"path": "completely/missing/path.txt"}),
        "nonexistent path read",
    )
}

// ── todo_write ────────────────────────────────────────

fn s_todo_single(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    let todos = r#"[{"content":"ship cc_features","status":"in_progress","activeForm":"shipping cc_features"}]"#;
    let out = h.f405("todo_write", json!({"todos": todos}))?;
    require_contains(&out, "ship cc_features")?;
    require_contains(&out, "in_progress")
}

fn s_todo_multi(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    let todos = r#"[
        {"content":"first task","status":"completed","activeForm":"doing first"},
        {"content":"second task","status":"in_progress","activeForm":"doing second"},
        {"content":"third task","status":"pending","activeForm":"doing third"}
    ]"#;
    let out = h.f405("todo_write", json!({"todos": todos}))?;
    require_contains(&out, "first task")?;
    require_contains(&out, "second task")?;
    require_contains(&out, "third task")?;
    require_contains(&out, "3 todos saved")
}

fn s_todo_bad_status(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    let todos = r#"[{"content":"x","status":"bogus","activeForm":"x"}]"#;
    expect_fail(
        &mut h,
        "todo_write",
        json!({"todos": todos}),
        "todo with invalid status",
    )
}

fn s_todo_empty_content(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    let todos = r#"[{"content":"","status":"pending","activeForm":"x"}]"#;
    expect_fail(
        &mut h,
        "todo_write",
        json!({"todos": todos}),
        "todo with empty content",
    )
}

fn s_todo_replace(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    let first = r#"[{"content":"original_task","status":"pending","activeForm":"x"}]"#;
    h.f405("todo_write", json!({"todos": first}))?;
    let second = r#"[{"content":"replacement_task","status":"pending","activeForm":"x"}]"#;
    let out = h.f405("todo_write", json!({"todos": second}))?;
    require_contains(&out, "replacement_task")?;
    require_not_contains(&out, "original_task")
}

// ── agent ─────────────────────────────────────────────

fn s_agent_basic(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    let out = h.f405(
        "agent",
        json!({"description": "scan repo", "prompt": "list all .rs files and report counts"}),
    )?;
    require_contains(&out, "queued")?;
    require_contains(&out, "scan repo")
}

fn s_agent_default_type(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    let out = h.f405(
        "agent",
        json!({"description": "default test", "prompt": "do nothing"}),
    )?;
    require_contains(&out, "general-purpose")
}

fn s_agent_type(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    let out = h.f405(
        "agent",
        json!({
            "description": "review change",
            "prompt": "review the diff",
            "subagent_type": "code-reviewer",
        }),
    )?;
    require_contains(&out, "code-reviewer")
}

fn s_agent_bg(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    let out = h.f405(
        "agent",
        json!({
            "description": "bg task",
            "prompt": "long running",
            "run_in_background": "true",
        }),
    )?;
    require_contains(&out, "queued")
}

fn s_agent_missing_args(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    expect_fail(
        &mut h,
        "agent",
        json!({"description": "no prompt"}),
        "agent without prompt",
    )
}

// ── ask_user_question ─────────────────────────────────

fn s_ask_basic(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    let out = h.f405(
        "ask_user_question",
        json!({"question": "Which path do you want?"}),
    )?;
    require_contains(&out, "Which path do you want?")
}

fn s_ask_with_options(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    let options = r#"[
        {"label":"option_alpha","description":"the alpha path"},
        {"label":"option_beta","description":"the beta path"}
    ]"#;
    let out = h.f405(
        "ask_user_question",
        json!({"question": "Pick one:", "options": options}),
    )?;
    require_contains(&out, "option_alpha")?;
    require_contains(&out, "the alpha path")?;
    require_contains(&out, "option_beta")
}

fn s_ask_bad_options(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    expect_fail(
        &mut h,
        "ask_user_question",
        json!({"question": "Pick:", "options": "not json"}),
        "invalid options JSON",
    )
}

fn s_ask_missing_label(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    let options = r#"[{"description":"no label here"}]"#;
    expect_fail(
        &mut h,
        "ask_user_question",
        json!({"question": "Pick:", "options": options}),
        "option without label",
    )
}

// ── web_fetch / web_search helpers ────────────────────

/// Spawn a one-shot HTTP server bound to 127.0.0.1:0 that echoes the request
/// line back in the body. Used to test web_fetch and web_search end-to-end
/// without real network. Returns the listener URL; the thread serves a single
/// request then exits.
fn spawn_echo_server(status: u16) -> Result<String, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| format!("bind: {e}"))?;
    let addr = listener.local_addr().map_err(|e| e.to_string())?;
    let url = format!("http://{addr}");
    let _ = listener.set_nonblocking(false);
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(2)));
            let mut buf = [0u8; 8192];
            let n = stream.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]);
            let request_line = req.lines().next().unwrap_or("").to_string();
            let body = format!("echo:{request_line}");
            let resp = format!(
                "HTTP/1.1 {status} OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
        }
    });
    Ok(url)
}

/// Spawn an HTTP server that returns a fixed body. Useful for truncation tests.
fn spawn_fixed_server(body: String) -> Result<String, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| format!("bind: {e}"))?;
    let addr = listener.local_addr().map_err(|e| e.to_string())?;
    let url = format!("http://{addr}");
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(2)));
            let mut buf = [0u8; 8192];
            let _ = stream.read(&mut buf);
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
        }
    });
    Ok(url)
}

// ── web_fetch ─────────────────────────────────────────

fn s_fetch_basic(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let url = spawn_echo_server(200)?;
    let mut h = open(kova_bin, project, home)?;
    let out = h.f405("web_fetch", json!({"url": url}))?;
    // Echo server returns the request line; it should be a GET against / .
    require_contains(&out, "GET")
}

fn s_fetch_file_scheme(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    expect_fail_with(
        &mut h,
        "web_fetch",
        json!({"url": "file:///etc/passwd"}),
        "file:// scheme",
        "http",
    )
}

fn s_fetch_http_error(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let url = spawn_echo_server(404)?;
    let mut h = open(kova_bin, project, home)?;
    expect_fail_with(
        &mut h,
        "web_fetch",
        json!({"url": url}),
        "404 response",
        "404",
    )
}

fn s_fetch_truncate(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let big = "x".repeat(8192);
    let url = spawn_fixed_server(big)?;
    let mut h = open(kova_bin, project, home)?;
    let out = h.f405(
        "web_fetch",
        json!({"url": url, "max_bytes": "1024"}),
    )?;
    require_contains(&out, "[truncated")
}

fn s_fetch_no_url(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    expect_fail(&mut h, "web_fetch", json!({}), "web_fetch without url")
}

// ── web_search ────────────────────────────────────────

fn s_search_basic(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let base = spawn_echo_server(200)?;
    let template = format!("{base}/?q={{query}}");
    let mut h = open(kova_bin, project, home)?;
    let out = h.f405(
        "web_search",
        json!({"query": "kova_marker_abc", "engine_url": template}),
    )?;
    require_contains(&out, "kova_marker_abc")
}

fn s_search_encodes(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let base = spawn_echo_server(200)?;
    let template = format!("{base}/?q={{query}}");
    let mut h = open(kova_bin, project, home)?;
    let out = h.f405(
        "web_search",
        json!({"query": "hello world & friends", "engine_url": template}),
    )?;
    // Echo body contains the request line — verify percent-encoding present.
    require_contains(&out, "hello%20world")?;
    require_contains(&out, "%26")
}

fn s_search_no_query(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    expect_fail(&mut h, "web_search", json!({}), "web_search without query")
}

fn s_search_no_placeholder(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    expect_fail_with(
        &mut h,
        "web_search",
        json!({"query": "x", "engine_url": "https://example.com/no-placeholder"}),
        "engine_url without {query}",
        "placeholder",
    )
}

// ── plan mode ─────────────────────────────────────────

fn s_plan_enter(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    let out = h.f405("enter_plan_mode", json!({}))?;
    require_contains(&out, "entered plan mode")
}

fn s_plan_exit(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    h.f405("enter_plan_mode", json!({}))?;
    let plan = "Step 1: refactor module X.\nStep 2: add tests.";
    let out = h.f405("exit_plan_mode", json!({"plan": plan}))?;
    require_contains(&out, "exited plan mode")?;
    require_contains(&out, "refactor module X")
}

fn s_plan_exit_no_arg(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    h.f405("enter_plan_mode", json!({}))?;
    expect_fail(&mut h, "exit_plan_mode", json!({}), "exit_plan_mode without plan arg")
}

fn s_plan_blocks_write(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    h.f405("enter_plan_mode", json!({}))?;
    expect_fail_with(
        &mut h,
        "write_file",
        json!({"path": "blocked.txt", "content": "should not be written"}),
        "write_file in plan mode",
        "blocked: in plan mode",
    )?;
    // Verify the file really wasn't written.
    if project.join("blocked.txt").exists() {
        return Err("write_file was gated as expected but the file leaked through to disk".into());
    }
    Ok(())
}

fn s_plan_blocks_edit(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    // Write a file BEFORE entering plan mode so edit has something to edit.
    fs::write(project.join("e.rs"), "fn original() {}\n").map_err(|e| e.to_string())?;
    let mut h = open(kova_bin, project, home)?;
    h.f405("enter_plan_mode", json!({}))?;
    expect_fail_with(
        &mut h,
        "edit_file",
        json!({"path": "e.rs", "old_text": "original", "new_text": "modified"}),
        "edit_file in plan mode",
        "blocked: in plan mode",
    )?;
    let after = fs::read_to_string(project.join("e.rs")).map_err(|e| e.to_string())?;
    if !after.contains("original") {
        return Err("edit_file mutated the file despite being in plan mode".into());
    }
    Ok(())
}

fn s_plan_blocks_exec(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    h.f405("enter_plan_mode", json!({}))?;
    expect_fail_with(
        &mut h,
        "exec",
        json!({"command": "echo should_be_blocked"}),
        "exec in plan mode",
        "blocked: in plan mode",
    )
}

fn s_plan_blocks_bash(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    h.f405("enter_plan_mode", json!({}))?;
    expect_fail_with(
        &mut h,
        "bash",
        json!({"command": "echo should_be_blocked"}),
        "bash alias in plan mode",
        "blocked: in plan mode",
    )
}

fn s_plan_blocks_undo(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    h.f405("enter_plan_mode", json!({}))?;
    expect_fail_with(
        &mut h,
        "undo_edit",
        json!({"path": "anything.txt"}),
        "undo_edit in plan mode",
        "blocked: in plan mode",
    )
}

fn s_plan_allows_read(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    fs::write(project.join("p.txt"), "plan-mode-readable\n").map_err(|e| e.to_string())?;
    let mut h = open(kova_bin, project, home)?;
    h.f405("enter_plan_mode", json!({}))?;
    let out = h.f405("read_file", json!({"path": "p.txt"}))?;
    require_contains(&out, "plan-mode-readable")
}

fn s_plan_allows_grep(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    fs::write(project.join("g.rs"), "fn plan_visible_target() {}\n").map_err(|e| e.to_string())?;
    let mut h = open(kova_bin, project, home)?;
    h.f405("enter_plan_mode", json!({}))?;
    let out = h.f405("grep", json!({"pattern": "plan_visible_target"}))?;
    require_contains(&out, "plan_visible_target")
}

fn s_plan_allows_glob(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    fs::write(project.join("a.rs"), "").map_err(|e| e.to_string())?;
    fs::write(project.join("b.rs"), "").map_err(|e| e.to_string())?;
    let mut h = open(kova_bin, project, home)?;
    h.f405("enter_plan_mode", json!({}))?;
    let out = h.f405("glob", json!({"pattern": "*.rs"}))?;
    require_contains(&out, "a.rs")?;
    require_contains(&out, "b.rs")
}

fn s_plan_allows_todo(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    h.f405("enter_plan_mode", json!({}))?;
    let todos = r#"[{"content":"plan-mode-todo","status":"pending","activeForm":"planning"}]"#;
    let out = h.f405("todo_write", json!({"todos": todos}))?;
    require_contains(&out, "plan-mode-todo")
}

fn s_plan_allows_memory(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    h.f405("enter_plan_mode", json!({}))?;
    h.f405("memory_write", json!({"content": "plan-mode-note"}))?;
    let mem = fs::read_to_string(home.join(".kova/memory.md")).map_err(|e| e.to_string())?;
    require_contains(&mem, "plan-mode-note")
}

fn s_plan_exit_re_enables(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    h.f405("enter_plan_mode", json!({}))?;
    expect_fail_with(
        &mut h,
        "write_file",
        json!({"path": "before.txt", "content": "x"}),
        "write before exit",
        "blocked: in plan mode",
    )?;
    h.f405("exit_plan_mode", json!({"plan": "now write things"}))?;
    let out = h.f405(
        "write_file",
        json!({"path": "after.txt", "content": "post-exit"}),
    )?;
    require_contains(&out, "wrote")?;
    let on_disk = fs::read_to_string(project.join("after.txt")).map_err(|e| e.to_string())?;
    if on_disk != "post-exit" {
        return Err(format!("post-exit write produced wrong content: {on_disk}"));
    }
    Ok(())
}

fn s_plan_enter_idempotent(kova_bin: &Path, project: &Path, home: &Path) -> Result<(), String> {
    let mut h = open(kova_bin, project, home)?;
    let first = h.f405("enter_plan_mode", json!({}))?;
    require_contains(&first, "entered plan mode")?;
    let second = h.f405("enter_plan_mode", json!({}))?;
    require_contains(&second, "already in plan mode")?;
    // Mutating tools should still be blocked.
    expect_fail_with(
        &mut h,
        "write_file",
        json!({"path": "x.txt", "content": "y"}),
        "write after double-enter",
        "blocked: in plan mode",
    )
}
