// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! training_mine — Extract labeled (prompt, tool_name, tool_input) training
//! data from Claude Code transcripts (~/.claude/projects/*/*.jsonl).
//!
//! Strategy: a session is a JSONL log of `user`/`assistant`/`system`/etc.
//! records. We pair each user *string* prompt (skipping tool_result wrappers)
//! with the tool_use blocks in the immediately-following first assistant turn.
//! Each tool_use becomes one T217 example; per-prompt deduplication is left to
//! the caller of the trainer.
//!
//! f412=parse_session_file, f413=mine_dir, f414=write_router_jsonl,
//! f415=map_claude_to_kova, f416=stats_summary.
//! t217=T217 (ToolUseExample), t218=T218 (MineStats).

#![allow(non_camel_case_types)]

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// t217=T217. One labeled training example: a user prompt and the tool call
/// the assistant chose in response. Used to train the tier-1 tool_router.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct T217 {
    /// User's prompt text (string-content user message, not a tool_result wrapper).
    pub prompt: String,
    /// Tool name as written in the Claude transcript (CamelCase: "Read", "Bash").
    pub tool_name_claude: String,
    /// Mapped kova tool name (snake_case: "read_file", "exec"). None if no
    /// kova-side equivalent is available yet.
    pub tool_name_kova: Option<String>,
    /// Tool input arguments, serialized as compact JSON.
    pub tool_input: String,
    /// cwd at time of call (from transcript).
    pub cwd: Option<String>,
    /// Git branch at time of call.
    pub git_branch: Option<String>,
    /// MCP session UUID.
    pub session_id: Option<String>,
    /// Source JSONL filename (not full path, to keep examples portable).
    pub source: String,
}

/// t218=T218. Aggregate stats from a mining run.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct T218 {
    /// Files scanned successfully.
    pub files: usize,
    /// Files skipped due to read/parse errors.
    pub files_failed: usize,
    /// Total user prompts seen (string-content user records).
    pub user_prompts: usize,
    /// Total tool_use blocks emitted as examples.
    pub examples: usize,
    /// Examples with a kova-mapped tool name.
    pub mapped: usize,
    /// Examples without a kova mapping (Claude-only tools).
    pub unmapped: usize,
    /// Per-Claude-tool counts (CamelCase name → count).
    pub per_tool: HashMap<String, usize>,
}

/// f415=map_claude_to_kova. Resolve a Claude Code tool name (CamelCase) to its
/// kova MCP tool name (snake_case). Returns None when kova has no equivalent.
pub fn f415(claude_name: &str) -> Option<&'static str> {
    match claude_name {
        "Read" => Some("read_file"),
        "Write" => Some("write_file"),
        "Edit" | "MultiEdit" => Some("edit_file"),
        "Bash" => Some("exec"),
        "Grep" => Some("grep"),
        "Glob" => Some("glob"),
        "TodoWrite" | "TaskCreate" | "TaskUpdate" | "TaskList" | "TaskGet" | "TaskOutput"
        | "TaskStop" => Some("todo_write"),
        "Agent" => Some("agent"),
        "AskUserQuestion" => Some("ask_user_question"),
        "WebFetch" => Some("web_fetch"),
        "WebSearch" => Some("web_search"),
        "EnterPlanMode" => Some("enter_plan_mode"),
        "ExitPlanMode" => Some("exit_plan_mode"),
        _ => None,
    }
}

/// f412=parse_session_file. Walk one JSONL transcript in order; emit one T217
/// per tool_use block in the first assistant turn after each string-content
/// user prompt. Records with malformed JSON are skipped silently.
pub fn f412(path: &Path) -> Result<Vec<T217>, String> {
    let file = fs::File::open(path).map_err(|e| format!("open {}: {}", path.display(), e))?;
    let source = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let reader = BufReader::new(file);

    let mut out = Vec::new();
    let mut current_prompt: Option<String> = None;
    let mut current_cwd: Option<String> = None;
    let mut current_branch: Option<String> = None;
    let mut current_session: Option<String> = None;
    // True between a fresh user prompt and the first assistant tool_use turn.
    // Resets to false after we capture the first tool-use-bearing assistant turn.
    let mut prompt_open = false;

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        if line.trim().is_empty() {
            continue;
        }
        let record: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let rec_type = record.get("type").and_then(Value::as_str).unwrap_or("");

        match rec_type {
            "user" => {
                let content = record
                    .get("message")
                    .and_then(|m| m.get("content"));
                // Only string-content user messages are real prompts. Array
                // content carries tool_result wrappers — those aren't prompts.
                if let Some(text) = content.and_then(Value::as_str)
                    && !is_meta_only(text)
                {
                    current_prompt = Some(text.to_string());
                    current_cwd = record
                        .get("cwd")
                        .and_then(Value::as_str)
                        .map(|s| s.to_string());
                    current_branch = record
                        .get("gitBranch")
                        .and_then(Value::as_str)
                        .map(|s| s.to_string());
                    current_session = record
                        .get("sessionId")
                        .and_then(Value::as_str)
                        .map(|s| s.to_string());
                    prompt_open = true;
                }
            }
            "assistant" => {
                if !prompt_open {
                    continue;
                }
                let blocks = match record
                    .get("message")
                    .and_then(|m| m.get("content"))
                    .and_then(Value::as_array)
                {
                    Some(arr) => arr,
                    None => continue,
                };
                let mut emitted_any = false;
                for block in blocks {
                    if block.get("type").and_then(Value::as_str) != Some("tool_use") {
                        continue;
                    }
                    let name = match block.get("name").and_then(Value::as_str) {
                        Some(n) => n.to_string(),
                        None => continue,
                    };
                    let input = block.get("input").cloned().unwrap_or(Value::Null);
                    let prompt = match current_prompt.as_ref() {
                        Some(p) => p.clone(),
                        None => continue,
                    };
                    out.push(T217 {
                        prompt,
                        tool_name_claude: name.clone(),
                        tool_name_kova: f415(&name).map(String::from),
                        tool_input: serde_json::to_string(&input).unwrap_or_default(),
                        cwd: current_cwd.clone(),
                        git_branch: current_branch.clone(),
                        session_id: current_session.clone(),
                        source: source.clone(),
                    });
                    emitted_any = true;
                }
                // After the first tool-use-bearing assistant turn, the prompt
                // is "used up" — subsequent assistant turns are reactions to
                // tool results, not direct responses to the original prompt.
                if emitted_any {
                    prompt_open = false;
                }
            }
            _ => {}
        }
    }
    Ok(out)
}

/// Filter user-content text that's actually a system meta-message rather than
/// a real prompt. These wrappers carry no semantic signal for a tool router.
fn is_meta_only(text: &str) -> bool {
    let t = text.trim();
    t.starts_with("<local-command-")
        || t.starts_with("<command-name>")
        || t.starts_with("<command-message>")
        || t.starts_with("<system-reminder>")
        || t.is_empty()
}

/// f413=mine_dir. Recursively walk a directory for `*.jsonl` files, parse each
/// via f412, and return (examples, stats). Errors per file are recorded in
/// stats.files_failed but don't abort the run. `stats.user_prompts` counts
/// unique prompts emitted via f412 — sessions with prompts but no tool_use
/// contribute zero examples (and zero to the count) by design; tool_router
/// training only cares about labeled cases.
pub fn f413(dir: &Path) -> (Vec<T217>, T218) {
    let mut examples = Vec::new();
    let mut stats = T218::default();
    // Use a u64 hash key in the prompt-uniqueness set instead of two Strings —
    // each insert is a stack-only hash, no per-example heap allocation.
    let mut seen_prompt_hashes: std::collections::HashSet<u64> =
        std::collections::HashSet::new();

    let files = walk_jsonl(dir);
    for path in files {
        match f412(&path) {
            Ok(mut found) => {
                stats.files += 1;
                for ex in &found {
                    *stats
                        .per_tool
                        .entry(ex.tool_name_claude.clone())
                        .or_insert(0) += 1;
                    if ex.tool_name_kova.is_some() {
                        stats.mapped += 1;
                    } else {
                        stats.unmapped += 1;
                    }
                    seen_prompt_hashes.insert(hash_prompt_key(&ex.source, &ex.prompt));
                }
                stats.examples += found.len();
                examples.append(&mut found);
            }
            Err(_) => stats.files_failed += 1,
        }
    }
    stats.user_prompts = seen_prompt_hashes.len();
    (examples, stats)
}

/// Hash (source_file, prompt-prefix) for the user_prompts uniqueness set.
/// 64-char prefix is enough to disambiguate distinct prompts in the same
/// session without paying for a full prompt hash on every example.
fn hash_prompt_key(source: &str, prompt: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    source.hash(&mut h);
    // Use bytes (not chars) so the slice math is straight indexing; UTF-8
    // boundary search is irrelevant for hashing.
    let prefix = if prompt.len() <= 64 {
        prompt.as_bytes()
    } else {
        // Walk back to the closest char boundary at or before 64 to avoid a
        // panic on multi-byte chars.
        let mut cut = 64;
        while cut > 0 && !prompt.is_char_boundary(cut) {
            cut -= 1;
        }
        &prompt.as_bytes()[..cut]
    };
    prefix.hash(&mut h);
    h.finish()
}

/// Recursively walk `dir`, returning every file with `.jsonl` extension at any
/// depth. Claude Code's current layout is `~/.claude/projects/<proj>/*.jsonl`
/// (two levels), but deeper structures (e.g., per-session subdirs) work too.
/// Symlinks are followed implicitly via `read_dir`. Errors per directory are
/// silently skipped — the caller sees this as missing files in the result.
fn walk_jsonl(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack: Vec<PathBuf> = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let entries = match fs::read_dir(&current) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let p = entry.path();
            let ftype = match entry.file_type() {
                Ok(t) => t,
                Err(_) => continue,
            };
            if ftype.is_dir() {
                stack.push(p);
            } else if p.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                out.push(p);
            }
        }
    }
    out
}

/// f414=write_router_jsonl. Given mined examples, write training files into
/// out_dir:
///   - `all.jsonl`           — every example, regardless of kova mapping
///   - `mapped.jsonl`        — only examples with a kova equivalent (for the
///                             tier-1 tool_router classifier — these are the
///                             tools kova can currently dispatch)
///   - `by_tool/<name>.jsonl` — per-tool buckets, named by kova snake_case
///                             (mapped examples only)
///   - `stats.json`          — T218 dump of the mining run
///
/// Returns total examples written across all files.
pub fn f414(examples: &[T217], stats: &T218, out_dir: &Path) -> Result<usize, String> {
    fs::create_dir_all(out_dir).map_err(|e| format!("mkdir {}: {}", out_dir.display(), e))?;
    let by_tool_dir = out_dir.join("by_tool");
    fs::create_dir_all(&by_tool_dir)
        .map_err(|e| format!("mkdir {}: {}", by_tool_dir.display(), e))?;

    let mut total = 0usize;

    // Bucket pass: build the mapped-by-tool index once so mapped.jsonl and
    // by_tool/*.jsonl share the same set of references without re-iterating
    // the full examples slice multiple times.
    let mut buckets: HashMap<&str, Vec<&T217>> = HashMap::new();
    for ex in examples {
        if let Some(name) = ex.tool_name_kova.as_deref() {
            buckets.entry(name).or_default().push(ex);
        }
    }

    // all.jsonl — every example. BufWriter + serde_json::to_writer keeps
    // allocation amortized; no per-row to_string churn.
    write_jsonl(&out_dir.join("all.jsonl"), examples.iter())?;
    total += examples.len();

    // mapped.jsonl — only kova-mapped examples.
    let mapped: Vec<&T217> = buckets.values().flat_map(|v| v.iter().copied()).collect();
    write_jsonl(&out_dir.join("mapped.jsonl"), mapped.iter().copied())?;

    // by_tool/*.jsonl — one bucket per kova tool.
    for (name, bucket) in &buckets {
        let path = by_tool_dir.join(format!("{}.jsonl", name));
        write_jsonl(&path, bucket.iter().copied())?;
    }

    // stats.json
    {
        let path = out_dir.join("stats.json");
        fs::write(
            &path,
            serde_json::to_string_pretty(stats).map_err(|e| format!("stats serialize: {}", e))?,
        )
        .map_err(|e| format!("write {}: {}", path.display(), e))?;
    }

    Ok(total)
}

/// Write a sequence of T217 examples as JSONL to `path`. Wraps the file in a
/// BufWriter and uses `serde_json::to_writer` (single allocation per row)
/// instead of the legacy `writeln!(f, "{}", to_string(ex))` pattern (three
/// allocations per row).
fn write_jsonl<'a>(
    path: &Path,
    rows: impl Iterator<Item = &'a T217>,
) -> Result<(), String> {
    let file = fs::File::create(path).map_err(|e| format!("create {}: {}", path.display(), e))?;
    let mut w = BufWriter::new(file);
    for ex in rows {
        serde_json::to_writer(&mut w, ex).map_err(|e| format!("serialize: {}", e))?;
        w.write_all(b"\n").map_err(|e| format!("write nl: {}", e))?;
    }
    w.flush().map_err(|e| format!("flush: {}", e))?;
    Ok(())
}

/// f416=stats_summary. Human-readable stats string for CLI reporting.
pub fn f416(stats: &T218) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    let _ = writeln!(
        s,
        "files: {} ok, {} failed",
        stats.files, stats.files_failed
    );
    let _ = writeln!(
        s,
        "examples: {} ({} mapped, {} unmapped)",
        stats.examples, stats.mapped, stats.unmapped
    );
    let _ = writeln!(s, "user prompts: {} (unique by prefix)", stats.user_prompts);
    let _ = writeln!(s, "per-tool (top 15):");
    let mut pairs: Vec<(&String, &usize)> = stats.per_tool.iter().collect();
    pairs.sort_by(|a, b| b.1.cmp(a.1));
    for (name, count) in pairs.iter().take(15) {
        let mapped_marker = if f415(name).is_some() { "" } else { " (unmapped)" };
        let _ = writeln!(s, "  {:5}  {}{}", count, name, mapped_marker);
    }
    s
}
