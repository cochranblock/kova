//! cases — CaseHarvester CSV ingest. Anne Arundel County family law cases.
//!
//! Source: exports.mdcaseexplorer.com/cc.csv.gz
//! Strategy: streaming HTTP → gzip decompress → CSV line-by-line → filter AA County family → sled

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use crate::legal::store::{self, CourtCase};
use std::io::{BufRead, BufReader};

const CSV_URL: &str = "https://exports.mdcaseexplorer.com/cc.csv.gz";

// Anne Arundel family law case type codes
const FAMILY_TYPES: &[&str] = &["D", "DV", "FC", "FP", "DR", "FL"];

/// Ingest CaseHarvester circuit court CSV for Anne Arundel County.
pub fn ingest(db: &sled::Db) -> anyhow::Result<u64> {
    println!("  [cases] fetching {}", CSV_URL);

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()?;

    let response = client.get(CSV_URL).send()?;
    if !response.status().is_success() {
        anyhow::bail!("HTTP {} from {}", response.status(), CSV_URL);
    }

    let etag = response.headers()
        .get("etag")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // Check if we already have this version
    if let Some(meta) = store::get_latest_meta(db, "caseharvester")? {
        if meta.last_etag == etag && etag.is_some() {
            println!("  [cases] no changes (etag match), skipping");
            return Ok(meta.records_ingested);
        }
    }

    // Stream decompress gzip
    let decoder = flate2::read::GzDecoder::new(response);
    let reader = BufReader::new(decoder);

    let mut count: u64 = 0;
    let mut header_map: Option<Vec<String>> = None;
    let mut skipped: u64 = 0;

    for (line_num, line_result) in reader.lines().enumerate() {
        let line = match line_result {
            Ok(l) => l,
            Err(_) => { skipped += 1; continue; }
        };

        // Parse header row
        if line_num == 0 {
            header_map = Some(line.split(',').map(|s| s.trim().to_lowercase()).collect());
            continue;
        }

        let headers = match &header_map {
            Some(h) => h,
            None => continue,
        };

        // Parse CSV fields (basic — handles most cases, not quoted commas)
        let fields: Vec<&str> = line.split(',').collect();
        if fields.len() < headers.len() {
            skipped += 1;
            continue;
        }

        let get = |name: &str| -> String {
            headers.iter().position(|h| h == name)
                .and_then(|i| fields.get(i))
                .map(|s| s.trim().trim_matches('"').to_string())
                .unwrap_or_default()
        };

        // Filter: Anne Arundel County only
        let county = get("court_system");
        if !county.to_lowercase().contains("anne arundel") {
            continue;
        }

        // Filter: family law case types
        let case_type = get("case_type");
        if !FAMILY_TYPES.iter().any(|&t| case_type.eq_ignore_ascii_case(t)) {
            continue;
        }

        let case = CourtCase {
            case_number: get("case_number"),
            filing_date: get("filing_date"),
            case_type,
            case_sub_type: get("case_sub_type"),
            judge: get("judge"),
            disposition: {
                let d = get("disposition");
                if d.is_empty() { None } else { Some(d) }
            },
            disposition_date: {
                let d = get("disposition_date");
                if d.is_empty() { None } else { Some(d) }
            },
            plaintiff: get("plaintiff"),
            defendant: get("defendant"),
            county: "Anne Arundel".into(),
        };

        let key = format!("AA:{}", case.case_number);
        store::put(db, store::TREE_CASES, &key, &case)?;
        count += 1;

        if count % 1000 == 0 {
            print!("\r  [cases] {} AA family records ingested, {} skipped", count, skipped);
        }
    }

    println!("\r  [cases] {} AA family records ingested, {} skipped", count, skipped);

    super::ingest::record_ingest(db, "caseharvester", count, etag)?;
    Ok(count)
}
