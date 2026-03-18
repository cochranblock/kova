// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! Code review agent. Sends diffs to LLM for analysis.
//! f185=review_diff, f186=review_staged, f187=review_branch, f188=format_review.
//! T118=ReviewRequest, T119=ReviewResult, T120=ReviewIssue, T121=Severity.

use std::path::Path;
use std::process::{Command, Stdio};

use crate::providers::T129;

// ── Types ───────────────────────────────────────────────

/// T118=T118. Input for code review.
#[allow(non_camel_case_types)]
pub struct T118 {
    pub diff: String,
    pub context: Option<String>,
    pub focus: Option<String>,
}

/// T119=T119. Structured review output.
#[allow(non_camel_case_types)]
pub struct T119 {
    pub summary: String,
    pub issues: Vec<T120>,
    /// Quality score 1-10. 10 = no issues, 1 = critical problems.
    pub score: u8,
}

/// T120=T120. Single finding from review.
#[allow(non_camel_case_types)]
pub struct T120 {
    pub severity: T121,
    pub file: String,
    pub line: Option<usize>,
    pub description: String,
}

/// T121=T121. Issue severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[allow(non_camel_case_types)]
pub enum T121 {
    /// Something done well.
    Praise,
    /// Minor improvement.
    Suggestion,
    /// Potential problem.
    Warning,
    /// Must fix before merge.
    Critical,
}

impl std::fmt::Display for T121 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            T121::Critical => write!(f, "CRITICAL"),
            T121::Warning => write!(f, "WARNING"),
            T121::Suggestion => write!(f, "SUGGESTION"),
            T121::Praise => write!(f, "PRAISE"),
        }
    }
}

// ── System Prompt ───────────────────────────────────────

const REVIEW_SYSTEM_PROMPT: &str = r#"You are a code review agent. Analyze the diff and produce a structured review.

Look for:
- Bugs and logic errors
- Security issues (injection, auth bypass, secret exposure)
- Performance problems (unnecessary allocations, O(n^2) loops)
- Missing error handling (unwrap in production paths, ignored Results)
- Style issues (naming, dead code, unclear logic)
- Things done well (good patterns, clean abstractions)

Respond in EXACTLY this format (no markdown fences, no extra text):

SUMMARY: <one sentence summary of the changes>
SCORE: <1-10>
ISSUES:
SEV:<Critical|Warning|Suggestion|Praise> FILE:<path> LINE:<number or none> DESC:<description>
SEV:<Critical|Warning|Suggestion|Praise> FILE:<path> LINE:<number or none> DESC:<description>
END

Rules:
- SCORE 1-3 = critical problems, 4-6 = needs work, 7-8 = good, 9-10 = excellent
- Every review must have at least one issue (even if just Praise)
- FILE should match the file paths from the diff
- LINE is the line number from the diff, or "none" if not applicable
- Keep descriptions concise — one sentence each
"#;

// ── Core Functions ──────────────────────────────────────

/// f185=review_diff. Send a diff to LLM for code review.
pub fn f185(diff: &str, provider: &T129) -> Result<T119, String> {
    review_diff_with_opts(
        &T118 {
            diff: diff.to_string(),
            context: None,
            focus: None,
        },
        provider,
    )
}

/// Max diff size in chars to send to LLM. Larger diffs get head+tail truncated.
const MAX_DIFF_CHARS: usize = 50_000;

/// Review with full request options.
fn review_diff_with_opts(
    req: &T118,
    provider: &T129,
) -> Result<T119, String> {
    let mut prompt = String::with_capacity(req.diff.len().min(MAX_DIFF_CHARS) + 256);

    if let Some(ctx) = &req.context {
        prompt.push_str("Context: ");
        prompt.push_str(ctx);
        prompt.push('\n');
    }
    if let Some(focus) = &req.focus {
        prompt.push_str("Focus on: ");
        prompt.push_str(focus);
        prompt.push('\n');
    }
    prompt.push_str("Diff to review:\n");

    // Truncate oversized diffs to keep within model context limits.
    if req.diff.chars().count() > MAX_DIFF_CHARS {
        let head: String = req.diff.chars().take(MAX_DIFF_CHARS * 60 / 100).collect();
        let total = req.diff.chars().count();
        let tail: String = req.diff.chars().skip(total - MAX_DIFF_CHARS * 30 / 100).collect();
        prompt.push_str(&head);
        prompt.push_str("\n\n[... diff truncated ...]\n\n");
        prompt.push_str(&tail);
    } else {
        prompt.push_str(&req.diff);
    }

    let resp = crate::providers::f199(provider, "", REVIEW_SYSTEM_PROMPT, &prompt)?;

    parse_review_response(&resp.text)
}

/// f186=review_staged. Review currently staged changes.
pub fn f186(
    project_dir: &Path,
    provider: &T129,
) -> Result<T119, String> {
    let diff = git_diff(project_dir, &["diff", "--staged"])?;
    if diff.trim().is_empty() {
        return Err("no staged changes to review".into());
    }
    f185(&diff, provider)
}

/// f187=review_branch. Review diff between current branch and base.
pub fn f187(
    project_dir: &Path,
    base: &str,
    provider: &T129,
) -> Result<T119, String> {
    let range = format!("{}...HEAD", base);
    let diff = git_diff(project_dir, &["diff", &range])?;
    if diff.trim().is_empty() {
        return Err(format!("no diff between {} and HEAD", base));
    }
    f185(&diff, provider)
}

/// f188=format_review. Human-readable review output.
pub fn f188(result: &T119) -> String {
    let mut out = String::with_capacity(512);

    // Header
    out.push_str(&format!("Score: {}/10\n", result.score));
    out.push_str(&format!("Summary: {}\n", result.summary));

    if result.issues.is_empty() {
        out.push_str("\nNo issues found.\n");
        return out;
    }

    out.push('\n');

    // Group by severity — Critical first, Praise last
    let mut sorted: Vec<&T120> = result.issues.iter().collect();
    sorted.sort_by(|a, b| b.severity.cmp(&a.severity));

    for issue in &sorted {
        let marker = match issue.severity {
            T121::Critical => "[!!]",
            T121::Warning => "[! ]",
            T121::Suggestion => "[~ ]",
            T121::Praise => "[+ ]",
        };

        let location = match issue.line {
            Some(ln) => format!("{}:{}", issue.file, ln),
            None => issue.file.clone(),
        };

        out.push_str(&format!(
            "{} {} — {}\n",
            marker, location, issue.description
        ));
    }

    out
}

// ── Git Helpers ─────────────────────────────────────────

/// Run git with args, return stdout.
fn git_diff(project_dir: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(project_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("git: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git failed: {}", stderr));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

// ── Response Parser ─────────────────────────────────────

/// Parse LLM review response into structured T119.
fn parse_review_response(raw: &str) -> Result<T119, String> {
    let mut summary = String::new();
    let mut score: u8 = 5;
    let mut issues = Vec::new();
    let mut in_issues = false;

    for line in raw.lines() {
        let trimmed = line.trim();

        if let Some(rest) = trimmed.strip_prefix("SUMMARY:") {
            summary = rest.trim().to_string();
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("SCORE:") {
            score = rest.trim().parse::<u8>().unwrap_or(5).clamp(1, 10);
            continue;
        }

        if trimmed == "ISSUES:" {
            in_issues = true;
            continue;
        }

        if trimmed == "END" {
            break;
        }

        if in_issues && trimmed.starts_with("SEV:")
            && let Some(issue) = parse_issue_line(trimmed)
        {
            issues.push(issue);
        }
    }

    if summary.is_empty() {
        // Fallback: use first non-empty line as summary
        summary = raw
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("review complete")
            .trim()
            .to_string();
    }

    Ok(T119 {
        summary,
        issues,
        score,
    })
}

/// Parse a single issue line: SEV:<sev> FILE:<file> LINE:<line> DESC:<desc>
fn parse_issue_line(line: &str) -> Option<T120> {
    let sev_str = extract_field(line, "SEV:", "FILE:")?;
    let file = extract_field(line, "FILE:", "LINE:")?;
    let line_str = extract_field(line, "LINE:", "DESC:")?;
    let desc = line.split("DESC:").nth(1)?.trim().to_string();

    let severity = match sev_str.trim().to_ascii_lowercase().as_str() {
        "critical" => T121::Critical,
        "warning" => T121::Warning,
        "suggestion" => T121::Suggestion,
        "praise" => T121::Praise,
        _ => T121::Suggestion,
    };

    let line_num = line_str.trim().parse::<usize>().ok();

    Some(T120 {
        severity,
        file: file.trim().to_string(),
        line: line_num,
        description: desc,
    })
}

/// Extract text between two field markers.
fn extract_field(line: &str, start: &str, end: &str) -> Option<String> {
    let s = line.find(start)? + start.len();
    let e = line.find(end)?;
    if s >= e {
        return None;
    }
    Some(line[s..e].to_string())
}

// ── Tests ───────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// f188=format_review. Produces readable output with markers.
    fn format_review_readable() {
        let result = T119 {
            summary: "Added error handling to parser".to_string(),
            score: 8,
            issues: vec![
                T120 {
                    severity: T121::Warning,
                    file: "src/parser.rs".to_string(),
                    line: Some(42),
                    description: "unwrap on user input".to_string(),
                },
                T120 {
                    severity: T121::Praise,
                    file: "src/parser.rs".to_string(),
                    line: None,
                    description: "good use of Result propagation".to_string(),
                },
                T120 {
                    severity: T121::Critical,
                    file: "src/auth.rs".to_string(),
                    line: Some(10),
                    description: "password logged in plaintext".to_string(),
                },
            ],
        };

        let out = f188(&result);
        assert!(out.contains("Score: 8/10"));
        assert!(out.contains("Added error handling"));
        // Critical first
        assert!(out.contains("[!!] src/auth.rs:10"));
        // Warning second
        assert!(out.contains("[! ] src/parser.rs:42"));
        // Praise last
        assert!(out.contains("[+ ] src/parser.rs"));
        // Critical appears before Warning in output
        let crit_pos = out.find("[!!]").unwrap();
        let warn_pos = out.find("[! ]").unwrap();
        let praise_pos = out.find("[+ ]").unwrap();
        assert!(crit_pos < warn_pos);
        assert!(warn_pos < praise_pos);
    }

    #[test]
    /// T121=Severity. Ordering: Praise < Suggestion < Warning < Critical.
    fn severity_ordering() {
        assert!(T121::Praise < T121::Suggestion);
        assert!(T121::Suggestion < T121::Warning);
        assert!(T121::Warning < T121::Critical);
    }

    #[test]
    /// T119=ReviewResult. Empty issues produces clean output.
    fn empty_issues() {
        let result = T119 {
            summary: "Clean diff, no problems".to_string(),
            score: 10,
            issues: vec![],
        };

        let out = f188(&result);
        assert!(out.contains("Score: 10/10"));
        assert!(out.contains("No issues found."));
    }

    #[test]
    /// parse_review_response. Parses structured LLM output.
    fn parse_response() {
        let raw = "\
SUMMARY: Fixed null pointer in handler
SCORE: 7
ISSUES:
SEV:Warning FILE:src/handler.rs LINE:33 DESC:Missing bounds check on index
SEV:Praise FILE:src/handler.rs LINE:none DESC:Clean error propagation
END";
        let result = parse_review_response(raw).unwrap();
        assert_eq!(result.summary, "Fixed null pointer in handler");
        assert_eq!(result.score, 7);
        assert_eq!(result.issues.len(), 2);
        assert_eq!(result.issues[0].severity, T121::Warning);
        assert_eq!(result.issues[0].file, "src/handler.rs");
        assert_eq!(result.issues[0].line, Some(33));
        assert_eq!(result.issues[1].severity, T121::Praise);
        assert_eq!(result.issues[1].line, None);
    }

    #[test]
    /// parse_review_response. Score clamped to 1-10.
    fn score_clamped() {
        let raw = "SUMMARY: test\nSCORE: 99\nISSUES:\nEND";
        let result = parse_review_response(raw).unwrap();
        assert_eq!(result.score, 10);

        let raw2 = "SUMMARY: test\nSCORE: 0\nISSUES:\nEND";
        let result2 = parse_review_response(raw2).unwrap();
        assert_eq!(result2.score, 1);
    }

    #[test]
    /// parse_review_response. Fallback summary when SUMMARY line missing.
    fn fallback_summary() {
        let raw = "Some random LLM output\nwithout structure";
        let result = parse_review_response(raw).unwrap();
        assert_eq!(result.summary, "Some random LLM output");
        assert_eq!(result.score, 5); // default
        assert!(result.issues.is_empty());
    }

    #[test]
    /// parse_issue_line. Case-insensitive severity matching.
    fn severity_case_insensitive() {
        let raw = "\
SUMMARY: test
SCORE: 7
ISSUES:
SEV:CRITICAL FILE:src/a.rs LINE:1 DESC:bad thing
SEV:warning FILE:src/b.rs LINE:2 DESC:meh
SEV:PRAISE FILE:src/c.rs LINE:none DESC:nice
END";
        let result = parse_review_response(raw).unwrap();
        assert_eq!(result.issues.len(), 3);
        assert_eq!(result.issues[0].severity, T121::Critical);
        assert_eq!(result.issues[1].severity, T121::Warning);
        assert_eq!(result.issues[2].severity, T121::Praise);
    }

    #[test]
    /// parse_review_response. Malformed LLM output (garbage, partial SEV lines).
    fn parse_malformed_llm_output() {
        let raw = "SEV:Warning FILE:src/x.rs LINE:bad DESC:no number";
        let result = parse_review_response(raw).unwrap();
        assert!(result.issues.is_empty() || result.issues[0].line.is_none());
    }

    #[test]
    /// format_review. Empty diff produces valid output.
    fn format_review_empty_diff() {
        let result = T119 {
            summary: "No changes".to_string(),
            score: 10,
            issues: vec![],
        };
        let out = f188(&result);
        assert!(out.contains("10/10"));
        assert!(out.contains("No changes"));
    }
}
