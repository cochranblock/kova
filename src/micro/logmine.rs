//! logmine — Extract training data from Claude Code conversation logs.
//!
//! Reads JSONL files from ~/.claude/projects/ and extracts:
//!   - (user_instruction, code_written) pairs from Edit/Write tool uses
//!   - (user_instruction, assistant_explanation) pairs from text responses
//!   - System prompt examples from conversation context
//!
//! Output: SFT-format JSONL for fine-tuning code models.
// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use std::collections::HashMap;
use std::path::PathBuf;

/// T146=MinedExample
/// A mined training example from conversation logs.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct T146 {
    /// The user's instruction/request.
    pub instruction: String,
    /// The assistant's response (code or explanation).
    pub response: String,
    /// What type of response: "code_edit", "code_write", "explanation", "bash"
    pub response_type: String,
    /// Source file path (for code edits/writes).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    /// Session ID.
    pub session_id: String,
}

/// T147=MineStats
/// Stats about mined data.
#[derive(Debug)]
pub struct T147 {
    pub sessions: usize,
    pub total_messages: usize,
    pub user_messages: usize,
    pub code_edits: usize,
    pub code_writes: usize,
    pub explanations: usize,
    pub total_examples: usize,
}

/// f237=mine_logs
/// Mine all Claude Code JSONL logs in ~/.claude/projects/.
pub fn f237() -> Result<(Vec<T146>, T147), String> {
    let log_dir = log_directory();
    if !log_dir.exists() {
        return Err(format!("log directory not found: {}", log_dir.display()));
    }

    let mut all_examples = Vec::new();
    let mut stats = T147 {
        sessions: 0, total_messages: 0, user_messages: 0,
        code_edits: 0, code_writes: 0, explanations: 0, total_examples: 0,
    };

    // Find all JSONL files
    let entries = std::fs::read_dir(&log_dir).map_err(|e| e.to_string())?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "jsonl") {
            match mine_single_log(&path) {
                Ok((examples, msg_count, user_count)) => {
                    stats.sessions += 1;
                    stats.total_messages += msg_count;
                    stats.user_messages += user_count;
                    for ex in &examples {
                        match ex.response_type.as_str() {
                            "code_edit" => stats.code_edits += 1,
                            "code_write" => stats.code_writes += 1,
                            "explanation" => stats.explanations += 1,
                            _ => {}
                        }
                    }
                    all_examples.extend(examples);
                }
                Err(e) => {
                    eprintln!("  SKIP {}: {}", path.display(), e);
                }
            }
        }
    }

    stats.total_examples = all_examples.len();
    Ok((all_examples, stats))
}

/// Mine a single JSONL conversation log.
fn mine_single_log(path: &PathBuf) -> Result<(Vec<T146>, usize, usize), String> {
    let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let session_id = path.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut examples = Vec::new();
    let mut msg_count = 0;
    let mut user_count = 0;

    // Parse messages into ordered sequence
    let mut messages: Vec<(String, serde_json::Value)> = Vec::new(); // (type, value)
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        if let Ok(obj) = serde_json::from_str::<serde_json::Value>(line) {
            let msg_type = obj.get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            messages.push((msg_type, obj));
            msg_count += 1;
        }
    }

    // Walk through messages: pair user instructions with assistant responses
    let mut last_user_instruction: Option<String> = None;

    for (msg_type, obj) in &messages {
        match msg_type.as_str() {
            "user" => {
                user_count += 1;
                // Extract user message text
                if let Some(content) = obj.get("message").and_then(|m| m.get("content"))
                    && let Some(text) = content.as_str()
                {
                    // Skip very short or tool_result messages
                    if text.len() > 10 {
                        last_user_instruction = Some(text.to_string());
                    }
                }
            }
            "assistant" => {
                let instruction = match &last_user_instruction {
                    Some(i) => i.clone(),
                    None => continue,
                };

                // Extract assistant content blocks
                if let Some(content) = obj.get("message").and_then(|m| m.get("content"))
                    && let Some(blocks) = content.as_array()
                {
                        for block in blocks {
                            let block_type = block.get("type")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");

                            match block_type {
                                "tool_use" => {
                                    let tool_name = block.get("name")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("");
                                    let input = block.get("input")
                                        .cloned()
                                        .unwrap_or(serde_json::Value::Null);

                                    match tool_name {
                                        "Edit" => {
                                            // Extract edit: file_path, old_string, new_string
                                            let file_path = input.get("file_path")
                                                .and_then(|v| v.as_str())
                                                .map(|s| s.to_string());
                                            let new_string = input.get("new_string")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("");

                                            if !new_string.is_empty() && new_string.len() > 20 {
                                                examples.push(T146 {
                                                    instruction: instruction.clone(),
                                                    response: new_string.to_string(),
                                                    response_type: "code_edit".into(),
                                                    file_path,
                                                    session_id: session_id.clone(),
                                                });
                                            }
                                        }
                                        "Write" => {
                                            let file_path = input.get("file_path")
                                                .and_then(|v| v.as_str())
                                                .map(|s| s.to_string());
                                            let content = input.get("content")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("");

                                            if !content.is_empty() && content.len() > 50 {
                                                examples.push(T146 {
                                                    instruction: instruction.clone(),
                                                    response: content.to_string(),
                                                    response_type: "code_write".into(),
                                                    file_path,
                                                    session_id: session_id.clone(),
                                                });
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                "text" => {
                                    let text = block.get("text")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("");
                                    // Only capture substantial text responses (>100 chars)
                                    // Skip error messages and API errors
                                    if text.len() > 100
                                        && !text.contains("API Error")
                                        && !text.contains("error")
                                    {
                                        examples.push(T146 {
                                            instruction: instruction.clone(),
                                            response: text.to_string(),
                                            response_type: "explanation".into(),
                                            file_path: None,
                                            session_id: session_id.clone(),
                                        });
                                    }
                                }
                                _ => {}
                            }
                        }
                }
            }
            _ => {}
        }
    }

    Ok((examples, msg_count, user_count))
}

/// f238=export_mined
/// Export mined examples to JSONL for training.
pub fn f238(examples: &[T146]) -> Result<PathBuf, String> {
    let dir = training_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    // SFT format (ChatML)
    let path = dir.join("mined_conversations.jsonl");
    let mut lines = Vec::new();
    for ex in examples {
        let chatml = serde_json::json!({
            "messages": [
                {"role": "system", "content": "You are a Rust systems programming assistant. Write clean, correct code."},
                {"role": "user", "content": ex.instruction},
                {"role": "assistant", "content": ex.response},
            ]
        });
        lines.push(serde_json::to_string(&chatml).map_err(|e| e.to_string())?);
    }
    std::fs::write(&path, lines.join("\n")).map_err(|e| e.to_string())?;

    // Also export code-only examples (Edit/Write responses only)
    let code_examples: Vec<&T146> = examples.iter()
        .filter(|e| e.response_type == "code_edit" || e.response_type == "code_write")
        .collect();

    let code_path = dir.join("mined_code_only.jsonl");
    let mut code_lines = Vec::new();
    for ex in &code_examples {
        let chatml = serde_json::json!({
            "messages": [
                {"role": "system", "content": "You are a Rust code generator. Output only code, no explanation."},
                {"role": "user", "content": ex.instruction},
                {"role": "assistant", "content": ex.response},
            ]
        });
        code_lines.push(serde_json::to_string(&chatml).map_err(|e| e.to_string())?);
    }
    std::fs::write(&code_path, code_lines.join("\n")).map_err(|e| e.to_string())?;

    eprintln!("{} total examples exported", examples.len());
    eprintln!("  {} code edits/writes", code_examples.len());
    eprintln!("  {}", path.display());
    eprintln!("  {}", code_path.display());

    Ok(path)
}

/// f239=print_mine_stats
/// Group mined examples by response type for stats.
pub fn f239(stats: &T147, examples: &[T146]) {
    println!("CONVERSATION LOG MINING");
    println!("─────────────────────────────────────────────────────────────────");
    println!("  Sessions scanned:    {}", stats.sessions);
    println!("  Total messages:      {}", stats.total_messages);
    println!("  User messages:       {}", stats.user_messages);
    println!("  Code edits mined:    {}", stats.code_edits);
    println!("  Code writes mined:   {}", stats.code_writes);
    println!("  Explanations mined:  {}", stats.explanations);
    println!("  Total examples:      {}", stats.total_examples);

    // Per-session breakdown
    let mut by_session: HashMap<String, usize> = HashMap::new();
    for ex in examples {
        *by_session.entry(ex.session_id.clone()).or_insert(0) += 1;
    }
    println!("\n  Per-session:");
    for (session, count) in by_session.iter() {
        println!("    {}...: {} examples", &session[..8.min(session.len())], count);
    }
}

fn log_directory() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".claude").join("projects").join("-Users-mcochran")
}

fn training_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".kova").join("micro").join("training")
}