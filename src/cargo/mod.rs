// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! Unified cargo executor. One copy of check/clippy/test. Used by pipeline, factory, moe, academy.

pub mod sandbox;

use std::path::Path;
use std::process::Command;

/// Run cargo with args in a directory. Returns (success, stderr).
pub fn run_cargo(project_dir: &Path, args: &[&str]) -> (bool, String) {
    match Command::new("cargo")
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

/// cargo check. Returns (success, stderr).
pub fn cargo_check(project_dir: &Path) -> (bool, String) {
    run_cargo(project_dir, &["check"])
}

/// cargo clippy -D warnings. Returns (success, stderr).
pub fn cargo_clippy(project_dir: &Path) -> (bool, String) {
    run_cargo(project_dir, &["clippy", "--", "-D", "warnings"])
}

/// cargo test. Returns (success, combined stderr+stdout).
pub fn cargo_test(project_dir: &Path) -> (bool, String) {
    match Command::new("cargo")
        .args(["test"])
        .current_dir(project_dir)
        .output()
    {
        Ok(o) => {
            let mut out = String::from_utf8_lossy(&o.stderr).into_owned();
            let stdout = String::from_utf8_lossy(&o.stdout);
            if !stdout.is_empty() {
                out.push('\n');
                out.push_str(&stdout);
            }
            (o.status.success(), out)
        }
        Err(e) => (false, e.to_string()),
    }
}

/// Extract core error identifier for loop detection (error code + line).
pub fn extract_error_key(stderr: &str) -> String {
    for line in stderr.lines() {
        if line.contains("error[E") {
            return line.trim().to_string();
        }
    }
    for line in stderr.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("error") {
            return trimmed.to_string();
        }
    }
    "unknown".into()
}

/// Truncate string to max chars.
pub fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max])
    }
}

/// Extract first ```rust ... ``` block (or bare ``` block) from text.
pub fn extract_rust_block(s: &str) -> Option<String> {
    let (start_tag, tag_len) = if let Some(pos) = s.find("```rust") {
        (pos, 7)
    } else if let Some(pos) = s.find("```\n") {
        (pos, 4)
    } else {
        return None;
    };
    let after_start = &s[start_tag + tag_len..];
    let end = after_start.find("```")?;
    Some(after_start[..end].trim().to_string())
}

/// Detect if a prompt is asking for a binary vs library code.
pub fn prompt_wants_binary(prompt: &str) -> bool {
    let lower = prompt.to_lowercase();
    lower.contains("cli ")
        || lower.contains("command line")
        || lower.contains("command-line")
        || lower.contains("executable")
        || lower.contains("binary")
        || lower.contains("tool that")
        || lower.contains("program that")
        || lower.contains("app that")
        || lower.contains("main()")
        || lower.contains("fn main")
        || lower.contains("takes a ")
        || lower.contains("prints ")
        || lower.contains("reads from")
        || lower.contains("accept")
}

/// Build system prompt for code generation.
pub fn build_system_prompt(wants_binary: bool) -> String {
    let code_type = if wants_binary {
        "Write a complete program with `fn main()`. The code will be compiled as src/main.rs."
    } else {
        "Write library code. The code will be compiled as src/lib.rs."
    };

    format!(
        "You are a Rust systems programming expert.\n\
        {}\n\
        Write clean, idiomatic Rust. No filler. No slop words.\n\
        IMPORTANT: Use only the Rust standard library. No external crates.\n\
        The code will be compiled in an isolated crate with zero dependencies.\n\
        IMPORTANT: All string types must match — don't mix &str with String in if/else or match arms.\n\
        Use `.to_string()` or `String::from()` to convert &str to String where needed.\n\
        Put all code in a single ```rust block. No text before or after the block.",
        code_type
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_rust_block_basic() {
        let s = "Here is code:\n```rust\nfn main() {}\n```\nDone.";
        assert_eq!(extract_rust_block(s).as_deref(), Some("fn main() {}"));
    }

    #[test]
    fn extract_rust_block_trimmed() {
        let s = "```rust\n  let x = 1;  \n```";
        assert_eq!(extract_rust_block(s).as_deref(), Some("let x = 1;"));
    }

    #[test]
    fn extract_rust_block_none_when_no_block() {
        assert!(extract_rust_block("no code here").is_none());
        assert!(extract_rust_block("```python\nx=1\n```").is_none());
    }

    #[test]
    fn extract_rust_block_bare_backticks() {
        let s = "```\nfn foo() {}\n```";
        assert_eq!(extract_rust_block(s).as_deref(), Some("fn foo() {}"));
    }

    #[test]
    fn truncate_short() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn truncate_long() {
        let result = truncate("hello world", 5);
        assert!(result.starts_with("hello"));
        assert!(result.ends_with("..."));
    }

    #[test]
    fn prompt_wants_binary_detects_cli() {
        assert!(prompt_wants_binary("Write a CLI tool"));
        assert!(prompt_wants_binary("Write fn main()"));
        assert!(!prompt_wants_binary("Write a function"));
    }

    #[test]
    fn build_system_prompt_binary() {
        let p = build_system_prompt(true);
        assert!(p.contains("fn main()"));
    }

    #[test]
    fn build_system_prompt_lib() {
        let p = build_system_prompt(false);
        assert!(p.contains("library code"));
    }

    #[test]
    fn cargo_check_clippy_test_on_valid_lib() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"gen\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(
            tmp.path().join("src/lib.rs"),
            "pub fn add(a: i32, b: i32) -> i32 { a + b }\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n    #[test]\n    fn it_works() { assert_eq!(add(1, 2), 3); }\n}\n",
        )
        .unwrap();
        let (ok, _) = cargo_check(tmp.path());
        assert!(ok, "cargo check must pass");
        let (ok, _) = cargo_clippy(tmp.path());
        assert!(ok, "cargo clippy must pass");
        let (ok, _) = cargo_test(tmp.path());
        assert!(ok, "cargo test must pass");
    }

    #[test]
    fn error_key_extraction() {
        let stderr = "error[E0382]: use of moved value: `x`\n  --> src/lib.rs:5:5";
        let key = extract_error_key(stderr);
        assert!(key.contains("E0382"));
    }
}
