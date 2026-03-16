// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! Code review agent. Sends diffs to LLM for analysis.
//! f185=review_diff, f186=review_staged, f187=review_branch, f188=format_review.
//! t118=ReviewRequest, t119=ReviewResult, t120=ReviewIssue, t121=Severity.

use std::path::Path;
use std::process::{Command, Stdio};

// ── Types ───────────────────────────────────────────────

/// t118=ReviewRequest. Input for code review.
pub struct ReviewRequest {
    pub diff: String,
    pub context: Option<String>,
    pub focus: Option<String>,
}

/// t119=ReviewResult. Structured review output.
pub struct ReviewResult {
    pub summary: String,
    pub issues: Vec<ReviewIssue>,
    /// Quality score 1-10. 10 = no issues, 1 = critical problems.
    pub score: u8,
}

/// t120=ReviewIssue. Single finding from review.
pub struct ReviewIssue {
    pub severity: Severity,
    pub file: String,
    pub line: Option<usize>,
    pub description: String,
}

/// t121=Severity. Issue severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Something done well.
    Praise,
    /// Minor improvement.
    Suggestion,
    /// Potential problem.
    Warning,
    /// Must fix before merge.
    Critical,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Critical => write!(f, "CRITICAL"),
            Severity::Warning => write!(f, "WARNING"),
            Severity::Suggestion => write!(f, "SUGGESTION"),
            Severity::Praise => write!(f, "PRAISE"),
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
pub fn review_diff(diff: &str, ollama_url: &str, model: &str) -> Result<ReviewResult, String> {
    review_diff_with_opts(
        &ReviewRequest {
            diff: diff.to_string(),
            context: None,
            focus: None,
        },
        ollama_url,
        model,
    )
}

/// Review with full request options.
fn review_diff_with_opts(
    req: &ReviewRequest,
    ollama_url: &str,
    model: &str,
) -> Result<ReviewResult, String> {
    let mut prompt = String::with_capacity(req.diff.len() + 256);

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
    prompt.push_str(&req.diff);

    let raw = crate::ollama::generate(ollama_url, model, REVIEW_SYSTEM_PROMPT, &prompt, Some(8192))?;

    parse_review_response(&raw)
}

/// f186=review_staged. Review currently staged changes.
pub fn review_staged(
    project_dir: &Path,
    ollama_url: &str,
    model: &str,
) -> Result<ReviewResult, String> {
    let diff = git_diff(project_dir, &["diff", "--staged"])?;
    if diff.trim().is_empty() {
        return Err("no staged changes to review".into());
    }
    review_diff(&diff, ollama_url, model)
}

/// f187=review_branch. Review diff between current branch and base.
pub fn review_branch(
    project_dir: &Path,
    base: &str,
    ollama_url: &str,
    model: &str,
) -> Result<ReviewResult, String> {
    let range = format!("{}...HEAD", base);
    let diff = git_diff(project_dir, &["diff", &range])?;
    if diff.trim().is_empty() {
        return Err(format!("no diff between {} and HEAD", base));
    }
    review_diff(&diff, ollama_url, model)
}

/// f188=format_review. Human-readable review output.
pub fn format_review(result: &ReviewResult) -> String {
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
    let mut sorted: Vec<&ReviewIssue> = result.issues.iter().collect();
    sorted.sort_by(|a, b| b.severity.cmp(&a.severity));

    for issue in &sorted {
        let marker = match issue.severity {
            Severity::Critical => "[!!]",
            Severity::Warning => "[! ]",
            Severity::Suggestion => "[~ ]",
            Severity::Praise => "[+ ]",
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

/// Parse LLM review response into structured ReviewResult.
fn parse_review_response(raw: &str) -> Result<ReviewResult, String> {
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

        if in_issues && trimmed.starts_with("SEV:") {
            if let Some(issue) = parse_issue_line(trimmed) {
                issues.push(issue);
            }
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

    Ok(ReviewResult {
        summary,
        issues,
        score,
    })
}

/// Parse a single issue line: SEV:<sev> FILE:<file> LINE:<line> DESC:<desc>
fn parse_issue_line(line: &str) -> Option<ReviewIssue> {
    let sev_str = extract_field(line, "SEV:", "FILE:")?;
    let file = extract_field(line, "FILE:", "LINE:")?;
    let line_str = extract_field(line, "LINE:", "DESC:")?;
    let desc = line.split("DESC:").nth(1)?.trim().to_string();

    let severity = match sev_str.trim().to_ascii_lowercase().as_str() {
        "critical" => Severity::Critical,
        "warning" => Severity::Warning,
        "suggestion" => Severity::Suggestion,
        "praise" => Severity::Praise,
        _ => Severity::Suggestion,
    };

    let line_num = line_str.trim().parse::<usize>().ok();

    Some(ReviewIssue {
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
        let result = ReviewResult {
            summary: "Added error handling to parser".to_string(),
            score: 8,
            issues: vec![
                ReviewIssue {
                    severity: Severity::Warning,
                    file: "src/parser.rs".to_string(),
                    line: Some(42),
                    description: "unwrap on user input".to_string(),
                },
                ReviewIssue {
                    severity: Severity::Praise,
                    file: "src/parser.rs".to_string(),
                    line: None,
                    description: "good use of Result propagation".to_string(),
                },
                ReviewIssue {
                    severity: Severity::Critical,
                    file: "src/auth.rs".to_string(),
                    line: Some(10),
                    description: "password logged in plaintext".to_string(),
                },
            ],
        };

        let out = format_review(&result);
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
    /// t121=Severity. Ordering: Praise < Suggestion < Warning < Critical.
    fn severity_ordering() {
        assert!(Severity::Praise < Severity::Suggestion);
        assert!(Severity::Suggestion < Severity::Warning);
        assert!(Severity::Warning < Severity::Critical);
    }

    #[test]
    /// t119=ReviewResult. Empty issues produces clean output.
    fn empty_issues() {
        let result = ReviewResult {
            summary: "Clean diff, no problems".to_string(),
            score: 10,
            issues: vec![],
        };

        let out = format_review(&result);
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
        assert_eq!(result.issues[0].severity, Severity::Warning);
        assert_eq!(result.issues[0].file, "src/handler.rs");
        assert_eq!(result.issues[0].line, Some(33));
        assert_eq!(result.issues[1].severity, Severity::Praise);
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
        assert_eq!(result.issues[0].severity, Severity::Critical);
        assert_eq!(result.issues[1].severity, Severity::Warning);
        assert_eq!(result.issues[2].severity, Severity::Praise);
    }
}
