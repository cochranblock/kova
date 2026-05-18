// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6

//! compiler_teacher — Stage 3 Flywheel. Captures (bad, error, good) training pairs
//! from Sponge Mesh corrections. Every mesh retry that succeeds = one training pair.
//!
//! Storage: redb at ~/.kova/training/compiler_pairs.redb
//! Key: blake3 hash of the error. Value: bincode+zstd serialized CompilerPair.
//!
//! The flywheel: corrections → training pairs → better experts → fewer corrections.

use redb::{Database, ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::LazyLock;

use super::ExpertKind;

const PAIRS_TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("compiler_pairs");

/// A single training pair from a Sponge Mesh correction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompilerPair {
    pub bad_code: String,
    pub error: String,
    pub good_code: String,
    pub expert: String,
    pub timestamp: u64,
}

/// Hint from past failures — used by experts to avoid repeating mistakes.
pub struct CompilerHint {
    pub error: String,
    pub good_code: String,
}

static DB: LazyLock<Option<Database>> = LazyLock::new(|| {
    let path = training_db_path();
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            eprintln!("[compiler_teacher] cannot create {}: {e}", parent.display());
        }
    }
    Database::create(&path)
        .map_err(|e| eprintln!("[compiler_teacher] redb open failed: {e}"))
        .ok()
});

fn training_db_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".kova/training/compiler_pairs.redb")
}

/// Save a (bad_code, error, good_code) training pair after a successful mesh retry.
pub fn save_pair(bad_code: &str, error: &str, good_code: &str, expert: &ExpertKind) {
    let Some(db) = DB.as_ref() else { return };

    let pair = CompilerPair {
        bad_code: bad_code.to_string(),
        error: error.to_string(),
        good_code: good_code.to_string(),
        expert: format!("{:?}", expert),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    };

    let key = blake3::hash(error.as_bytes());
    let Ok(encoded) = bincode::serde::encode_to_vec(&pair, bincode::config::standard()) else { return };
    let Ok(compressed) = zstd::encode_all(encoded.as_slice(), 3) else { return };

    let Ok(txn) = db.begin_write() else { return };
    {
        let Ok(mut table) = txn.open_table(PAIRS_TABLE) else { return };
        if let Err(e) = table.insert(key.as_bytes().as_slice(), compressed.as_slice()) {
            eprintln!("[compiler_teacher] save failed: {e}");
            return;
        }
    }
    let _ = txn.commit();
}

/// Look up a past failure hint for an expert type. Returns the most recent match.
pub fn lookup_hint(expert: &ExpertKind) -> Option<CompilerHint> {
    let db = DB.as_ref()?;
    let expert_str = format!("{:?}", expert);
    let txn = db.begin_read().ok()?;
    let table = txn.open_table(PAIRS_TABLE).ok()?;

    let mut best: Option<CompilerPair> = None;
    for item in table.iter().ok()? {
        let (_, v) = item.ok()?;
        let decompressed = zstd::decode_all(v.value()).unwrap_or_else(|_| v.value().to_vec());
        let Ok((pair, _)) = bincode::serde::decode_from_slice::<CompilerPair, _>(
            &decompressed,
            bincode::config::standard(),
        ) else {
            continue;
        };
        if pair.expert == expert_str
            && best.as_ref().map_or(true, |b: &CompilerPair| pair.timestamp > b.timestamp)
        {
            best = Some(pair);
        }
    }

    best.map(|p| CompilerHint { error: p.error, good_code: p.good_code })
}

/// Read all pairs. Used by `kova train-data`.
pub fn all_pairs() -> Vec<CompilerPair> {
    let Some(db) = DB.as_ref() else { return Vec::new() };
    let Ok(txn) = db.begin_read() else { return Vec::new() };
    let table = match txn.open_table(PAIRS_TABLE) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };
    let Ok(iter) = table.iter() else { return Vec::new() };

    let mut pairs = Vec::new();
    for item in iter {
        let Ok((_, v)) = item else { continue };
        let decompressed = zstd::decode_all(v.value()).unwrap_or_else(|_| v.value().to_vec());
        if let Ok((pair, _)) = bincode::serde::decode_from_slice::<CompilerPair, _>(
            &decompressed,
            bincode::config::standard(),
        ) {
            pairs.push(pair);
        }
    }
    pairs
}

/// Print training data stats and JSONL output.
pub fn dump_training_data() {
    let pairs = all_pairs();
    if pairs.is_empty() {
        println!("No training pairs collected yet.");
        return;
    }

    let mut by_expert: std::collections::HashMap<&str, usize> = Default::default();
    let mut error_counts: std::collections::HashMap<String, usize> = Default::default();

    for pair in &pairs {
        *by_expert.entry(&pair.expert).or_default() += 1;
        let first_line = pair.error.lines().next().unwrap_or("unknown");
        *error_counts.entry(first_line.to_string()).or_default() += 1;
    }

    eprintln!("=== Training Data Stats ===");
    eprintln!("Total pairs: {}", pairs.len());
    eprintln!("\nPer-expert:");
    let mut expert_vec: Vec<_> = by_expert.iter().collect();
    expert_vec.sort_by(|a, b| b.1.cmp(a.1));
    for (expert, count) in &expert_vec {
        eprintln!("  {}: {}", expert, count);
    }
    eprintln!("\nMost common errors:");
    let mut error_vec: Vec<_> = error_counts.iter().collect();
    error_vec.sort_by(|a, b| b.1.cmp(a.1));
    for (err, count) in error_vec.iter().take(10) {
        eprintln!("  [{}] {}", count, err);
    }

    eprintln!("\n=== JSONL Output ===");
    for pair in &pairs {
        let json = serde_json::json!({
            "bad_code": pair.bad_code,
            "error": pair.error,
            "good_code": pair.good_code,
            "expert": pair.expert,
            "timestamp": pair.timestamp,
        });
        println!("{}", json);
    }
}
