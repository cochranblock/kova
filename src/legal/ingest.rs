//! ingest — shared ingest infrastructure, CLI dispatch, IngestMeta tracking.

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use crate::legal::store::{self, IngestMeta};
use std::time::{SystemTime, UNIX_EPOCH};

/// Record a completed ingest run.
pub fn record_ingest(db: &sled::Db, source: &str, records: u64, etag: Option<String>) -> anyhow::Result<()> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let batch_id = format!("{}", now);

    let meta = IngestMeta {
        source: source.into(),
        last_run: now,
        last_etag: etag,
        last_modified: None,
        records_ingested: records,
        batch_id: batch_id.clone(),
    };

    store::put(db, store::TREE_META, &format!("{}:{}", source, batch_id), &meta)?;
    Ok(())
}

/// Print ingest status for all sources.
pub fn print_status(db: &sled::Db) -> anyhow::Result<()> {
    let sources = ["caseharvester", "msde_complaints", "appellate", "stats", "court_info"];

    println!("[legal] Ingest Status");
    println!("{:20} {:>8} {:>12} {}", "Source", "Records", "Last Run", "Batch ID");
    println!("{}", "-".repeat(65));

    for source in &sources {
        match store::get_latest_meta(db, source)? {
            Some(meta) => {
                let ago = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|n| n.as_secs() - meta.last_run)
                    .unwrap_or(0);
                let ago_str = if ago < 3600 {
                    format!("{}m ago", ago / 60)
                } else if ago < 86400 {
                    format!("{}h ago", ago / 3600)
                } else {
                    format!("{}d ago", ago / 86400)
                };
                println!("{:20} {:>8} {:>12} {}", source, meta.records_ingested, ago_str, meta.batch_id);
            }
            None => {
                println!("{:20} {:>8} {:>12} {}", source, "-", "never", "-");
            }
        }
    }

    // Tree counts
    println!();
    println!("[legal] Sled Tree Counts");
    let trees = [
        ("legal_cases", "Court cases"),
        ("legal_schedule", "Hearings"),
        ("legal_complaints", "Complaints"),
        ("legal_opinions", "Opinions"),
        ("legal_stats", "Stats"),
        ("legal_judges", "Judges"),
    ];

    for (tree, label) in &trees {
        let count = store::count(db, tree)?;
        println!("  {:20} {}", label, count);
    }

    Ok(())
}

/// Run all ingest sources.
pub fn ingest_all(db: &sled::Db) -> anyhow::Result<()> {
    println!("[legal] Running all ingest sources...\n");

    println!("[1/5] CaseHarvester (court cases)...");
    match super::cases::ingest(db) {
        Ok(n) => println!("       {} records\n", n),
        Err(e) => println!("       error: {}\n", e),
    }

    println!("[2/5] MSDE Complaints...");
    match super::complaints::ingest(db) {
        Ok(n) => println!("       {} records\n", n),
        Err(e) => println!("       error: {}\n", e),
    }

    println!("[3/5] Appellate Opinions...");
    match super::opinions::ingest(db) {
        Ok(n) => println!("       {} records\n", n),
        Err(e) => println!("       error: {}\n", e),
    }

    println!("[4/5] Dashboard Stats...");
    match super::stats::ingest(db) {
        Ok(n) => println!("       {} records\n", n),
        Err(e) => println!("       error: {}\n", e),
    }

    println!("[5/5] Court Info...");
    match super::court::ingest(db) {
        Ok(n) => println!("       {} records\n", n),
        Err(e) => println!("       error: {}\n", e),
    }

    println!("[legal] Ingest complete.");
    print_status(db)?;
    Ok(())
}
