// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! Agentic tool loop. LLM calls tools, gets results, repeats until done.
//! f147=agent_turn, f148=agent_loop.
//! t106=AgentAction.

#![allow(non_camel_case_types)]

use std::io::Write;
use std::path::Path;

use crate::context_mgr::{self, t111};
use crate::tools::{self, t104};

/// t106=AgentAction.
pub enum t106 {
    Continue { tool_results: Vec<t104> },
    Done(String),
}

/// f147=agent_turn. Single agent turn: inference → parse tool calls → execute → return action.
/// Streams LLM tokens to stdout. Returns AgentAction.
pub fn f147(
    model_path: &Path,
    system_prompt: &str,
    conversation: &str,
    project_dir: &Path,
) -> t106 {
    // Run inference with streaming.
    let rx = crate::inference::f76(model_path, system_prompt, &[], conversation);

    let mut full_response = String::new();
    let mut stdout = std::io::stdout();

    for token in rx {
        let s: &str = &token;
        full_response.push_str(s);
        // Don't print JSON tool blocks to terminal raw — detect and dim them.
        print!("{}", s);
        let _ = stdout.flush();
    }
    println!();

    // Parse tool calls from response.
    let calls = tools::f140(&full_response);

    if calls.is_empty() {
        return t106::Done(full_response);
    }

    // Execute each tool call.
    let mut results = Vec::new();
    for call in &calls {
        eprintln!("\x1b[90m[tool: {}]\x1b[0m", call.tool);
        let result = tools::f141(call, project_dir);
        let status = if result.success {
            "\x1b[32mok\x1b[0m"
        } else {
            "\x1b[31merr\x1b[0m"
        };
        eprintln!("\x1b[90m[{}: {}]\x1b[0m", call.tool, status);
        // Print truncated result preview.
        let preview = truncate_output(&result.output, 20);
        if !preview.is_empty() {
            eprintln!("\x1b[90m{}\x1b[0m", preview);
        }
        results.push(result);
    }

    t106::Continue {
        tool_results: results,
    }
}

/// Context budget for agent loop. 8k tokens total, 1k system, 512 tools.
const AGENT_BUDGET: t111 = t111 {
    max_tokens: 8192,
    system_reserved: 1024,
    tool_reserved: 512,
};

/// Max tokens for a single tool result before trimming.
const TOOL_OUTPUT_MAX_TOKENS: usize = 1024;

/// f148=agent_loop. Run agent turns until done or max iterations.
pub fn f148(
    model_path: &Path,
    system_prompt: &str,
    user_input: &str,
    project_dir: &Path,
    max_iterations: u32,
) -> String {
    let mut conversation = String::new();
    conversation.push_str(&format!("User: {}\n\nAssistant: ", user_input));

    for i in 0..max_iterations {
        // Trim conversation to fit context window before each inference call.
        conversation = context_mgr::f171(&conversation, &AGENT_BUDGET);

        let action = f147(model_path, system_prompt, &conversation, project_dir);

        match action {
            t106::Done(response) => {
                return response;
            }
            t106::Continue { tool_results } => {
                // Append tool results to conversation for next turn.
                conversation.push_str("\n\nTool results:\n");
                for r in &tool_results {
                    // Trim long tool outputs to stay within budget.
                    let trimmed_output =
                        context_mgr::f172(&r.output, TOOL_OUTPUT_MAX_TOKENS);
                    conversation.push_str(&format!(
                        "[{}] {}: {}\n",
                        r.tool,
                        if r.success { "ok" } else { "err" },
                        truncate_output(&trimmed_output, 100),
                    ));
                }
                conversation.push_str("\nAssistant: ");

                if i + 1 >= max_iterations {
                    eprintln!(
                        "\x1b[33m[agent: max iterations ({}) reached]\x1b[0m",
                        max_iterations
                    );
                }
            }
        }
    }

    "Agent loop reached max iterations.".to_string()
}

fn truncate_output(s: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = s.lines().collect();
    if lines.len() <= max_lines {
        return s.to_string();
    }
    let half = max_lines / 2;
    let head: Vec<&str> = lines[..half].to_vec();
    let tail: Vec<&str> = lines[lines.len() - half..].to_vec();
    format!(
        "{}\n... ({} lines omitted) ...\n{}",
        head.join("\n"),
        lines.len() - max_lines,
        tail.join("\n")
    )
}
