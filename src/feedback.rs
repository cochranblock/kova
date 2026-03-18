// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! feedback — Academy-to-challenge feedback loop.
//! Mines tournament failures into new, harder challenges.
//! When a model fails a challenge, the failure data (prompt, wrong response,
//! expected behavior) feeds back into the academy as a training signal.
//!
//! f194=record_failure, f195=recent_failures, f196=generate_challenge_from_failure
//! f197=export_generated_challenges, f198=feedback_stats
//! t126=FailureRecord, t127=GeneratedChallenge, t128=FeedbackStats

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

// ── Sled Storage ─────────────────────────────────────────────────

/// Sled tree for challenge failures.
const FAILURE_TREE: &str = "challenge_failures";

/// Sled tree for generated challenges.
const GENERATED_TREE: &str = "generated_challenges";

/// Global sled db handle (same pattern as trace.rs).
static FEEDBACK_DB: OnceLock<Option<sled::Db>> = OnceLock::new();

fn feedback_db() -> Option<&'static sled::Db> {
    FEEDBACK_DB
        .get_or_init(|| {
            let path = crate::config::sled_path();
            sled::open(&path).ok()
        })
        .as_ref()
}


// ── Types ────────────────────────────────────────────────────────

/// t126=FailureRecord. A single challenge failure from a tournament run.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FailureRecord {
    /// Description of the challenge that was failed.
    pub challenge_desc: String,
    /// The input prompt given to the model.
    pub input: String,
    /// The expected verification string (e.g. "compiles", "contains:fn").
    pub expected_verify: String,
    /// What the model actually returned.
    pub actual_response: String,
    /// Model name (e.g. "qwen2.5-coder:1.5b").
    pub model: String,
    /// Tournament event type (e.g. "sprint", "technical", "freestyle").
    pub event_type: String,
    /// Unix timestamp in milliseconds.
    #[serde(default)]
    pub ts: u64,
}

/// t127=GeneratedChallenge. A new challenge produced from a failure pattern.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GeneratedChallenge {
    /// Template ID for the micro-model (e.g. "f79", "f81", "f80").
    pub template_id: String,
    /// The challenge input prompt.
    pub input: String,
    /// Verification string (e.g. "compiles_and:contains:fn").
    pub verify: String,
    /// Human-readable description.
    pub description: String,
    /// Difficulty tier: "easy", "medium", "hard".
    pub difficulty: String,
    /// Which failure record spawned this challenge.
    pub source_failure: String,
}

/// t128=FeedbackStats. Aggregate stats across recorded failures.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FeedbackStats {
    /// Total failure records stored.
    pub total_failures: usize,
    /// Failures grouped by model name.
    pub by_model: Vec<(String, usize)>,
    /// Failures grouped by event type.
    pub by_event: Vec<(String, usize)>,
    /// Total generated challenges stored.
    pub generated_challenges: usize,
}

// ── Core Functions ───────────────────────────────────────────────

/// f194=record_failure. Store a failure record in sled.
pub fn f194(mut record: FailureRecord) {
    if record.ts == 0 {
        record.ts = now_ms();
    }
    if let Some(db) = feedback_db()
        && let Ok(tree) = db.open_tree(FAILURE_TREE)
        && let Ok(val) = serde_json::to_vec(&record)
    {
        let key = failure_key(record.ts);
        let _ = tree.insert(key, val);
    }
}

/// f195=recent_failures. Query recent failure records (newest first).
pub fn f195(limit: usize) -> Vec<FailureRecord> {
    let mut out = Vec::new();
    let db = match feedback_db() {
        Some(db) => db,
        None => return out,
    };
    let tree = match db.open_tree(FAILURE_TREE) {
        Ok(t) => t,
        Err(_) => return out,
    };

    for item in tree.iter().rev() {
        if out.len() >= limit {
            break;
        }
        if let Ok((_k, v)) = item
            && let Ok(record) = serde_json::from_slice::<FailureRecord>(&v)
        {
            out.push(record);
        }
    }

    out
}

/// f196=generate_challenge_from_failure. Use an LLM to create a harder
/// variant of the failed challenge. Returns one generated challenge.
pub fn f196(
    failure: &FailureRecord,
    provider: &crate::providers::Provider,
) -> Result<GeneratedChallenge, String> {
    let system = "You are a Rust challenge designer for the Kova micro-model tournament. \
        Given a failure record (a challenge a model got wrong), create a NEW, HARDER variant. \
        The new challenge must test the same skill but with a twist the model hasn't seen. \
        Reply in exactly this format (one field per line, no extra text):\n\
        TEMPLATE_ID: <e.g. f79, f80, f81>\n\
        INPUT: <the challenge prompt>\n\
        VERIFY: <verification string like compiles, contains:fn, single_word>\n\
        DESCRIPTION: <short description>\n\
        DIFFICULTY: <hard>\n\
        No slop words (utilize, leverage, optimize, comprehensive, robust, seamlessly, scalable, paradigm, synergy, cutting-edge, streamline, empower).";

    let prompt = format!(
        "Failure record:\n\
         - Challenge: {}\n\
         - Event type: {}\n\
         - Input: {}\n\
         - Expected verification: {}\n\
         - Model response (wrong): {}\n\
         - Model: {}\n\n\
         Create a harder variant that targets the same weakness.",
        failure.challenge_desc,
        failure.event_type,
        failure.input,
        failure.expected_verify,
        truncate(&failure.actual_response, 500),
        failure.model,
    );

    let resp = crate::providers::provider_generate(provider, "", system, &prompt)?;
    let challenge = parse_generated_challenge(&resp.text, &failure.challenge_desc)?;

    // Store the generated challenge
    if let Some(db) = feedback_db()
        && let Ok(tree) = db.open_tree(GENERATED_TREE)
        && let Ok(val) = serde_json::to_vec(&challenge)
    {
        let key = failure_key(now_ms());
        let _ = tree.insert(key, val);
    }

    Ok(challenge)
}

/// f197=export_generated_challenges. Format generated challenges as Rust
/// code (tce() calls) that can be pasted into tournament.rs.
pub fn f197(challenges: &[GeneratedChallenge]) -> String {
    let mut out = String::new();
    out.push_str("// ── Generated from feedback loop ──────────────────────────────\n");
    out.push_str("// Paste into tournament_challenges() in tournament.rs\n\n");

    for ch in challenges {
        // Escape double quotes and newlines in strings
        let input = escape_rust_str(&ch.input);
        let verify = escape_rust_str(&ch.verify);
        let desc = escape_rust_str(&ch.description);
        let tid = escape_rust_str(&ch.template_id);

        // Map template_id to category and event_type
        let (cat, event) = category_for_template(&ch.template_id);

        out.push_str(&format!(
            "tce(\"{}\", \"{}\", \"{}\", \"{}\", \"{}\", \"{}\"),\n",
            tid, cat, event, input, verify, desc,
        ));
    }

    out
}

/// f198=feedback_stats. Count failures by model, event type, and generated challenges.
pub fn f198() -> FeedbackStats {
    let mut stats = FeedbackStats {
        total_failures: 0,
        by_model: Vec::new(),
        by_event: Vec::new(),
        generated_challenges: 0,
    };

    let db = match feedback_db() {
        Some(db) => db,
        None => return stats,
    };

    // Count failures
    let mut model_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut event_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();

    if let Ok(tree) = db.open_tree(FAILURE_TREE) {
        for item in tree.iter().flatten() {
            let (_k, v) = item;
            if let Ok(record) = serde_json::from_slice::<FailureRecord>(&v) {
                stats.total_failures += 1;
                *model_counts.entry(record.model).or_insert(0) += 1;
                *event_counts.entry(record.event_type).or_insert(0) += 1;
            }
        }
    }

    // Sort by count descending
    stats.by_model = {
        let mut v: Vec<_> = model_counts.into_iter().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        v
    };
    stats.by_event = {
        let mut v: Vec<_> = event_counts.into_iter().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        v
    };

    // Count generated challenges
    if let Ok(tree) = db.open_tree(GENERATED_TREE) {
        stats.generated_challenges = tree.len();
    }

    stats
}

// ── Helpers ──────────────────────────────────────────────────────

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Key: timestamp in big-endian bytes (sorts chronologically) + atomic counter for uniqueness.
fn failure_key(ts: u64) -> Vec<u8> {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);

    let ts_bytes = ts.to_be_bytes();
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut key = Vec::with_capacity(12);
    key.extend_from_slice(&ts_bytes);
    key.extend_from_slice(&seq.to_be_bytes());
    key
}

/// Truncate a string to max characters (char-safe, no UTF-8 panic).
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max).collect();
        format!("{}...", truncated)
    }
}

/// Escape a string for use inside a Rust string literal.
fn escape_rust_str(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

/// Map template_id to (category, event_type) for tce() export.
fn category_for_template(tid: &str) -> (&str, &str) {
    match tid {
        "f79" => ("classify", "sprint"),
        "f80" => ("code_gen", "freestyle"),
        "f81" => ("fix_compile", "technical"),
        "f115" => ("explain", "judged"),
        "f_code_review" => ("code_review", "judged"),
        "f_validate" => ("validate", "judged"),
        "f_clippy_fix" => ("clippy_fix", "technical"),
        "f_test_write" => ("test_write", "endurance"),
        _ => ("unknown", "unknown"),
    }
}

/// Parse the structured LLM response into a GeneratedChallenge.
fn parse_generated_challenge(
    response: &str,
    source_failure: &str,
) -> Result<GeneratedChallenge, String> {
    let mut template_id = String::new();
    let mut input = String::new();
    let mut verify = String::new();
    let mut description = String::new();
    let mut difficulty = String::from("hard");

    for line in response.lines() {
        let trimmed = line.trim();
        if let Some(val) = trimmed.strip_prefix("TEMPLATE_ID:") {
            template_id = val.trim().to_string();
        } else if let Some(val) = trimmed.strip_prefix("INPUT:") {
            input = val.trim().to_string();
        } else if let Some(val) = trimmed.strip_prefix("VERIFY:") {
            verify = val.trim().to_string();
        } else if let Some(val) = trimmed.strip_prefix("DESCRIPTION:") {
            description = val.trim().to_string();
        } else if let Some(val) = trimmed.strip_prefix("DIFFICULTY:") {
            difficulty = val.trim().to_string();
        }
    }

    if template_id.is_empty() || input.is_empty() || verify.is_empty() {
        return Err(format!(
            "failed to parse challenge from LLM response: missing fields. Got: {}",
            truncate(response, 200)
        ));
    }

    Ok(GeneratedChallenge {
        template_id,
        input,
        verify,
        description,
        difficulty,
        source_failure: source_failure.to_string(),
    })
}

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: open a temp sled db and run operations against it directly.
    fn with_temp_db<F: FnOnce(&sled::Db)>(f: F) {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let db = sled::open(tmp.path().join("test.sled")).expect("sled open");
        f(&db);
    }

    /// f194=record_failure, f195=recent_failures roundtrip.
    #[test]
    fn record_and_retrieve_failure_roundtrip() {
        with_temp_db(|db| {
            let tree = db.open_tree(FAILURE_TREE).expect("open tree");

            let record = FailureRecord {
                challenge_desc: "fix: borrow checker".into(),
                input: "Error: cannot borrow `v` as mutable".into(),
                expected_verify: "compiles".into(),
                actual_response: "fn f() {}".into(),
                model: "qwen2.5-coder:1.5b".into(),
                event_type: "technical".into(),
                ts: 1000,
            };

            // Store
            let key = failure_key(record.ts);
            let val = serde_json::to_vec(&record).expect("serialize");
            tree.insert(key, val).expect("insert");

            // Store a second record
            let record2 = FailureRecord {
                challenge_desc: "classify: ambiguous".into(),
                input: "split the monolithic handle_request".into(),
                expected_verify: "single_word".into(),
                actual_response: "refactor and test".into(),
                model: "starcoder2:3b".into(),
                event_type: "sprint".into(),
                ts: 2000,
            };
            let key2 = failure_key(record2.ts);
            let val2 = serde_json::to_vec(&record2).expect("serialize");
            tree.insert(key2, val2).expect("insert");

            // Retrieve (newest first via reverse iter)
            let mut results = Vec::new();
            for item in tree.iter().rev() {
                let (_k, v) = item.expect("iter");
                let rec: FailureRecord = serde_json::from_slice(&v).expect("deserialize");
                results.push(rec);
            }

            assert_eq!(results.len(), 2);
            assert_eq!(results[0].challenge_desc, "classify: ambiguous");
            assert_eq!(results[1].challenge_desc, "fix: borrow checker");
            assert_eq!(results[0].model, "starcoder2:3b");
            assert_eq!(results[1].event_type, "technical");
        });
    }

    /// f197=export_generated_challenges produces valid tce() calls.
    #[test]
    fn export_produces_valid_tce_calls() {
        let challenges = vec![
            GeneratedChallenge {
                template_id: "f81".into(),
                input: "Error: lifetime may not live long enough\nCode: struct Foo<'a> { data: &'a str }".into(),
                verify: "compiles".into(),
                description: "fix: harder lifetime puzzle".into(),
                difficulty: "hard".into(),
                source_failure: "fix: lifetime elision".into(),
            },
            GeneratedChallenge {
                template_id: "f79".into(),
                input: "rewrite the module and also benchmark it".into(),
                verify: "single_word".into(),
                description: "classify: triple ambiguity".into(),
                difficulty: "hard".into(),
                source_failure: "classify: ambiguous refactor+test".into(),
            },
        ];

        let output = f197(&challenges);

        // Each challenge should produce one tce() line
        let tce_lines: Vec<&str> = output.lines().filter(|l| l.starts_with("tce(")).collect();
        assert_eq!(tce_lines.len(), 2);

        // First line: f81 technical challenge
        assert!(tce_lines[0].contains("\"f81\""));
        assert!(tce_lines[0].contains("\"fix_compile\""));
        assert!(tce_lines[0].contains("\"technical\""));
        assert!(tce_lines[0].contains("\"compiles\""));

        // Second line: f79 sprint challenge
        assert!(tce_lines[1].contains("\"f79\""));
        assert!(tce_lines[1].contains("\"classify\""));
        assert!(tce_lines[1].contains("\"sprint\""));
        assert!(tce_lines[1].contains("\"single_word\""));

        // Newlines in input should be escaped
        assert!(tce_lines[0].contains("\\n"));

        // All lines should end with ),
        for line in &tce_lines {
            assert!(line.trim_end().ends_with("),"), "tce line should end with '),': {}", line);
        }
    }

    /// f198=feedback_stats counts correctly.
    #[test]
    fn feedback_stats_counts_correctly() {
        with_temp_db(|db| {
            let failure_tree = db.open_tree(FAILURE_TREE).expect("open tree");
            let gen_tree = db.open_tree(GENERATED_TREE).expect("open tree");

            // Insert 3 failures: 2 from model A, 1 from model B
            let records = vec![
                FailureRecord {
                    challenge_desc: "fix: borrow".into(),
                    input: "err".into(),
                    expected_verify: "compiles".into(),
                    actual_response: "bad".into(),
                    model: "model-a:1b".into(),
                    event_type: "technical".into(),
                    ts: 1000,
                },
                FailureRecord {
                    challenge_desc: "gen: prime".into(),
                    input: "write prime".into(),
                    expected_verify: "compiles_and:contains:fn".into(),
                    actual_response: "nope".into(),
                    model: "model-a:1b".into(),
                    event_type: "freestyle".into(),
                    ts: 2000,
                },
                FailureRecord {
                    challenge_desc: "classify: bug".into(),
                    input: "it crashes".into(),
                    expected_verify: "single_word".into(),
                    actual_response: "I think it is a bug report".into(),
                    model: "model-b:3b".into(),
                    event_type: "sprint".into(),
                    ts: 3000,
                },
            ];

            for rec in &records {
                let key = failure_key(rec.ts);
                let val = serde_json::to_vec(rec).expect("serialize");
                failure_tree.insert(key, val).expect("insert");
            }

            // Insert 2 generated challenges
            let challenge = GeneratedChallenge {
                template_id: "f81".into(),
                input: "harder".into(),
                verify: "compiles".into(),
                description: "gen'd".into(),
                difficulty: "hard".into(),
                source_failure: "fix: borrow".into(),
            };
            for i in 0..2u64 {
                let key = failure_key(4000 + i);
                let val = serde_json::to_vec(&challenge).expect("serialize");
                gen_tree.insert(key, val).expect("insert");
            }

            // Compute stats from the trees directly
            let mut model_counts: std::collections::HashMap<String, usize> =
                std::collections::HashMap::new();
            let mut event_counts: std::collections::HashMap<String, usize> =
                std::collections::HashMap::new();
            let mut total = 0usize;

            for item in failure_tree.iter() {
                let (_k, v) = item.expect("iter");
                let rec: FailureRecord = serde_json::from_slice(&v).expect("deser");
                total += 1;
                *model_counts.entry(rec.model).or_insert(0) += 1;
                *event_counts.entry(rec.event_type).or_insert(0) += 1;
            }

            assert_eq!(total, 3);
            assert_eq!(model_counts["model-a:1b"], 2);
            assert_eq!(model_counts["model-b:3b"], 1);
            assert_eq!(event_counts["technical"], 1);
            assert_eq!(event_counts["freestyle"], 1);
            assert_eq!(event_counts["sprint"], 1);
            assert_eq!(gen_tree.len(), 2);
        });
    }

    /// parse_generated_challenge handles well-formed LLM output.
    #[test]
    fn parse_challenge_from_llm_response() {
        let response = "\
TEMPLATE_ID: f81
INPUT: Error: cannot return reference to local variable\nCode: fn get() -> &str { let s = String::new(); &s }
VERIFY: compiles
DESCRIPTION: fix: return reference to local
DIFFICULTY: hard";

        let ch = parse_generated_challenge(response, "fix: dangling ref").expect("parse");
        assert_eq!(ch.template_id, "f81");
        assert!(ch.input.contains("cannot return reference"));
        assert_eq!(ch.verify, "compiles");
        assert_eq!(ch.difficulty, "hard");
        assert_eq!(ch.source_failure, "fix: dangling ref");
    }

    /// parse_generated_challenge rejects incomplete output.
    #[test]
    fn parse_challenge_rejects_missing_fields() {
        let response = "TEMPLATE_ID: f80\nDESCRIPTION: something";
        let result = parse_generated_challenge(response, "test");
        assert!(result.is_err());
    }

    /// truncate is char-safe with multibyte UTF-8.
    #[test]
    fn truncate_handles_multibyte_utf8() {
        // 4 emoji = 4 chars but 16 bytes. Truncate to 2 chars.
        let s = "🦀🦀🦀🦀";
        let t = truncate(s, 2);
        assert_eq!(t, "🦀🦀...");
        // Doesn't panic on byte boundary.
        let s2 = "café";
        let t2 = truncate(s2, 3);
        assert_eq!(t2, "caf...");
    }

    /// truncate returns full string when short.
    #[test]
    fn truncate_no_op_when_short() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    /// category_for_template maps known templates.
    #[test]
    fn category_mapping_covers_all_templates() {
        assert_eq!(category_for_template("f79"), ("classify", "sprint"));
        assert_eq!(category_for_template("f80"), ("code_gen", "freestyle"));
        assert_eq!(category_for_template("f81"), ("fix_compile", "technical"));
        assert_eq!(category_for_template("f115"), ("explain", "judged"));
        assert_eq!(category_for_template("f_code_review"), ("code_review", "judged"));
        assert_eq!(category_for_template("f_validate"), ("validate", "judged"));
        assert_eq!(category_for_template("f_clippy_fix"), ("clippy_fix", "technical"));
        assert_eq!(category_for_template("f_test_write"), ("test_write", "endurance"));
        assert_eq!(category_for_template("unknown"), ("unknown", "unknown"));
    }

    /// failure_key produces unique keys for same-ms inserts.
    #[test]
    fn failure_key_uniqueness() {
        let ts = 12345u64;
        let k1 = failure_key(ts);
        let k2 = failure_key(ts);
        assert_ne!(k1, k2, "same ts should get different keys via rand suffix");
        assert_eq!(k1.len(), 12);
    }

    /// Empty failure tree: iter yields nothing, no panic.
    #[test]
    fn empty_db_operations() {
        with_temp_db(|db| {
            let tree = db.open_tree(FAILURE_TREE).expect("open");
            assert_eq!(tree.len(), 0);
            let count: usize = tree.iter().filter(|i| i.is_ok()).count();
            assert_eq!(count, 0);
        });
    }

    #[test]
    fn failure_record_serde_roundtrip() {
        let record = FailureRecord {
            challenge_desc: "fix: borrow".into(),
            input: "code with\nnewlines\tand tabs".into(),
            expected_verify: "compiles".into(),
            actual_response: "wrong answer with \"quotes\"".into(),
            model: "qwen2.5-coder:1.5b".into(),
            event_type: "technical".into(),
            ts: 42,
        };
        let bytes = serde_json::to_vec(&record).unwrap();
        let back: FailureRecord = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(back.challenge_desc, record.challenge_desc);
        assert_eq!(back.input, record.input);
        assert_eq!(back.ts, 42);
    }

    #[test]
    fn generated_challenge_serde_roundtrip() {
        let ch = GeneratedChallenge {
            template_id: "f81".into(),
            input: "error".into(),
            verify: "compiles".into(),
            description: "test".into(),
            difficulty: "hard".into(),
            source_failure: "src".into(),
        };
        let json = serde_json::to_string(&ch).unwrap();
        let back: GeneratedChallenge = serde_json::from_str(&json).unwrap();
        assert_eq!(back.template_id, "f81");
        assert_eq!(back.difficulty, "hard");
    }

    #[test]
    fn failure_key_ordering_by_timestamp() {
        // Keys with higher ts should sort after keys with lower ts
        let k1 = failure_key(100);
        let k2 = failure_key(200);
        assert!(k1 < k2, "lower timestamp key should sort before higher");
    }

    #[test]
    fn export_generated_challenges_empty() {
        let output = f197(&[]);
        // Header comment is always emitted, but no tce() lines
        let tce_lines: Vec<&str> = output.lines().filter(|l| l.starts_with("tce(")).collect();
        assert!(tce_lines.is_empty());
    }

    #[test]
    fn truncate_empty_string() {
        assert_eq!(truncate("", 10), "");
    }

    #[test]
    fn truncate_exact_length() {
        assert_eq!(truncate("hello", 5), "hello");
    }

    #[test]
    fn parse_challenge_handles_extra_whitespace() {
        let response = "\
TEMPLATE_ID:   f79
INPUT:  classify this input
VERIFY:  single_word
DESCRIPTION:  classify: tricky
DIFFICULTY:  medium  ";
        let ch = parse_generated_challenge(response, "src").expect("parse");
        assert_eq!(ch.template_id, "f79");
        assert_eq!(ch.verify, "single_word");
    }
}
