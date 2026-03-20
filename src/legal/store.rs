//! store — sled tree schema, types, and query helpers for legal data.

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use serde::{Deserialize, Serialize};

// ── Tree names ──

pub const TREE_CASES: &str = "legal_cases";
pub const TREE_SCHEDULE: &str = "legal_schedule";
pub const TREE_COMPLAINTS: &str = "legal_complaints";
pub const TREE_OPINIONS: &str = "legal_opinions";
pub const TREE_STATS: &str = "legal_stats";
pub const TREE_JUDGES: &str = "legal_judges";
pub const TREE_META: &str = "legal_meta";

// ── Types ──

/// Court case from CaseHarvester CSV export.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CourtCase {
    pub case_number: String,
    pub filing_date: String,
    pub case_type: String,
    pub case_sub_type: String,
    pub judge: String,
    pub disposition: Option<String>,
    pub disposition_date: Option<String>,
    pub plaintiff: String,
    pub defendant: String,
    pub county: String,
}

/// Court hearing from CaseHarvester schedule CSV.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CourtHearing {
    pub case_number: String,
    pub hearing_date: String,
    pub hearing_type: String,
    pub judge: String,
    pub courtroom: String,
}

/// MSDE special education complaint letter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplaintLetter {
    pub complaint_id: String,
    pub fiscal_year: String,
    pub school_system: String,
    pub filing_date: String,
    pub violations_found: Vec<String>,
    pub corrective_actions: Vec<String>,
    pub full_text: String,
    pub pdf_url: String,
}

/// MD appellate court opinion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppellateOpinion {
    pub case_name: String,
    pub citation: String,
    pub date: String,
    pub judges: Vec<String>,
    pub holding: String,
    pub full_text: String,
    pub categories: Vec<String>,
    pub pdf_url: String,
}

/// Court statistics per county/type/year.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CourtStats {
    pub county: String,
    pub case_type: String,
    pub year: u16,
    pub cases_filed: u32,
    pub cases_disposed: u32,
    pub median_days_to_disposition: Option<u32>,
}

/// Judge profile from circuit court website.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgeProfile {
    pub name: String,
    pub division: String,
    pub case_types: Vec<String>,
}

/// Ingest tracking metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestMeta {
    pub source: String,
    pub last_run: u64,
    pub last_etag: Option<String>,
    pub last_modified: Option<String>,
    pub records_ingested: u64,
    pub batch_id: String,
}

// ── Sled helpers ──

/// Put a value into a named sled tree with bincode + zstd compression.
pub fn put<V: Serialize>(db: &sled::Db, tree_name: &str, key: &str, value: &V) -> anyhow::Result<()> {
    let tree = db.open_tree(tree_name)?;
    let encoded = bincode::serde::encode_to_vec(value, bincode::config::standard())?;
    let compressed = zstd::encode_all(encoded.as_slice(), 3)?;
    tree.insert(key.as_bytes(), compressed)?;
    Ok(())
}

/// Get a value from a named sled tree.
pub fn get<V: for<'de> Deserialize<'de>>(db: &sled::Db, tree_name: &str, key: &str) -> anyhow::Result<Option<V>> {
    let tree = db.open_tree(tree_name)?;
    match tree.get(key.as_bytes())? {
        Some(bytes) => {
            let decompressed = zstd::decode_all(bytes.as_ref())?;
            let (value, _) = bincode::serde::decode_from_slice(&decompressed, bincode::config::standard())?;
            Ok(Some(value))
        }
        None => Ok(None),
    }
}

/// Scan all entries in a tree with a key prefix.
pub fn scan_prefix<V: for<'de> Deserialize<'de>>(db: &sled::Db, tree_name: &str, prefix: &str) -> anyhow::Result<Vec<(String, V)>> {
    let tree = db.open_tree(tree_name)?;
    let mut results = Vec::new();
    for item in tree.scan_prefix(prefix.as_bytes()) {
        let (key_bytes, val_bytes) = item?;
        let key = String::from_utf8_lossy(&key_bytes).to_string();
        let decompressed = zstd::decode_all(val_bytes.as_ref())?;
        let (value, _): (V, _) = bincode::serde::decode_from_slice(&decompressed, bincode::config::standard())?;
        results.push((key, value));
    }
    Ok(results)
}

/// Count entries in a tree.
pub fn count(db: &sled::Db, tree_name: &str) -> anyhow::Result<usize> {
    let tree = db.open_tree(tree_name)?;
    Ok(tree.len())
}

// ── Query helpers for experts ──

/// Get all cases for a specific judge.
pub fn query_judge_cases(db: &sled::Db, judge_name: &str) -> anyhow::Result<Vec<CourtCase>> {
    let all: Vec<(String, CourtCase)> = scan_prefix(db, TREE_CASES, "AA:")?;
    Ok(all.into_iter()
        .map(|(_, c)| c)
        .filter(|c| c.judge.to_lowercase().contains(&judge_name.to_lowercase()))
        .collect())
}

/// Get all family law cases.
pub fn query_family_cases(db: &sled::Db) -> anyhow::Result<Vec<CourtCase>> {
    let all: Vec<(String, CourtCase)> = scan_prefix(db, TREE_CASES, "AA:")?;
    Ok(all.into_iter()
        .map(|(_, c)| c)
        .filter(|c| matches!(c.case_type.as_str(), "D" | "DV" | "FC" | "FP"))
        .collect())
}

/// Get all AACPS complaints.
pub fn query_complaints(db: &sled::Db) -> anyhow::Result<Vec<ComplaintLetter>> {
    let all: Vec<(String, ComplaintLetter)> = scan_prefix(db, TREE_COMPLAINTS, "AACPS:")?;
    Ok(all.into_iter().map(|(_, c)| c).collect())
}

/// Get appellate opinions matching a keyword.
pub fn query_opinions(db: &sled::Db, keyword: &str) -> anyhow::Result<Vec<AppellateOpinion>> {
    let all: Vec<(String, AppellateOpinion)> = scan_prefix(db, TREE_OPINIONS, "")?;
    let kw = keyword.to_lowercase();
    Ok(all.into_iter()
        .map(|(_, o)| o)
        .filter(|o| {
            o.full_text.to_lowercase().contains(&kw)
                || o.holding.to_lowercase().contains(&kw)
                || o.case_name.to_lowercase().contains(&kw)
                || o.categories.iter().any(|c| c.to_lowercase().contains(&kw))
        })
        .collect())
}

/// Get judge profile.
pub fn query_judge_profile(db: &sled::Db, name: &str) -> anyhow::Result<Option<JudgeProfile>> {
    get(db, TREE_JUDGES, &format!("AA:{}", name))
}

/// Get latest ingest meta for a source.
pub fn get_latest_meta(db: &sled::Db, source: &str) -> anyhow::Result<Option<IngestMeta>> {
    let metas: Vec<(String, IngestMeta)> = scan_prefix(db, TREE_META, &format!("{}:", source))?;
    Ok(metas.into_iter().map(|(_, m)| m).max_by_key(|m| m.last_run))
}
