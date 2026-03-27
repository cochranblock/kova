//! train — Training data export from tournament results.
//!
//! Generates DPO (Direct Preference Optimization) preference pairs from
//! tournament match data. Same challenge, different model responses:
//!   chosen  = response that passed verification
//!   rejected = response that failed
//!
//! Output formats:
//!   dpo   — (prompt, chosen, rejected) triples for DPO/KTO/ORPO
//!   sft   — (prompt, response) pairs from passing responses only (supervised)
//!   chatml — ChatML format for MLX/unsloth fine-tuning
//!
//! The data is the moat. The algorithm is a config flag.
// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use std::collections::HashMap;
use std::path::PathBuf;

use super::tournament::{T165, T162};
use super::registry::T149;

// ── DPO Pair ────────────────────────────────────────────────────

/// T167=T175
/// A single DPO preference pair.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct T167 {
    /// System prompt context (from template).
    pub system: String,
    /// The challenge input / user prompt.
    pub prompt: String,
    /// Response that passed verification.
    pub chosen: String,
    /// Response that failed verification.
    pub rejected: String,
    /// Challenge category (classify, code_gen, fix_compile, etc.).
    pub category: String,
    /// Challenge description.
    pub challenge: String,
    /// Models that produced these responses.
    pub chosen_model: String,
    pub rejected_model: String,
}

/// T168=SftExample
/// A supervised fine-tuning example.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct T168 {
    pub system: String,
    pub prompt: String,
    pub response: String,
    pub category: String,
    pub model: String,
}

/// T169=ChatMlExample
/// ChatML formatted example (for MLX/unsloth).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct T169 {
    pub messages: Vec<T170>,
}

/// T170=ChatMlMessage
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct T170 {
    pub role: String,
    pub content: String,
}

// ── Export Functions ─────────────────────────────────────────────

/// Build challenge-to-input lookup from registry.
fn challenge_inputs(registry: &T149) -> HashMap<String, (String, String)> {
    let challenges = super::tournament::f248(registry);
    let mut map = HashMap::new();
    for ch in challenges {
        // key = challenge description (matches T162.challenge)
        let sys = registry.get(&ch.template_id)
            .map(|t| t.system_prompt.clone())
            .unwrap_or_default();
        map.insert(ch.description.clone(), (ch.input.clone(), sys));
    }
    map
}

/// f255=extract_dpo_pairs
/// Extract DPO preference pairs from tournament results.
///
/// For each challenge, pairs every passing response with every failing response.
/// More pairs = more signal. Filters out error responses (no useful rejected signal).
pub fn f255(result: &T165, registry: &T149) -> Vec<T167> {
    let inputs = challenge_inputs(registry);
    let mut pairs = Vec::new();

    // Group matches by challenge description
    let mut by_challenge: HashMap<String, Vec<&T162>> = HashMap::new();
    for m in &result.matches {
        by_challenge.entry(m.challenge.clone()).or_default().push(m);
    }

    for (challenge, matches) in &by_challenge {
        let (input, system) = match inputs.get(challenge) {
            Some(v) => v.clone(),
            None => continue,
        };

        let passed: Vec<&&T162> = matches.iter()
            .filter(|m| m.passed && !m.response.is_empty())
            .collect();
        let failed: Vec<&&T162> = matches.iter()
            .filter(|m| !m.passed && !m.response.is_empty() && !m.response.starts_with("ERROR:"))
            .collect();

        // Cross-product: every passing response paired with every failing response
        for p in &passed {
            for f in &failed {
                pairs.push(T167 {
                    system: system.clone(),
                    prompt: input.clone(),
                    chosen: p.response.clone(),
                    rejected: f.response.clone(),
                    category: p.category.clone(),
                    challenge: challenge.clone(),
                    chosen_model: p.competitor.model.clone(),
                    rejected_model: f.competitor.model.clone(),
                });
            }
        }
    }

    pairs
}

/// f256=extract_sft
/// Extract supervised fine-tuning examples (passing responses only).
pub fn f256(result: &T165, registry: &T149) -> Vec<T168> {
    let inputs = challenge_inputs(registry);
    let mut examples = Vec::new();

    for m in &result.matches {
        if !m.passed || m.response.is_empty() { continue; }

        let (input, system) = match inputs.get(&m.challenge) {
            Some(v) => v.clone(),
            None => continue,
        };

        examples.push(T168 {
            system,
            prompt: input,
            response: m.response.clone(),
            category: m.category.clone(),
            model: m.competitor.model.clone(),
        });
    }

    examples
}

/// f257=to_chatml
/// Convert SFT examples to ChatML format.
pub fn f257(examples: &[T168]) -> Vec<T169> {
    examples.iter().map(|ex| {
        let mut messages = Vec::new();
        if !ex.system.is_empty() {
            messages.push(T170 {
                role: "system".into(),
                content: ex.system.clone(),
            });
        }
        messages.push(T170 {
            role: "user".into(),
            content: ex.prompt.clone(),
        });
        messages.push(T170 {
            role: "assistant".into(),
            content: ex.response.clone(),
        });
        T169 { messages }
    }).collect()
}

/// T171=ChatMlDpo
/// Convert DPO pairs to ChatML-DPO format (for TRL/unsloth).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct T171 {
    pub prompt: Vec<T170>,
    pub chosen: Vec<T170>,
    pub rejected: Vec<T170>,
}

/// f258=to_chatml_dpo
pub fn f258(pairs: &[T167]) -> Vec<T171> {
    pairs.iter().map(|p| {
        let mut prompt_msgs = Vec::new();
        if !p.system.is_empty() {
            prompt_msgs.push(T170 {
                role: "system".into(),
                content: p.system.clone(),
            });
        }
        prompt_msgs.push(T170 {
            role: "user".into(),
            content: p.prompt.clone(),
        });

        T171 {
            prompt: prompt_msgs,
            chosen: vec![T170 {
                role: "assistant".into(),
                content: p.chosen.clone(),
            }],
            rejected: vec![T170 {
                role: "assistant".into(),
                content: p.rejected.clone(),
            }],
        }
    }).collect()
}

// ── Export to disk ───────────────────────────────────────────────

/// f259=export_training_data
/// Export training data to ~/.kova/micro/training/
pub fn f259(
    result: &T165,
    registry: &T149,
    format: &str,
) -> Result<PathBuf, String> {
    let base = training_dir();
    std::fs::create_dir_all(&base).map_err(|e| e.to_string())?;

    match format {
        "dpo" => {
            let pairs = f255(result, registry);
            if pairs.is_empty() {
                return Err("no DPO pairs found — need both passing and failing responses for same challenges".into());
            }

            // JSONL format (one JSON object per line)
            let path = base.join("dpo_pairs.jsonl");
            let mut lines = Vec::new();
            for p in &pairs {
                lines.push(serde_json::to_string(p).map_err(|e| e.to_string())?);
            }
            std::fs::write(&path, lines.join("\n")).map_err(|e| e.to_string())?;

            // Also export ChatML-DPO format for TRL
            let chatml = f258(&pairs);
            let chatml_path = base.join("dpo_chatml.jsonl");
            let mut chatml_lines = Vec::new();
            for c in &chatml {
                chatml_lines.push(serde_json::to_string(c).map_err(|e| e.to_string())?);
            }
            std::fs::write(&chatml_path, chatml_lines.join("\n")).map_err(|e| e.to_string())?;

            eprintln!("{} DPO pairs exported", pairs.len());
            eprintln!("  {}", path.display());
            eprintln!("  {}", chatml_path.display());
            Ok(path)
        }
        "sft" => {
            let examples = f256(result, registry);
            if examples.is_empty() {
                return Err("no SFT examples found — need passing responses".into());
            }

            // JSONL
            let path = base.join("sft_examples.jsonl");
            let mut lines = Vec::new();
            for ex in &examples {
                lines.push(serde_json::to_string(ex).map_err(|e| e.to_string())?);
            }
            std::fs::write(&path, lines.join("\n")).map_err(|e| e.to_string())?;

            // ChatML format
            let chatml = f257(&examples);
            let chatml_path = base.join("sft_chatml.jsonl");
            let mut chatml_lines = Vec::new();
            for c in &chatml {
                chatml_lines.push(serde_json::to_string(c).map_err(|e| e.to_string())?);
            }
            std::fs::write(&chatml_path, chatml_lines.join("\n")).map_err(|e| e.to_string())?;

            eprintln!("{} SFT examples exported", examples.len());
            eprintln!("  {}", path.display());
            eprintln!("  {}", chatml_path.display());
            Ok(path)
        }
        "all" => {
            // Export both
            let _ = f259(result, registry, "dpo");
            let _ = f259(result, registry, "sft");

            // Summary stats
            let dpo = f255(result, registry);
            let sft = f256(result, registry);
            let summary_path = base.join("summary.json");
            let summary = serde_json::json!({
                "timestamp": result.timestamp,
                "total_matches": result.matches.len(),
                "matches_with_responses": result.matches.iter().filter(|m| !m.response.is_empty()).count(),
                "dpo_pairs": dpo.len(),
                "sft_examples": sft.len(),
                "categories": result.matches.iter().map(|m| m.category.clone()).collect::<std::collections::HashSet<_>>(),
                "models_contributing": result.matches.iter().map(|m| m.competitor.model.clone()).collect::<std::collections::HashSet<_>>(),
            });
            let json = serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())?;
            std::fs::write(&summary_path, json).map_err(|e| e.to_string())?;

            Ok(base)
        }
        _ => Err(format!("unknown format: {} (use dpo, sft, or all)", format)),
    }
}

/// f260=training_stats
/// Stats about exported training data.
pub fn f260(result: &T165, registry: &T149) {
    let dpo = f255(result, registry);
    let sft = f256(result, registry);

    let total = result.matches.len();
    let with_response = result.matches.iter().filter(|m| !m.response.is_empty()).count();
    let passed = result.matches.iter().filter(|m| m.passed).count();
    let failed_with_response = result.matches.iter()
        .filter(|m| !m.passed && !m.response.is_empty() && !m.response.starts_with("ERROR:"))
        .count();

    println!("TRAINING DATA SUMMARY");
    println!("─────────────────────────────────────────────────────────────────");
    println!("  Total matches:          {}", total);
    println!("  With response text:     {}", with_response);
    println!("  Passed (SFT-ready):     {}", passed);
    println!("  Failed (DPO-rejected):  {}", failed_with_response);
    println!("  DPO preference pairs:   {}", dpo.len());
    println!("  SFT examples:           {}", sft.len());
    println!("─────────────────────────────────────────────────────────────────");

    // Per-category breakdown
    let mut cat_stats: HashMap<String, (usize, usize)> = HashMap::new();
    for m in &result.matches {
        let e = cat_stats.entry(m.category.clone()).or_insert((0, 0));
        e.0 += 1;
        if m.passed { e.1 += 1; }
    }

    println!("\n  Per-category:");
    let mut cats: Vec<_> = cat_stats.into_iter().collect();
    cats.sort_by_key(|(k, _)| k.clone());
    for (cat, (total, passed)) in &cats {
        let dpo_cat = dpo.iter().filter(|p| p.category == *cat).count();
        println!("    {:<16} {:>3} matches, {:>3} pass, {:>4} DPO pairs", cat, total, passed, dpo_cat);
    }
}

/// f263=export_classifier_sft
/// Export classifier-format training data from tournament results.
/// Each match becomes {user: challenge_input, assistant: category_label}.
/// This is what candle_train actually needs (bare class labels, not full responses).
pub fn f263(result: &T165, registry: &T149) -> Result<PathBuf, String> {
    let base = training_dir();
    std::fs::create_dir_all(&base).map_err(|e| e.to_string())?;

    let inputs = challenge_inputs(registry);
    let sys = "Classify the input into exactly one category. Reply with only the category name.\nCategories: classify, clippy_fix, code_gen, code_review, explain, fix_compile, test_write, validate";

    let mut lines = Vec::new();

    // Export every match as a classifier example (category is the label)
    for m in &result.matches {
        if m.category.is_empty() { continue; }

        let input_text = if let Some((input, _)) = inputs.get(&m.challenge) {
            input.clone()
        } else {
            // Fall back to challenge description if input not found
            m.challenge.clone()
        };

        if input_text.is_empty() { continue; }

        let entry = serde_json::json!({
            "messages": [
                {"role": "system", "content": sys},
                {"role": "user", "content": input_text},
                {"role": "assistant", "content": m.category}
            ]
        });
        lines.push(serde_json::to_string(&entry).map_err(|e| e.to_string())?);
    }

    // Load existing lines (preserve manually-added data)
    let path = base.join("classifier_sft.jsonl");
    let mut existing: Vec<String> = if path.exists() {
        std::fs::read_to_string(&path).unwrap_or_default()
            .lines().filter(|l| !l.trim().is_empty()).map(|l| l.to_string()).collect()
    } else {
        Vec::new()
    };
    let existing_count = existing.len();

    // Deduplicate: check user content to avoid dupes
    let mut seen: std::collections::HashSet<String> = existing.iter()
        .filter_map(|l| {
            serde_json::from_str::<serde_json::Value>(l).ok()
                .and_then(|v| v["messages"].as_array()
                    .and_then(|msgs| msgs.iter().find(|m| m["role"] == "user"))
                    .and_then(|m| m["content"].as_str().map(|s| s.to_string())))
        })
        .collect();

    let mut added = 0;
    for line in lines {
        // Extract user content for dedup check
        let user_content = serde_json::from_str::<serde_json::Value>(&line).ok()
            .and_then(|v| v["messages"].as_array()
                .and_then(|msgs| msgs.iter().find(|m| m["role"] == "user"))
                .and_then(|m| m["content"].as_str().map(|s| s.to_string())));
        if let Some(uc) = user_content {
            if seen.insert(uc) {
                existing.push(line);
                added += 1;
            }
        }
    }

    std::fs::write(&path, existing.join("\n") + "\n").map_err(|e| e.to_string())?;

    eprintln!("[classifier] {} new + {} existing = {} total in {}", added, existing_count, existing.len(), path.display());

    // Per-category counts
    let mut cats: HashMap<String, usize> = HashMap::new();
    for m in &result.matches {
        if !m.category.is_empty() {
            *cats.entry(m.category.clone()).or_default() += 1;
        }
    }
    let mut cat_list: Vec<_> = cats.into_iter().collect();
    cat_list.sort_by_key(|(k, _)| k.clone());
    for (cat, count) in &cat_list {
        eprintln!("  {}: {}", cat, count);
    }

    Ok(path)
}

fn training_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".kova").join("micro").join("training")
}

/// f261=training_path
/// Path to the training data directory.
pub fn f261() -> PathBuf {
    training_dir()
}

/// f264=mine_classifier_labels
/// Keyword-match mined conversation logs to classifier categories.
/// Appends to classifier_sft.jsonl with dedup.
pub fn f264(examples: &[super::logmine::T146]) -> Result<(PathBuf, usize), String> {
    let sys = "Classify the input into exactly one category. Reply with only the category name.\nCategories: classify, clippy_fix, code_gen, code_review, explain, fix_compile, test_write, validate";

    // Keyword → category mapping
    let rules: &[(&[&str], &str)] = &[
        (&["clippy", "lint", "warning"], "clippy_fix"),
        (&["fix compile", "compile error", "build error", "cannot borrow", "lifetime", "trait.*not implemented", "mismatched types"], "fix_compile"),
        (&["write test", "add test", "unit test", "integration test", "test coverage"], "test_write"),
        (&["explain", "what does", "how does", "why does", "what is"], "explain"),
        (&["review", "check this", "is this correct", "any issues"], "code_review"),
        (&["validate", "verify", "check if", "check that", "confirm"], "validate"),
        (&["classify", "categorize", "what kind", "what type", "triage", "sort this"], "classify"),
        (&["write a function", "implement", "create a", "generate", "add a", "build a"], "code_gen"),
    ];

    let out_path = training_dir().join("classifier_sft.jsonl");
    let existing = if out_path.exists() {
        std::fs::read_to_string(&out_path).unwrap_or_default()
    } else {
        String::new()
    };

    let mut lines: Vec<String> = existing.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.to_string())
        .collect();
    let before = lines.len();

    // Dedup set from existing
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for line in &lines {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
            if let Some(msgs) = v["messages"].as_array() {
                if let Some(user) = msgs.iter().find(|m| m["role"] == "user") {
                    if let Some(c) = user["content"].as_str() {
                        seen.insert(c.to_string());
                    }
                }
            }
        }
    }

    let mut added = 0;
    for ex in examples {
        let text = ex.instruction.to_lowercase();
        // Match first matching rule
        let category = rules.iter().find_map(|(keywords, cat)| {
            if keywords.iter().any(|kw| text.contains(kw)) {
                Some(*cat)
            } else {
                None
            }
        });

        if let Some(cat) = category {
            if seen.insert(ex.instruction.clone()) {
                let entry = serde_json::json!({
                    "messages": [
                        {"role": "system", "content": sys},
                        {"role": "user", "content": ex.instruction},
                        {"role": "assistant", "content": cat}
                    ]
                });
                lines.push(serde_json::to_string(&entry).unwrap());
                added += 1;
            }
        }
    }

    if added > 0 {
        std::fs::create_dir_all(training_dir())
            .map_err(|e| format!("create dir: {}", e))?;
        std::fs::write(&out_path, lines.join("\n") + "\n")
            .map_err(|e| format!("write: {}", e))?;
    }

    eprintln!("[mine-classifier] {} existing + {} new = {} total",
        before, added, lines.len());

    Ok((out_path, added))
}