//! court — Anne Arundel Circuit Court info scraper. Judges, family division, local rules.
//!
//! Source: circuitcourt.org

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use crate::legal::store::{self, JudgeProfile};

const JUDGES_URL: &str = "https://www.circuitcourt.org/about-us/judges";

/// Ingest AA Circuit Court judge info.
pub fn ingest(db: &sled::Db) -> anyhow::Result<u64> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let mut count: u64 = 0;

    // Try to scrape the judges page
    println!("  [court] fetching judge roster from {}", JUDGES_URL);
    let html = match client.get(JUDGES_URL).send() {
        Ok(resp) if resp.status().is_success() => resp.text()?,
        _ => {
            println!("  [court] could not reach circuitcourt.org, using known roster");
            seed_known_judges(db)?;
            return Ok(11);
        }
    };

    // Extract judge names from HTML
    let mut found_names: Vec<String> = Vec::new();
    for prefix in &["Judge ", "Hon. ", "Hon "] {
        for chunk in html.split(prefix) {
            // Take the next few words as the name
            let words: Vec<&str> = chunk.split_whitespace().take(4).collect();
            if words.len() >= 2 {
                let name = words[..words.len().min(3)].join(" ");
                // Basic validation: starts with uppercase
                if name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
                    && name.len() > 5
                    && !found_names.contains(&name)
                {
                    found_names.push(name);
                }
            }
        }
    }

    if found_names.is_empty() {
        println!("  [court] no judges extracted from HTML, using known roster");
        count = seed_known_judges(db)?;
    } else {
        for name in &found_names {
            let profile = JudgeProfile {
                name: name.clone(),
                division: detect_division(&html, name),
                case_types: vec!["Family".into(), "Civil".into()],
            };

            let key = format!("AA:{}", name);
            store::put(db, store::TREE_JUDGES, &key, &profile)?;
            count += 1;
        }
        println!("  [court] {} judges ingested from website", count);
    }

    super::ingest::record_ingest(db, "court_info", count, None)?;
    Ok(count)
}

fn detect_division(html: &str, judge_name: &str) -> String {
    // Check if the judge's name appears near "Family" or "Civil" keywords
    let lower = html.to_lowercase();
    let name_lower = judge_name.to_lowercase();
    if let Some(pos) = lower.find(&name_lower) {
        let context = &lower[pos.saturating_sub(200)..lower.len().min(pos + 200)];
        if context.contains("family") {
            return "Family".into();
        }
        if context.contains("criminal") {
            return "Criminal".into();
        }
    }
    "General".into()
}

/// Seed with known Anne Arundel Circuit Court judges.
fn seed_known_judges(db: &sled::Db) -> anyhow::Result<u64> {
    let judges = [
        ("Pamela K. Alban", "Family", vec!["Family", "Civil"]),
        ("Thomas F. Casey", "General", vec!["Criminal", "Civil"]),
        ("Christine M. Celeste", "General", vec!["Civil"]),
        ("Mark W. Crooks", "General", vec!["Criminal"]),
        ("Ginina A. Jackson-Stevenson", "General", vec!["Civil", "Family"]),
        ("Michael E. Malone", "General", vec!["Criminal"]),
        ("Stacy W. McCormack", "General", vec!["Civil", "Family"]),
        ("Elizabeth S. Morris", "General", vec!["Civil"]),
        ("Robert J. Thompson", "General", vec!["Criminal", "Civil"]),
        ("Richard R. Trunnell", "General", vec!["Criminal"]),
        ("Cathleen M. Vitale", "General", vec!["Civil", "Family"]),
    ];

    let mut count: u64 = 0;
    for (name, division, types) in &judges {
        let profile = JudgeProfile {
            name: name.to_string(),
            division: division.to_string(),
            case_types: types.iter().map(|t| t.to_string()).collect(),
        };
        store::put(db, store::TREE_JUDGES, &format!("AA:{}", name), &profile)?;
        count += 1;
    }

    println!("  [court] seeded {} known judges", count);
    Ok(count)
}
