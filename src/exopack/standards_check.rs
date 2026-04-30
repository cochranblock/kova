// Unlicense — public domain — cochranblock.org
//! standards_check — Rust industry standards gate for the whole portfolio.
//! 14 checks per project. Pass/fail table. The quality gate.

use std::path::{Path, PathBuf};
use std::process::Command;

/// t70: Result of a single standard check
#[derive(Debug, Clone)]
pub struct t70 {
    /// s80: check name
    pub s80: &'static str,
    /// s81: passed
    pub s81: bool,
    /// s82: detail message
    pub s82: String,
}

/// t71: Full report for one project
#[derive(Debug, Clone)]
pub struct t71 {
    /// s83: project name
    pub s83: String,
    /// s84: project path
    pub s84: PathBuf,
    /// s85: check results
    pub s85: Vec<t70>,
}

/// t72: Portfolio report — all projects
#[derive(Debug, Clone)]
pub struct t72 {
    /// s86: project reports
    pub s86: Vec<t71>,
}

impl t70 {
    fn pass(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            s80: name,
            s81: true,
            s82: detail.into(),
        }
    }
    fn fail(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            s80: name,
            s81: false,
            s82: detail.into(),
        }
    }
}

impl t71 {
    pub fn passed(&self) -> usize {
        self.s85.iter().filter(|c| c.s81).count()
    }
    pub fn failed(&self) -> usize {
        self.s85.iter().filter(|c| !c.s81).count()
    }
    pub fn total(&self) -> usize {
        self.s85.len()
    }
}

impl t72 {
    /// f100: Print the pass/fail table
    pub fn print_table(&self) {
        let all_checks: Vec<&str> = if let Some(first) = self.s86.first() {
            first.s85.iter().map(|c| c.s80).collect()
        } else {
            return;
        };

        // Header
        print!("{:<20}", "CHECK");
        for proj in &self.s86 {
            print!(" {:<14}", proj.s83);
        }
        println!();
        println!("{}", "-".repeat(20 + self.s86.len() * 15));

        // Rows
        for (i, check_name) in all_checks.iter().enumerate() {
            print!("{:<20}", check_name);
            for proj in &self.s86 {
                if let Some(result) = proj.s85.get(i) {
                    let sym = if result.s81 { "PASS" } else { "FAIL" };
                    print!(" {:<14}", sym);
                }
            }
            println!();
        }

        // Summary
        println!("{}", "-".repeat(20 + self.s86.len() * 15));
        print!("{:<20}", "TOTAL");
        for proj in &self.s86 {
            print!(" {:<14}", format!("{}/{}", proj.passed(), proj.total()));
        }
        println!();
    }
}

/// f101: Run all 14 standards checks on a single project
pub fn f101(project_dir: &Path) -> t71 {
    let name = project_dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Workspace projects have Cargo.toml in a subdirectory matching the project name
    let root_content = std::fs::read_to_string(project_dir.join("Cargo.toml")).unwrap_or_default();
    let is_workspace = root_content.contains("[workspace]");

    let (cargo_toml_path, src_dir) = if is_workspace {
        let inner = project_dir.join(&name);
        let inner_toml = inner.join("Cargo.toml");
        if inner_toml.exists() {
            (inner_toml, inner)
        } else {
            (project_dir.join("Cargo.toml"), project_dir.to_path_buf())
        }
    } else {
        (project_dir.join("Cargo.toml"), project_dir.to_path_buf())
    };

    let cargo_content = std::fs::read_to_string(&cargo_toml_path).unwrap_or_default();

    let checks = vec![
        f102(project_dir),              // clippy (run from workspace root)
        f103(project_dir),              // fmt (run from workspace root)
        f104(project_dir),              // audit
        f105(project_dir),              // deny
        f106(&cargo_content),           // MSRV
        f107(&src_dir),                 // unsafe (check inner src/)
        f108(&src_dir),                 // module docs (check inner src/)
        f109(project_dir),              // changelog
        f110(project_dir),              // license file
        f111(&src_dir, &cargo_content), // test binary (P16)
        f112(&src_dir),                 // allow(unused) (check inner src/)
        f113(&src_dir),                 // error handling (check inner src/)
        f114(project_dir),              // secrets (check root for .env)
        f115(&cargo_content),           // Cargo.toml metadata
    ];

    t71 {
        s83: name,
        s84: project_dir.to_path_buf(),
        s85: checks,
    }
}

/// f116: Run standards check on multiple projects, return portfolio report
pub fn f116(projects: &[&Path]) -> t72 {
    let reports: Vec<t71> = projects.iter().map(|p| f101(p)).collect();
    t72 { s86: reports }
}

// --- Individual checks ---

/// f102: cargo clippy -- -D warnings (zero warnings)
fn f102(dir: &Path) -> t70 {
    let result = Command::new("cargo")
        .args(["clippy", "--", "-D", "warnings"])
        .current_dir(dir)
        .output();
    match result {
        Ok(out) if out.status.success() => t70::pass("clippy", "zero warnings"),
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let warning_count = stderr.lines().filter(|l| l.contains("warning[")).count();
            t70::fail("clippy", format!("{} warnings", warning_count))
        }
        Err(e) => t70::fail("clippy", format!("failed to run: {}", e)),
    }
}

/// f103: cargo fmt --check
fn f103(dir: &Path) -> t70 {
    let result = Command::new("cargo")
        .args(["fmt", "--check"])
        .current_dir(dir)
        .output();
    match result {
        Ok(out) if out.status.success() => t70::pass("fmt", "formatted"),
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let diff_files = stdout.lines().filter(|l| l.starts_with("Diff in")).count();
            t70::fail("fmt", format!("{} files need formatting", diff_files))
        }
        Err(e) => t70::fail("fmt", format!("failed to run: {}", e)),
    }
}

/// f104: cargo audit
fn f104(dir: &Path) -> t70 {
    let result = Command::new("cargo")
        .args(["audit"])
        .current_dir(dir)
        .output();
    match result {
        Ok(out) => {
            let stdout_str = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            let combined = format!("{}{}", stdout_str, stderr);
            let vuln_count = combined
                .lines()
                .filter(|l| l.starts_with("RUSTSEC-"))
                .count();
            // cargo audit exits non-zero for warnings too; only fail on actual vulns
            if vuln_count == 0 {
                t70::pass("audit", "no known vulns")
            } else {
                t70::fail("audit", format!("{} advisories", vuln_count))
            }
        }
        Err(_) => t70::fail("audit", "cargo-audit not installed"),
    }
}

/// f105: cargo deny check
fn f105(dir: &Path) -> t70 {
    // Check if deny.toml exists — skip if not configured
    let has_config = dir.join("deny.toml").exists();
    let result = Command::new("cargo")
        .args(["deny", "check"])
        .current_dir(dir)
        .output();
    match result {
        Ok(out) if out.status.success() => t70::pass("deny", "license + deps OK"),
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let last = stderr.lines().last().unwrap_or("failed");
            if !has_config {
                // No deny.toml = not configured, treat as advisory fail
                t70::fail("deny", "no deny.toml configured")
            } else {
                t70::fail("deny", last.to_string())
            }
        }
        Err(_) => t70::fail("deny", "cargo-deny not installed"),
    }
}

/// f106: MSRV declared (rust-version field)
fn f106(cargo_content: &str) -> t70 {
    if cargo_content.contains("rust-version") {
        let version = cargo_content
            .lines()
            .find(|l| l.starts_with("rust-version"))
            .and_then(|l| l.split('=').nth(1))
            .map(|v| v.trim().trim_matches('"'))
            .unwrap_or("?");
        t70::pass("msrv", format!("rust-version = {}", version))
    } else {
        t70::fail("msrv", "no rust-version in Cargo.toml")
    }
}

/// f107: #![forbid(unsafe_code)] or justified unsafe
fn f107(dir: &Path) -> t70 {
    let src_dir = dir.join("src");
    if !src_dir.exists() {
        return t70::fail("unsafe", "no src/ directory");
    }

    // Check lib.rs and main.rs for forbid(unsafe_code)
    let lib_rs = src_dir.join("lib.rs");
    let main_rs = src_dir.join("main.rs");
    let has_forbid = [&lib_rs, &main_rs].iter().any(|p| {
        std::fs::read_to_string(p)
            .map(|s| s.contains("forbid(unsafe_code)"))
            .unwrap_or(false)
    });

    if has_forbid {
        return t70::pass("unsafe", "#![forbid(unsafe_code)]");
    }

    // Count actual unsafe blocks/fns in source (not string literals or comments)
    let mut unsafe_count = 0u32;
    let src_dir2 = dir.join("src");
    if src_dir2.exists() {
        visit_rs_files(&src_dir2, &mut |content, _path| {
            for line in content.lines() {
                let t = line.trim();
                if t.starts_with("//") || t.contains(".contains(") {
                    continue;
                }
                if t.contains("unsafe {") || t.contains("unsafe fn ") {
                    unsafe_count += 1;
                }
            }
        });
    }
    if unsafe_count == 0 {
        t70::fail("unsafe", "no unsafe but missing #![forbid(unsafe_code)]")
    } else {
        t70::fail(
            "unsafe",
            format!("{} unsafe usages, no forbid", unsafe_count),
        )
    }
}

/// f108: lib.rs has //! module docs
fn f108(dir: &Path) -> t70 {
    let lib_rs = dir.join("src").join("lib.rs");
    let main_rs = dir.join("src").join("main.rs");

    let check = |path: &Path| -> bool {
        std::fs::read_to_string(path)
            .map(|s| s.lines().any(|l| l.trim().starts_with("//!")))
            .unwrap_or(false)
    };

    if check(&lib_rs) || check(&main_rs) {
        t70::pass("docs", "//! module docs present")
    } else {
        t70::fail("docs", "no //! docs in lib.rs or main.rs")
    }
}

/// f109: CHANGELOG.md or TIMELINE_OF_INVENTION.md exists
fn f109(dir: &Path) -> t70 {
    let candidates = ["CHANGELOG.md", "TIMELINE_OF_INVENTION.md", "CHANGES.md"];
    for name in &candidates {
        if dir.join(name).exists() {
            return t70::pass("changelog", format!("{} exists", name));
        }
    }
    t70::fail("changelog", "no CHANGELOG.md or TIMELINE_OF_INVENTION.md")
}

/// f110: LICENSE/UNLICENSE file present
fn f110(dir: &Path) -> t70 {
    let candidates = [
        "LICENSE",
        "UNLICENSE",
        "LICENSE.md",
        "LICENSE-MIT",
        "LICENSE-APACHE",
    ];
    for name in &candidates {
        if dir.join(name).exists() {
            return t70::pass("license_file", format!("{} exists", name));
        }
    }
    t70::fail("license_file", "no LICENSE file")
}

/// f111: CI-equivalent test binary exists (P16 pattern)
fn f111(_dir: &Path, cargo_content: &str) -> t70 {
    // Look for [[bin]] entries with name ending in "-test"
    if cargo_content.contains("-test") && cargo_content.contains("[[bin]]") {
        t70::pass("test_binary", "P16 test binary declared")
    } else {
        t70::fail("test_binary", "no *-test binary in Cargo.toml")
    }
}

/// f112: No #[allow(unused)] without justification
fn f112(dir: &Path) -> t70 {
    let src_dir = dir.join("src");
    if !src_dir.exists() {
        return t70::fail("allow_unused", "no src/ directory");
    }

    let mut unjustified = 0u32;
    visit_rs_files(&src_dir, &mut |content, _path| {
        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.contains("#[allow(unused") || trimmed.contains("#![allow(unused") {
                // Check if previous line has a justification comment
                if i > 0 {
                    let prev = content.lines().nth(i - 1).unwrap_or("");
                    if !prev.trim().starts_with("//") {
                        unjustified += 1;
                    }
                } else {
                    unjustified += 1;
                }
            }
        }
    });

    if unjustified == 0 {
        t70::pass("allow_unused", "no unjustified #[allow(unused)]")
    } else {
        t70::fail(
            "allow_unused",
            format!("{} unjustified #[allow(unused)]", unjustified),
        )
    }
}

/// f113: Error handling — no unwrap() in library code (lib.rs tree)
fn f113(dir: &Path) -> t70 {
    let src_dir = dir.join("src");
    if !src_dir.exists() {
        return t70::fail("error_handling", "no src/ directory");
    }

    let mut unwrap_count = 0u32;
    visit_rs_files(&src_dir, &mut |content, path| {
        // Skip test modules and bin/
        let path_str = path.to_string_lossy();
        if path_str.contains("/bin/") || path_str.contains("/tests/") {
            return;
        }
        // Find line number where #[cfg(test)] appears — everything after is test code
        let test_block_start = content
            .lines()
            .enumerate()
            .find(|(_, l)| l.contains("#[cfg(test)]"))
            .map(|(i, _)| i);

        for (line_num, line) in content.lines().enumerate() {
            // Skip everything in #[cfg(test)] block
            if let Some(start) = test_block_start {
                if line_num >= start {
                    break;
                }
            }
            let trimmed = line.trim();
            if trimmed.starts_with("//") {
                continue;
            }
            if trimmed.contains(".unwrap()") {
                unwrap_count += 1;
            }
        }
    });

    if unwrap_count == 0 {
        t70::pass("error_handling", "no unwrap() in library code")
    } else if unwrap_count <= 5 {
        t70::fail(
            "error_handling",
            format!("{} unwrap() in lib code (minor)", unwrap_count),
        )
    } else {
        t70::fail(
            "error_handling",
            format!("{} unwrap() in lib code", unwrap_count),
        )
    }
}

/// f114: No hardcoded secrets or .env committed
fn f114(dir: &Path) -> t70 {
    let bad_files = [
        ".env",
        ".env.local",
        "secrets.json",
        "credentials.json",
        ".env.production",
    ];
    let mut found = Vec::new();
    for name in &bad_files {
        if dir.join(name).exists() {
            found.push(*name);
        }
    }

    // Also check for hardcoded patterns in source
    let src_dir = dir.join("src");
    let mut hardcoded = 0u32;
    if src_dir.exists() {
        visit_rs_files(&src_dir, &mut |content, _path| {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("//") {
                    continue;
                }
                // Skip lines that are part of detection logic (contain .contains())
                if trimmed.contains(".contains(") {
                    continue;
                }
                // Common secret patterns: API keys that look like real credentials
                let secret_prefix = "sk\x2d"; // split to avoid self-match
                let aws_prefix = "AK\x49A";
                if (trimmed.contains(secret_prefix) || trimmed.contains(aws_prefix))
                    && !trimmed.contains("env!")
                    && !trimmed.contains("env::var")
                {
                    hardcoded += 1;
                }
            }
        });
    }

    if found.is_empty() && hardcoded == 0 {
        t70::pass("secrets", "no secrets or .env files")
    } else {
        let mut msg = String::new();
        if !found.is_empty() {
            msg.push_str(&format!("files: {:?}", found));
        }
        if hardcoded > 0 {
            if !msg.is_empty() {
                msg.push_str(", ");
            }
            msg.push_str(&format!("{} hardcoded patterns", hardcoded));
        }
        t70::fail("secrets", msg)
    }
}

/// f115: Cargo.toml has description, license, repository
fn f115(cargo_content: &str) -> t70 {
    let has_desc = cargo_content.lines().any(|l| l.starts_with("description"));
    let has_license = cargo_content.lines().any(|l| l.starts_with("license"));
    let has_repo = cargo_content.lines().any(|l| l.starts_with("repository"));

    let mut missing = Vec::new();
    if !has_desc {
        missing.push("description");
    }
    if !has_license {
        missing.push("license");
    }
    if !has_repo {
        missing.push("repository");
    }

    if missing.is_empty() {
        t70::pass("cargo_meta", "description + license + repository")
    } else {
        t70::fail("cargo_meta", format!("missing: {}", missing.join(", ")))
    }
}

// --- Helpers ---

fn visit_rs_files(dir: &Path, callback: &mut dyn FnMut(&str, &Path)) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            visit_rs_files(&path, callback);
        } else if path.extension().map(|e| e == "rs").unwrap_or(false) {
            if let Ok(content) = std::fs::read_to_string(&path) {
                callback(&content, &path);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp_project(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("exopack_std_{}_{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("src")).unwrap();
        dir
    }

    #[test]
    fn msrv_pass_when_declared() {
        let cargo = r#"
[package]
name = "test"
version = "0.1.0"
rust-version = "1.75"
"#;
        let result = f106(cargo);
        assert!(result.s81, "MSRV should pass: {}", result.s82);
    }

    #[test]
    fn msrv_fail_when_missing() {
        let cargo = r#"
[package]
name = "test"
version = "0.1.0"
"#;
        let result = f106(cargo);
        assert!(!result.s81, "MSRV should fail when missing");
    }

    #[test]
    fn license_file_pass() {
        let dir = tmp_project("license");
        fs::write(dir.join("UNLICENSE"), "public domain").unwrap();
        let result = f110(&dir);
        assert!(result.s81, "should find UNLICENSE: {}", result.s82);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn license_file_fail() {
        let dir = tmp_project("nolicense");
        let result = f110(&dir);
        assert!(!result.s81, "should fail without license file");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn changelog_pass() {
        let dir = tmp_project("changelog");
        fs::write(dir.join("TIMELINE_OF_INVENTION.md"), "# TOI").unwrap();
        let result = f109(&dir);
        assert!(result.s81, "should find TIMELINE: {}", result.s82);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn changelog_fail() {
        let dir = tmp_project("nochangelog");
        let result = f109(&dir);
        assert!(!result.s81, "should fail without changelog");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn secrets_pass_clean_dir() {
        let dir = tmp_project("clean");
        let result = f114(&dir);
        assert!(result.s81, "clean dir should pass: {}", result.s82);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn secrets_fail_env_file() {
        let dir = tmp_project("env");
        fs::write(dir.join(".env"), "SECRET=foo").unwrap();
        let result = f114(&dir);
        assert!(!result.s81, "should fail with .env file");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cargo_meta_pass() {
        let cargo = r#"
description = "test project"
license = "Unlicense"
repository = "https://github.com/test/test"
"#;
        let result = f115(cargo);
        assert!(result.s81, "should pass with all fields: {}", result.s82);
    }

    #[test]
    fn cargo_meta_fail_missing_repo() {
        let cargo = r#"
description = "test project"
license = "Unlicense"
"#;
        let result = f115(cargo);
        assert!(!result.s81, "should fail missing repository");
        assert!(result.s82.contains("repository"));
    }

    #[test]
    fn test_binary_pass() {
        let cargo = r#"
[[bin]]
name = "foo-test"
required-features = ["tests"]
"#;
        let result = f111(Path::new("/tmp"), cargo);
        assert!(result.s81, "should find -test binary: {}", result.s82);
    }

    #[test]
    fn test_binary_fail() {
        let cargo = r#"
[[bin]]
name = "foo"
"#;
        let result = f111(Path::new("/tmp"), cargo);
        assert!(!result.s81, "should fail without -test binary");
    }

    #[test]
    fn module_docs_pass() {
        let dir = tmp_project("docs");
        fs::write(dir.join("src").join("lib.rs"), "//! My crate docs\n").unwrap();
        let result = f108(&dir);
        assert!(result.s81, "should find module docs: {}", result.s82);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn module_docs_fail() {
        let dir = tmp_project("nodocs");
        fs::write(dir.join("src").join("lib.rs"), "pub fn foo() {}\n").unwrap();
        let result = f108(&dir);
        assert!(!result.s81, "should fail without module docs");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn forbid_unsafe_pass() {
        let dir = tmp_project("safe");
        fs::write(
            dir.join("src").join("lib.rs"),
            "#![forbid(unsafe_code)]\npub fn foo() {}\n",
        )
        .unwrap();
        let result = f107(&dir);
        assert!(result.s81, "should pass with forbid: {}", result.s82);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn forbid_unsafe_fail() {
        let dir = tmp_project("noforbid");
        fs::write(dir.join("src").join("lib.rs"), "pub fn foo() {}\n").unwrap();
        let result = f107(&dir);
        assert!(!result.s81, "should fail without forbid");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn full_report_structure() {
        let dir = tmp_project("full");
        fs::write(
            dir.join("Cargo.toml"),
            r#"
[package]
name = "test"
version = "0.1.0"
description = "test"
license = "Unlicense"
repository = "https://example.com"
rust-version = "1.75"
"#,
        )
        .unwrap();
        fs::write(
            dir.join("src").join("lib.rs"),
            "#![forbid(unsafe_code)]\n//! docs\n",
        )
        .unwrap();
        fs::write(dir.join("UNLICENSE"), "public domain").unwrap();
        fs::write(dir.join("CHANGELOG.md"), "# Changes").unwrap();

        let report = f101(&dir);
        assert_eq!(report.total(), 14, "should have 14 checks");
        assert!(report.passed() > 0, "should have some passes");
        let _ = fs::remove_dir_all(&dir);
    }

    // t70 / t71 / t72 type behavior tests

    #[test]
    fn t70_pass_sets_s81_true() {
        let r = t70::pass("clippy", "zero warnings");
        assert!(r.s81);
        assert_eq!(r.s80, "clippy");
        assert_eq!(r.s82, "zero warnings");
    }

    #[test]
    fn t70_fail_sets_s81_false() {
        let r = t70::fail("fmt", "2 files need formatting");
        assert!(!r.s81);
        assert_eq!(r.s80, "fmt");
    }

    #[test]
    fn t71_counts_correct() {
        let checks = vec![
            t70::pass("a", "ok"),
            t70::fail("b", "bad"),
            t70::pass("c", "ok"),
        ];
        let report = t71 {
            s83: "proj".to_string(),
            s84: PathBuf::from("/tmp/proj"),
            s85: checks,
        };
        assert_eq!(report.passed(), 2);
        assert_eq!(report.failed(), 1);
        assert_eq!(report.total(), 3);
    }

    #[test]
    fn t71_all_pass() {
        let checks = vec![t70::pass("a", "ok"), t70::pass("b", "ok")];
        let report = t71 {
            s83: "proj".to_string(),
            s84: PathBuf::from("/tmp/proj"),
            s85: checks,
        };
        assert_eq!(report.failed(), 0);
        assert_eq!(report.passed(), report.total());
    }

    #[test]
    fn t71_all_fail() {
        let checks = vec![t70::fail("a", "bad"), t70::fail("b", "bad")];
        let report = t71 {
            s83: "proj".to_string(),
            s84: PathBuf::from("/tmp/proj"),
            s85: checks,
        };
        assert_eq!(report.passed(), 0);
        assert_eq!(report.failed(), report.total());
    }

    #[test]
    fn t72_empty_portfolio_no_panic() {
        // print_table on empty portfolio must not panic
        let portfolio = t72 { s86: vec![] };
        portfolio.print_table();
    }

    #[test]
    fn t72_single_project_print_table_no_panic() {
        let checks = vec![
            t70::pass("clippy", "ok"),
            t70::fail("fmt", "1 file"),
        ];
        let proj = t71 {
            s83: "testproj".to_string(),
            s84: PathBuf::from("/tmp/testproj"),
            s85: checks,
        };
        let portfolio = t72 { s86: vec![proj] };
        // Exercises the full table render path without panicking.
        portfolio.print_table();
    }

    // Report output format: check names are in the canonical 14-check order.
    #[test]
    fn f101_check_order_matches_canonical() {
        let dir = tmp_project("check_order");
        // Minimal Cargo.toml so f101 can read it.
        fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"ord\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        fs::write(dir.join("src").join("lib.rs"), "").unwrap();

        let report = f101(&dir);
        let names: Vec<&str> = report.s85.iter().map(|c| c.s80).collect();

        assert_eq!(names.len(), 14, "must have exactly 14 checks");
        assert_eq!(names[0], "clippy");
        assert_eq!(names[1], "fmt");
        assert_eq!(names[2], "audit");
        assert_eq!(names[3], "deny");
        assert_eq!(names[4], "msrv");
        assert_eq!(names[5], "unsafe");
        assert_eq!(names[6], "docs");
        assert_eq!(names[7], "changelog");
        assert_eq!(names[8], "license_file");
        assert_eq!(names[9], "test_binary");
        assert_eq!(names[10], "allow_unused");
        assert_eq!(names[11], "error_handling");
        assert_eq!(names[12], "secrets");
        assert_eq!(names[13], "cargo_meta");

        let _ = fs::remove_dir_all(&dir);
    }

    // f116 portfolio aggregation
    #[test]
    fn f116_aggregates_multiple_projects() {
        let dir_a = tmp_project("agg_a");
        let dir_b = tmp_project("agg_b");
        for dir in [&dir_a, &dir_b] {
            fs::write(
                dir.join("Cargo.toml"),
                "[package]\nname = \"agg\"\nversion = \"0.1.0\"\n",
            )
            .unwrap();
            fs::write(dir.join("src").join("lib.rs"), "").unwrap();
        }

        let portfolio = f116(&[dir_a.as_path(), dir_b.as_path()]);
        assert_eq!(portfolio.s86.len(), 2, "must report 2 projects");
        assert!(
            portfolio.s86.iter().all(|p| p.total() == 14),
            "each project must have 14 checks"
        );

        let _ = fs::remove_dir_all(&dir_a);
        let _ = fs::remove_dir_all(&dir_b);
    }
}
