//! training_data — Export scored LLM interactions as training datasets (DPO/SFT).
//! f181=f181, f182=f182, f183=f183, f184=f184
//! t116=T116, t117=T117
// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use crate::trace::{self, T109};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// t116=T116. Single scored LLM interaction for fine-tuning.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct T116 {
    pub prompt: String,
    pub response: String,
    pub model: String,
    pub score: f32,
    pub passed: bool,
    pub latency_ms: u64,
    pub category: String,
}

/// t117=T117. Output format for training data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum T117 {
    Jsonl,
    Csv,
    Dpo,
}

impl T117 {
    pub fn f316(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "jsonl" => Some(Self::Jsonl),
            "csv" => Some(Self::Csv),
            "dpo" => Some(Self::Dpo),
            _ => None,
        }
    }

    pub fn extension(&self) -> &str {
        match self {
            Self::Jsonl | Self::Dpo => "jsonl",
            Self::Csv => "csv",
        }
    }
}

/// DPO pair: same prompt, chosen (higher score) vs rejected (lower score).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct T175 {
    pub prompt: String,
    pub chosen: String,
    pub rejected: String,
    pub chosen_model: String,
    pub rejected_model: String,
    pub chosen_score: f32,
    pub rejected_score: f32,
    pub category: String,
}

/// Default output directory: ~/.kova/training_data/
pub fn f318() -> PathBuf {
    crate::config::kova_dir().join("training_data")
}

/// Convert an T109 to a T116. Scores traces by status and latency.
fn trace_to_example(trace: &T109) -> T116 {
    let passed = trace.status == "ok";
    // Score: 1.0 for success, 0.0 for failure. Penalize slow responses.
    let base_score: f32 = if passed { 1.0 } else { 0.0 };
    let latency_penalty = if trace.latency_ms > 10_000 {
        0.2
    } else if trace.latency_ms > 5_000 {
        0.1
    } else {
        0.0
    };
    let score = (base_score - latency_penalty).max(0.0);

    // Derive category from call_type + backend.
    let category = format!("{}:{}", trace.backend, trace.call_type);

    // Prompt/response are stored as byte lengths in traces. Reconstruct placeholder
    // text showing the size since the actual content isn't persisted in T109.
    let prompt = format!(
        "[{}B prompt via {} on {}]",
        trace.prompt_bytes, trace.model, trace.node
    );
    let response = format!(
        "[{}B response, {}ms, status={}]",
        trace.response_bytes, trace.latency_ms, trace.status
    );

    T116 {
        prompt,
        response,
        model: trace.model.clone(),
        score,
        passed,
        latency_ms: trace.latency_ms,
        category,
    }
}

/// f181=f181. Read LLM traces from sled, convert to training examples,
/// export in the given format. Returns count of examples written.
pub fn f181(format: T117, output: Option<PathBuf>) -> anyhow::Result<usize> {
    let traces = trace::f162(usize::MAX);
    if traces.is_empty() {
        anyhow::bail!("no LLM traces found — run some inference first");
    }

    let examples: Vec<T116> = traces.iter().map(trace_to_example).collect();

    let out_dir = output
        .clone()
        .map(|p| {
            if p.extension().is_some() {
                p.parent().unwrap_or(Path::new(".")).to_path_buf()
            } else {
                p.clone()
            }
        })
        .unwrap_or_else(f318);
    std::fs::create_dir_all(&out_dir)?;

    let default_filename = match format {
        T117::Jsonl => "training.jsonl",
        T117::Csv => "training.csv",
        T117::Dpo => "dpo_pairs.jsonl",
    };

    let out_path = match output {
        Some(ref p) if p.extension().is_some() => p.clone(),
        Some(ref p) => p.join(default_filename),
        None => out_dir.join(default_filename),
    };

    let count = match format {
        T117::Jsonl => f182(&examples, &out_path)?,
        T117::Csv => f183(&examples, &out_path)?,
        T117::Dpo => f184(&examples, &out_path)?,
    };

    println!("Exported {} entries to {}", count, out_path.display());
    Ok(count)
}

/// f182=f182. Write training examples as JSONL (one JSON object per line).
/// Standard format for SFT fine-tuning pipelines.
pub fn f182(examples: &[T116], path: &Path) -> anyhow::Result<usize> {
    use std::io::Write;
    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
    let mut count = 0;
    for ex in examples {
        serde_json::to_writer(&mut f, ex)?;
        writeln!(f)?;
        count += 1;
    }
    f.flush()?;
    Ok(count)
}

/// f183=f183. Write training examples as CSV with headers.
pub fn f183(examples: &[T116], path: &Path) -> anyhow::Result<usize> {
    use std::io::Write;
    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
    writeln!(f, "prompt,response,model,score,passed,latency_ms,category")?;
    let mut count = 0;
    for ex in examples {
        writeln!(
            f,
            "{},{},{},{:.4},{},{},{}",
            csv_escape(&ex.prompt),
            csv_escape(&ex.response),
            csv_escape(&ex.model),
            ex.score,
            ex.passed,
            ex.latency_ms,
            csv_escape(&ex.category),
        )?;
        count += 1;
    }
    f.flush()?;
    Ok(count)
}

/// f184=f184. Build DPO training pairs: for each prompt that has both
/// a higher-scored and lower-scored response, emit (prompt, chosen, rejected).
/// Groups by category, pairs best vs worst within each group.
pub fn f184(examples: &[T116], path: &Path) -> anyhow::Result<usize> {
    use std::io::Write;

    // Group examples by category.
    let mut by_category: HashMap<String, Vec<&T116>> = HashMap::new();
    for ex in examples {
        by_category.entry(ex.category.clone()).or_default().push(ex);
    }

    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
    let mut count = 0;

    for (category, mut group) in by_category {
        // Sort by score descending.
        group.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        // Pair each high-scoring example with each lower-scoring one.
        // Only pair if there's a meaningful score difference.
        for i in 0..group.len() {
            for j in (i + 1)..group.len() {
                let chosen = group[i];
                let rejected = group[j];
                if (chosen.score - rejected.score).abs() < 0.01 {
                    continue;
                }
                let pair = T175 {
                    prompt: chosen.prompt.clone(),
                    chosen: chosen.response.clone(),
                    rejected: rejected.response.clone(),
                    chosen_model: chosen.model.clone(),
                    rejected_model: rejected.model.clone(),
                    chosen_score: chosen.score,
                    rejected_score: rejected.score,
                    category: category.clone(),
                };
                serde_json::to_writer(&mut f, &pair)?;
                writeln!(f)?;
                count += 1;
            }
        }
    }

    f.flush()?;
    Ok(count)
}

/// Escape a field for CSV: wrap in quotes if it contains comma, quote, CR, or newline.
fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sample_examples() -> Vec<T116> {
        vec![
            T116 {
                prompt: "Write a hello world function".into(),
                response: "fn hello() { println!(\"Hello, world!\"); }".into(),
                model: "qwen2.5-coder:1.5b".into(),
                score: 1.0,
                passed: true,
                latency_ms: 500,
                category: "ollama:generate".into(),
            },
            T116 {
                prompt: "Write a hello world function".into(),
                response: "ERROR: timeout".into(),
                model: "qwen2.5-coder:0.5b".into(),
                score: 0.0,
                passed: false,
                latency_ms: 15000,
                category: "ollama:generate".into(),
            },
            T116 {
                prompt: "Classify this intent".into(),
                response: "build".into(),
                model: "qwen2.5-coder:1.5b".into(),
                score: 0.9,
                passed: true,
                latency_ms: 6000,
                category: "ollama:chat".into(),
            },
        ]
    }

    /// f182=f182. Verify JSONL output: one valid JSON object per line.
    #[test]
    fn export_jsonl_writes_valid_jsonl() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.jsonl");
        let examples = sample_examples();

        let count = f182(&examples, &path).unwrap();
        assert_eq!(count, 3);

        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 3);

        // Each line must parse as a valid T116.
        for line in &lines {
            let ex: T116 = serde_json::from_str(line).unwrap();
            assert!(!ex.model.is_empty());
        }
    }

    /// f183=f183. Verify CSV output: header row + data rows.
    #[test]
    fn export_csv_writes_valid_csv_with_headers() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.csv");
        let examples = sample_examples();

        let count = f183(&examples, &path).unwrap();
        assert_eq!(count, 3);

        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        // Header + 3 data rows.
        assert_eq!(lines.len(), 4);
        assert!(lines[0].starts_with("prompt,response,model,score,passed,latency_ms,category"));

        // Verify data rows contain expected model names.
        assert!(content.contains("qwen2.5-coder:1.5b"));
        assert!(content.contains("qwen2.5-coder:0.5b"));
    }

    /// f184=f184. Verify DPO pairs: chosen has higher score than rejected.
    #[test]
    fn export_dpo_pairs_creates_correct_pairs() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("dpo.jsonl");
        let examples = sample_examples();

        let count = f184(&examples, &path).unwrap();
        // "ollama:generate" category has score 1.0 vs 0.0 → 1 pair.
        // "ollama:chat" category has only 1 example → 0 pairs.
        assert_eq!(count, 1);

        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 1);

        let pair: T175 = serde_json::from_str(lines[0]).unwrap();
        assert!(pair.chosen_score > pair.rejected_score);
        assert_eq!(pair.category, "ollama:generate");
        assert_eq!(pair.chosen_model, "qwen2.5-coder:1.5b");
        assert_eq!(pair.rejected_model, "qwen2.5-coder:0.5b");
    }

    /// Verify csv_escape handles special characters.
    #[test]
    fn csv_escape_handles_special_chars() {
        assert_eq!(csv_escape("simple"), "simple");
        assert_eq!(csv_escape("has,comma"), "\"has,comma\"");
        assert_eq!(csv_escape("has\"quote"), "\"has\"\"quote\"");
        assert_eq!(csv_escape("has\nnewline"), "\"has\nnewline\"");
        assert_eq!(csv_escape("has\rcarriage"), "\"has\rcarriage\"");
    }

    /// T117 f316 handles case and unknown.
    #[test]
    fn export_format_from_str() {
        assert_eq!(T117::f316("jsonl"), Some(T117::Jsonl));
        assert_eq!(T117::f316("CSV"), Some(T117::Csv));
        assert_eq!(T117::f316("DPO"), Some(T117::Dpo));
        assert_eq!(T117::f316("nope"), None);
    }

    /// T117 extensions.
    #[test]
    fn export_format_extensions() {
        assert_eq!(T117::Jsonl.extension(), "jsonl");
        assert_eq!(T117::Csv.extension(), "csv");
        assert_eq!(T117::Dpo.extension(), "jsonl");
    }
}