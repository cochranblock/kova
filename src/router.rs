//! Router model. Intent classification. f79=classify.

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use std::path::Path;
use std::sync::{mpsc, Arc};
use std::thread;

use kalosm::language::{ChatModelExt, Parse};

const CLASSIFY_PROMPT: &str = r#"Classify the user's message as exactly one of: code_gen, refactor, explain, fix, run, custom, needs_clarification.

Rules:
- compiler error, type mismatch, borrow error, "doesn't compile", "fix" = fix
- write, create, add, implement, build, generate = code_gen
- what does, how does, explain, why = explain
- rename, extract, move, restructure, clean up = refactor
- run, execute, test, deploy = run
- If ambiguous, reply needs_clarification with a "question" field and optionally "choices" (2-5 options).

Reply with valid JSON only: {"classification": "..."} or {"classification": "needs_clarification", "question": "Which file?", "choices": ["compute.rs", "plan.rs"]}"#;

/// Structured output for grammar-constrained Router. Maps to T94.
#[derive(Clone, Debug, kalosm::language::Parse, kalosm::language::Schema)]
struct RouterOutput {
    classification: String,
    question: Option<String>,
    choices: Option<Vec<String>>,
}

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
    /// Suggested question when NeedsClarification. Canned fallback if model didn't provide one.
    /// Uses elicitor::f302 when choices are available.
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
                // Canned choices for fix/add when model didn't provide any
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

    /// Choices when NeedsClarification. None if freeform only.
    pub fn f364(&self) -> Option<&[String]> {
        match self {
            T94::NeedsClarification { choices, .. } => choices.as_deref(),
            _ => None,
        }
    }

    /// Use coder model for response (code_gen, refactor, explain, fix, custom).
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
        let rt = match tokio::runtime::Runtime::new() {
            Ok(r) => r,
            Err(e) => {
                let _ = tx.send(T94::Error(format!("tokio: {}", e)));
                return;
            }
        };
        rt.block_on(async {
            let result = run_classify(&path, &input).await;
            let _ = tx.send(result);
        });
    });

    rx
}

async fn run_classify(model_path: &Path, user_input: &str) -> T94 {
    use kalosm::language::StreamExt;

    let model = match crate::inference::get_or_load_model(model_path).await {
        Ok(m) => m,
        Err(e) => return T94::Error(format!("model load: {}", e)),
    };

    if crate::config::router_structured()
        && let Ok(result) = run_classify_structured(&model, user_input).await
    {
        return result;
    }

    let mut chat = model.chat().with_system_prompt(CLASSIFY_PROMPT);
    let mut response = chat(user_input);

    let mut out = String::new();
    while let Some(token) = response.next().await {
        out.push_str(&token.to_string());
    }

    T94::parse(&out)
}

async fn run_classify_structured(
    model: &kalosm::language::Llama,
    user_input: &str,
) -> Result<T94, ()> {
    let task = model
        .task(CLASSIFY_PROMPT)
        .with_constraints(Arc::new(RouterOutput::new_parser()));

    let stream = task.run(user_input);
    let output = stream.await.map_err(|_| ())?;

    Ok(router_output_to_result(&output))
}

fn router_output_to_result(out: &RouterOutput) -> T94 {
    let c = out.classification.to_lowercase().trim().to_string();
    let t = c.as_str();
    if t.contains("needs_clarification") || t.contains("clarification") {
        return T94::NeedsClarification {
            question: out.question.clone(),
            choices: out.choices.clone(),
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