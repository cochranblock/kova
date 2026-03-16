// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! CI mode. Headless quality gate: watch for changes, run check/clippy/test.
//! f177=ci_check, f178=ci_watch, f179=ci_once, f180=print_ci_result.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// t114=CiConfig. Controls what CI runs and how often.
pub struct CiConfig {
    pub project_dir: PathBuf,
    pub watch_interval_secs: u64,
    pub run_clippy: bool,
    pub run_tests: bool,
}

impl Default for CiConfig {
    fn default() -> Self {
        Self {
            project_dir: std::env::current_dir().unwrap_or_default(),
            watch_interval_secs: 5,
            run_clippy: true,
            run_tests: true,
        }
    }
}

/// t115=CiResult. Structured output from a CI run.
pub struct CiResult {
    pub passed: bool,
    pub check_ok: bool,
    pub clippy_ok: Option<bool>,
    pub tests_ok: Option<bool>,
    pub errors: Vec<String>,
    pub duration_ms: u64,
}

/// f177=ci_check. Run cargo check, optionally clippy and tests. Returns structured result.
pub fn ci_check(project_dir: &Path, config: &CiConfig) -> CiResult {
    let start = Instant::now();
    let mut errors = Vec::new();

    // Stage 1: cargo check
    let (check_ok, check_stderr) = run_cargo(project_dir, &["check"]);
    if !check_ok {
        errors.push(format!("check: {}", truncate_stderr(&check_stderr)));
    }

    // Stage 2: clippy (if enabled and check passed)
    let clippy_ok = if config.run_clippy && check_ok {
        let (ok, stderr) = run_cargo(project_dir, &["clippy", "--", "-D", "warnings"]);
        if !ok {
            errors.push(format!("clippy: {}", truncate_stderr(&stderr)));
        }
        Some(ok)
    } else if config.run_clippy {
        // Skip clippy when check fails
        Some(false)
    } else {
        None
    };

    // Stage 3: tests (if enabled and check passed)
    let tests_ok = if config.run_tests && check_ok {
        let (ok, stderr) = run_cargo(project_dir, &["test"]);
        if !ok {
            errors.push(format!("test: {}", truncate_stderr(&stderr)));
        }
        Some(ok)
    } else if config.run_tests {
        Some(false)
    } else {
        None
    };

    let passed = check_ok
        && clippy_ok.unwrap_or(true)
        && tests_ok.unwrap_or(true);

    CiResult {
        passed,
        check_ok,
        clippy_ok,
        tests_ok,
        errors,
        duration_ms: start.elapsed().as_millis() as u64,
    }
}

/// f178=ci_watch. Poll loop: detect file changes via mtime, run ci_check on change.
pub fn ci_watch(config: &CiConfig) -> anyhow::Result<()> {
    let interval = std::time::Duration::from_secs(config.watch_interval_secs);
    let mut snapshots = snapshot_mtimes(&config.project_dir);

    println!("ci: watching {} (every {}s)", config.project_dir.display(), config.watch_interval_secs);
    println!("ci: check=on clippy={} tests={}", on_off(config.run_clippy), on_off(config.run_tests));

    // Run once at start
    let result = ci_check(&config.project_dir, config);
    print_ci_result(&result);

    loop {
        std::thread::sleep(interval);

        let current = snapshot_mtimes(&config.project_dir);
        if current == snapshots {
            continue;
        }
        snapshots = current;

        println!("\nci: change detected, running...");
        let result = ci_check(&config.project_dir, config);
        print_ci_result(&result);
    }
}

/// f179=ci_once. Single CI run on a project directory.
pub fn ci_once(project_dir: &Path) -> anyhow::Result<CiResult> {
    let config = CiConfig {
        project_dir: project_dir.to_path_buf(),
        ..Default::default()
    };
    Ok(ci_check(project_dir, &config))
}

/// f180=print_ci_result. Formatted output with pass/fail status.
pub fn print_ci_result(result: &CiResult) {
    let status = if result.passed { "PASS" } else { "FAIL" };
    println!("ci: {} ({}ms)", status, result.duration_ms);
    println!("  check: {}", pass_fail(result.check_ok));
    if let Some(ok) = result.clippy_ok {
        println!("  clippy: {}", pass_fail(ok));
    }
    if let Some(ok) = result.tests_ok {
        println!("  tests: {}", pass_fail(ok));
    }
    for err in &result.errors {
        println!("  err: {}", err);
    }
}

// --- internals ---

fn run_cargo(project_dir: &Path, args: &[&str]) -> (bool, String) {
    match std::process::Command::new("cargo")
        .args(args)
        .current_dir(project_dir)
        .output()
    {
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr).into_owned();
            (o.status.success(), stderr)
        }
        Err(e) => (false, e.to_string()),
    }
}

fn truncate_stderr(s: &str) -> String {
    let lines: Vec<&str> = s.lines().collect();
    if lines.len() <= 10 {
        s.trim().to_string()
    } else {
        let kept: Vec<&str> = lines[..10].to_vec();
        format!("{}\n  ... ({} more lines)", kept.join("\n"), lines.len() - 10)
    }
}

fn pass_fail(ok: bool) -> &'static str {
    if ok { "ok" } else { "FAILED" }
}

fn on_off(b: bool) -> &'static str {
    if b { "on" } else { "off" }
}

/// Collect mtime for all .rs and Cargo.toml files under src/.
fn snapshot_mtimes(project_dir: &Path) -> HashMap<PathBuf, u128> {
    let mut map = HashMap::new();

    // Check Cargo.toml
    let cargo_toml = project_dir.join("Cargo.toml");
    if let Ok(meta) = std::fs::metadata(&cargo_toml) {
        if let Ok(modified) = meta.modified() {
            let secs = modified
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            map.insert(cargo_toml, secs);
        }
    }

    // Walk src/ for .rs files
    let src_dir = project_dir.join("src");
    if src_dir.is_dir() {
        walk_rs_files(&src_dir, &mut map);
    }

    map
}

fn walk_rs_files(dir: &Path, map: &mut HashMap<PathBuf, u128>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_rs_files(&path, map);
        } else if path.extension().is_some_and(|e| e == "rs") {
            if let Ok(meta) = std::fs::metadata(&path) {
                if let Ok(modified) = meta.modified() {
                    let secs = modified
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis();
                    map.insert(path, secs);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// f177=ci_check. Valid temp project passes all stages.
    #[test]
    fn ci_check_valid_project_passes() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            r#"[package]
name = "ci-test"
version = "0.1.0"
edition = "2021"
"#,
        )
        .unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(
            tmp.path().join("src/lib.rs"),
            "pub fn add(a: i32, b: i32) -> i32 { a + b }\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn it_works() { assert_eq!(super::add(1, 2), 3); }\n}\n",
        )
        .unwrap();

        let config = CiConfig {
            project_dir: tmp.path().to_path_buf(),
            run_clippy: true,
            run_tests: true,
            ..Default::default()
        };
        let result = ci_check(tmp.path(), &config);
        assert!(result.passed, "valid project must pass CI");
        assert!(result.check_ok);
        assert_eq!(result.clippy_ok, Some(true));
        assert_eq!(result.tests_ok, Some(true));
        assert!(result.errors.is_empty());
    }

    /// f177=ci_check. Broken code fails check stage.
    #[test]
    fn ci_check_broken_code_fails() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            r#"[package]
name = "ci-broken"
version = "0.1.0"
edition = "2021"
"#,
        )
        .unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src/lib.rs"), "fn broken( {").unwrap();

        let config = CiConfig {
            project_dir: tmp.path().to_path_buf(),
            run_clippy: true,
            run_tests: true,
            ..Default::default()
        };
        let result = ci_check(tmp.path(), &config);
        assert!(!result.passed, "broken code must fail CI");
        assert!(!result.check_ok);
        assert!(!result.errors.is_empty());
    }

    /// f180=print_ci_result. Formatting includes status and stages.
    #[test]
    fn ci_result_formatting() {
        let result = CiResult {
            passed: true,
            check_ok: true,
            clippy_ok: Some(true),
            tests_ok: Some(true),
            errors: vec![],
            duration_ms: 42,
        };
        // Verify print_ci_result doesn't panic
        print_ci_result(&result);

        // Verify pass/fail labels
        assert_eq!(pass_fail(true), "ok");
        assert_eq!(pass_fail(false), "FAILED");
    }

    /// f179=ci_once. Single run returns result.
    #[test]
    fn ci_once_returns_result() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            r#"[package]
name = "ci-once"
version = "0.1.0"
edition = "2021"
"#,
        )
        .unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src/lib.rs"), "").unwrap();

        let result = ci_once(tmp.path()).unwrap();
        assert!(result.passed);
        assert!(result.check_ok);
    }
}
