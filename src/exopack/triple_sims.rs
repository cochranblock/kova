// Unlicense — public domain — cochranblock.org
//! f60 = triple_sims — run test runner 3 times; all must pass.
//! TRIPLE SIMS: 3 sequential passes, exit 0 only if all pass.

use std::path::Path;
use std::process::Command;
use std::time::Instant;

/// f61 = run_cargo_test_n. Runs `cargo test` N times in project_dir. Returns (ok, stderr).
/// For kova and other orchestrators. N=3 = TRIPLE SIMS.
pub fn f61(project_dir: &Path, n: u32) -> (bool, String) {
    f61_with_args(project_dir, n, &[])
}

/// f61_with_args. f61 variant with extra cargo args (e.g. --no-default-features).
pub fn f61_with_args(project_dir: &Path, n: u32, args: &[&str]) -> (bool, String) {
    for i in 1..=n {
        let mut cmd = Command::new("cargo");
        cmd.arg("test").current_dir(project_dir);
        cmd.args(args);
        match cmd.output() {
            Ok(o) if o.status.success() => continue,
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr).into_owned();
                return (
                    false,
                    format!("TRIPLE SIMS pass {}/{} failed:\n{}", i, n, stderr),
                );
            }
            Err(e) => return (false, format!("cargo test: {}", e)),
        }
    }
    (true, String::new())
}

/// f63 = discover_test_bin. Discover -test binary name from Cargo.toml. Returns first [[bin]] with name ending in "-test".
pub fn f63_discover_test_bin(project_dir: &Path) -> Option<String> {
    let manifest = project_dir.join("Cargo.toml");
    let content = std::fs::read_to_string(&manifest).ok()?;
    let mut in_bin = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("[[bin]]") {
            in_bin = true;
            continue;
        }
        if in_bin && trimmed.starts_with("name = ") {
            if let Some(name) = trimmed
                .strip_prefix("name = ")
                .and_then(|s| s.strip_prefix('"'))
                .and_then(|s| s.strip_suffix('"'))
            {
                if name.ends_with("-test") {
                    return Some(name.to_string());
                }
            }
            in_bin = false;
        }
    }
    None
}

/// f62=live_demo. Build and run a -test binary with live stdout/stderr for evaluation in motion.
/// Uses Stdio::inherit so output streams to the terminal. Returns exit status.
pub fn f62_live_demo(
    project_dir: &Path,
    bin_name: &str,
    cargo_args: &[&str],
) -> std::io::Result<std::process::ExitStatus> {
    let manifest = project_dir.join("Cargo.toml");
    if !manifest.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Cargo.toml not found in {}", project_dir.display()),
        ));
    }

    // Build
    let mut build = std::process::Command::new("cargo");
    build
        .arg("build")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--bin")
        .arg(bin_name)
        .args(cargo_args)
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit());
    let status = build.status()?;
    if !status.success() {
        return Ok(status);
    }

    // Run with live output. TEST_DEMO=1 enables demo mode for self-evaluation.
    let mut run = std::process::Command::new("cargo");
    run.arg("run")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--bin")
        .arg(bin_name)
        .args(cargo_args)
        .env("TEST_DEMO", "1")
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .stdin(std::process::Stdio::inherit());
    run.status()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn test_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("{}_{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn discover_test_bin_finds_test_binary() {
        let dir = test_dir("exopack_test_discover");
        let manifest = dir.join("Cargo.toml");
        let mut f = std::fs::File::create(&manifest).unwrap();
        writeln!(f, "[package]\nname = \"foo\"\nversion = \"0.1.0\"\n").unwrap();
        writeln!(
            f,
            "[[bin]]\nname = \"foo-test\"\npath = \"src/bin/test.rs\""
        )
        .unwrap();
        drop(f);

        assert_eq!(f63_discover_test_bin(&dir), Some("foo-test".to_string()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn discover_test_bin_returns_none_without_test_binary() {
        let dir = test_dir("exopack_test_discover_none");
        let manifest = dir.join("Cargo.toml");
        let mut f = std::fs::File::create(&manifest).unwrap();
        writeln!(f, "[package]\nname = \"bar\"\nversion = \"0.1.0\"\n").unwrap();
        writeln!(f, "[[bin]]\nname = \"bar\"\npath = \"src/main.rs\"").unwrap();
        drop(f);

        assert_eq!(f63_discover_test_bin(&dir), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn discover_test_bin_missing_manifest() {
        let dir = test_dir("exopack_test_discover_missing");
        let _ = std::fs::remove_file(dir.join("Cargo.toml"));
        assert_eq!(f63_discover_test_bin(&dir), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn discover_test_bin_skips_non_test_first_bin() {
        // Two [[bin]] sections; first does not end with -test, second does.
        let dir = test_dir("exopack_multi_bin");
        let manifest = dir.join("Cargo.toml");
        let mut f = std::fs::File::create(&manifest).unwrap();
        writeln!(f, "[package]\nname = \"multi\"\nversion = \"0.1.0\"\n").unwrap();
        writeln!(f, "[[bin]]\nname = \"multi\"\npath = \"src/main.rs\"\n").unwrap();
        writeln!(f, "[[bin]]\nname = \"multi-test\"\npath = \"src/bin/test.rs\"").unwrap();
        drop(f);
        assert_eq!(f63_discover_test_bin(&dir), Some("multi-test".to_string()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn discover_test_bin_ignores_test_prefix_not_suffix() {
        // "test-foo" has "test" in it but not as a -test suffix — must not match.
        let dir = test_dir("exopack_prefix_test");
        let manifest = dir.join("Cargo.toml");
        let mut f = std::fs::File::create(&manifest).unwrap();
        writeln!(f, "[package]\nname = \"pfx\"\nversion = \"0.1.0\"\n").unwrap();
        writeln!(f, "[[bin]]\nname = \"test-pfx\"\npath = \"src/main.rs\"").unwrap();
        drop(f);
        assert_eq!(f63_discover_test_bin(&dir), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn discover_test_bin_exact_suffix_test_only() {
        // A binary named exactly "test" — does not end with "-test".
        let dir = test_dir("exopack_exact_test");
        let manifest = dir.join("Cargo.toml");
        let mut f = std::fs::File::create(&manifest).unwrap();
        writeln!(f, "[package]\nname = \"ex\"\nversion = \"0.1.0\"\n").unwrap();
        writeln!(f, "[[bin]]\nname = \"test\"\npath = \"src/main.rs\"").unwrap();
        drop(f);
        assert_eq!(f63_discover_test_bin(&dir), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    // f60 async runner — requires tokio runtime
    #[tokio::test]
    async fn triple_sims_all_pass_returns_true() {
        let result = f60(|| async { true }).await;
        assert!(result, "all-pass run must return true");
    }

    #[tokio::test]
    async fn triple_sims_immediate_fail_returns_false() {
        let result = f60(|| async { false }).await;
        assert!(!result, "always-fail run must return false");
    }

    #[tokio::test]
    async fn triple_sims_fails_on_third_pass() {
        use std::sync::{
            atomic::{AtomicU32, Ordering},
            Arc,
        };
        let count = Arc::new(AtomicU32::new(0));
        let result = f60(|| {
            let c = count.clone();
            async move {
                // passes 1st and 2nd, fails 3rd
                c.fetch_add(1, Ordering::SeqCst) < 2
            }
        })
        .await;
        assert!(!result, "run that fails on 3rd pass must return false");
    }

    #[tokio::test]
    async fn triple_sims_exactly_three_runs() {
        use std::sync::{
            atomic::{AtomicU32, Ordering},
            Arc,
        };
        let count = Arc::new(AtomicU32::new(0));
        let c = count.clone();
        let result = f60(|| {
            let c2 = c.clone();
            async move {
                c2.fetch_add(1, Ordering::SeqCst);
                true
            }
        })
        .await;
        assert!(result);
        assert_eq!(
            count.load(Ordering::SeqCst),
            3,
            "f60 must invoke closure exactly 3 times on success"
        );
    }

    // f61 — run_cargo_test_n: validate fail path when binary missing
    #[test]
    fn run_cargo_test_n_fails_with_nonexistent_dir() {
        let dir = std::path::Path::new("/tmp/exopack_nonexistent_cargo_proj_xyz");
        let (ok, msg) = f61(dir, 1);
        assert!(!ok, "must fail for nonexistent project dir");
        assert!(!msg.is_empty(), "error message must not be empty");
    }

    #[test]
    fn run_cargo_test_n_returns_true_for_zero_runs() {
        // n=0 means no runs — loop body never executes, returns (true, "")
        let dir = std::env::temp_dir();
        let (ok, msg) = f61(&dir, 0);
        assert!(ok, "zero runs should vacuously pass");
        assert!(msg.is_empty());
    }

    // f62_live_demo: validate error path when Cargo.toml is absent
    #[test]
    fn live_demo_errors_on_missing_manifest() {
        let dir = std::path::Path::new("/tmp/exopack_no_manifest_xyz");
        let result = f62_live_demo(dir, "foo-test", &[]);
        assert!(result.is_err(), "must return Err when Cargo.toml absent");
    }
}

/// f60 = triple_sims_run. Runs `run_once` 3 times. Returns true iff all pass.
/// Prints pass count and timing. Use from test binary: exit 0 when true.
pub async fn f60<F, Fut>(run_once: F) -> bool
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    const N: u32 = 3;
    let mut passed = 0u32;
    for i in 1..=N {
        let start = Instant::now();
        let ok = run_once().await;
        let ms = start.elapsed().as_millis();
        if ok {
            passed += 1;
            println!("TRIPLE SIMS pass {}/{} OK ({}ms)", i, N, ms);
        } else {
            eprintln!("TRIPLE SIMS pass {}/{} FAILED ({}ms)", i, N, ms);
            return false;
        }
    }
    println!("TRIPLE SIMS: {}/{} passes OK", passed, N);
    passed == N
}
