// Unlicense — public domain — cochranblock.org
//! compaction — test harness for context compaction patterns.
//! Verifies: budget threshold, turn splitting, summary insertion, token savings.
//! Pure std — no inference dep. Tests the structural contract.

/// t75: Context budget for compaction decisions
#[derive(Debug, Clone)]
pub struct t75 {
    /// s91: total available tokens
    pub s91: usize,
    /// s92: tokens already used by system prompt / tools
    pub s92: usize,
}

/// t76: Compaction result
#[derive(Debug, Clone)]
pub struct t76 {
    /// s93: compaction was triggered
    pub s93: bool,
    /// s94: original token count
    pub s94: usize,
    /// s95: compacted token count
    pub s95: usize,
    /// s96: turns before compaction
    pub s96: usize,
    /// s97: turns after compaction
    pub s97: usize,
    /// s98: output text
    pub s98: String,
}

/// Compaction threshold (matches kova COMPACT_THRESHOLD)
const COMPACT_THRESHOLD: f64 = 0.80;

/// Recent turns to keep intact (matches kova COMPACT_KEEP_RECENT)
const COMPACT_KEEP_RECENT: usize = 4;

/// f130: Estimate token count (chars / 4, matches kova f170 heuristic)
pub fn f130(text: &str) -> usize {
    text.len() / 4
}

/// f131: Split conversation into turns by role markers.
/// Recognizes "User:", "Assistant:", "System:" prefixes on their own line.
pub fn f131(conversation: &str) -> Vec<String> {
    let mut turns = Vec::new();
    let mut current = String::new();

    for line in conversation.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("User:")
            || trimmed.starts_with("Assistant:")
            || trimmed.starts_with("System:")
        {
            if !current.trim().is_empty() {
                turns.push(current.clone());
            }
            current.clear();
        }
        current.push_str(line);
        current.push('\n');
    }

    if !current.trim().is_empty() {
        turns.push(current);
    }

    turns
}

/// f132: Run context compaction with a pluggable summarizer.
/// The summarizer function takes old turn text and returns a summary.
/// This allows tests to inject deterministic summaries without inference.
pub fn f132(conversation: &str, budget: &t75, summarize: impl FnOnce(&str) -> String) -> t76 {
    let available = budget.s91.saturating_sub(budget.s92);
    let current_tokens = f130(conversation);
    let threshold = (available as f64 * COMPACT_THRESHOLD) as usize;

    if current_tokens <= threshold {
        return t76 {
            s93: false,
            s94: current_tokens,
            s95: current_tokens,
            s96: f131(conversation).len(),
            s97: f131(conversation).len(),
            s98: conversation.to_string(),
        };
    }

    let turns = f131(conversation);
    let turn_count = turns.len();

    if turns.len() <= COMPACT_KEEP_RECENT + 1 {
        // Too few turns to compact — return as-is
        return t76 {
            s93: false,
            s94: current_tokens,
            s95: current_tokens,
            s96: turn_count,
            s97: turn_count,
            s98: conversation.to_string(),
        };
    }

    let split_point = turns.len().saturating_sub(COMPACT_KEEP_RECENT);
    let old_turns = &turns[..split_point];
    let recent_turns = &turns[split_point..];

    let old_text: String = old_turns.concat();
    let summary = summarize(&old_text);

    let mut result = String::with_capacity(conversation.len() / 2);
    result.push_str(&format!(
        "[Context compacted — {} turns summarized]\n\n",
        old_turns.len()
    ));
    result.push_str("Summary of earlier conversation:\n");
    result.push_str(&summary);
    result.push_str("\n\n");

    for turn in recent_turns {
        result.push_str(turn);
    }

    let compacted_tokens = f130(&result);

    t76 {
        s93: true,
        s94: current_tokens,
        s95: compacted_tokens,
        s96: turn_count,
        s97: recent_turns.len() + 1, // summary counts as 1 turn
        s98: result,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_conversation(turn_count: usize, chars_per_turn: usize) -> String {
        let mut conv = String::new();
        for i in 0..turn_count {
            let role = if i % 2 == 0 { "User" } else { "Assistant" };
            let padding = "x".repeat(chars_per_turn);
            conv.push_str(&format!("{}:\nturn {} {}\n\n", role, i, padding));
        }
        conv
    }

    #[test]
    fn under_threshold_no_compaction() {
        let conv = make_conversation(4, 10);
        let budget = t75 { s91: 10000, s92: 0 };
        let result = f132(&conv, &budget, |_| panic!("should not summarize"));
        assert!(!result.s93, "should not trigger compaction");
        assert_eq!(result.s98, conv);
    }

    #[test]
    fn over_threshold_triggers_compaction() {
        // 10 turns x 200 chars = ~500 tokens, budget 400 = over 80%
        let conv = make_conversation(10, 200);
        let budget = t75 { s91: 400, s92: 0 };
        let result = f132(&conv, &budget, |old| {
            format!("Summary: {} chars compressed", old.len())
        });
        assert!(result.s93, "should trigger compaction");
        assert!(result.s95 < result.s94, "compacted should be smaller");
        assert!(result.s98.contains("[Context compacted"));
        assert!(result.s98.contains("Summary of earlier conversation:"));
    }

    #[test]
    fn keeps_recent_turns_intact() {
        let conv = make_conversation(10, 200);
        let budget = t75 { s91: 400, s92: 0 };
        let result = f132(&conv, &budget, |_| "brief summary".into());

        // Last 4 turns should be preserved verbatim
        let turns = f131(&conv);
        for turn in &turns[turns.len() - COMPACT_KEEP_RECENT..] {
            assert!(
                result.s98.contains(turn.trim()),
                "recent turn should be preserved: {}...",
                &turn[..turn.len().min(40)]
            );
        }
    }

    #[test]
    fn too_few_turns_no_compaction() {
        // Only 5 turns — with COMPACT_KEEP_RECENT=4, can't compact
        let conv = make_conversation(5, 200);
        let budget = t75 { s91: 100, s92: 0 };
        let result = f132(&conv, &budget, |_| panic!("should not summarize"));
        assert!(!result.s93, "too few turns to compact");
    }

    #[test]
    fn token_estimation() {
        assert_eq!(f130(""), 0);
        assert_eq!(f130("abcd"), 1);
        assert_eq!(f130("abcdefgh"), 2);
        // 400 chars = ~100 tokens
        let text = "x".repeat(400);
        assert_eq!(f130(&text), 100);
    }

    #[test]
    fn turn_splitting() {
        let conv = "User:\nhello\n\nAssistant:\nhi there\n\nUser:\nhow are you\n";
        let turns = f131(conv);
        assert_eq!(turns.len(), 3);
        assert!(turns[0].contains("hello"));
        assert!(turns[1].contains("hi there"));
        assert!(turns[2].contains("how are you"));
    }

    #[test]
    fn compaction_report_metrics() {
        let conv = make_conversation(12, 300);
        let budget = t75 { s91: 500, s92: 0 };
        let result = f132(&conv, &budget, |_| "short".into());
        assert!(result.s93);
        assert_eq!(result.s96, 12);
        assert_eq!(result.s97, COMPACT_KEEP_RECENT + 1);
        assert!(result.s94 > result.s95);
    }

    #[test]
    fn pluggable_summarizer() {
        let conv = make_conversation(10, 200);
        let budget = t75 { s91: 400, s92: 0 };

        // Summarizer that extracts key facts
        let result = f132(&conv, &budget, |old| {
            let turn_count = old.matches("turn").count();
            format!("Discussed {} items", turn_count)
        });

        assert!(result.s93);
        assert!(result.s98.contains("Discussed"));
    }
}
