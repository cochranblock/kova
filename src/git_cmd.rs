// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

// Copyright (c) 2026 The Cochran Block. All rights reserved.
//! Tokenized git commands. §13 compressed output for AI context.
//! g0=status g1=diff g2=log g3=push g4=pull g5=commit g6=branch g7=stash g8=add g9=reset.
//! f156=git_exec, f157=compress_status, f158=compress_diff, f159=compress_log, f160=git_cmd_dispatch.

use clap::ValueEnum;
use regex::Regex;
use std::path::PathBuf;
use std::process::{Command, Stdio};

// ── Command Enum (t107) ───────────────────────────────────

/// t107=GitCmd. Tokenized git command variants.
#[derive(Clone, Copy, ValueEnum, Debug)]
#[allow(non_camel_case_types)]
pub enum t107 {
    /// g0: git status (compressed).
    #[value(name = "g0")]
    G0,
    /// g1: git diff (compressed, no headers).
    #[value(name = "g1")]
    G1,
    /// g2: git log --oneline (last N).
    #[value(name = "g2")]
    G2,
    /// g3: git push.
    #[value(name = "g3")]
    G3,
    /// g4: git pull.
    #[value(name = "g4")]
    G4,
    /// g5: git commit -m.
    #[value(name = "g5")]
    G5,
    /// g6: git branch (list + current).
    #[value(name = "g6")]
    G6,
    /// g7: git stash.
    #[value(name = "g7")]
    G7,
    /// g8: git add.
    #[value(name = "g8")]
    G8,
    /// g9: git diff --staged (compressed).
    #[value(name = "g9")]
    G9,
}

// ── Result Type (t108) ───────────────────────────────────

/// t108=GitResult. Compressed git output.
#[allow(non_camel_case_types)]
pub struct t108 {
    /// g_cmd: command token.
    pub cmd: &'static str,
    /// ok/err.
    pub ok: bool,
    /// compressed output.
    pub out: String,
}

fn print_result(r: &t108) {
    let status = if r.ok { "ok" } else { "err" };
    eprintln!("{}\t{}", r.cmd, status);
    if !r.out.is_empty() {
        eprintln!("{}", r.out);
    }
}

// ── Compression Functions ──────────────────────────────────

/// f157=compress_status. M/A/D/? + short path, one per line.
fn f157(raw: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    for line in raw.lines() {
        // Git porcelain: XY<space>filename. Don't trim leading space (it's the index status).
        if line.len() < 4 {
            continue;
        }
        let xy = &line[..2];
        let file = line[3..].trim();
        let short_file = compress_path(file);
        let tag = match xy.trim() {
            "M" | " M" | "MM" => "M",
            "A" | " A" => "A",
            "D" | " D" => "D",
            "R" => "R",
            "??" => "?",
            "UU" => "C", // conflict
            _ => xy.trim(),
        };
        lines.push(format!("{}\t{}", tag, short_file));
    }
    if lines.is_empty() {
        "clean".to_string()
    } else {
        lines.join("\n")
    }
}

/// f158=compress_diff. Strip headers, keep only +/- lines with file:line context.
fn f158(raw: &str) -> String {
    let re_file = Regex::new(r"^\+\+\+ b/(.+)$").unwrap();
    let re_hunk = Regex::new(r"^@@ -\d+(?:,\d+)? \+(\d+)").unwrap();
    let re_skip = Regex::new(r"^(---|diff --git|index |new file|deleted file)").unwrap();
    let re_add = Regex::new(r"^\+(?!\+\+)(.*)$").unwrap();
    let re_del = Regex::new(r"^-(?!--)(.*)$").unwrap();

    let mut lines: Vec<String> = Vec::new();
    let mut current_file = String::new();
    let mut hunk_line: u32 = 0;
    let mut added: u32 = 0;
    let mut removed: u32 = 0;

    for line in raw.lines() {
        if let Some(caps) = re_file.captures(line) {
            current_file = compress_path(&caps[1]);
            continue;
        }
        if re_skip.is_match(line) {
            continue;
        }
        if let Some(caps) = re_hunk.captures(line) {
            hunk_line = caps[1].parse().unwrap_or(0);
            continue;
        }
        if let Some(caps) = re_add.captures(line) {
            let trimmed = caps[1].trim();
            if !trimmed.is_empty() {
                lines.push(format!("{}:{} +{}", current_file, hunk_line, trimmed));
                added += 1;
            }
            hunk_line += 1;
        } else if let Some(caps) = re_del.captures(line) {
            let trimmed = caps[1].trim();
            if !trimmed.is_empty() {
                lines.push(format!("{}:{} -{}", current_file, hunk_line, trimmed));
                removed += 1;
            }
        } else {
            hunk_line += 1;
        }
    }

    if lines.len() > 30 {
        let total = lines.len();
        lines.truncate(30);
        lines.push(format!("...+{}", total - 30));
    }

    if lines.is_empty() {
        "no changes".to_string()
    } else {
        let mut out = format!("+{}/-{}\n", added, removed);
        out.push_str(&lines.join("\n"));
        out
    }
}

/// f159=compress_log. Already oneline, just strip hashes to 7 chars.
fn f159(raw: &str) -> String {
    let re_hash = Regex::new(r"^([a-f0-9]{7})[a-f0-9]*\s+(.*)$").unwrap();
    raw.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            if let Some(caps) = re_hash.captures(l) {
                format!("{} {}", &caps[1], caps[2].trim())
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Strip paths: /Users/foo/kova/src/lib.rs → src/lib.rs
fn compress_path(p: &str) -> String {
    let re = Regex::new(r".*?(/src/.+)$").unwrap();
    let re_kova = Regex::new(r".*?kova/(.+)$").unwrap();
    if let Some(caps) = re.captures(p) {
        caps[1][1..].to_string() // strip leading /
    } else if let Some(caps) = re_kova.captures(p) {
        caps[1].to_string()
    } else {
        p.to_string()
    }
}

// ── Core Execution ──────────────────────────────────────

/// f156=git_exec. Execute a git command, return compressed result.
fn f156(args: &[&str], work_dir: &PathBuf) -> (bool, String, String) {
    match Command::new("git")
        .args(args)
        .current_dir(work_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
    {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            (output.status.success(), stdout, stderr)
        }
        Err(e) => (false, String::new(), e.to_string()),
    }
}

fn work_dir() -> PathBuf {
    // For git: find the nearest .git parent, not workspace root.
    let mut dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    loop {
        if dir.join(".git").exists() {
            return dir;
        }
        if !dir.pop() {
            break;
        }
    }
    // Fallback: try workspace root's children that have .git.
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let root = crate::config::workspace_root(&cwd);
    if root.join(".git").exists() {
        return root;
    }
    cwd
}

// ── Dispatcher (f160) ────────────────────────────────────

/// f160=git_cmd_dispatch. Central dispatcher for tokenized git.
pub fn f160(
    cmd: t107,
    count: u32,
    message: Option<String>,
    files: Vec<String>,
    _staged: bool,
) -> anyhow::Result<()> {
    let wd = work_dir();

    let result = match cmd {
        t107::G0 => {
            // git status --porcelain
            let (ok, stdout, stderr) = f156(&["status", "--porcelain"], &wd);
            let out = if ok { f157(&stdout) } else { stderr };
            t108 { cmd: "g0", ok, out }
        }
        t107::G1 => {
            // git diff (unstaged)
            let (ok, stdout, stderr) = f156(&["diff"], &wd);
            let out = if ok { f158(&stdout) } else { stderr };
            t108 { cmd: "g1", ok, out }
        }
        t107::G2 => {
            // git log --oneline -N
            let n = format!("-{}", count);
            let (ok, stdout, stderr) = f156(&["log", "--oneline", &n], &wd);
            let out = if ok { f159(&stdout) } else { stderr };
            t108 { cmd: "g2", ok, out }
        }
        t107::G3 => {
            // git push
            let (ok, stdout, stderr) = f156(&["push"], &wd);
            let out = if ok {
                if stdout.trim().is_empty() && stderr.trim().is_empty() {
                    "pushed".to_string()
                } else {
                    // Push output goes to stderr
                    let combined = format!("{}{}", stdout.trim(), stderr.trim());
                    // Compress: just keep the ref line
                    combined
                        .lines()
                        .find(|l| l.contains("->"))
                        .unwrap_or("pushed")
                        .trim()
                        .to_string()
                }
            } else {
                stderr
            };
            t108 { cmd: "g3", ok, out }
        }
        t107::G4 => {
            // git pull
            let (ok, stdout, stderr) = f156(&["pull"], &wd);
            let out = if ok {
                if stdout.contains("Already up to date") {
                    "current".to_string()
                } else {
                    // Count files changed
                    let changed = stdout.lines().filter(|l| l.contains('|')).count();
                    format!("pulled +{} files", changed)
                }
            } else {
                stderr
            };
            t108 { cmd: "g4", ok, out }
        }
        t107::G5 => {
            // git commit -m "msg"
            let msg = message.unwrap_or_else(|| "update".to_string());
            let (ok, stdout, stderr) = f156(&["commit", "-m", &msg], &wd);
            let out = if ok {
                // Extract short hash + summary
                stdout
                    .lines()
                    .next()
                    .unwrap_or("committed")
                    .trim()
                    .to_string()
            } else {
                stderr
                    .lines()
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or("commit failed")
                    .trim()
                    .to_string()
            };
            t108 { cmd: "g5", ok, out }
        }
        t107::G6 => {
            // git branch
            let (ok, stdout, stderr) = f156(
                &[
                    "branch",
                    "--format=%(if)%(HEAD)%(then)* %(end)%(refname:short)",
                ],
                &wd,
            );
            let out = if ok {
                stdout
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .map(|l| l.trim().to_string())
                    .collect::<Vec<_>>()
                    .join(" ")
            } else {
                stderr
            };
            t108 { cmd: "g6", ok, out }
        }
        t107::G7 => {
            // git stash
            let (ok, stdout, stderr) = f156(&["stash"], &wd);
            let out = if ok {
                stdout
                    .lines()
                    .next()
                    .unwrap_or("stashed")
                    .trim()
                    .to_string()
            } else {
                stderr
            };
            t108 { cmd: "g7", ok, out }
        }
        t107::G8 => {
            // git add <files> or git add -A
            let (ok, _stdout, stderr) = if files.is_empty() {
                f156(&["add", "-A"], &wd)
            } else {
                let mut args = vec!["add"];
                let file_refs: Vec<&str> = files.iter().map(|s| s.as_str()).collect();
                args.extend(file_refs);
                f156(&args, &wd)
            };
            let out = if ok { "staged".to_string() } else { stderr };
            t108 { cmd: "g8", ok, out }
        }
        t107::G9 => {
            // git diff --staged (or --cached)
            let args = vec!["diff", "--staged"];
            let (ok, stdout, stderr) = f156(&args, &wd);
            let out = if ok { f158(&stdout) } else { stderr };
            t108 { cmd: "g9", ok, out }
        }
    };

    print_result(&result);
    if !result.ok {
        anyhow::bail!("git {} failed", result.cmd);
    }
    Ok(())
}

// ── Tests ──────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f157_status_clean() {
        assert_eq!(f157(""), "clean");
    }

    #[test]
    fn f157_status_modified() {
        let raw = " M src/main.rs\n?? new_file.txt\n";
        let out = f157(raw);
        assert!(out.contains("M\tsrc/main.rs"));
        assert!(out.contains("?\tnew_file.txt"));
    }

    #[test]
    fn f158_diff_compress() {
        let raw = "diff --git a/src/lib.rs b/src/lib.rs\n\
                    index abc1234..def5678 100644\n\
                    --- a/src/lib.rs\n\
                    +++ b/src/lib.rs\n\
                    @@ -10,3 +10,4 @@\n\
                     existing line\n\
                    +new line here\n\
                    -old line gone\n";
        let out = f158(raw);
        assert!(out.contains("+1/-1"));
        assert!(out.contains("+new line here"));
        assert!(out.contains("-old line gone"));
    }

    #[test]
    fn f159_log_compress() {
        let raw = "abc1234def5 Fix the thing\n9876543abcd Add feature\n";
        let out = f159(raw);
        assert!(out.contains("abc1234 Fix the thing"));
        assert!(out.contains("9876543 Add feature"));
    }

    #[test]
    fn compress_path_strips_prefix() {
        assert_eq!(compress_path("/Users/foo/kova/src/lib.rs"), "src/lib.rs");
        assert_eq!(compress_path("README.md"), "README.md");
    }
}