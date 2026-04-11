//! Router model. Intent classification. f79=classify.
//! Uses candle GGUF inference for local classification.

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use std::path::Path;
use std::sync::{mpsc, Arc};
use std::thread;

const CLASSIFY_PROMPT: &str = r#"Classify the user's message as exactly one of: code_gen, refactor, explain, fix, run, custom, needs_clarification.

Rules:
- compiler error, type mismatch, borrow error, "doesn't compile", "fix" = fix
- write, create, add, implement, build, generate = code_gen
- what does, how does, explain, why = explain
- rename, extract, move, restructure, clean up = refactor
- run, execute, test, deploy = run
- If ambiguous, reply needs_clarification with a "question" field and optionally "choices" (2-5 options).

Reply with valid JSON only: {"classification": "..."} or {"classification": "needs_clarification", "question": "Which file?", "choices": ["compute.rs", "plan.rs"]}"#;

/// Router classification result.
#[derive(Debug, Clone, PartialEq, Eq)]
/// t94=T94. Router classification output.
pub enum T94 {
    CodeGen,
    Refactor,
    Explain,
    Fix,
    Run,
    Custom,
    NeedsClarification {
        question: Option<String>,
        choices: Option<Vec<String>>,
    },
    Error(String),
}

impl T94 {
    pub fn f363(&self, original_msg: &str) -> String {
        match self {
            T94::NeedsClarification { question, choices } => {
                let q = question
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| {
                        let lower = original_msg.to_lowercase();
                        if lower.contains("fix") || lower.contains("bug") {
                            "Which file?"
                        } else if lower.contains("add") || lower.contains("implement") {
                            "Which file should I add this to?"
                        } else {
                            "Could you clarify what you need?"
                        }
                    });
                if let Some(ch) = choices && !ch.is_empty() {
                    return crate::elicitor::f302(q, Some(ch));
                }
                let lower = original_msg.to_lowercase();
                if lower.contains("fix") || lower.contains("bug") {
                    let canned = vec![
                        "compute.rs".into(),
                        "plan.rs".into(),
                        "lib.rs".into(),
                        "other".into(),
                    ];
                    return crate::elicitor::f302(q, Some(&canned));
                }
                if lower.contains("add") || lower.contains("implement") {
                    let canned = vec!["lib.rs".into(), "main.rs".into(), "other".into()];
                    return crate::elicitor::f302(q, Some(&canned));
                }
                q.to_string()
            }
            _ => "Could you clarify what you need?".into(),
        }
    }

    pub fn f364(&self) -> Option<&[String]> {
        match self {
            T94::NeedsClarification { choices, .. } => choices.as_deref(),
            _ => None,
        }
    }

    pub fn f365(&self) -> bool {
        matches!(
            self,
            T94::CodeGen
                | T94::Refactor
                | T94::Explain
                | T94::Fix
                | T94::Custom
        )
    }

    fn parse(s: &str) -> Self {
        let lower = s.to_lowercase().trim().to_string();
        let t = lower.as_str();
        if t.contains("needs_clarification") || t.contains("clarification") {
            let question = t
                .split('|')
                .nth(1)
                .map(|q| q.trim().to_string())
                .filter(|q| !q.is_empty());
            return T94::NeedsClarification {
                question,
                choices: None,
            };
        }
        if t.contains("code_gen") || t.contains("code gen") {
            return T94::CodeGen;
        }
        if t.contains("refactor") {
            return T94::Refactor;
        }
        if t.contains("explain") {
            return T94::Explain;
        }
        if t.contains("fix") {
            return T94::Fix;
        }
        if t.contains("run") {
            return T94::Run;
        }
        if t.contains("custom") {
            return T94::Custom;
        }
        T94::CodeGen
    }
}

/// f79=classify. Spawn router inference. Returns receiver for single T94.
pub fn f79(model_path: &Path, user_input: &str) -> mpsc::Receiver<T94> {
    let (tx, rx) = mpsc::channel();
    let path = model_path.to_path_buf();
    let input = user_input.to_string();

    thread::spawn(move || {
        let result = run_classify(&path, &input);
        let _ = tx.send(result);
    });

    rx
}

fn run_classify(model_path: &Path, user_input: &str) -> T94 {
    let response = match crate::inference::f80(model_path, CLASSIFY_PROMPT, user_input) {
        Ok(s) => s,
        Err(e) => return T94::Error(format!("inference: {}", e)),
    };

    // Try JSON parse first
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&response) {
        if let Some(c) = v.get("classification").and_then(|c| c.as_str()) {
            let mut result = T94::parse(c);
            if let T94::NeedsClarification {
                ref mut question,
                ref mut choices,
            } = result
            {
                if question.is_none() {
                    *question = v
                        .get("question")
                        .and_then(|q| q.as_str())
                        .map(|s| s.to_string());
                }
                if choices.is_none() {
                    *choices = v.get("choices").and_then(|c| {
                        c.as_array().map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect()
                        })
                    });
                }
            }
            return result;
        }
    }

    T94::parse(&response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kova_test;
    use assert_matches::assert_matches;

    kova_test!(f79, parse_needs_clarification_with_question, {
        let r = T94::parse("needs_clarification|Which file?");
        assert_matches!(r, T94::NeedsClarification { question: Some(q), .. } => {
            assert_eq!(q, "which file?");
        });
    });

    kova_test!(f79, parse_needs_clarification_without_question, {
        let r = T94::parse("needs_clarification");
        assert_matches!(
            r,
            T94::NeedsClarification {
                question: None,
                choices: None
            }
        );
    });

    kova_test!(f79, clarification_question_canned, {
        let r = T94::NeedsClarification {
            question: None,
            choices: None,
        };
        assert!(r.f363("fix the bug").contains("file"));
        assert!(r.f363("add a retry").contains("file"));
        assert_eq!(
            r.f363("something vague"),
            "Could you clarify what you need?"
        );
    });
}
