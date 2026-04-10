//! Tool definitions and dispatch for agentic mode.
//! t101=ToolDef, t102=ToolParam, t103=ToolCall, t104=ToolResult, t105=ToolRegistry.
//! f140=parse_tool_calls, f141=dispatch_tool, f142-f146,f150,f155=individual tools.
//! f383=checkpoint, f384=undo_edit.
// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

#![allow(non_camel_case_types)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// t101=ToolDef.
pub struct t101 {
    pub name: &'static str,
    pub description: &'static str,
    pub params: &'static [t102],
}

/// t102=ToolParam.
pub struct t102 {
    pub name: &'static str,
    pub param_type: &'static str,
    pub required: bool,
    pub description: &'static str,
}

/// t103=ToolCall. Parsed from LLM output.
#[derive(Debug, Clone)]
pub struct t103 {
    pub tool: String,
    pub args: HashMap<String, String>,
}

/// t104=ToolResult.
#[derive(Debug, Clone)]
pub struct t104 {
    pub tool: String,
    pub success: bool,
    pub output: String,
}

// ── Tool Registry (t105) ─────────────────────────────────

/// All available tools.
pub static TOOLS: &[t101] = &[
    t101 {
        name: "read_file",
        description: "Read file contents. Returns file text with line numbers.",
        params: &[
            t102 {
                name: "path",
                param_type: "string",
                required: true,
                description: "File path (absolute or relative to project).",
            },
            t102 {
                name: "offset",
                param_type: "number",
                required: false,
                description: "Start line (1-indexed).",
            },
            t102 {
                name: "limit",
                param_type: "number",
                required: false,
                description: "Max lines to read.",
            },
        ],
    },
    t101 {
        name: "write_file",
        description: "Write content to a file. Creates dirs if needed.",
        params: &[
            t102 {
                name: "path",
                param_type: "string",
                required: true,
                description: "File path.",
            },
            t102 {
                name: "content",
                param_type: "string",
                required: true,
                description: "File content.",
            },
        ],
    },
    t101 {
        name: "edit_file",
        description: "Replace exact text in a file. old_text must be unique in the file.",
        params: &[
            t102 {
                name: "path",
                param_type: "string",
                required: true,
                description: "File path.",
            },
            t102 {
                name: "old_text",
                param_type: "string",
                required: true,
                description: "Text to find and replace.",
            },
            t102 {
                name: "new_text",
                param_type: "string",
                required: true,
                description: "Replacement text.",
            },
        ],
    },
    t101 {
        name: "exec",
        description: "Execute a shell command. Uses $SHELL (default /bin/sh). Returns stdout+stderr and exit code.",
        params: &[
            t102 {
                name: "command",
                param_type: "string",
                required: true,
                description: "Shell command.",
            },
            t102 {
                name: "cwd",
                param_type: "string",
                required: false,
                description: "Working directory.",
            },
            t102 {
                name: "timeout",
                param_type: "number",
                required: false,
                description: "Timeout in seconds (default 120).",
            },
        ],
    },
    t101 {
        name: "glob",
        description: "Find files matching a glob pattern.",
        params: &[
            t102 {
                name: "pattern",
                param_type: "string",
                required: true,
                description: "Glob pattern (e.g. **/*.rs).",
            },
            t102 {
                name: "path",
                param_type: "string",
                required: false,
                description: "Search root (default: project dir).",
            },
        ],
    },
    t101 {
        name: "grep",
        description: "Search file contents for a pattern. Returns file:line:content matches.",
        params: &[
            t102 {
                name: "pattern",
                param_type: "string",
                required: true,
                description: "Search string (plain text, not regex).",
            },
            t102 {
                name: "path",
                param_type: "string",
                required: false,
                description: "Search root (default: project dir).",
            },
            t102 {
                name: "glob",
                param_type: "string",
                required: false,
                description: "File filter glob (e.g. *.rs).",
            },
        ],
    },
    t101 {
        name: "memory_write",
        description:
            "Save a note to persistent memory (~/.kova/memory.md). Survives across sessions.",
        params: &[t102 {
            name: "content",
            param_type: "string",
            required: true,
            description: "Content to append.",
        }],
    },
    t101 {
        name: "code_review",
        description: "Review a diff or file for bugs, security issues, and style. Returns structured findings with severity.",
        params: &[
            t102 {
                name: "diff",
                param_type: "string",
                required: true,
                description: "Diff text or code to review.",
            },
        ],
    },
    t101 {
        name: "code_outline",
        description: "Extract code structure (functions, structs, enums, traits, impls) from a Rust source file.",
        params: &[
            t102 {
                name: "path",
                param_type: "string",
                required: true,
                description: "Path to .rs file.",
            },
        ],
    },
    t101 {
        name: "record_failure",
        description: "Record a challenge failure for the feedback loop. Feeds into harder challenge generation.",
        params: &[
            t102 {
                name: "challenge",
                param_type: "string",
                required: true,
                description: "Challenge description that was failed.",
            },
            t102 {
                name: "input",
                param_type: "string",
                required: true,
                description: "The input prompt given.",
            },
            t102 {
                name: "expected",
                param_type: "string",
                required: true,
                description: "Expected verification string.",
            },
            t102 {
                name: "actual",
                param_type: "string",
                required: true,
                description: "What the model actually returned.",
            },
            t102 {
                name: "model",
                param_type: "string",
                required: true,
                description: "Model name.",
            },
        ],
    },
    t101 {
        name: "rag_search",
        description: "Search indexed codebase for relevant code chunks. Returns file paths, line ranges, and code snippets.",
        params: &[
            t102 {
                name: "query",
                param_type: "string",
                required: true,
                description: "Natural language query (e.g. 'error handling in storage module').",
            },
            t102 {
                name: "k",
                param_type: "number",
                required: false,
                description: "Number of results (default: 5).",
            },
        ],
    },
    t101 {
        name: "pixel_forge",
        description: "Generate pixel art sprites via Pixel Forge. Auto-detects GPU, picks best model. Returns JSON with base64 PNGs.",
        params: &[
            t102 {
                name: "class",
                param_type: "string",
                required: false,
                description: "Sprite class: character, weapon, potion, terrain, enemy, tree, building, animal, effect, food, armor, tool, vehicle, ui, misc. Default: character.",
            },
            t102 {
                name: "count",
                param_type: "number",
                required: false,
                description: "Number of sprites to generate. Default: 4.",
            },
            t102 {
                name: "palette",
                param_type: "string",
                required: false,
                description: "Color palette: stardew, starbound, snes, nes, gameboy, pico8, endesga. Default: stardew.",
            },
            t102 {
                name: "cmd",
                param_type: "string",
                required: false,
                description: "Plugin command: generate, probe, models, classes, palettes, version. Default: generate.",
            },
        ],
    },
    t101 {
        name: "undo_edit",
        description: "Restore a file to its last checkpoint (taken before any write/edit). Returns the restored content preview.",
        params: &[
            t102 {
                name: "path",
                param_type: "string",
                required: true,
                description: "File path to restore.",
            },
        ],
    },
];

// ── Tool Call Parsing (f140) ─────────────────────────────

/// f140=parse_tool_calls. Extract tool calls from LLM output.
/// Looks for JSON blocks: {"tool": "name", "args": {...}}
pub fn f140(text: &str) -> Vec<t103> {
    let mut calls = Vec::new();

    // Strategy 1: find ```json ... ``` blocks.
    let mut search = text;
    while let Some(start) = search.find("```json") {
        let after = &search[start + 7..];
        if let Some(end) = after.find("```") {
            let block = after[..end].trim();
            if let Some(call) = parse_single_tool_call(block) {
                calls.push(call);
            } else {
                // Try as array.
                for call in parse_tool_call_array(block) {
                    calls.push(call);
                }
            }
            search = &after[end + 3..];
        } else {
            break;
        }
    }

    // Strategy 2: find bare JSON objects with "tool" key.
    if calls.is_empty() {
        let mut i = 0;
        let bytes = text.as_bytes();
        while i < bytes.len() {
            if bytes[i] == b'{'
                && let Some(json_str) = extract_json_object(&text[i..])
            {
                if json_str.contains("\"tool\"")
                    && let Some(call) = parse_single_tool_call(json_str)
                {
                    calls.push(call);
                }
                i += json_str.len();
                continue;
            }
            i += 1;
        }
    }

    calls
}

fn extract_json_object(s: &str) -> Option<&str> {
    if !s.starts_with('{') {
        return None;
    }
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape = false;
    for (i, ch) in s.char_indices() {
        if escape {
            escape = false;
            continue;
        }
        if ch == '\\' && in_string {
            escape = true;
            continue;
        }
        if ch == '"' {
            in_string = !in_string;
            continue;
        }
        if in_string {
            continue;
        }
        if ch == '{' {
            depth += 1;
        }
        if ch == '}' {
            depth -= 1;
            if depth == 0 {
                return Some(&s[..=i]);
            }
        }
    }
    None
}

fn parse_single_tool_call(json_str: &str) -> Option<t103> {
    let v: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let tool = v.get("tool")?.as_str()?.to_string();
    let args_val = v.get("args")?;
    let mut args = HashMap::new();
    if let Some(obj) = args_val.as_object() {
        for (k, v) in obj {
            let val = match v {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            args.insert(k.clone(), val);
        }
    }
    Some(t103 { tool, args })
}

fn parse_tool_call_array(json_str: &str) -> Vec<t103> {
    let v: serde_json::Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let arr = match v.as_array() {
        Some(a) => a,
        None => return Vec::new(),
    };
    arr.iter()
        .filter_map(|item| {
            let tool = item.get("tool")?.as_str()?.to_string();
            let args_val = item.get("args")?;
            let mut args = HashMap::new();
            if let Some(obj) = args_val.as_object() {
                for (k, v) in obj {
                    let val = match v {
                        serde_json::Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    args.insert(k.clone(), val);
                }
            }
            Some(t103 { tool, args })
        })
        .collect()
}

// ── Tool Dispatch (f141) ─────────────────────────────────

/// Check if guarded permission mode is active. KOVA_PERMS=guarded enables gates.
fn is_guarded() -> bool {
    std::env::var("KOVA_PERMS").as_deref() == Ok("guarded")
}

/// Permission gate: prompt user on stderr, read y/n from stdin.
/// Returns true if approved (or if not in guarded mode).
fn perm_gate(action: &str, detail: &str) -> bool {
    if !is_guarded() {
        return true;
    }
    eprint!(
        "\x1b[33m[perm] {} — {}\x1b[0m\n\x1b[33m  approve? (y/n): \x1b[0m",
        action, detail
    );
    let _ = std::io::Write::flush(&mut std::io::stderr());
    let mut input = String::new();
    if std::io::BufRead::read_line(&mut std::io::stdin().lock(), &mut input).is_err() {
        return false;
    }
    matches!(input.trim(), "y" | "Y" | "yes")
}

/// f141=dispatch_tool. Execute a tool call, return result.
/// When KOVA_PERMS=guarded, gates exec and git commit/push with user prompt.
pub fn f141(call: &t103, project_dir: &Path) -> t104 {
    match call.tool.as_str() {
        "read_file" => f142(call, project_dir),
        "write_file" => {
            // Checkpoint before write.
            if let Ok(p) = require_arg(call, "path") {
                f383(&resolve_path(p, project_dir));
            }
            f143(call, project_dir)
        }
        "edit_file" => {
            // Checkpoint before edit.
            if let Ok(p) = require_arg(call, "path") {
                f383(&resolve_path(p, project_dir));
            }
            f144(call, project_dir)
        }
        "exec" => {
            // Gate 1: shell execution requires approval in guarded mode.
            let cmd_preview = get_arg(call, "command").unwrap_or("(no command)");
            if !perm_gate("exec", cmd_preview) {
                return t104 {
                    tool: call.tool.clone(),
                    success: false,
                    output: "denied by user".into(),
                };
            }
            // Gate 2: git commit/push gets a second specific gate.
            if is_git_mutation(cmd_preview) && !perm_gate("git mutation", cmd_preview) {
                return t104 {
                    tool: call.tool.clone(),
                    success: false,
                    output: "git operation denied by user".into(),
                };
            }
            f145(call, project_dir)
        }
        // Backward compat: accept "bash" as alias for "exec".
        "bash" => {
            let cmd_preview = get_arg(call, "command").unwrap_or("(no command)");
            if !perm_gate("exec", cmd_preview) {
                return t104 {
                    tool: call.tool.clone(),
                    success: false,
                    output: "denied by user".into(),
                };
            }
            if is_git_mutation(cmd_preview) && !perm_gate("git mutation", cmd_preview) {
                return t104 {
                    tool: call.tool.clone(),
                    success: false,
                    output: "git operation denied by user".into(),
                };
            }
            f145(call, project_dir)
        }
        "glob" => f146(call, project_dir),
        "grep" => f150(call, project_dir),
        "memory_write" => f155(call),
        "code_review" => f207(call, project_dir),
        #[cfg(not(feature = "inference"))]
        "code_review" => t104 {
            tool: call.tool.clone(),
            success: false,
            output: "code_review requires inference feature".into(),
        },
        "code_outline" => f208(call, project_dir),
        "record_failure" => f209(call),
        #[cfg(not(feature = "inference"))]
        "record_failure" => t104 {
            tool: call.tool.clone(),
            success: false,
            output: "record_failure requires inference feature".into(),
        },
        "rag_search" => f166(call),
        "pixel_forge" => f220(call),
        "undo_edit" => f384(call, project_dir),
        _ => t104 {
            tool: call.tool.clone(),
            success: false,
            output: format!("Unknown tool: {}", call.tool),
        },
    }
}

/// Check if a shell command is a git mutation (commit, push, force operations).
fn is_git_mutation(cmd: &str) -> bool {
    let trimmed = cmd.trim();
    // Match git commit, git push, git reset --hard, git checkout --, etc.
    trimmed.starts_with("git commit")
        || trimmed.starts_with("git push")
        || trimmed.contains("git reset --hard")
        || trimmed.contains("git checkout --")
        || trimmed.contains("git clean -f")
        || trimmed.contains("--force")
        || trimmed.contains("--no-verify")
}

fn resolve_path(raw: &str, project_dir: &Path) -> PathBuf {
    let p = PathBuf::from(raw);
    let resolved = if p.is_absolute() {
        p
    } else {
        project_dir.join(p)
    };
    // Canonicalize to resolve .. and symlinks, then verify it's within project_dir.
    // If canonicalization fails (file doesn't exist yet), normalize manually.
    match resolved.canonicalize() {
        Ok(canon) => {
            if let Ok(proj_canon) = project_dir.canonicalize() {
                if canon.starts_with(&proj_canon) {
                    canon
                } else {
                    // Path escapes project boundary — clamp to project dir.
                    proj_canon.join(canon.file_name().unwrap_or_default())
                }
            } else {
                canon
            }
        }
        Err(_) => {
            // File doesn't exist yet (write_file). Strip .. components manually.
            let mut clean = PathBuf::new();
            for component in resolved.components() {
                match component {
                    std::path::Component::ParentDir => { let _ = clean.pop(); }
                    c => clean.push(c.as_os_str()),
                }
            }
            // Verify cleaned path is within project_dir.
            if let Ok(proj_canon) = project_dir.canonicalize() {
                if clean.starts_with(&proj_canon) {
                    clean
                } else {
                    proj_canon.join(clean.file_name().unwrap_or_default())
                }
            } else {
                clean
            }
        }
    }
}

fn get_arg<'a>(call: &'a t103, key: &str) -> Option<&'a str> {
    call.args.get(key).map(|s| s.as_str())
}

fn require_arg<'a>(call: &'a t103, key: &str) -> Result<&'a str, t104> {
    get_arg(call, key).ok_or_else(|| t104 {
        tool: call.tool.clone(),
        success: false,
        output: format!("Missing required arg: {}", key),
    })
}

// ── f142: read_file ──────────────────────────────────────

fn f142(call: &t103, project_dir: &Path) -> t104 {
    let path = match require_arg(call, "path") {
        Ok(p) => resolve_path(p, project_dir),
        Err(e) => return e,
    };
    let offset: usize = get_arg(call, "offset")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1)
        .max(1)
        - 1;
    let limit: usize = get_arg(call, "limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(2000);

    match std::fs::read_to_string(&path) {
        Ok(content) => {
            let lines: Vec<&str> = content.lines().collect();
            let end = (offset + limit).min(lines.len());
            let numbered: Vec<String> = lines[offset..end]
                .iter()
                .enumerate()
                .map(|(i, l)| format!("{:>5}\t{}", offset + i + 1, l))
                .collect();
            t104 {
                tool: call.tool.clone(),
                success: true,
                output: numbered.join("\n"),
            }
        }
        Err(e) => t104 {
            tool: call.tool.clone(),
            success: false,
            output: format!("read error: {}", e),
        },
    }
}

// ── f143: write_file ─────────────────────────────────────

fn f143(call: &t103, project_dir: &Path) -> t104 {
    let path = match require_arg(call, "path") {
        Ok(p) => resolve_path(p, project_dir),
        Err(e) => return e,
    };
    let content = match require_arg(call, "content") {
        Ok(c) => c,
        Err(e) => return e,
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::write(&path, content) {
        Ok(_) => t104 {
            tool: call.tool.clone(),
            success: true,
            output: format!("wrote {}", path.display()),
        },
        Err(e) => t104 {
            tool: call.tool.clone(),
            success: false,
            output: format!("write error: {}", e),
        },
    }
}

// ── f144: edit_file ──────────────────────────────────────

fn f144(call: &t103, project_dir: &Path) -> t104 {
    let path = match require_arg(call, "path") {
        Ok(p) => resolve_path(p, project_dir),
        Err(e) => return e,
    };
    let old_text = match require_arg(call, "old_text") {
        Ok(t) => t,
        Err(e) => return e,
    };
    let new_text = match require_arg(call, "new_text") {
        Ok(t) => t,
        Err(e) => return e,
    };
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            return t104 {
                tool: call.tool.clone(),
                success: false,
                output: format!("read: {}", e),
            }
        }
    };
    let count = content.matches(old_text).count();
    if count == 0 {
        return t104 {
            tool: call.tool.clone(),
            success: false,
            output: "old_text not found in file".into(),
        };
    }
    if count > 1 {
        return t104 {
            tool: call.tool.clone(),
            success: false,
            output: format!("old_text matches {} times, must be unique", count),
        };
    }
    let new_content = content.replacen(old_text, new_text, 1);
    match std::fs::write(&path, &new_content) {
        Ok(_) => t104 {
            tool: call.tool.clone(),
            success: true,
            output: format!("edited {}", path.display()),
        },
        Err(e) => t104 {
            tool: call.tool.clone(),
            success: false,
            output: format!("write: {}", e),
        },
    }
}

// ── f145: bash ───────────────────────────────────────────

fn f145(call: &t103, project_dir: &Path) -> t104 {
    let cmd = match require_arg(call, "command") {
        Ok(c) => c,
        Err(e) => return e,
    };
    let cwd = get_arg(call, "cwd")
        .map(PathBuf::from)
        .unwrap_or_else(|| project_dir.to_path_buf());
    let timeout_secs: u64 = get_arg(call, "timeout")
        .and_then(|s| s.parse().ok())
        .unwrap_or(120);

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
    let child = Command::new(&shell)
        .args(["-c", cmd])
        .current_dir(&cwd)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();

    match child {
        Ok(mut child) => {
            // Enforce timeout: poll in a loop, kill if exceeded
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => break, // process exited
                    Ok(None) => {
                        if std::time::Instant::now() >= deadline {
                            let _ = child.kill();
                            let _ = child.wait();
                            return t104 {
                                tool: call.tool.clone(),
                                success: false,
                                output: format!("killed: exceeded {}s timeout", timeout_secs),
                            };
                        }
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    }
                    Err(e) => {
                        return t104 {
                            tool: call.tool.clone(),
                            success: false,
                            output: format!("wait: {}", e),
                        };
                    }
                }
            }
            let output = match child.wait_with_output() {
                Ok(o) => o,
                Err(e) => {
                    return t104 {
                        tool: call.tool.clone(),
                        success: false,
                        output: format!("wait: {}", e),
                    }
                }
            };
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let code = output.status.code().unwrap_or(-1);
            let mut out = String::new();
            if !stdout.is_empty() {
                out.push_str(&stdout);
            }
            if !stderr.is_empty() {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(&stderr);
            }
            // Truncate for context savings.
            if out.len() > 10000 {
                let lines: Vec<&str> = out.lines().collect();
                let total = lines.len();
                if total > 100 {
                    let head: Vec<&str> = lines[..50].to_vec();
                    let tail: Vec<&str> = lines[total - 50..].to_vec();
                    out = format!(
                        "{}\n... ({} lines omitted) ...\n{}",
                        head.join("\n"),
                        total - 100,
                        tail.join("\n")
                    );
                }
            }
            t104 {
                tool: call.tool.clone(),
                success: output.status.success(),
                output: format!("[exit {}]\n{}", code, out.trim()),
            }
        }
        Err(e) => t104 {
            tool: call.tool.clone(),
            success: false,
            output: format!("spawn: {}", e),
        },
    }
}

// ── f146: glob ───────────────────────────────────────────

fn f146(call: &t103, project_dir: &Path) -> t104 {
    let pattern = match require_arg(call, "pattern") {
        Ok(p) => p,
        Err(e) => return e,
    };
    let root = get_arg(call, "path")
        .map(|s| resolve_path(s, project_dir))
        .unwrap_or_else(|| project_dir.to_path_buf());

    let full_pattern = root.join(pattern);
    let full_str = full_pattern.to_string_lossy().to_string();

    match glob::glob(&full_str) {
        Ok(entries) => {
            let mut matches: Vec<String> = Vec::new();
            for path in entries.take(200).flatten() {
                let display = path
                    .strip_prefix(&root)
                    .unwrap_or(&path)
                    .display()
                    .to_string();
                matches.push(display);
            }
            if matches.is_empty() {
                t104 {
                    tool: call.tool.clone(),
                    success: true,
                    output: "no matches".into(),
                }
            } else {
                t104 {
                    tool: call.tool.clone(),
                    success: true,
                    output: matches.join("\n"),
                }
            }
        }
        Err(e) => t104 {
            tool: call.tool.clone(),
            success: false,
            output: format!("glob: {}", e),
        },
    }
}

// ── f150: grep ───────────────────────────────────────────

fn f150(call: &t103, project_dir: &Path) -> t104 {
    let pattern = match require_arg(call, "pattern") {
        Ok(p) => p,
        Err(e) => return e,
    };
    let root = get_arg(call, "path")
        .map(|s| resolve_path(s, project_dir))
        .unwrap_or_else(|| project_dir.to_path_buf());
    let file_glob = get_arg(call, "glob");

    let result_args: Vec<String> = vec![
        "-rn".into(),
        "--include".into(),
        file_glob.unwrap_or("*").to_string(),
    ];

    match Command::new("grep")
        .args(&result_args)
        .arg(pattern)
        .arg(root.to_str().unwrap_or("."))
        .output()
    {
        Ok(out) => {
            let text = String::from_utf8_lossy(&out.stdout);
            let lines: Vec<&str> = text.lines().take(100).collect();
            let stripped: Vec<String> = lines
                .iter()
                .map(|l| {
                    // Strip project_dir prefix.
                    let root_str = root.to_string_lossy();
                    l.strip_prefix(root_str.as_ref())
                        .unwrap_or(l)
                        .strip_prefix('/')
                        .unwrap_or(l)
                        .to_string()
                })
                .collect();
            if stripped.is_empty() {
                t104 {
                    tool: call.tool.clone(),
                    success: true,
                    output: "no matches".into(),
                }
            } else {
                let total = text.lines().count();
                let suffix = if total > 100 {
                    format!("\n... +{} more", total - 100)
                } else {
                    String::new()
                };
                t104 {
                    tool: call.tool.clone(),
                    success: true,
                    output: format!("{}{}", stripped.join("\n"), suffix),
                }
            }
        }
        Err(e) => t104 {
            tool: call.tool.clone(),
            success: false,
            output: format!("grep: {}", e),
        },
    }
}

// ── f155: memory_write ───────────────────────────────────

fn f155(call: &t103) -> t104 {
    let content = match require_arg(call, "content") {
        Ok(c) => c,
        Err(e) => return e,
    };
    let memory_path = crate::config::kova_dir().join("memory.md");
    let existing = std::fs::read_to_string(&memory_path).unwrap_or_default();
    let new_content = if existing.is_empty() {
        format!("# Kova Memory\n\n{}\n", content)
    } else {
        format!("{}\n{}\n", existing.trim_end(), content)
    };
    match std::fs::write(&memory_path, new_content) {
        Ok(_) => t104 {
            tool: call.tool.clone(),
            success: true,
            output: "saved to memory".into(),
        },
        Err(e) => t104 {
            tool: call.tool.clone(),
            success: false,
            output: format!("write: {}", e),
        },
    }
}

// ── f166: rag_search ────────────────────────────────────

fn f166(call: &t103) -> t104 {
    let query = match require_arg(call, "query") {
        Ok(q) => q,
        Err(e) => return e,
    };
    let k: usize = call
        .args
        .get("k")
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);

    let store_path = crate::rag::T200::default_path();
    let store = match crate::rag::T200::open(&store_path) {
        Ok(s) => s,
        Err(e) => {
            return t104 {
                tool: call.tool.clone(),
                success: false,
                output: format!("rag store: {}", e),
            }
        }
    };

    match crate::rag::search(&store, query, k) {
        Ok(results) => {
            if results.is_empty() {
                return t104 {
                    tool: call.tool.clone(),
                    success: true,
                    output: "No results. Run `kova rag index` first.".into(),
                };
            }
            let context = crate::rag::f346(&results, 4000);
            t104 {
                tool: call.tool.clone(),
                success: true,
                output: context,
            }
        }
        Err(e) => t104 {
            tool: call.tool.clone(),
            success: false,
            output: format!("rag search: {}", e),
        },
    }
}

// ── f207: code_review ─────────────────────────────────────

/// f207=code_review tool. Send diff to LLM for review.
fn f207(call: &t103, _project_dir: &Path) -> t104 {
    let diff = match require_arg(call, "diff") {
        Ok(d) => d,
        Err(e) => return e,
    };

    let provider = crate::providers::f333();

    match crate::review::f185(diff, &provider) {
        Ok(result) => t104 {
            tool: call.tool.clone(),
            success: true,
            output: crate::review::f188(&result),
        },
        Err(e) => t104 {
            tool: call.tool.clone(),
            success: false,
            output: format!("review failed: {}", e),
        },
    }
}

// ── f208: code_outline ────────────────────────────────────

/// f208=code_outline tool. Extract symbols from a Rust file.
fn f208(call: &t103, project_dir: &Path) -> t104 {
    let raw_path = match require_arg(call, "path") {
        Ok(p) => p,
        Err(e) => return e,
    };
    let path = resolve_path(raw_path, project_dir);

    match crate::syntax::f206(&path) {
        Ok(symbols) => {
            let outline = crate::syntax::f205(&symbols);
            t104 {
                tool: call.tool.clone(),
                success: true,
                output: if outline.is_empty() {
                    "no symbols found".into()
                } else {
                    outline
                },
            }
        }
        Err(e) => t104 {
            tool: call.tool.clone(),
            success: false,
            output: format!("outline: {}", e),
        },
    }
}

// ── f209: record_failure ──────────────────────────────────

/// f209=record_failure tool. Store a challenge failure for curriculum feedback.
fn f209(call: &t103) -> t104 {
    let challenge = match require_arg(call, "challenge") {
        Ok(c) => c.to_string(),
        Err(e) => return e,
    };
    let input = match require_arg(call, "input") {
        Ok(i) => i.to_string(),
        Err(e) => return e,
    };
    let expected = match require_arg(call, "expected") {
        Ok(e) => e.to_string(),
        Err(e) => return e,
    };
    let actual = match require_arg(call, "actual") {
        Ok(a) => a.to_string(),
        Err(e) => return e,
    };
    let model = match require_arg(call, "model") {
        Ok(m) => m.to_string(),
        Err(e) => return e,
    };

    crate::feedback::f194(crate::feedback::T126 {
        challenge_desc: challenge,
        input,
        expected_verify: expected,
        actual_response: actual,
        model,
        event_type: "agent".into(),
        ts: 0,
    });

    t104 {
        tool: call.tool.clone(),
        success: true,
        output: "failure recorded".into(),
    }
}

// ── Format Tools for System Prompt (f149) ────────────────

/// f149=format_tool_prompt. Format tool definitions for LLM system prompt.
pub fn f149() -> String {
    let mut out = String::from("## Available Tools\n\nTo use a tool, output a JSON block:\n```json\n{\"tool\": \"tool_name\", \"args\": {\"param\": \"value\"}}\n```\nYou may call multiple tools by outputting multiple JSON blocks.\nWhen your task is complete, respond normally without tool calls.\n\n");
    for tool in TOOLS {
        out.push_str(&format!("### {}\n{}\n", tool.name, tool.description));
        out.push_str("Parameters:\n");
        for p in tool.params {
            let req = if p.required { "required" } else { "optional" };
            out.push_str(&format!(
                "- `{}` ({}): {} [{}]\n",
                p.name, p.param_type, p.description, req
            ));
        }
        out.push('\n');
    }
    out
}

// ── f220: pixel_forge ──────────────────────────────────

/// f220=pixel_forge tool. Discovers pixel-forge binary, sends plugin request.
fn f220(call: &t103) -> t104 {
    let cmd = call.args.get("cmd").map(|s| s.as_str()).unwrap_or("generate");

    // Discover binary
    let bin = find_pixel_forge();
    let bin = match bin {
        Some(b) => b,
        None => return t104 {
            tool: call.tool.clone(),
            success: false,
            output: "pixel-forge binary not found. Build with: cargo build -p pixel-forge --release".into(),
        },
    };

    // Build request
    let args = match cmd {
        "generate" => {
            let class = call.args.get("class").map(|s| s.as_str()).unwrap_or("character");
            let count = call.args.get("count")
                .and_then(|s| s.parse::<u32>().ok()).unwrap_or(4);
            let palette = call.args.get("palette").map(|s| s.as_str()).unwrap_or("stardew");
            serde_json::json!({
                "class": class,
                "count": count,
                "steps": 40,
                "palette": palette,
            })
        }
        _ => serde_json::Value::Null,
    };

    let request = serde_json::json!({
        "cmd": cmd,
        "args": args,
    });

    // Spawn plugin process
    let mut child = match Command::new(&bin)
        .args(["plugin"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return t104 {
            tool: call.tool.clone(),
            success: false,
            output: format!("spawn failed: {e}"),
        },
    };

    // Send request
    if let Some(ref mut stdin) = child.stdin {
        use std::io::Write;
        let _ = writeln!(stdin, "{}", serde_json::to_string(&request).unwrap());
    }
    drop(child.stdin.take());

    // Read response
    let output = match child.wait_with_output() {
        Ok(o) => o,
        Err(e) => return t104 {
            tool: call.tool.clone(),
            success: false,
            output: format!("wait failed: {e}"),
        },
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let resp: serde_json::Value = match serde_json::from_str(stdout.trim()) {
        Ok(v) => v,
        Err(e) => return t104 {
            tool: call.tool.clone(),
            success: false,
            output: format!("parse error: {e}\nraw: {}", stdout.trim()),
        },
    };

    let ok = resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
    if ok {
        t104 {
            tool: call.tool.clone(),
            success: true,
            output: serde_json::to_string_pretty(&resp.get("data").unwrap_or(&serde_json::Value::Null))
                .unwrap_or_default(),
        }
    } else {
        let err = resp.get("error").and_then(|v| v.as_str()).unwrap_or("unknown");
        t104 {
            tool: call.tool.clone(),
            success: false,
            output: err.to_string(),
        }
    }
}

/// Discover pixel-forge binary on disk.
fn find_pixel_forge() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let candidates = [
        home.join("target/release/pixel-forge"),
        home.join("target/debug/pixel-forge"),
        home.join("bin/pixel-forge"),
        home.join(".cargo/bin/pixel-forge"),
        home.join("pixel-forge/target/release/pixel-forge"),
        home.join("pixel-forge/target/debug/pixel-forge"),
    ];
    for c in &candidates {
        if c.exists() {
            return Some(c.clone());
        }
    }
    // Try which
    if let Ok(output) = Command::new("which").arg("pixel-forge").output()
        && output.status.success()
    {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path.is_empty() {
            return Some(PathBuf::from(path));
        }
    }
    None
}

// ── f383: checkpoint, f384: undo_edit ───────────────────

/// Sled tree name for file checkpoints.
const CHECKPOINT_TREE: &str = "checkpoints";

/// Checkpoint DB path. Tests override via CHECKPOINT_DB thread-local.
fn checkpoint_db_path() -> std::path::PathBuf {
    CHECKPOINT_DB.with(|cell| {
        cell.borrow().clone().unwrap_or_else(crate::config::sled_path)
    })
}

std::thread_local! {
    static CHECKPOINT_DB: std::cell::RefCell<Option<std::path::PathBuf>> = const { std::cell::RefCell::new(None) };
}

/// f383=checkpoint. Snapshot file contents into sled before write/edit.
/// Key format: checkpoint:{filepath}:{unix_timestamp_ms}
/// Also stores latest checkpoint pointer: checkpoint_latest:{filepath}
fn f383(path: &Path) {
    // Only checkpoint if file exists (new files have nothing to restore).
    let content = match std::fs::read(path) {
        Ok(c) => c,
        Err(_) => return,
    };

    let store_path = checkpoint_db_path();
    let db = match sled::open(&store_path) {
        Ok(db) => db,
        Err(_) => return,
    };
    let tree = match db.open_tree(CHECKPOINT_TREE) {
        Ok(t) => t,
        Err(_) => return,
    };

    let path_str = path.to_string_lossy();
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);

    let key = format!("checkpoint:{}:{}", path_str, ts);
    let latest_key = format!("checkpoint_latest:{}", path_str);

    let _ = tree.insert(key.as_bytes(), content);
    let _ = tree.insert(latest_key.as_bytes(), key.as_bytes());

    eprintln!(
        "\x1b[90m[checkpoint: {}]\x1b[0m",
        path.file_name().unwrap_or_default().to_string_lossy()
    );
}

/// f384=undo_edit. Restore file from last checkpoint.
fn f384(call: &t103, project_dir: &Path) -> t104 {
    let path = match require_arg(call, "path") {
        Ok(p) => resolve_path(p, project_dir),
        Err(e) => return e,
    };

    let store_path = checkpoint_db_path();
    let db = match sled::open(&store_path) {
        Ok(db) => db,
        Err(e) => {
            return t104 {
                tool: call.tool.clone(),
                success: false,
                output: format!("sled open: {}", e),
            }
        }
    };
    let tree = match db.open_tree(CHECKPOINT_TREE) {
        Ok(t) => t,
        Err(e) => {
            return t104 {
                tool: call.tool.clone(),
                success: false,
                output: format!("tree open: {}", e),
            }
        }
    };

    let path_str = path.to_string_lossy();
    let latest_key = format!("checkpoint_latest:{}", path_str);

    // Look up the latest checkpoint key.
    let checkpoint_key = match tree.get(latest_key.as_bytes()) {
        Ok(Some(key_bytes)) => key_bytes,
        _ => {
            return t104 {
                tool: call.tool.clone(),
                success: false,
                output: format!("no checkpoint for {}", path.display()),
            }
        }
    };

    // Retrieve the checkpoint content.
    let content = match tree.get(&checkpoint_key) {
        Ok(Some(data)) => data,
        _ => {
            return t104 {
                tool: call.tool.clone(),
                success: false,
                output: "checkpoint data missing".into(),
            }
        }
    };

    // Restore the file.
    match std::fs::write(&path, content.as_ref()) {
        Ok(_) => {
            let preview_len = content.len().min(200);
            let preview = String::from_utf8_lossy(&content[..preview_len]);
            t104 {
                tool: call.tool.clone(),
                success: true,
                output: format!(
                    "restored {} ({} bytes)\n{}{}",
                    path.display(),
                    content.len(),
                    preview,
                    if content.len() > 200 { "..." } else { "" }
                ),
            }
        }
        Err(e) => t104 {
            tool: call.tool.clone(),
            success: false,
            output: format!("restore write: {}", e),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f140_parse_json_block() {
        let text = r#"Let me read that file.
```json
{"tool": "read_file", "args": {"path": "src/lib.rs"}}
```
"#;
        let calls = f140(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool, "read_file");
        assert_eq!(calls[0].args.get("path").unwrap(), "src/lib.rs");
    }

    #[test]
    fn f140_parse_bare_json() {
        let text =
            r#"I'll check the file. {"tool": "read_file", "args": {"path": "Cargo.toml"}} Done."#;
        let calls = f140(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool, "read_file");
    }

    #[test]
    fn f140_no_tool_calls() {
        let text = "Here is some regular text with no tool calls.";
        let calls = f140(text);
        assert!(calls.is_empty());
    }

    #[test]
    fn f142_read_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("test.txt"), "line1\nline2\nline3").unwrap();
        let call = t103 {
            tool: "read_file".into(),
            args: [("path".into(), "test.txt".into())].into(),
        };
        let result = f142(&call, tmp.path());
        assert!(result.success);
        assert!(result.output.contains("line1"));
        assert!(result.output.contains("line3"));
    }

    #[test]
    fn f144_edit_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("test.rs"), "fn old() {}").unwrap();
        let call = t103 {
            tool: "edit_file".into(),
            args: [
                ("path".into(), "test.rs".into()),
                ("old_text".into(), "fn old()".into()),
                ("new_text".into(), "fn new_fn()".into()),
            ]
            .into(),
        };
        let result = f144(&call, tmp.path());
        assert!(result.success);
        let content = std::fs::read_to_string(tmp.path().join("test.rs")).unwrap();
        assert!(content.contains("fn new_fn()"));
    }

    #[test]
    fn resolve_path_blocks_traversal() {
        let tmp = tempfile::TempDir::new().unwrap();
        let project = tmp.path().canonicalize().unwrap();
        // Relative path with .. should not escape project dir.
        let resolved = resolve_path("../../etc/passwd", &project);
        // The resolved path should not contain /etc/passwd as a real path.
        assert!(
            !resolved.starts_with("/etc"),
            "path traversal escaped project: {:?}",
            resolved
        );
    }

    #[test]
    fn resolve_path_normal_relative() {
        let tmp = tempfile::TempDir::new().unwrap();
        let project = tmp.path().canonicalize().unwrap();
        std::fs::write(project.join("foo.txt"), "x").unwrap();
        let resolved = resolve_path("foo.txt", &project);
        assert!(resolved.starts_with(&project));
    }

    // ── TEST-1: Dispatch tests ──────────────────────────

    #[test]
    fn f141_dispatch_unknown_tool() {
        let tmp = tempfile::TempDir::new().unwrap();
        let call = t103 {
            tool: "nonexistent_tool".into(),
            args: HashMap::new(),
        };
        let result = f141(&call, tmp.path());
        assert!(!result.success);
        assert!(result.output.contains("Unknown tool"));
    }

    #[test]
    fn f141_dispatch_read_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("hello.rs"), "fn main() {}").unwrap();
        let call = t103 {
            tool: "read_file".into(),
            args: [("path".into(), "hello.rs".into())].into(),
        };
        let result = f141(&call, tmp.path());
        assert!(result.success);
        assert!(result.output.contains("fn main()"));
        assert_eq!(result.tool, "read_file");
    }

    #[test]
    fn f141_dispatch_write_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let call = t103 {
            tool: "write_file".into(),
            args: [
                ("path".into(), "new.txt".into()),
                ("content".into(), "hello world".into()),
            ]
            .into(),
        };
        let result = f141(&call, tmp.path());
        assert!(result.success);
        let content = std::fs::read_to_string(tmp.path().join("new.txt")).unwrap();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn f141_dispatch_edit_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("e.rs"), "fn old() {}").unwrap();
        let call = t103 {
            tool: "edit_file".into(),
            args: [
                ("path".into(), "e.rs".into()),
                ("old_text".into(), "fn old()".into()),
                ("new_text".into(), "fn fresh()".into()),
            ]
            .into(),
        };
        let result = f141(&call, tmp.path());
        assert!(result.success);
        let content = std::fs::read_to_string(tmp.path().join("e.rs")).unwrap();
        assert!(content.contains("fn fresh()"));
    }

    #[test]
    fn f141_dispatch_glob() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("a.rs"), "").unwrap();
        std::fs::write(tmp.path().join("b.rs"), "").unwrap();
        std::fs::write(tmp.path().join("c.txt"), "").unwrap();
        let call = t103 {
            tool: "glob".into(),
            args: [("pattern".into(), "*.rs".into())].into(),
        };
        let result = f141(&call, tmp.path());
        assert!(result.success);
        assert!(result.output.contains("a.rs"));
        assert!(result.output.contains("b.rs"));
        assert!(!result.output.contains("c.txt"));
    }

    #[test]
    fn f141_dispatch_bash() {
        let tmp = tempfile::TempDir::new().unwrap();
        let call = t103 {
            tool: "bash".into(),
            args: [("command".into(), "echo kova-test-output".into())].into(),
        };
        let result = f141(&call, tmp.path());
        assert!(result.success);
        assert!(result.output.contains("kova-test-output"));
    }

    #[test]
    fn f141_dispatch_grep() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("search.txt"), "needle in haystack\nno match").unwrap();
        let call = t103 {
            tool: "grep".into(),
            args: [
                ("pattern".into(), "needle".into()),
                ("path".into(), "search.txt".into()),
            ]
            .into(),
        };
        let result = f141(&call, tmp.path());
        assert!(result.success);
        assert!(result.output.contains("needle"));
    }

    #[test]
    fn f141_dispatch_code_outline() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("outline.rs"),
            "pub fn hello() {\n    println!(\"hi\");\n}\n\nstruct Foo {\n    x: i32,\n}\n",
        )
        .unwrap();
        let call = t103 {
            tool: "code_outline".into(),
            args: [("path".into(), "outline.rs".into())].into(),
        };
        let result = f141(&call, tmp.path());
        assert!(result.success);
        assert!(result.output.contains("hello"));
        assert!(result.output.contains("Foo"));
    }

    #[test]
    fn f141_dispatch_record_failure() {
        let call = t103 {
            tool: "record_failure".into(),
            args: [
                ("challenge".into(), "test challenge".into()),
                ("input".into(), "test input".into()),
                ("expected".into(), "compiles".into()),
                ("actual".into(), "error output".into()),
                ("model".into(), "test-model".into()),
            ]
            .into(),
        };
        let result = f141(&call, Path::new("/tmp"));
        assert!(result.success);
        assert!(result.output.contains("recorded"));
    }

    #[test]
    fn f141_dispatch_record_failure_missing_arg() {
        let call = t103 {
            tool: "record_failure".into(),
            args: [("challenge".into(), "only one arg".into())].into(),
        };
        let result = f141(&call, Path::new("/tmp"));
        assert!(!result.success);
        assert!(result.output.contains("Missing"));
    }

    // ── TEST-1: Parse edge cases ────────────────────────

    #[test]
    fn f140_multiple_tool_calls() {
        let text = r#"
```json
{"tool": "read_file", "args": {"path": "a.rs"}}
```
Then another:
```json
{"tool": "write_file", "args": {"path": "b.rs", "content": "hello"}}
```
"#;
        let calls = f140(text);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].tool, "read_file");
        assert_eq!(calls[1].tool, "write_file");
    }

    #[test]
    fn f140_malformed_json_skipped() {
        let text = r#"
```json
{"tool": "read_file", "args": {"path": "a.rs"}
```
This JSON is missing a closing brace — should be skipped.
```json
{"tool": "bash", "args": {"command": "echo hi"}}
```
"#;
        let calls = f140(text);
        // At least the valid one should parse
        assert!(calls.iter().any(|c| c.tool == "bash"));
    }

    #[test]
    fn f140_json_without_tool_key_ignored() {
        let text = r#"{"name": "not a tool", "value": 42}"#;
        let calls = f140(text);
        assert!(calls.is_empty());
    }

    #[test]
    fn f140_empty_string() {
        assert!(f140("").is_empty());
    }

    #[test]
    fn f140_tool_call_with_extra_fields() {
        let text = r#"{"tool": "bash", "args": {"command": "ls"}, "extra": true}"#;
        let calls = f140(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool, "bash");
    }

    // ── TEST-1: Tool registry ───────────────────────────

    #[test]
    fn tool_registry_all_tools_have_names() {
        for tool in TOOLS {
            assert!(!tool.name.is_empty(), "tool has empty name");
            assert!(!tool.description.is_empty(), "tool {} has empty description", tool.name);
        }
    }

    #[test]
    fn tool_registry_no_duplicate_names() {
        let mut names = std::collections::HashSet::new();
        for tool in TOOLS {
            assert!(names.insert(tool.name), "duplicate tool name: {}", tool.name);
        }
    }

    #[test]
    fn f149_format_includes_all_tools() {
        let prompt = f149();
        for tool in TOOLS {
            assert!(
                prompt.contains(tool.name),
                "f149 output missing tool: {}",
                tool.name
            );
        }
    }

    #[test]
    fn f143_write_file_creates_subdirs() {
        let tmp = tempfile::TempDir::new().unwrap();
        let project = tmp.path().canonicalize().unwrap();
        let call = t103 {
            tool: "write_file".into(),
            args: [
                ("path".into(), "sub/dir/file.txt".into()),
                ("content".into(), "nested".into()),
            ]
            .into(),
        };
        let result = f143(&call, &project);
        assert!(result.success, "write should succeed: {}", result.output);
        let content = std::fs::read_to_string(project.join("sub/dir/file.txt")).unwrap();
        assert_eq!(content, "nested");
    }

    #[test]
    fn f142_read_nonexistent() {
        let tmp = tempfile::TempDir::new().unwrap();
        let call = t103 {
            tool: "read_file".into(),
            args: [("path".into(), "nope.txt".into())].into(),
        };
        let result = f142(&call, tmp.path());
        assert!(!result.success);
    }

    #[test]
    fn f144_edit_no_match() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("f.rs"), "fn hello() {}").unwrap();
        let call = t103 {
            tool: "edit_file".into(),
            args: [
                ("path".into(), "f.rs".into()),
                ("old_text".into(), "fn missing()".into()),
                ("new_text".into(), "fn replaced()".into()),
            ]
            .into(),
        };
        let result = f144(&call, tmp.path());
        assert!(!result.success);
    }

    #[test]
    fn f142_read_with_offset_and_limit() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("lines.txt"), "L1\nL2\nL3\nL4\nL5").unwrap();
        let call = t103 {
            tool: "read_file".into(),
            args: [
                ("path".into(), "lines.txt".into()),
                ("offset".into(), "2".into()),
                ("limit".into(), "2".into()),
            ]
            .into(),
        };
        let result = f142(&call, tmp.path());
        assert!(result.success);
        assert!(result.output.contains("L2"));
        assert!(result.output.contains("L3"));
        // Should NOT contain L1 or L5
        assert!(!result.output.contains("L1\n"));
    }

    #[test]
    fn f208_code_outline_empty_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("empty.rs"), "").unwrap();
        let call = t103 {
            tool: "code_outline".into(),
            args: [("path".into(), "empty.rs".into())].into(),
        };
        let result = f208(&call, tmp.path());
        assert!(result.success);
        assert!(result.output.contains("no symbols"));
    }

    #[test]
    fn f208_code_outline_nonexistent_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let call = t103 {
            tool: "code_outline".into(),
            args: [("path".into(), "nope.rs".into())].into(),
        };
        let result = f208(&call, tmp.path());
        assert!(!result.success);
    }

    // ── Checkpoint/Undo tests ──────────────────────────────

    /// Helper: set up an isolated sled DB for checkpoint tests.
    fn with_test_sled<F: FnOnce(&Path)>(f: F) {
        let sled_tmp = tempfile::TempDir::new().unwrap();
        let sled_path = sled_tmp.path().join("test_sled.db");
        CHECKPOINT_DB.with(|cell| {
            *cell.borrow_mut() = Some(sled_path.clone());
        });
        f(&sled_path);
        CHECKPOINT_DB.with(|cell| {
            *cell.borrow_mut() = None;
        });
    }

    #[test]
    fn f383_checkpoint_stores_content() {
        with_test_sled(|sled_path| {
            let tmp = tempfile::TempDir::new().unwrap();
            let file = tmp.path().join("test.txt");
            std::fs::write(&file, "original content").unwrap();

            f383(&file);

            let db = sled::open(sled_path).unwrap();
            let tree = db.open_tree(CHECKPOINT_TREE).unwrap();

            let latest_key = format!("checkpoint_latest:{}", file.to_string_lossy());
            let checkpoint_key = tree.get(latest_key.as_bytes()).unwrap();
            assert!(checkpoint_key.is_some(), "latest checkpoint pointer should exist");

            let data = tree.get(&checkpoint_key.unwrap()).unwrap();
            assert!(data.is_some(), "checkpoint data should exist");
            assert_eq!(data.unwrap().as_ref(), b"original content");
        });
    }

    #[test]
    fn f384_undo_restores_file() {
        with_test_sled(|_| {
            let tmp = tempfile::TempDir::new().unwrap();
            let project = tmp.path().canonicalize().unwrap();
            let file = project.join("undo_test.txt");
            std::fs::write(&file, "before edit").unwrap();

            f383(&file);
            std::fs::write(&file, "after edit").unwrap();
            assert_eq!(std::fs::read_to_string(&file).unwrap(), "after edit");

            let call = t103 {
                tool: "undo_edit".into(),
                args: [("path".into(), "undo_test.txt".into())].into(),
            };
            let result = f384(&call, &project);
            assert!(result.success, "undo should succeed: {}", result.output);
            assert_eq!(std::fs::read_to_string(&file).unwrap(), "before edit");
        });
    }

    #[test]
    fn f384_undo_no_checkpoint() {
        with_test_sled(|_| {
            let tmp = tempfile::TempDir::new().unwrap();
            let call = t103 {
                tool: "undo_edit".into(),
                args: [("path".into(), "never_checkpointed.txt".into())].into(),
            };
            let result = f384(&call, tmp.path());
            assert!(!result.success);
            assert!(result.output.contains("no checkpoint"));
        });
    }

    // ── Exec tool tests ────────────────────────────────────

    #[test]
    fn f141_dispatch_exec() {
        let tmp = tempfile::TempDir::new().unwrap();
        let call = t103 {
            tool: "exec".into(),
            args: [("command".into(), "echo exec-test".into())].into(),
        };
        let result = f141(&call, tmp.path());
        assert!(result.success);
        assert!(result.output.contains("exec-test"));
    }

    #[test]
    fn exec_tool_in_registry() {
        assert!(TOOLS.iter().any(|t| t.name == "exec"), "exec tool missing from registry");
    }

    #[test]
    fn undo_edit_tool_in_registry() {
        assert!(TOOLS.iter().any(|t| t.name == "undo_edit"), "undo_edit tool missing from registry");
    }

    // ── Permission gate tests ──────────────────────────────

    #[test]
    fn is_git_mutation_detects_commit() {
        assert!(is_git_mutation("git commit -m 'test'"));
        assert!(is_git_mutation("git push origin main"));
        assert!(is_git_mutation("git reset --hard HEAD~1"));
        assert!(is_git_mutation("git checkout -- ."));
        assert!(is_git_mutation("git push --force"));
    }

    #[test]
    fn is_git_mutation_allows_safe_ops() {
        assert!(!is_git_mutation("git status"));
        assert!(!is_git_mutation("git diff"));
        assert!(!is_git_mutation("git log --oneline"));
        assert!(!is_git_mutation("git branch -a"));
        assert!(!is_git_mutation("git stash list"));
    }

    #[test]
    fn is_guarded_reads_env() {
        // SAFETY: test runs single-threaded, no concurrent env access.
        unsafe {
            // Default (no env) = not guarded.
            std::env::remove_var("KOVA_PERMS");
            assert!(!is_guarded());

            // Explicit open = not guarded.
            std::env::set_var("KOVA_PERMS", "open");
            assert!(!is_guarded());

            // Guarded = guarded.
            std::env::set_var("KOVA_PERMS", "guarded");
            assert!(is_guarded());

            // Cleanup.
            std::env::remove_var("KOVA_PERMS");
        }
    }

    #[test]
    fn f141_write_creates_checkpoint() {
        with_test_sled(|sled_path| {
            let tmp = tempfile::TempDir::new().unwrap();
            let project = tmp.path().canonicalize().unwrap();
            let file = project.join("cp_test.txt");
            std::fs::write(&file, "checkpoint me").unwrap();

            let call = t103 {
                tool: "write_file".into(),
                args: [
                    ("path".into(), "cp_test.txt".into()),
                    ("content".into(), "new content".into()),
                ]
                .into(),
            };
            let result = f141(&call, &project);
            assert!(result.success);

            assert_eq!(std::fs::read_to_string(&file).unwrap(), "new content");

            let db = sled::open(sled_path).unwrap();
            let tree = db.open_tree(CHECKPOINT_TREE).unwrap();
            let latest_key = format!("checkpoint_latest:{}", file.to_string_lossy());
            let ck = tree.get(latest_key.as_bytes()).unwrap().unwrap();
            let data = tree.get(&ck).unwrap().unwrap();
            assert_eq!(data.as_ref(), b"checkpoint me");
        });
    }
}