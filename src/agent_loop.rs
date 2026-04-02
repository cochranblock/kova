//! Agentic tool loop. LLM calls tools, gets results, repeats until done.
//! f147=agent_turn, f148=agent_loop.
//! t106=AgentAction.
// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

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
    // Run inference with streaming. f382 picks local/remote/auto.
    let rx = crate::inference::f382(model_path, system_prompt, &[], conversation);

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
        // Auto-compact conversation when nearing context limit (80% threshold).
        // Uses LLM to summarize older turns, keeping recent ones intact.
        // Falls back to static trim if compaction still exceeds budget.
        conversation = context_mgr::f380(&conversation, &AGENT_BUDGET, model_path);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_output_short_text() {
        let s = "line1\nline2\nline3";
        assert_eq!(truncate_output(s, 5), s);
    }

    #[test]
    fn truncate_output_exact_limit() {
        let s = "a\nb\nc";
        assert_eq!(truncate_output(s, 3), s);
    }

    #[test]
    fn truncate_output_long_text() {
        let lines: Vec<String> = (0..100).map(|i| format!("line{}", i)).collect();
        let s = lines.join("\n");
        let result = truncate_output(&s, 10);
        assert!(result.contains("line0"));
        assert!(result.contains("line99"));
        assert!(result.contains("omitted"));
    }

    #[test]
    fn truncate_output_empty() {
        assert_eq!(truncate_output("", 10), "");
    }

    #[test]
    fn truncate_output_single_line() {
        assert_eq!(truncate_output("only", 10), "only");
    }

    #[test]
    fn agent_budget_values() {
        assert!(AGENT_BUDGET.max_tokens > 0);
        assert!(AGENT_BUDGET.system_reserved < AGENT_BUDGET.max_tokens);
        assert!(AGENT_BUDGET.tool_reserved < AGENT_BUDGET.max_tokens);
        assert!(
            AGENT_BUDGET.system_reserved + AGENT_BUDGET.tool_reserved < AGENT_BUDGET.max_tokens
        );
    }

    #[test]
    fn t106_variants() {
        let done = t106::Done("answer".into());
        match done {
            t106::Done(s) => assert_eq!(s, "answer"),
            _ => panic!("expected Done"),
        }

        let cont = t106::Continue {
            tool_results: vec![t104 {
                tool: "test".into(),
                success: true,
                output: "ok".into(),
            }],
        };
        match cont {
            t106::Continue { tool_results } => {
                assert_eq!(tool_results.len(), 1);
                assert!(tool_results[0].success);
            }
            _ => panic!("expected Continue"),
        }
    }

    #[test]
    fn tool_output_max_tokens_reasonable() {
        assert!(TOOL_OUTPUT_MAX_TOKENS > 100);
        assert!(TOOL_OUTPUT_MAX_TOKENS <= AGENT_BUDGET.max_tokens);
    }
}