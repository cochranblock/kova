// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! Tokenized cargo commands. §13 compressed output for AI context.
//! xN=command, pN=project alias, rN=output field.
//! Token in → cargo execute → compress out.

#![allow(non_camel_case_types)]

use clap::ValueEnum;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Instant;

// ── Project Alias Map ────────────────────────────────────
// pN → (crate name, default features).

const PROJECT_MAP: &[(&str, &str)] = &[
    ("p0", "kova"),
    ("p1", "approuter"),
    ("p2", "cochranblock"),
    ("p3", "oakilydokily"),
    ("p4", "rogue-repo"),
    ("p5", "ronin-sites"),
    ("p6", "kova-core"),
    ("p7", "exopack"),
    ("p8", "whyyoulying"),
    ("p9", "wowasticker"),
    ("p10", "kova-web"),
];

/// Resolve pN → crate name, or pass through.
fn resolve_project(s: &str) -> &str {
    PROJECT_MAP
        .iter()
        .find(|(k, _)| *k == s)
        .map(|(_, v)| *v)
        .unwrap_or(s)
}

/// Reverse: crate name → pN token.
fn to_project_token(name: &str) -> &str {
    PROJECT_MAP
        .iter()
        .find(|(_, v)| *v == name)
        .map(|(k, _)| *k)
        .unwrap_or(name)
}

// ── Command Enum (t99) ───────────────────────────────────

/// t99=CargoCmd. Tokenized cargo command variants.
#[derive(Clone, Copy, ValueEnum, Debug)]
pub enum t99 {
    /// x0: cargo build.
    #[value(name = "x0")]
    X0,
    /// x1: cargo check.
    #[value(name = "x1")]
    X1,
    /// x2: cargo test.
    #[value(name = "x2")]
    X2,
    /// x3: cargo clippy.
    #[value(name = "x3")]
    X3,
    /// x4: cargo run.
    #[value(name = "x4")]
    X4,
    /// x5: cargo build --release.
    #[value(name = "x5")]
    X5,
    /// x6: cargo clean.
    #[value(name = "x6")]
    X6,
    /// x7: cargo doc.
    #[value(name = "x7")]
    X7,
    /// x8: cargo fmt --check.
    #[value(name = "x8")]
    X8,
    /// x9: cargo bench.
    #[value(name = "x9")]
    X9,
}

impl t99 {
    fn base_args(&self) -> Vec<&'static str> {
        match self {
            t99::X0 => vec!["build"],
            t99::X1 => vec!["check"],
            t99::X2 => vec!["test"],
            t99::X3 => vec!["clippy", "--", "-D", "warnings"],
            t99::X4 => vec!["run"],
            t99::X5 => vec!["build", "--release"],
            t99::X6 => vec!["clean"],
            t99::X7 => vec!["doc", "--no-deps"],
            t99::X8 => vec!["fmt", "--check"],
            t99::X9 => vec!["bench"],
        }
    }

    fn name(&self) -> &'static str {
        match self {
            t99::X0 => "build",
            t99::X1 => "check",
            t99::X2 => "test",
            t99::X3 => "clippy",
            t99::X4 => "run",
            t99::X5 => "build-rel",
            t99::X6 => "clean",
            t99::X7 => "doc",
            t99::X8 => "fmt-chk",
            t99::X9 => "bench",
        }
    }

    /// True if clippy args need splitting (-- -D warnings goes after other args).
    fn has_lint_separator(&self) -> bool {
        matches!(self, t99::X3)
    }
}

// ── Result Type (t100) ───────────────────────────────────

/// t100=CargoResult. Compressed cargo output.
pub struct t100 {
    /// r0: command token (x0..x9).
    pub r0: String,
    /// r1: project token (p0..p10).
    pub r1: String,
    /// r2: ok/err.
    pub r2: bool,
    /// r3: duration seconds.
    pub r3: f64,
    /// r4: warning count.
    pub r4: u32,
    /// r5: error count.
    pub r5: u32,
    /// r6: compressed output (errors only, stripped).
    pub r6: String,
    /// r7: test counts "pass/fail/ignore" (x2 only).
    pub r7: Option<String>,
}

// ── Output Compression ──────────────────────────────────

/// Parse cargo's --message-format=json output. 60-80% smaller than text.
/// Extracts only: file, line, level, code, message. Skips everything else.
fn compress_json_messages(json_lines: &str) -> (u32, u32, String, Option<String>) {
    let mut warnings = 0u32;
    let mut errors = 0u32;
    let mut compressed: Vec<String> = Vec::new();
    let mut test_summary: Option<String> = None;

    for line in json_lines.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || !trimmed.starts_with('{') {
            // Non-JSON lines: test output goes to stdout/stderr unstructured.
            parse_test_line(trimmed, &mut errors, &mut compressed, &mut test_summary);
            continue;
        }

        let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) else {
            continue;
        };

        let reason = val.get("reason").and_then(|r| r.as_str()).unwrap_or("");

        match reason {
            "compiler-message" => {
                let Some(msg) = val.get("message") else {
                    continue;
                };
                let level = msg.get("level").and_then(|l| l.as_str()).unwrap_or("");
                let text = msg.get("message").and_then(|m| m.as_str()).unwrap_or("");
                let code = msg
                    .get("code")
                    .and_then(|c| c.get("code"))
                    .and_then(|c| c.as_str())
                    .unwrap_or("");

                // Extract primary span: file:line.
                let span_str = msg
                    .get("spans")
                    .and_then(|s| s.as_array())
                    .and_then(|spans| {
                        spans.iter().find(|s| {
                            s.get("is_primary")
                                .and_then(|p| p.as_bool())
                                .unwrap_or(false)
                        })
                    })
                    .map(|s| {
                        let file = s.get("file_name").and_then(|f| f.as_str()).unwrap_or("");
                        let line = s.get("line_start").and_then(|l| l.as_u64()).unwrap_or(0);
                        let short = compress_path(file);
                        format!("{}:{}", short, line)
                    })
                    .unwrap_or_default();

                match level {
                    "error" => {
                        errors += 1;
                        if code.is_empty() {
                            compressed.push(format!("E {}: {}", span_str, text));
                        } else {
                            compressed.push(format!("E[{}] {}: {}", code, span_str, text));
                        }
                    }
                    "warning" => {
                        warnings += 1;
                        // Only keep warnings with codes (skip "N warnings generated" meta).
                        if !code.is_empty() && compressed.len() < 15 {
                            compressed.push(format!("W[{}] {}: {}", code, span_str, text));
                        }
                    }
                    _ => {} // note, help — skip.
                }
            }
            "compiler-artifact" | "build-script-executed" | "build-finished" => {
                // Skip: AI doesn't need "Compiling foo v0.1.0" lines.
            }
            _ => {}
        }
    }

    // Cap output.
    if compressed.len() > 15 {
        let total = compressed.len();
        compressed.truncate(15);
        compressed.push(format!("...+{}", total - 15));
    }

    (warnings, errors, compressed.join("\n"), test_summary)
}

/// Parse unstructured test output lines (test runner doesn't use JSON format).
fn parse_test_line(
    trimmed: &str,
    errors: &mut u32,
    compressed: &mut Vec<String>,
    test_summary: &mut Option<String>,
) {
    if trimmed.starts_with("test result:") {
        let pass = extract_test_count(trimmed, "passed").unwrap_or(0);
        let fail = extract_test_count(trimmed, "failed").unwrap_or(0);
        let ign = extract_test_count(trimmed, "ignored").unwrap_or(0);
        *test_summary = Some(format!("{}/{}/{}", pass, fail, ign));
        if fail > 0 {
            *errors += fail;
        }
    }
    if trimmed.starts_with("---- ") && trimmed.ends_with(" ----") {
        let test_name = trimmed
            .trim_start_matches("---- ")
            .trim_end_matches(" ----");
        compressed.push(format!("FAIL:{}", test_name));
    }
    if trimmed.starts_with("thread '") && trimmed.contains("panicked at") {
        compressed.push(compress_path(trimmed));
    }
}

/// Fallback: text-mode compression for commands that don't support --message-format=json.
fn compress_output_text(stderr: &str, stdout: &str) -> (u32, u32, String, Option<String>) {
    let mut warnings = 0u32;
    let mut errors = 0u32;
    let mut compressed: Vec<String> = Vec::new();
    let mut test_summary: Option<String> = None;

    for line in stderr.lines().chain(stdout.lines()) {
        let trimmed = line.trim();
        if trimmed.starts_with("error[") || trimmed.starts_with("error:") {
            errors += 1;
            compressed.push(compress_path(trimmed));
        }
        if trimmed.contains("generated") && trimmed.contains("warning")
            && let Some(n) = extract_count(trimmed)
        {
            warnings = n;
        }
        parse_test_line(trimmed, &mut errors, &mut compressed, &mut test_summary);
    }

    if compressed.len() > 10 {
        let total = compressed.len();
        compressed.truncate(10);
        compressed.push(format!("...+{}", total - 10));
    }

    (warnings, errors, compressed.join("\n"), test_summary)
}

/// Strip paths: /Users/foo/bar/src/lib.rs → src/lib.rs
fn compress_path(p: &str) -> String {
    let mut out = p.to_string();
    if let Some(idx) = out.find("/src/") {
        let prefix_end = idx + 1; // keep "src/"
        if out.starts_with("error") || out.starts_with("E[") || out.starts_with("W[") {
            // Keep the error prefix.
        } else {
            out = out[prefix_end..].to_string();
        }
    }
    out
}

fn extract_count(line: &str) -> Option<u32> {
    line.split_whitespace().find_map(|w| w.parse::<u32>().ok())
}

fn extract_test_count(line: &str, label: &str) -> Option<u32> {
    let idx = line.find(label)?;
    let before = &line[..idx];
    before.split_whitespace().last()?.parse::<u32>().ok()
}

/// Commands that support --message-format=json.
fn supports_json(cmd: &t99) -> bool {
    matches!(
        cmd,
        t99::X0 | t99::X1 | t99::X2 | t99::X3 | t99::X5 | t99::X9
    )
}

// ── Output Printing ──────────────────────────────────────

/// Output field tokens for cargo results.
/// r0=cmd r1=project r2=status r3=time r4=warn r5=err r6=output r7=tests
fn print_result(result: &t100, expand: bool) {
    let status = if result.r2 { "ok" } else { "err" };
    if expand {
        eprintln!("cmd\tproj\tstatus\ttime\twarn\terr\ttests");
    } else {
        eprintln!("r0\tr1\tr2\tr3\tr4\tr5\tr7");
    }
    eprintln!(
        "{}\t{}\t{}\t{:.1}s\t{}\t{}\t{}",
        result.r0,
        result.r1,
        status,
        result.r3,
        result.r4,
        result.r5,
        result.r7.as_deref().unwrap_or("—"),
    );
    if !result.r6.is_empty() {
        eprintln!("{}", result.r6);
    }
}

fn print_multi(results: &[t100], expand: bool) {
    if expand {
        eprintln!("cmd\tproj\tstatus\ttime\twarn\terr\ttests");
    } else {
        eprintln!("r0\tr1\tr2\tr3\tr4\tr5\tr7");
    }
    for r in results {
        let status = if r.r2 { "ok" } else { "err" };
        eprintln!(
            "{}\t{}\t{}\t{:.1}s\t{}\t{}\t{}",
            r.r0,
            r.r1,
            status,
            r.r3,
            r.r4,
            r.r5,
            r.r7.as_deref().unwrap_or("—"),
        );
    }
    // Print errors below the table.
    for r in results {
        if !r.r6.is_empty() {
            eprintln!("[{}:{}]", r.r0, r.r1);
            eprintln!("{}", r.r6);
        }
    }
}

// ── Core Execution ──────────────────────────────────────

/// f133=cargo_exec. Execute single cargo command with compressed output.
fn f133(
    cmd: &t99,
    project: &str,
    features: Option<&str>,
    bin: Option<&str>,
    extra_args: &[String],
) -> t100 {
    let crate_name = resolve_project(project);
    let project_token = to_project_token(crate_name).to_string();

    // Try build preset from config.
    let preset = crate::config::load_build_preset(crate_name);

    // Build cargo args.
    let mut args: Vec<String> = Vec::new();

    // Base command args (but split clippy's -- -D warnings).
    let base = cmd.base_args();
    if cmd.has_lint_separator() {
        // For clippy: add "clippy" first, package/features, then "-- -D warnings".
        args.push(base[0].to_string());
    } else {
        for a in &base {
            args.push(a.to_string());
        }
    }

    // Package.
    args.push("-p".into());
    args.push(crate_name.into());

    // Features: explicit > preset > none.
    let feat = features.map(|f| f.to_string()).or_else(|| {
        preset.as_ref().and_then(|p| {
            if p.features.is_empty() {
                None
            } else {
                Some(p.features.join(","))
            }
        })
    });
    if let Some(f) = &feat {
        args.push("--features".into());
        args.push(f.clone());
    }

    // Target from preset.
    if let Some(ref p) = preset && let Some(ref t) = p.target {
        args.push("--target".into());
        args.push(t.clone());
    }

    // Bin.
    if let Some(b) = bin {
        args.push("--bin".into());
        args.push(b.into());
    }

    // Extra args before lint separator.
    for a in extra_args {
        args.push(a.clone());
    }

    // Append clippy lint args after everything.
    if cmd.has_lint_separator() {
        for a in &base[1..] {
            args.push(a.to_string());
        }
    }

    // Resolve working directory.
    let work_dir = workspace_root();

    // Use --message-format=json for supported commands (60-80% output reduction).
    let use_json = supports_json(cmd);
    if use_json {
        args.push("--message-format=json".to_string());
    }

    let start = Instant::now();
    let child = Command::new("cargo")
        .args(&args)
        .current_dir(&work_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    match child {
        Ok(output) => {
            let output = output.wait_with_output().unwrap_or_else(|e| {
                eprintln!("cargo wait failed: {}", e);
                std::process::Output {
                    status: std::process::ExitStatus::default(),
                    stdout: Vec::new(),
                    stderr: e.to_string().into_bytes(),
                }
            });
            let elapsed = start.elapsed().as_secs_f64();
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();

            let (warnings, errors, compressed, test_summary) = if use_json {
                // JSON output goes to stdout; test output goes to stderr.
                let combined = format!("{}\n{}", stdout, stderr);
                compress_json_messages(&combined)
            } else {
                compress_output_text(&stderr, &stdout)
            };

            t100 {
                r0: cmd_token(cmd).to_string(),
                r1: project_token,
                r2: output.status.success(),
                r3: elapsed,
                r4: warnings,
                r5: errors,
                r6: compressed,
                r7: test_summary,
            }
        }
        Err(e) => t100 {
            r0: cmd_token(cmd).to_string(),
            r1: project_token,
            r2: false,
            r3: start.elapsed().as_secs_f64(),
            r4: 0,
            r5: 1,
            r6: e.to_string(),
            r7: None,
        },
    }
}

fn cmd_token(cmd: &t99) -> &'static str {
    match cmd {
        t99::X0 => "x0",
        t99::X1 => "x1",
        t99::X2 => "x2",
        t99::X3 => "x3",
        t99::X4 => "x4",
        t99::X5 => "x5",
        t99::X6 => "x6",
        t99::X7 => "x7",
        t99::X8 => "x8",
        t99::X9 => "x9",
    }
}

/// f134=cargo_exec_multi. Run command on multiple projects in parallel.
fn f134(
    cmd: &t99,
    projects: &[String],
    features: Option<&str>,
    extra_args: &[String],
) -> Vec<t100> {
    let (tx, rx) = std::sync::mpsc::channel::<t100>();
    let handles: Vec<_> = projects
        .iter()
        .map(|proj| {
            let tx = tx.clone();
            let cmd = *cmd;
            let proj = proj.clone();
            let features = features.map(|f| f.to_string());
            let extra = extra_args.to_vec();
            std::thread::spawn(move || {
                let result = f133(&cmd, &proj, features.as_deref(), None, &extra);
                let _ = tx.send(result);
            })
        })
        .collect();
    drop(tx);
    let results: Vec<t100> = rx.into_iter().collect();
    for h in handles {
        let _ = h.join();
    }
    results
}

/// f135=cargo_exec_chain. Run multiple commands on same project sequentially. Stop on first error.
fn f135(cmds: &[t99], project: &str, features: Option<&str>, extra_args: &[String]) -> Vec<t100> {
    let mut results = Vec::new();
    for cmd in cmds {
        let r = f133(cmd, project, features, None, extra_args);
        let failed = !r.r2;
        results.push(r);
        if failed {
            break;
        }
    }
    results
}

fn workspace_root() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let root = crate::config::workspace_root(&cwd);
    if root.exists() {
        root
    } else {
        cwd
    }
}

// ── Dispatcher (f136) ────────────────────────────────────

/// f136=cargo_cmd_dispatch. Central dispatcher for tokenized cargo.
#[allow(clippy::too_many_arguments)]
pub fn f136(
    cmd: t99,
    project: Option<String>,
    features: Option<String>,
    bin: Option<String>,
    extra: Vec<String>,
    all: bool,
    chain: Option<String>,
    expand: bool,
) -> anyhow::Result<()> {
    // Chain mode: x1,x2,x3 on same project sequentially.
    if let Some(chain_str) = chain {
        let proj = project.as_deref().unwrap_or("p0");
        let cmds: Vec<t99> = chain_str
            .split(',')
            .filter_map(|s| parse_cmd_token(s.trim()))
            .collect();
        if cmds.is_empty() {
            anyhow::bail!("No valid command tokens in chain. Use x0,x1,x2,...");
        }
        let results = f135(&cmds, proj, features.as_deref(), &extra);
        print_multi(&results, expand);
        if results.iter().any(|r| !r.r2) {
            anyhow::bail!("Chain stopped on error");
        }
        return Ok(());
    }

    // All mode: run on all workspace crates in parallel.
    if all {
        let projects: Vec<String> = PROJECT_MAP.iter().map(|(k, _)| k.to_string()).collect();
        let results = f134(&cmd, &projects, features.as_deref(), &extra);
        print_multi(&results, expand);
        if results.iter().any(|r| !r.r2) {
            anyhow::bail!("Some projects failed");
        }
        return Ok(());
    }

    // Single project.
    let proj = project.as_deref().unwrap_or("p0");
    let result = f133(&cmd, proj, features.as_deref(), bin.as_deref(), &extra);
    let failed = !result.r2;
    print_result(&result, expand);
    if failed {
        anyhow::bail!("cargo {} failed", cmd.name());
    }
    Ok(())
}

fn parse_cmd_token(s: &str) -> Option<t99> {
    match s {
        "x0" => Some(t99::X0),
        "x1" => Some(t99::X1),
        "x2" => Some(t99::X2),
        "x3" => Some(t99::X3),
        "x4" => Some(t99::X4),
        "x5" => Some(t99::X5),
        "x6" => Some(t99::X6),
        "x7" => Some(t99::X7),
        "x8" => Some(t99::X8),
        "x9" => Some(t99::X9),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compress_json_error_with_code() {
        let json = r#"{"reason":"compiler-message","message":{"level":"error","message":"mismatched types","code":{"code":"E0308"},"spans":[{"file_name":"src/lib.rs","line_start":42,"is_primary":true}]}}"#;
        let (w, e, out, _) = compress_json_messages(json);
        assert_eq!(e, 1);
        assert_eq!(w, 0);
        assert!(out.contains("E[E0308]"), "got: {}", out);
        assert!(out.contains("src/lib.rs:42"), "got: {}", out);
        assert!(out.contains("mismatched types"), "got: {}", out);
    }

    #[test]
    fn compress_json_warning_with_code() {
        let json = r#"{"reason":"compiler-message","message":{"level":"warning","message":"unused variable","code":{"code":"unused_variables"},"spans":[{"file_name":"src/main.rs","line_start":10,"is_primary":true}]}}"#;
        let (w, e, out, _) = compress_json_messages(json);
        assert_eq!(w, 1);
        assert_eq!(e, 0);
        assert!(out.contains("W[unused_variables]"), "got: {}", out);
    }

    #[test]
    fn compress_json_skips_artifacts() {
        let json = r#"{"reason":"compiler-artifact","target":{"name":"kova"}}"#;
        let (w, e, out, _) = compress_json_messages(json);
        assert_eq!(w, 0);
        assert_eq!(e, 0);
        assert!(out.is_empty());
    }

    #[test]
    fn compress_json_skips_notes() {
        let json = r#"{"reason":"compiler-message","message":{"level":"note","message":"some help text","code":null,"spans":[]}}"#;
        let (w, e, out, _) = compress_json_messages(json);
        assert_eq!(w, 0);
        assert_eq!(e, 0);
        assert!(out.is_empty());
    }

    #[test]
    fn compress_json_test_results() {
        let input = "test result: ok. 15 passed; 2 failed; 1 ignored; 0 measured";
        let (_, e, _, summary) = compress_json_messages(input);
        assert_eq!(summary, Some("15/2/1".to_string()));
        assert_eq!(e, 2);
    }

    #[test]
    fn compress_json_failed_test_name() {
        let input = "---- my_module::tests::my_test ----";
        let (_, _, out, _) = compress_json_messages(input);
        assert!(
            out.contains("FAIL:my_module::tests::my_test"),
            "got: {}",
            out
        );
    }

    #[test]
    fn compress_json_caps_output() {
        let mut lines = String::new();
        for i in 0..20 {
            lines.push_str(&format!(
                r#"{{"reason":"compiler-message","message":{{"level":"error","message":"err{}","code":null,"spans":[]}}}}"#,
                i
            ));
            lines.push('\n');
        }
        let (_, e, out, _) = compress_json_messages(&lines);
        assert_eq!(e, 20);
        assert!(out.contains("...+5"), "should cap at 15, got: {}", out);
    }

    #[test]
    fn compress_json_strips_path() {
        let json = r#"{"reason":"compiler-message","message":{"level":"error","message":"boom","code":{"code":"E0001"},"spans":[{"file_name":"/Users/foo/kova/src/lib.rs","line_start":5,"is_primary":true}]}}"#;
        let (_, _, out, _) = compress_json_messages(json);
        // Should not contain absolute path
        assert!(!out.contains("/Users/foo"), "got: {}", out);
        assert!(out.contains("src/lib.rs:5"), "got: {}", out);
    }

    #[test]
    fn compress_text_fallback() {
        let stderr = "error[E0308]: mismatched types\nwarning: 3 warnings generated\n";
        let (w, e, out, _) = compress_output_text(stderr, "");
        assert_eq!(e, 1);
        assert_eq!(w, 3);
        assert!(out.contains("E0308"), "got: {}", out);
    }

    #[test]
    fn resolve_project_known() {
        assert_eq!(resolve_project("p0"), "kova");
        assert_eq!(resolve_project("p2"), "cochranblock");
    }

    #[test]
    fn resolve_project_passthrough() {
        assert_eq!(resolve_project("unknown-crate"), "unknown-crate");
    }

    #[test]
    fn to_project_token_roundtrip() {
        assert_eq!(to_project_token("kova"), "p0");
        assert_eq!(to_project_token("approuter"), "p1");
        assert_eq!(to_project_token("nonexistent"), "nonexistent");
    }

    #[test]
    fn parse_cmd_token_valid() {
        assert!(matches!(parse_cmd_token("x0"), Some(t99::X0)));
        assert!(matches!(parse_cmd_token("x9"), Some(t99::X9)));
    }

    #[test]
    fn parse_cmd_token_invalid() {
        assert!(parse_cmd_token("x10").is_none());
        assert!(parse_cmd_token("foo").is_none());
    }

    #[test]
    fn supports_json_check() {
        assert!(supports_json(&t99::X0)); // build
        assert!(supports_json(&t99::X1)); // check
        assert!(supports_json(&t99::X3)); // clippy
        assert!(!supports_json(&t99::X6)); // clean
        assert!(!supports_json(&t99::X8)); // fmt
    }
}
