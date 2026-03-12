// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6
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

use std::collections::HashMap;
use std::path::PathBuf;

use super::tournament::{TournamentResult, MatchResult};
use super::registry::MicroRegistry;

// ── DPO Pair ────────────────────────────────────────────────────

/// A single DPO preference pair.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DpoPair {
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

/// A supervised fine-tuning example.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SftExample {
    pub system: String,
    pub prompt: String,
    pub response: String,
    pub category: String,
    pub model: String,
}

/// ChatML formatted example (for MLX/unsloth).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatMlExample {
    pub messages: Vec<ChatMlMessage>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatMlMessage {
    pub role: String,
    pub content: String,
}

// ── Export Functions ─────────────────────────────────────────────

/// Build challenge-to-input lookup from registry.
fn challenge_inputs(registry: &MicroRegistry) -> HashMap<String, (String, String)> {
    let challenges = super::tournament::get_challenges(registry);
    let mut map = HashMap::new();
    for ch in challenges {
        // key = challenge description (matches MatchResult.challenge)
        let sys = registry.get(&ch.template_id)
            .map(|t| t.system_prompt.clone())
            .unwrap_or_default();
        map.insert(ch.description.clone(), (ch.input.clone(), sys));
    }
    map
}

/// Extract DPO preference pairs from tournament results.
///
/// For each challenge, pairs every passing response with every failing response.
/// More pairs = more signal. Filters out error responses (no useful rejected signal).
pub fn extract_dpo_pairs(result: &TournamentResult, registry: &MicroRegistry) -> Vec<DpoPair> {
    let inputs = challenge_inputs(registry);
    let mut pairs = Vec::new();

    // Group matches by challenge description
    let mut by_challenge: HashMap<String, Vec<&MatchResult>> = HashMap::new();
    for m in &result.matches {
        by_challenge.entry(m.challenge.clone()).or_default().push(m);
    }

    for (challenge, matches) in &by_challenge {
        let (input, system) = match inputs.get(challenge) {
            Some(v) => v.clone(),
            None => continue,
        };

        let passed: Vec<&&MatchResult> = matches.iter()
            .filter(|m| m.passed && !m.response.is_empty())
            .collect();
        let failed: Vec<&&MatchResult> = matches.iter()
            .filter(|m| !m.passed && !m.response.is_empty() && !m.response.starts_with("ERROR:"))
            .collect();

        // Cross-product: every passing response paired with every failing response
        for p in &passed {
            for f in &failed {
                pairs.push(DpoPair {
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

/// Extract supervised fine-tuning examples (passing responses only).
pub fn extract_sft(result: &TournamentResult, registry: &MicroRegistry) -> Vec<SftExample> {
    let inputs = challenge_inputs(registry);
    let mut examples = Vec::new();

    for m in &result.matches {
        if !m.passed || m.response.is_empty() { continue; }

        let (input, system) = match inputs.get(&m.challenge) {
            Some(v) => v.clone(),
            None => continue,
        };

        examples.push(SftExample {
            system,
            prompt: input,
            response: m.response.clone(),
            category: m.category.clone(),
            model: m.competitor.model.clone(),
        });
    }

    examples
}

/// Convert SFT examples to ChatML format.
pub fn to_chatml(examples: &[SftExample]) -> Vec<ChatMlExample> {
    examples.iter().map(|ex| {
        let mut messages = Vec::new();
        if !ex.system.is_empty() {
            messages.push(ChatMlMessage {
                role: "system".into(),
                content: ex.system.clone(),
            });
        }
        messages.push(ChatMlMessage {
            role: "user".into(),
            content: ex.prompt.clone(),
        });
        messages.push(ChatMlMessage {
            role: "assistant".into(),
            content: ex.response.clone(),
        });
        ChatMlExample { messages }
    }).collect()
}

/// Convert DPO pairs to ChatML-DPO format (for TRL/unsloth).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatMlDpo {
    pub prompt: Vec<ChatMlMessage>,
    pub chosen: Vec<ChatMlMessage>,
    pub rejected: Vec<ChatMlMessage>,
}

pub fn to_chatml_dpo(pairs: &[DpoPair]) -> Vec<ChatMlDpo> {
    pairs.iter().map(|p| {
        let mut prompt_msgs = Vec::new();
        if !p.system.is_empty() {
            prompt_msgs.push(ChatMlMessage {
                role: "system".into(),
                content: p.system.clone(),
            });
        }
        prompt_msgs.push(ChatMlMessage {
            role: "user".into(),
            content: p.prompt.clone(),
        });

        ChatMlDpo {
            prompt: prompt_msgs,
            chosen: vec![ChatMlMessage {
                role: "assistant".into(),
                content: p.chosen.clone(),
            }],
            rejected: vec![ChatMlMessage {
                role: "assistant".into(),
                content: p.rejected.clone(),
            }],
        }
    }).collect()
}

// ── Export to disk ───────────────────────────────────────────────

/// Export training data to ~/.kova/micro/training/
pub fn export_training_data(
    result: &TournamentResult,
    registry: &MicroRegistry,
    format: &str,
) -> Result<PathBuf, String> {
    let base = training_dir();
    std::fs::create_dir_all(&base).map_err(|e| e.to_string())?;

    match format {
        "dpo" => {
            let pairs = extract_dpo_pairs(result, registry);
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
            let chatml = to_chatml_dpo(&pairs);
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
            let examples = extract_sft(result, registry);
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
            let chatml = to_chatml(&examples);
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
            let _ = export_training_data(result, registry, "dpo");
            let _ = export_training_data(result, registry, "sft");

            // Summary stats
            let dpo = extract_dpo_pairs(result, registry);
            let sft = extract_sft(result, registry);
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

/// Stats about exported training data.
pub fn training_stats(result: &TournamentResult, registry: &MicroRegistry) {
    let dpo = extract_dpo_pairs(result, registry);
    let sft = extract_sft(result, registry);

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

fn training_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".kova").join("micro").join("training")
}

/// Path to the training data directory.
pub fn training_path() -> PathBuf {
    training_dir()
}
