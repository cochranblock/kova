//! feedback — Academy-to-challenge feedback loop.
//! Mines tournament failures into new, harder challenges.
//!
//! f194=record_failure, f195=recent_failures, f196=generate_challenge_from_failure
//! f197=export_generated_challenges, f198=feedback_stats
//! t126=T126, t127=T127, t128=T128
// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use redb::{ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

const FAILURE_TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("challenge_failures");
const GENERATED_TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("generated_challenges");

// ── Types ────────────────────────────────────────────────────────

/// t126=T126. A single challenge failure from a tournament run.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct T126 {
    pub challenge_desc: String,
    pub input: String,
    pub expected_verify: String,
    pub actual_response: String,
    pub model: String,
    pub event_type: String,
    #[serde(default)]
    pub ts: u64,
}

/// t127=T127. A new challenge produced from a failure pattern.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct T127 {
    pub template_id: String,
    pub input: String,
    pub verify: String,
    pub description: String,
    pub difficulty: String,
    pub source_failure: String,
}

/// t128=T128. Aggregate stats across recorded failures.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct T128 {
    pub total_failures: usize,
    pub by_model: Vec<(String, usize)>,
    pub by_event: Vec<(String, usize)>,
    pub generated_challenges: usize,
}

// ── Public API ───────────────────────────────────────────────────

/// f194=record_failure. Persist a challenge failure.
pub fn f194(record: T126) {
    let Some(db) = crate::storage::global_db() else { return };
    let Ok(val) = serde_json::to_vec(&record) else { return };
    let key = failure_key(record.ts);
    let Ok(txn) = db.begin_write() else { return };
    {
        let Ok(mut table) = txn.open_table(FAILURE_TABLE) else { return };
        let _ = table.insert(key.as_slice(), val.as_slice());
    }
    let _ = txn.commit();
}

/// f195=recent_failures. Query recent failure records (newest first).
pub fn f195(limit: usize) -> Vec<T126> {
    let Some(db) = crate::storage::global_db() else { return Vec::new() };
    let Ok(txn) = db.begin_read() else { return Vec::new() };
    let table = match txn.open_table(FAILURE_TABLE) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };
    let Ok(iter) = table.iter() else { return Vec::new() };
    let mut out = Vec::new();
    for item in iter.rev() {
        if out.len() >= limit { break; }
        if let Ok((_, v)) = item
            && let Ok(record) = serde_json::from_slice::<T126>(v.value())
        {
            out.push(record);
        }
    }
    out
}

/// f196=generate_challenge_from_failure. Use an LLM to create a harder variant.
pub fn f196(
    failure: &T126,
    provider: &crate::providers::T129,
) -> Result<T127, String> {
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

    let resp = crate::providers::f199(provider, "", system, &prompt)?;
    let challenge = parse_generated_challenge(&resp.text, &failure.challenge_desc)?;

    if let Some(db) = crate::storage::global_db()
        && let Ok(val) = serde_json::to_vec(&challenge)
        && let Ok(txn) = db.begin_write()
    {
        {
            if let Ok(mut table) = txn.open_table(GENERATED_TABLE) {
                let key = failure_key(now_ms());
                let _ = table.insert(key.as_slice(), val.as_slice());
            }
        }
        let _ = txn.commit();
    }

    Ok(challenge)
}

/// f197=export_generated_challenges. Format as Rust tce() calls.
pub fn f197(challenges: &[T127]) -> String {
    let mut out = String::new();
    out.push_str("// ── Generated from feedback loop ──────────────────────────────\n");
    out.push_str("// Paste into tournament_challenges() in tournament.rs\n\n");
    for ch in challenges {
        let (cat, event) = category_for_template(&ch.template_id);
        out.push_str(&format!(
            "tce(\"{}\", \"{}\", \"{}\", \"{}\", \"{}\", \"{}\"),\n",
            escape_rust_str(&ch.template_id),
            cat,
            event,
            escape_rust_str(&ch.input),
            escape_rust_str(&ch.verify),
            escape_rust_str(&ch.description),
        ));
    }
    out
}

/// f198=feedback_stats.
pub fn f198() -> T128 {
    let mut stats = T128::default();
    let Some(db) = crate::storage::global_db() else { return stats };
    let Ok(txn) = db.begin_read() else { return stats };

    let mut model_counts: std::collections::HashMap<String, usize> = Default::default();
    let mut event_counts: std::collections::HashMap<String, usize> = Default::default();

    if let Ok(table) = txn.open_table(FAILURE_TABLE)
        && let Ok(iter) = table.iter()
    {
        for item in iter.flatten() {
            let (_, v) = item;
            if let Ok(record) = serde_json::from_slice::<T126>(v.value()) {
                stats.total_failures += 1;
                *model_counts.entry(record.model).or_default() += 1;
                *event_counts.entry(record.event_type).or_default() += 1;
            }
        }
    }

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

    if let Ok(table) = txn.open_table(GENERATED_TABLE)
        && let Ok(iter) = table.iter()
    {
        stats.generated_challenges = iter.count();
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

fn failure_key(ts: u64) -> Vec<u8> {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut key = Vec::with_capacity(12);
    key.extend_from_slice(&ts.to_be_bytes());
    key.extend_from_slice(&seq.to_be_bytes());
    key
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max).collect();
        format!("{}...", t)
    }
}

fn escape_rust_str(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

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

fn parse_generated_challenge(response: &str, source_failure: &str) -> Result<T127, String> {
    let mut template_id = String::new();
    let mut input = String::new();
    let mut verify = String::new();
    let mut description = String::new();
    let mut difficulty = String::from("hard");

    for line in response.lines() {
        let t = line.trim();
        if let Some(v) = t.strip_prefix("TEMPLATE_ID:") { template_id = v.trim().to_string(); }
        else if let Some(v) = t.strip_prefix("INPUT:") { input = v.trim().to_string(); }
        else if let Some(v) = t.strip_prefix("VERIFY:") { verify = v.trim().to_string(); }
        else if let Some(v) = t.strip_prefix("DESCRIPTION:") { description = v.trim().to_string(); }
        else if let Some(v) = t.strip_prefix("DIFFICULTY:") { difficulty = v.trim().to_string(); }
    }

    if template_id.is_empty() || input.is_empty() || verify.is_empty() {
        return Err(format!(
            "failed to parse challenge: missing fields. Got: {}",
            truncate(response, 200)
        ));
    }

    Ok(T127 { template_id, input, verify, description, difficulty, source_failure: source_failure.to_string() })
}

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use redb::{Database, ReadableTable};

    fn with_temp_db<F: FnOnce(&Database)>(f: F) {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let db = Database::create(tmp.path().join("test.redb")).expect("redb open");
        f(&db);
    }

    fn db_insert(db: &Database, table: TableDefinition<&[u8], &[u8]>, key: &[u8], val: &[u8]) {
        let txn = db.begin_write().unwrap();
        { let mut t = txn.open_table(table).unwrap(); t.insert(key, val).unwrap(); }
        txn.commit().unwrap();
    }

    fn db_count(db: &Database, table: TableDefinition<&[u8], &[u8]>) -> usize {
        let txn = db.begin_read().unwrap();
        match txn.open_table(table) {
            Ok(t) => t.iter().unwrap().count(),
            Err(_) => 0,
        }
    }

    #[test]
    fn record_and_retrieve_failure_roundtrip() {
        with_temp_db(|db| {
            let record = T126 {
                challenge_desc: "fix: borrow checker".into(),
                input: "Error: cannot borrow `v` as mutable".into(),
                expected_verify: "compiles".into(),
                actual_response: "fn f() {}".into(),
                model: "custom-model:local".into(),
                event_type: "technical".into(),
                ts: 1000,
            };
            let record2 = T126 {
                challenge_desc: "classify: ambiguous".into(),
                input: "split the monolithic handle_request".into(),
                expected_verify: "single_word".into(),
                actual_response: "refactor and test".into(),
                model: "starcoder2:3b".into(),
                event_type: "sprint".into(),
                ts: 2000,
            };

            let k1 = failure_key(record.ts);
            let k2 = failure_key(record2.ts);
            db_insert(db, FAILURE_TABLE, &k1, &serde_json::to_vec(&record).unwrap());
            db_insert(db, FAILURE_TABLE, &k2, &serde_json::to_vec(&record2).unwrap());

            // Retrieve newest-first via reverse iteration.
            let txn = db.begin_read().unwrap();
            let table = txn.open_table(FAILURE_TABLE).unwrap();
            let mut results: Vec<T126> = table
                .iter().unwrap().rev()
                .map(|e| serde_json::from_slice::<T126>(e.unwrap().1.value()).unwrap())
                .collect();

            assert_eq!(results.len(), 2);
            assert_eq!(results[0].challenge_desc, "classify: ambiguous");
            assert_eq!(results[1].challenge_desc, "fix: borrow checker");
            assert_eq!(results[0].model, "starcoder2:3b");
            assert_eq!(results[1].event_type, "technical");
        });
    }

    #[test]
    fn export_produces_valid_tce_calls() {
        let challenges = vec![
            T127 {
                template_id: "f81".into(),
                input: "Error: lifetime may not live long enough\nCode: struct Foo<'a> { data: &'a str }".into(),
                verify: "compiles".into(),
                description: "fix: harder lifetime puzzle".into(),
                difficulty: "hard".into(),
                source_failure: "fix: lifetime elision".into(),
            },
            T127 {
                template_id: "f79".into(),
                input: "rewrite the module and also benchmark it".into(),
                verify: "single_word".into(),
                description: "classify: triple ambiguity".into(),
                difficulty: "hard".into(),
                source_failure: "classify: ambiguous refactor+test".into(),
            },
        ];

        let output = f197(&challenges);
        let tce_lines: Vec<&str> = output.lines().filter(|l| l.starts_with("tce(")).collect();
        assert_eq!(tce_lines.len(), 2);
        assert!(tce_lines[0].contains("\"f81\""));
        assert!(tce_lines[0].contains("\"fix_compile\""));
        assert!(tce_lines[0].contains("\"technical\""));
        assert!(tce_lines[0].contains("\"compiles\""));
        assert!(tce_lines[1].contains("\"f79\""));
        assert!(tce_lines[1].contains("\"classify\""));
        assert!(tce_lines[1].contains("\"sprint\""));
        assert!(tce_lines[1].contains("\"single_word\""));
        assert!(tce_lines[0].contains("\\n"));
        for line in &tce_lines {
            assert!(line.trim_end().ends_with("),"), "tce line should end with '),': {}", line);
        }
    }

    #[test]
    fn feedback_stats_counts_correctly() {
        with_temp_db(|db| {
            let records = vec![
                T126 { challenge_desc: "fix: borrow".into(), input: "err".into(), expected_verify: "compiles".into(), actual_response: "bad".into(), model: "model-a:1b".into(), event_type: "technical".into(), ts: 1000 },
                T126 { challenge_desc: "gen: prime".into(), input: "write prime".into(), expected_verify: "compiles_and:contains:fn".into(), actual_response: "nope".into(), model: "model-a:1b".into(), event_type: "freestyle".into(), ts: 2000 },
                T126 { challenge_desc: "classify: bug".into(), input: "it crashes".into(), expected_verify: "single_word".into(), actual_response: "I think it is a bug report".into(), model: "model-b:3b".into(), event_type: "sprint".into(), ts: 3000 },
            ];
            for rec in &records {
                db_insert(db, FAILURE_TABLE, &failure_key(rec.ts), &serde_json::to_vec(rec).unwrap());
            }
            let challenge = T127 { template_id: "f81".into(), input: "harder".into(), verify: "compiles".into(), description: "gen'd".into(), difficulty: "hard".into(), source_failure: "fix: borrow".into() };
            for i in 0..2u64 {
                db_insert(db, GENERATED_TABLE, &failure_key(4000 + i), &serde_json::to_vec(&challenge).unwrap());
            }

            let txn = db.begin_read().unwrap();
            let failure_table = txn.open_table(FAILURE_TABLE).unwrap();
            let mut model_counts: std::collections::HashMap<String, usize> = Default::default();
            let mut event_counts: std::collections::HashMap<String, usize> = Default::default();
            let mut total = 0usize;
            for item in failure_table.iter().unwrap() {
                let (_, v) = item.unwrap();
                let rec: T126 = serde_json::from_slice(v.value()).unwrap();
                total += 1;
                *model_counts.entry(rec.model).or_default() += 1;
                *event_counts.entry(rec.event_type).or_default() += 1;
            }
            assert_eq!(total, 3);
            assert_eq!(model_counts["model-a:1b"], 2);
            assert_eq!(model_counts["model-b:3b"], 1);
            assert_eq!(event_counts["technical"], 1);
            assert_eq!(event_counts["freestyle"], 1);
            assert_eq!(event_counts["sprint"], 1);
            assert_eq!(db_count(db, GENERATED_TABLE), 2);
        });
    }

    #[test]
    fn parse_challenge_from_llm_response() {
        let response = "TEMPLATE_ID: f81\nINPUT: Error: cannot return reference to local variable\nVERIFY: compiles\nDESCRIPTION: fix: return reference to local\nDIFFICULTY: hard";
        let ch = parse_generated_challenge(response, "fix: dangling ref").expect("parse");
        assert_eq!(ch.template_id, "f81");
        assert!(ch.input.contains("cannot return reference"));
        assert_eq!(ch.verify, "compiles");
        assert_eq!(ch.difficulty, "hard");
        assert_eq!(ch.source_failure, "fix: dangling ref");
    }

    #[test]
    fn parse_challenge_rejects_missing_fields() {
        let result = parse_generated_challenge("TEMPLATE_ID: f80\nDESCRIPTION: something", "test");
        assert!(result.is_err());
    }

    #[test]
    fn truncate_handles_multibyte_utf8() {
        assert_eq!(truncate("🦀🦀🦀🦀", 2), "🦀🦀...");
        assert_eq!(truncate("café", 3), "caf...");
    }

    #[test]
    fn truncate_no_op_when_short() {
        assert_eq!(truncate("hello", 10), "hello");
    }

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

    #[test]
    fn failure_key_uniqueness() {
        let k1 = failure_key(12345);
        let k2 = failure_key(12345);
        assert_ne!(k1, k2);
        assert_eq!(k1.len(), 12);
    }

    #[test]
    fn empty_db_operations() {
        with_temp_db(|db| {
            assert_eq!(db_count(db, FAILURE_TABLE), 0);
        });
    }

    #[test]
    fn failure_record_serde_roundtrip() {
        let record = T126 {
            challenge_desc: "fix: borrow".into(),
            input: "code with\nnewlines\tand tabs".into(),
            expected_verify: "compiles".into(),
            actual_response: "wrong answer with \"quotes\"".into(),
            model: "custom-model:local".into(),
            event_type: "technical".into(),
            ts: 42,
        };
        let bytes = serde_json::to_vec(&record).unwrap();
        let back: T126 = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(back.challenge_desc, record.challenge_desc);
        assert_eq!(back.input, record.input);
        assert_eq!(back.ts, 42);
    }

    #[test]
    fn generated_challenge_serde_roundtrip() {
        let ch = T127 { template_id: "f81".into(), input: "error".into(), verify: "compiles".into(), description: "test".into(), difficulty: "hard".into(), source_failure: "src".into() };
        let json = serde_json::to_string(&ch).unwrap();
        let back: T127 = serde_json::from_str(&json).unwrap();
        assert_eq!(back.template_id, "f81");
        assert_eq!(back.difficulty, "hard");
    }

    #[test]
    fn failure_key_ordering_by_timestamp() {
        let k1 = failure_key(100);
        let k2 = failure_key(200);
        assert!(k1 < k2, "lower timestamp key should sort before higher");
    }

    #[test]
    fn export_generated_challenges_empty() {
        let output = f197(&[]);
        let tce_lines: Vec<&str> = output.lines().filter(|l| l.starts_with("tce(")).collect();
        assert!(tce_lines.is_empty());
    }
}
