// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! Elicitor module. Format questions, parse short replies, build restatements.
//! Used by GUI and serve for clarification flow.

/// Parsed user reply to a clarification question.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum T177 {
    /// User picked choice by index: (a) → 0, (b) → 1, etc.
    Choice(usize),
    /// User typed freeform text (e.g. "compute.rs").
    Freeform(String),
    /// User confirmed (y/yes) or declined (n/no).
    Confirm(bool),
    /// User cancelled: cancel, n, no, stop.
    Cancel,
}

/// Format a question with optional choices as "(a) X (b) Y (c) Z".
pub fn f302(question: &str, choices: Option<&[String]>) -> String {
    let q = question.trim();
    let Some(ch) = choices else {
        return q.to_string();
    };
    if ch.is_empty() {
        return q.to_string();
    }
    let letters = [b'a', b'b', b'c', b'd', b'e'];
    let parts: Vec<String> = ch
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let letter = letters.get(i).map(|&c| c as char).unwrap_or('?');
            format!("({}) {}", letter, s)
        })
        .collect();
    format!("{} {}", q, parts.join(" "))
}

/// Parse user input into T177.
/// - "y", "yes" → Confirm(true)
/// - "n", "no", "cancel", "stop" → Cancel (or Confirm(false) for n/no when not in clarification)
/// - "a", "1" when num_choices given → Choice(0)
/// - "b", "2" → Choice(1), etc.
/// - Exact match of a choice string → Choice(index)
/// - Otherwise → Freeform(trimmed)
pub fn f303(input: &str, num_choices: Option<usize>) -> T177 {
    let s = input.trim().to_lowercase();
    if s.is_empty() {
        return T177::Cancel;
    }
    // Cancel keywords
    if matches!(s.as_str(), "cancel" | "stop" | "abort") {
        return T177::Cancel;
    }
    // Confirm
    if matches!(s.as_str(), "y" | "yes") {
        return T177::Confirm(true);
    }
    if matches!(s.as_str(), "n" | "no") {
        return T177::Confirm(false);
    }
    // Choice by letter: a=0, b=1, c=2, d=3, e=4
    if s.len() == 1 {
        let c = s.chars().next().unwrap();
        if let Some(idx) = (b'a'..=b'e').position(|x| x as char == c)
            && num_choices.is_none_or(|n| idx < n)
        {
            return T177::Choice(idx);
        }
        if let Some(idx) = (b'1'..=b'5').position(|x| (x - b'0') as char == c)
            && num_choices.is_none_or(|n| idx < n)
        {
            return T177::Choice(idx);
        }
    }
    // Choice by number "1", "2", etc.
    if let Ok(n) = s.parse::<usize>() && n >= 1 {
        let idx = n - 1;
        if num_choices.is_none_or(|max| idx < max) {
            return T177::Choice(idx);
        }
    }
    T177::Freeform(input.trim().to_string())
}

/// Build restatement: "I'll add X to Y. Proceed? (y/n)"
pub fn f304(action: &str, target: &str) -> String {
    let a = action.trim();
    let t = target.trim();
    if t.is_empty() {
        format!("{}. Proceed? (y/n)", a)
    } else {
        format!("I'll {} in {}. Proceed? (y/n)", a, t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_question_no_choices() {
        assert_eq!(f302("Which file?", None), "Which file?");
    }

    #[test]
    fn format_question_with_choices() {
        let ch = ["compute.rs".into(), "plan.rs".into(), "lib.rs".into()];
        assert_eq!(
            f302("Which file?", Some(&ch)),
            "Which file? (a) compute.rs (b) plan.rs (c) lib.rs"
        );
    }

    #[test]
    fn parse_reply_confirm() {
        assert_eq!(f303("y", None), T177::Confirm(true));
        assert_eq!(f303("yes", None), T177::Confirm(true));
        assert_eq!(f303("Y", None), T177::Confirm(true));
    }

    #[test]
    fn parse_reply_cancel() {
        assert_eq!(f303("n", None), T177::Confirm(false));
        assert_eq!(f303("no", None), T177::Confirm(false));
        assert_eq!(f303("cancel", None), T177::Cancel);
        assert_eq!(f303("stop", None), T177::Cancel);
    }

    #[test]
    fn parse_reply_choice_letter() {
        assert_eq!(f303("a", Some(3)), T177::Choice(0));
        assert_eq!(f303("b", Some(3)), T177::Choice(1));
        assert_eq!(f303("c", Some(3)), T177::Choice(2));
    }

    #[test]
    fn parse_reply_choice_number() {
        assert_eq!(f303("1", Some(3)), T177::Choice(0));
        assert_eq!(f303("2", Some(3)), T177::Choice(1));
    }

    #[test]
    fn parse_reply_freeform() {
        assert_eq!(
            f303("compute.rs", Some(3)),
            T177::Freeform("compute.rs".into())
        );
    }

    #[test]
    fn build_restatement_basic() {
        assert_eq!(
            f304("add retry helper", "compute.rs"),
            "I'll add retry helper in compute.rs. Proceed? (y/n)"
        );
    }
}
