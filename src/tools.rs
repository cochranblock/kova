// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! Tool definitions and dispatch for agentic mode.
//! t101=ToolDef, t102=ToolParam, t103=ToolCall, t104=ToolResult, t105=ToolRegistry.
//! f140=parse_tool_calls, f141=dispatch_tool, f142-f146,f150,f155=individual tools.

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
        name: "bash",
        description: "Execute a shell command. Returns stdout+stderr and exit code.",
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
            if bytes[i] == b'{' {
                if let Some(json_str) = extract_json_object(&text[i..]) {
                    if json_str.contains("\"tool\"") {
                        if let Some(call) = parse_single_tool_call(json_str) {
                            calls.push(call);
                        }
                    }
                    i += json_str.len();
                    continue;
                }
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

/// f141=dispatch_tool. Execute a tool call, return result.
pub fn f141(call: &t103, project_dir: &Path) -> t104 {
    match call.tool.as_str() {
        "read_file" => f142(call, project_dir),
        "write_file" => f143(call, project_dir),
        "edit_file" => f144(call, project_dir),
        "bash" => f145(call, project_dir),
        "glob" => f146(call, project_dir),
        "grep" => f150(call, project_dir),
        "memory_write" => f155(call),
        _ => t104 {
            tool: call.tool.clone(),
            success: false,
            output: format!("Unknown tool: {}", call.tool),
        },
    }
}

fn resolve_path(raw: &str, project_dir: &Path) -> PathBuf {
    let p = PathBuf::from(raw);
    if p.is_absolute() {
        p
    } else {
        project_dir.join(p)
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
    let _timeout_secs: u64 = get_arg(call, "timeout")
        .and_then(|s| s.parse().ok())
        .unwrap_or(120);

    let child = Command::new("sh")
        .args(["-c", cmd])
        .current_dir(&cwd)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();

    match child {
        Ok(child) => {
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
}
