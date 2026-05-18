//! Priority queue for subatomic models backed by redb.
//!
//! Key format: `pq/{score_u32_big_endian_inverted}:{model_name}`
//! Using inverted score in the key means lower byte values = higher priority,
//! so redb's natural B-tree ascending order iterates hot models first.
//!
//! f430=bump, f431=decay_all, f432=top_n, f433=score_of, f434=reset.
// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6

use redb::TableDefinition;

const PQ_TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("pq");

/// Score ceiling — model score is clamped to this.
const MAX_SCORE: u32 = 100_000;
/// How much a bump raises a model's score.
const BUMP_AMOUNT: u32 = 1_000;
/// Multiplier applied per decay tick (0.95 = 5% decay).
const DECAY_FACTOR: f64 = 0.95;

fn score_key(score: u32, name: &str) -> Vec<u8> {
    // Invert score so highest-priority model sorts first in B-tree.
    let inv = MAX_SCORE.saturating_sub(score);
    let mut key = b"pq/".to_vec();
    key.extend_from_slice(&inv.to_be_bytes());
    key.push(b':');
    key.extend_from_slice(name.as_bytes());
    key
}

fn parse_key(key: &[u8]) -> Option<(u32, String)> {
    // "pq/" (3) + 4 bytes score + ':' (1) + name
    if key.len() < 9 || &key[..3] != b"pq/" {
        return None;
    }
    let inv_bytes: [u8; 4] = key[3..7].try_into().ok()?;
    let inv = u32::from_be_bytes(inv_bytes);
    let score = MAX_SCORE.saturating_sub(inv);
    if key[7] != b':' {
        return None;
    }
    let name = std::str::from_utf8(&key[8..]).ok()?.to_string();
    Some((score, name))
}

fn db() -> Option<std::sync::Arc<redb::Database>> {
    crate::storage::global_db()
}

/// f430=bump. Raise `model`'s priority score by BUMP_AMOUNT. Creates entry if absent.
/// Called by the intent classifier whenever a model is relevant to current input.
pub fn f430(model: &str) {
    let Some(db) = db() else { return };
    let current = f433(model).unwrap_or(0);
    let new_score = (current + BUMP_AMOUNT).min(MAX_SCORE);
    let _ = write_score(&db, model, current, new_score);
}

/// f431=decay_all. Apply DECAY_FACTOR to every model's score.
/// Call periodically (e.g. each REPL turn) to let cold models sink.
pub fn f431() {
    let Some(db) = db() else { return };
    let entries = match read_all(&db) {
        Ok(e) => e,
        Err(_) => return,
    };
    for (score, name) in entries {
        let new_score = ((score as f64) * DECAY_FACTOR) as u32;
        let _ = write_score(&db, &name, score, new_score);
    }
}

/// f432=top_n. Return the `n` highest-priority model names in order.
pub fn f432(n: usize) -> Vec<String> {
    let Some(db) = db() else { return vec![] };
    match read_all(&db) {
        Ok(entries) => entries.into_iter().take(n).map(|(_, name)| name).collect(),
        Err(_) => vec![],
    }
}

/// f433=score_of. Current priority score for `model`, or None if not tracked.
pub fn f433(model: &str) -> Option<u32> {
    let db = db()?;
    let txn = db.begin_read().ok()?;
    let table = txn.open_table(PQ_TABLE).ok()?;
    // Scan for any key with this model name (score part may vary).
    let prefix = "pq/";
    let prefix_bytes = prefix.as_bytes();
    let mut end = prefix_bytes.to_vec();
    if let Some(last) = end.last_mut() { *last = last.wrapping_add(1); }

    table
        .range(prefix_bytes..end.as_slice())
        .ok()?
        .filter_map(|r| r.ok())
        .find_map(|(k, _v)| {
            let (score, name) = parse_key(k.value())?;
            if name == model { Some(score) } else { None }
        })
}

/// f434=reset. Remove a model from the priority queue entirely.
pub fn f434(model: &str) {
    let Some(db) = db() else { return };
    let current = match f433(model) {
        Some(s) => s,
        None => return,
    };
    let old_key = score_key(current, model);
    let Ok(txn) = db.begin_write() else { return };
    {
        let Ok(mut table) = txn.open_table(PQ_TABLE) else { return };
        let _ = table.remove(old_key.as_slice());
    }
    let _ = txn.commit();
}

/// Bump a model's intent-driven priority based on classification confidence.
/// Higher confidence = bigger bump. Called from intent classifier hook.
pub fn bump_with_conf(model: &str, confidence: f32) {
    let Some(db) = db() else { return };
    let current = f433(model).unwrap_or(0);
    let bump = ((BUMP_AMOUNT as f32) * confidence.clamp(0.0, 1.0)) as u32;
    let new_score = (current + bump).min(MAX_SCORE);
    let _ = write_score(&db, model, current, new_score);
}

// ── internals ────────────────────────────────────────────────────────────────

fn write_score(
    db: &redb::Database,
    name: &str,
    old_score: u32,
    new_score: u32,
) -> Result<(), String> {
    let old_key = score_key(old_score, name);
    let new_key = score_key(new_score, name);
    let txn = db.begin_write().map_err(|e| e.to_string())?;
    {
        let mut table = txn.open_table(PQ_TABLE).map_err(|e| e.to_string())?;
        // Remove old key (different score = different key bytes).
        if old_score != new_score {
            let _ = table.remove(old_key.as_slice());
        }
        // Write new key with empty value (score is encoded in the key).
        table.insert(new_key.as_slice(), b"".as_slice()).map_err(|e| e.to_string())?;
    }
    txn.commit().map_err(|e| e.to_string())?;
    Ok(())
}

fn read_all(db: &redb::Database) -> Result<Vec<(u32, String)>, String> {
    let txn = db.begin_read().map_err(|e| e.to_string())?;
    let table = match txn.open_table(PQ_TABLE) {
        Ok(t) => t,
        Err(_) => return Ok(vec![]),
    };
    let prefix = b"pq/";
    let mut end = prefix.to_vec();
    if let Some(last) = end.last_mut() { *last = last.wrapping_add(1); }

    let entries: Vec<(u32, String)> = table
        .range(prefix.as_slice()..end.as_slice())
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .filter_map(|(k, _v)| parse_key(k.value()))
        .collect();

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_key_roundtrip() {
        for score in [0u32, 1, 999, 50_000, 100_000] {
            let key = score_key(score, "slop_detector");
            let (got_score, got_name) = parse_key(&key).unwrap();
            assert_eq!(got_score, score, "score mismatch at {score}");
            assert_eq!(got_name, "slop_detector");
        }
    }

    #[test]
    fn higher_score_sorts_first() {
        let key_hot = score_key(80_000, "hot_model");
        let key_cold = score_key(100, "cold_model");
        // Lower key bytes = sorted first in B-tree = hot model comes first.
        assert!(key_hot < key_cold, "hot model must sort before cold model");
    }

    #[test]
    fn parse_key_rejects_bad_input() {
        assert!(parse_key(b"bad").is_none());
        assert!(parse_key(b"pq/tooshort").is_none());
        assert!(parse_key(b"other/key").is_none());
    }

    #[test]
    fn bump_decay_score_of_roundtrip() {
        // Uses a temporary isolated DB (doesn't touch the global DB).
        let tmp = tempfile::TempDir::new().unwrap();
        let db = redb::Database::create(tmp.path().join("pq_test.redb")).unwrap();
        let db = std::sync::Arc::new(db);

        // Manually test write_score + read_all.
        write_score(&db, "model_a", 0, 5_000).unwrap();
        write_score(&db, "model_b", 0, 3_000).unwrap();
        write_score(&db, "model_c", 0, 8_000).unwrap();

        let entries = read_all(&db).unwrap();
        assert_eq!(entries.len(), 3);
        // Should be sorted highest → lowest.
        assert_eq!(entries[0].0, 8_000);
        assert_eq!(entries[0].1, "model_c");
        assert_eq!(entries[1].0, 5_000);
        assert_eq!(entries[2].0, 3_000);
    }

    #[test]
    fn bump_overwrites_old_key() {
        let tmp = tempfile::TempDir::new().unwrap();
        let db = std::sync::Arc::new(
            redb::Database::create(tmp.path().join("pq_test2.redb")).unwrap()
        );

        // Write score 1000, then bump to 2000 — old key must be gone.
        write_score(&db, "model_x", 0, 1_000).unwrap();
        write_score(&db, "model_x", 1_000, 2_000).unwrap();

        let entries = read_all(&db).unwrap();
        assert_eq!(entries.len(), 1, "old key must be removed on bump");
        assert_eq!(entries[0].0, 2_000);
    }

    #[test]
    fn decay_reduces_score() {
        let orig = 10_000u32;
        let decayed = ((orig as f64) * DECAY_FACTOR) as u32;
        assert!(decayed < orig);
        assert!(decayed > 9_000); // 5% decay, not catastrophic
    }
}
