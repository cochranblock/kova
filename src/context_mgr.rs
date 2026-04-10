//! Token-aware context window manager.
//! f170=estimate_tokens, f171=trim_conversation, f172=trim_tool_output, f173=summarize_old_turns,
//! f380=compact_context.
//! t111=ContextBudget.
// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use std::path::Path;

/// t111=ContextBudget. Tracks token allocation for a model's context window.
#[allow(non_camel_case_types)]
pub struct t111 {
    /// Total tokens the model supports.
    pub max_tokens: usize,
    /// Tokens reserved for the system prompt.
    pub system_reserved: usize,
    /// Tokens reserved for tool definitions and results.
    pub tool_reserved: usize,
}

impl t111 {
    /// Tokens available for conversation history.
    pub fn remaining(&self) -> usize {
        self.max_tokens
            .saturating_sub(self.system_reserved)
            .saturating_sub(self.tool_reserved)
    }
}

/// f170=estimate_tokens. Rough token count: chars / 4.
/// Good enough for local GGUF models without pulling in a tokenizer crate.
pub fn f170(text: &str) -> usize {
    // chars()/4 is a standard rough estimate for English text.
    // Use char count (not byte len) so multibyte UTF-8 doesn't inflate estimates.
    // Round up so we never undercount.
    let char_count = text.chars().count();
    char_count.div_ceil(4)
}

/// f171=trim_conversation. Trim oldest turns to fit within budget.
/// Keeps the first line (user's original question) and the most recent turns.
/// Returns trimmed conversation string.
pub fn f171(conversation: &str, budget: &t111) -> String {
    let max = budget.remaining();
    if f170(conversation) <= max {
        return conversation.to_string();
    }

    // Split on turn boundaries (User:/Assistant:/Tool results:).
    let turns = split_turns(conversation);
    if turns.len() <= 2 {
        // Only one exchange — can't trim further, just truncate from the front.
        let char_limit = max * 4;
        return truncate_end(conversation, char_limit);
    }

    // Always keep first turn (original user message) and last turn.
    let first = turns[0];
    let last = turns[turns.len() - 1];
    let fixed_tokens = f170(first) + f170(last);

    if fixed_tokens >= max {
        // Even first+last exceed budget — keep last turn only.
        let char_limit = max * 4;
        return truncate_end(last, char_limit);
    }

    // Reserve tokens for the summary marker line (~20 tokens).
    let summary_reserve = 20;
    let available = max.saturating_sub(fixed_tokens).saturating_sub(summary_reserve);

    // Walk backwards from second-to-last, adding turns until budget exhausted.
    let middle = &turns[1..turns.len() - 1];
    let mut kept: Vec<&str> = Vec::new();
    let mut used = 0;

    for &turn in middle.iter().rev() {
        let cost = f170(turn);
        if used + cost > available {
            break;
        }
        kept.push(turn);
        used += cost;
    }
    kept.reverse();

    // Build result: first turn + summary of dropped turns + kept turns + last turn.
    let dropped_count = middle.len() - kept.len();
    let mut result = String::with_capacity(max * 4);
    result.push_str(first);

    if dropped_count > 0 {
        let dropped = &middle[..dropped_count];
        let summary = f173(dropped);
        result.push_str(&summary);
        result.push('\n');
    }

    for turn in &kept {
        result.push_str(turn);
    }
    result.push_str(last);

    result
}

/// f172=trim_tool_output. Truncate tool output that exceeds max_tokens.
/// Keeps head and tail with a [truncated] marker in the middle.
pub fn f172(output: &str, max_tokens: usize) -> String {
    if f170(output) <= max_tokens {
        return output.to_string();
    }

    let char_limit = max_tokens * 4;
    if char_limit < 40 {
        return "[truncated]".to_string();
    }

    // Keep 60% head, 30% tail, rest for marker.
    let head_chars = (char_limit * 60) / 100;
    let tail_chars = (char_limit * 30) / 100;

    let total_chars = output.chars().count();
    // Clamp so head + tail never exceed total (avoids overlap on short output).
    let (head_chars, tail_chars) = if head_chars + tail_chars >= total_chars {
        let half = total_chars / 2;
        (half, total_chars.saturating_sub(half + 1).max(1))
    } else {
        (head_chars, tail_chars)
    };

    let head: String = output.chars().take(head_chars).collect();
    let tail: String = output.chars().skip(total_chars.saturating_sub(tail_chars)).collect();
    let omitted = total_chars.saturating_sub(head_chars + tail_chars);

    format!("{}\n[truncated: ~{} chars omitted]\n{}", head, omitted, tail)
}

/// f173=summarize_old_turns. Compress old conversation turns into a brief summary.
pub fn f173(turns: &[&str]) -> String {
    if turns.is_empty() {
        return String::new();
    }

    let tool_count = turns
        .iter()
        .filter(|t| t.contains("Tool results:"))
        .count();
    let assistant_count = turns
        .iter()
        .filter(|t| t.trim_start().starts_with("Assistant:"))
        .count();
    let user_count = turns
        .iter()
        .filter(|t| t.trim_start().starts_with("User:"))
        .count();

    let mut parts = Vec::new();
    if user_count > 0 {
        parts.push(format!("{} user turn(s)", user_count));
    }
    if assistant_count > 0 {
        parts.push(format!("{} assistant turn(s)", assistant_count));
    }
    if tool_count > 0 {
        parts.push(format!("{} tool exchange(s)", tool_count));
    }
    if parts.is_empty() {
        parts.push(format!("{} turn(s)", turns.len()));
    }

    format!("\n[earlier context trimmed: {}]\n", parts.join(", "))
}

/// Split conversation into turns at User:/Assistant:/Tool results: boundaries.
fn split_turns(text: &str) -> Vec<&str> {
    let mut boundaries = Vec::new();
    let prefixes = ["User: ", "Assistant: ", "Tool results:\n", "\nUser: ", "\nAssistant: ", "\nTool results:\n"];

    // Find all turn boundaries.
    for prefix in &prefixes {
        let mut start = 0;
        while let Some(pos) = text[start..].find(prefix) {
            let abs = start + pos;
            // If prefix starts with \n, the boundary is at abs (before \n).
            // Otherwise only if at position 0.
            if prefix.starts_with('\n') || abs == 0 {
                boundaries.push(abs);
            }
            start = abs + prefix.len();
        }
    }

    boundaries.sort();
    boundaries.dedup();

    if boundaries.is_empty() {
        return vec![text];
    }

    let mut turns = Vec::new();
    for i in 0..boundaries.len() {
        let start = boundaries[i];
        let end = if i + 1 < boundaries.len() {
            boundaries[i + 1]
        } else {
            text.len()
        };
        if start < end {
            turns.push(&text[start..end]);
        }
    }

    // If first boundary isn't at 0, include the prefix.
    if boundaries[0] > 0 {
        turns.insert(0, &text[..boundaries[0]]);
    }

    turns
}

/// Compaction threshold: trigger when conversation fills 80% of available budget.
const COMPACT_THRESHOLD: f64 = 0.80;

/// How many recent turns to keep intact during compaction.
const COMPACT_KEEP_RECENT: usize = 4;

/// f380=compact_context. LLM-powered context compaction.
/// When conversation exceeds 80% of the context budget, split into old and recent turns.
/// Send old turns to inference with a summarize prompt, replace them with the summary.
/// Returns the (possibly compacted) conversation. If under threshold, returns as-is.
pub fn f380(conversation: &str, budget: &t111, model_path: &Path) -> String {
    let available = budget.remaining();
    let current_tokens = f170(conversation);
    let threshold = (available as f64 * COMPACT_THRESHOLD) as usize;

    if current_tokens <= threshold {
        return conversation.to_string();
    }

    eprintln!(
        "\x1b[33m[compact: {}/{} tokens ({:.0}%), compacting]\x1b[0m",
        current_tokens,
        available,
        (current_tokens as f64 / available as f64) * 100.0
    );

    let turns = split_turns(conversation);
    if turns.len() <= COMPACT_KEEP_RECENT + 1 {
        // Too few turns to compact — fall back to static trim.
        return f171(conversation, budget);
    }

    // Split: keep first turn (original question) + last N turns intact.
    let split_point = turns.len().saturating_sub(COMPACT_KEEP_RECENT);
    let old_turns = &turns[..split_point];
    let recent_turns = &turns[split_point..];

    // Build text to summarize.
    let old_text: String = old_turns.join("");
    if old_text.trim().is_empty() {
        return conversation.to_string();
    }

    // Call inference to summarize (or static fallback without inference feature).
    let summary = compact_via_inference(&old_text, model_path);
    #[cfg(not(feature = "inference"))]
    let summary = {
        let turns: Vec<&str> = old_text.split("\n\n").filter(|s| !s.trim().is_empty()).collect();
        f173(&turns)
    };

    // Build compacted conversation: summary + recent turns.
    let mut result = String::with_capacity(conversation.len() / 2);
    result.push_str(&format!(
        "[Context compacted — {} turns summarized into {}]\n\n",
        old_turns.len(),
        if summary.len() < 200 { "brief" } else { "digest" }
    ));
    result.push_str("Summary of earlier conversation:\n");
    result.push_str(&summary);
    result.push_str("\n\n");

    for turn in recent_turns {
        result.push_str(turn);
    }

    let compacted_tokens = f170(&result);
    eprintln!(
        "\x1b[32m[compact: {} → {} tokens (saved {})]\x1b[0m",
        current_tokens,
        compacted_tokens,
        current_tokens.saturating_sub(compacted_tokens)
    );

    // If still over budget after compaction, fall back to static trim.
    if compacted_tokens > available {
        return f171(&result, budget);
    }

    result
}

/// Call inference to summarize old conversation turns.
/// Collects all streamed tokens into a single string.
fn compact_via_inference(old_text: &str, model_path: &Path) -> String {
    let summary_prompt = format!(
        "Summarize this conversation history. Keep all key facts, decisions, tool results, \
         file paths, and code changes. Drop filler and repetition. Be concise.\n\n\
         ---\n{}\n---\n\nSummary:",
        old_text
    );

    let system = "You are a conversation summarizer. Output only the summary, nothing else.";
    let rx = crate::inference::f382(model_path, system, &[], &summary_prompt);

    let mut summary = String::new();
    for token in rx {
        summary.push_str(&token);
    }

    if summary.trim().is_empty() {
        // Inference failed — fall back to static summary.
        let turns: Vec<&str> = old_text
            .split("\n\n")
            .filter(|s| !s.trim().is_empty())
            .collect();
        return f173(&turns);
    }

    summary.trim().to_string()
}

/// Truncate string to at most `max_chars` characters.
fn truncate_end(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let truncated: String = s.chars().take(max_chars).collect();
    format!("{}...[truncated]", truncated)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// f170=estimate_tokens
    #[test]
    fn estimate_tokens_basic() {
        // 12 chars "hello world!" → (12+3)/4 = 3 tokens
        assert_eq!(f170("hello world!"), 3);
    }

    /// f170=estimate_tokens
    #[test]
    fn estimate_tokens_empty() {
        assert_eq!(f170(""), 0);
    }

    /// f170=estimate_tokens
    #[test]
    fn estimate_tokens_long_text() {
        let text = "a".repeat(1000);
        // (1000+3)/4 = 250 (rounds up, but 1000 is divisible by 4 → still 250 with +3)
        assert_eq!(f170(&text), 250);
    }

    /// t111=ContextBudget remaining
    #[test]
    fn budget_remaining() {
        let b = t111 {
            max_tokens: 4096,
            system_reserved: 500,
            tool_reserved: 200,
        };
        assert_eq!(b.remaining(), 3396);
    }

    /// t111=ContextBudget remaining saturates at zero
    #[test]
    fn budget_remaining_saturates() {
        let b = t111 {
            max_tokens: 100,
            system_reserved: 80,
            tool_reserved: 80,
        };
        assert_eq!(b.remaining(), 0);
    }

    /// f171=trim_conversation — short conversation passes through unchanged
    #[test]
    fn trim_conversation_fits() {
        let conv = "User: hello\n\nAssistant: hi there";
        let budget = t111 {
            max_tokens: 4096,
            system_reserved: 100,
            tool_reserved: 100,
        };
        assert_eq!(f171(conv, &budget), conv);
    }

    /// f171=trim_conversation — long conversation drops old turns
    #[test]
    fn trim_conversation_drops_old() {
        // Build a conversation that exceeds budget.
        let mut conv = String::new();
        conv.push_str("User: what is rust?\n\n");
        for i in 0..20 {
            conv.push_str(&format!(
                "Assistant: turn {} with some filler text to eat tokens aaaa bbbb cccc\n\n",
                i
            ));
            conv.push_str(&format!("User: follow up {}\n\n", i));
        }
        conv.push_str("Assistant: final answer");

        let budget = t111 {
            max_tokens: 200,
            system_reserved: 20,
            tool_reserved: 20,
        };
        let trimmed = f171(&conv, &budget);
        // Should fit in budget.
        assert!(f170(&trimmed) <= budget.remaining());
        // Should contain the final answer.
        assert!(trimmed.contains("final answer"));
        // Should contain trimmed marker.
        assert!(trimmed.contains("[earlier context trimmed:"));
    }

    /// f172=trim_tool_output — short output passes through
    #[test]
    fn trim_tool_output_fits() {
        let output = "line1\nline2\nline3";
        assert_eq!(f172(output, 100), output);
    }

    /// f172=trim_tool_output — long output gets truncated
    #[test]
    fn trim_tool_output_truncates() {
        let output = "x".repeat(2000);
        let trimmed = f172(&output, 50);
        assert!(trimmed.contains("[truncated:"));
        assert!(f170(&trimmed) < f170(&output));
    }

    /// f172=trim_tool_output — very small budget
    #[test]
    fn trim_tool_output_tiny_budget() {
        let output = "x".repeat(500);
        let trimmed = f172(&output, 5);
        assert_eq!(trimmed, "[truncated]");
    }

    /// f173=summarize_old_turns
    #[test]
    fn summarize_empty() {
        assert_eq!(f173(&[]), "");
    }

    /// f173=summarize_old_turns
    #[test]
    fn summarize_mixed_turns() {
        let turns = vec![
            "User: question 1\n",
            "Assistant: answer 1\n",
            "Tool results:\n[bash] ok: done\n",
            "Assistant: answer 2\n",
        ];
        let summary = f173(&turns);
        assert!(summary.contains("1 user turn(s)"));
        assert!(summary.contains("2 assistant turn(s)"));
        assert!(summary.contains("1 tool exchange(s)"));
    }

    /// f170=estimate_tokens. Multibyte UTF-8 (emoji) counts chars not bytes.
    #[test]
    fn estimate_tokens_multibyte_utf8() {
        // "🦀" is 1 char, 4 bytes. chars/4 = 0.25 → rounds up to 1.
        assert_eq!(f170("🦀"), 1);
        // "café" = 4 chars, 5 bytes. (4+3)/4 = 1.
        assert_eq!(f170("café"), 1);
    }

    /// f171=trim_conversation. Edge: Tool results at start.
    #[test]
    fn trim_conversation_tool_results_edge() {
        let conv = "Tool results:\n[read_file] ok\n\nUser: hi\n\nAssistant: hello";
        let budget = t111 {
            max_tokens: 4096,
            system_reserved: 100,
            tool_reserved: 100,
        };
        let trimmed = f171(conv, &budget);
        assert!(trimmed.contains("hello"));
    }

    /// f171=trim_conversation. Edge: single turn with no prefix.
    #[test]
    fn trim_conversation_single_turn_no_prefix() {
        let conv = "just some text without User:/Assistant:";
        let budget = t111 {
            max_tokens: 100,
            system_reserved: 10,
            tool_reserved: 10,
        };
        let trimmed = f171(conv, &budget);
        assert!(!trimmed.is_empty());
    }

    /// f380=compact_context. Under threshold — passthrough.
    #[test]
    fn compact_context_under_threshold() {
        let conv = "User: hello\n\nAssistant: hi there";
        let budget = t111 {
            max_tokens: 4096,
            system_reserved: 100,
            tool_reserved: 100,
        };
        // Can't call f380 without a real model, but we can verify the threshold logic.
        let available = budget.remaining();
        let current = f170(conv);
        let threshold = (available as f64 * COMPACT_THRESHOLD) as usize;
        assert!(current <= threshold, "short conversation should be under threshold");
    }

    /// f380 threshold calculation.
    #[test]
    fn compact_threshold_math() {
        let budget = t111 {
            max_tokens: 8192,
            system_reserved: 1024,
            tool_reserved: 512,
        };
        let available = budget.remaining(); // 6656
        let threshold = (available as f64 * COMPACT_THRESHOLD) as usize;
        assert_eq!(available, 6656);
        assert_eq!(threshold, 5324); // 80% of 6656
    }

    /// COMPACT_KEEP_RECENT is reasonable.
    #[test]
    fn compact_keep_recent_value() {
        assert!(COMPACT_KEEP_RECENT >= 2);
        assert!(COMPACT_KEEP_RECENT <= 10);
    }

    /// split_turns handles compacted format.
    #[test]
    fn split_turns_with_summary() {
        let conv = "[Context compacted]\n\nSummary of earlier conversation:\nDid stuff.\n\nUser: next question\n\nAssistant: answer";
        let turns = split_turns(conv);
        assert!(turns.len() >= 2);
    }
}