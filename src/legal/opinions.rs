//! opinions — MD appellate family law opinion scraper.
//!
//! Source: mdcourts.gov/opinions
//! Strategy: search by family law keywords → scrape result listings → download opinion PDFs

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use crate::legal::store::{self, AppellateOpinion};

const SEARCH_BASE: &str = "https://www.mdcourts.gov/cgi-bin/indexlist.pl";

const SEARCH_KEYWORDS: &[&str] = &[
    "custody modification",
    "best interest child",
    "parental alienation",
    "material change circumstances",
    "special education custody",
];

/// Ingest MD appellate family law opinions.
pub fn ingest(db: &sled::Db) -> anyhow::Result<u64> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()?;

    let mut total: u64 = 0;

    // Scrape opinion index pages by year
    for year in 2018..=2026 {
        let url = format!("{}?court=both&year={}&order=bydate&submit=Submit", SEARCH_BASE, year);
        println!("  [opinions] scraping {} index...", year);

        let html = match client.get(&url).send() {
            Ok(resp) if resp.status().is_success() => resp.text()?,
            Ok(resp) => {
                println!("  [opinions] {}: HTTP {}", year, resp.status());
                continue;
            }
            Err(e) => {
                println!("  [opinions] {}: {}", year, e);
                continue;
            }
        };

        // Extract opinion links from the index page
        for chunk in html.split("href=\"") {
            let (pdf_path, case_text) = match extract_link_and_text(chunk) {
                Some(pair) => pair,
                None => continue,
            };

            // Filter for family law relevance by checking case text
            let case_lower = case_text.to_lowercase();
            let is_family = SEARCH_KEYWORDS.iter().any(|kw| case_lower.contains(kw))
                || case_lower.contains("custody")
                || case_lower.contains("visitation")
                || case_lower.contains("child support")
                || case_lower.contains("divorce");

            if !is_family {
                continue;
            }

            let pdf_url = if pdf_path.starts_with("http") {
                pdf_path.to_string()
            } else {
                format!("https://www.mdcourts.gov{}", pdf_path)
            };

            // Use case text as citation key
            let citation = normalize_citation(&case_text);
            if citation.is_empty() { continue; }

            // Check if already ingested
            if store::get::<AppellateOpinion>(db, store::TREE_OPINIONS, &citation)?.is_some() {
                continue;
            }

            // Download PDF
            let pdf_bytes = match client.get(&pdf_url).send() {
                Ok(resp) if resp.status().is_success() => resp.bytes()?.to_vec(),
                _ => continue,
            };

            // Extract text
            let full_text = match pdf_extract::extract_text_from_mem(&pdf_bytes) {
                Ok(t) => t,
                Err(_) => continue,
            };

            // Extract holding (first substantial paragraph after "HELD" or "held")
            let holding = extract_holding(&full_text);

            // Categorize
            let categories = categorize_opinion(&full_text);

            let opinion = AppellateOpinion {
                case_name: case_text.to_string(),
                citation: citation.clone(),
                date: format!("{}", year),
                judges: Vec::new(), // Would need PDF parsing
                holding,
                full_text,
                categories,
                pdf_url,
            };

            store::put(db, store::TREE_OPINIONS, &citation, &opinion)?;
            total += 1;

            if total % 10 == 0 {
                println!("  [opinions] {} family law opinions ingested", total);
            }
        }
    }

    super::ingest::record_ingest(db, "appellate", total, None)?;
    Ok(total)
}

/// Extract PDF link and case text from an href chunk.
fn extract_link_and_text(chunk: &str) -> Option<(String, String)> {
    let end_quote = chunk.find('"')?;
    let href = &chunk[..end_quote];
    if !href.ends_with(".pdf") { return None; }

    // Find the link text between > and <
    let gt = chunk.find('>')?;
    let lt = chunk[gt..].find('<')?;
    let text = chunk[gt + 1..gt + lt].trim();
    if text.is_empty() { return None; }

    Some((href.to_string(), text.to_string()))
}

fn normalize_citation(text: &str) -> String {
    text.trim()
        .replace('\n', " ")
        .replace("  ", " ")
        .chars()
        .take(200)
        .collect()
}

fn extract_holding(text: &str) -> String {
    // Look for "HELD" marker in appellate opinions
    let lower = text.to_lowercase();
    if let Some(pos) = lower.find("held:") {
        let start = pos + 5;
        let end = text[start..].find("\n\n").map(|p| start + p).unwrap_or((start + 500).min(text.len()));
        return text[start..end].trim().to_string();
    }

    // Fallback: take first paragraph that's longer than 100 chars
    for para in text.split("\n\n") {
        let trimmed = para.trim();
        if trimmed.len() > 100 {
            return trimmed.chars().take(500).collect();
        }
    }

    String::new()
}

fn categorize_opinion(text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    let mut cats = Vec::new();

    let checks = [
        ("custody", "custody"),
        ("modification", "modification"),
        ("best interest", "best_interest"),
        ("alienation", "alienation"),
        ("visitation", "visitation"),
        ("child support", "child_support"),
        ("abuse", "abuse"),
        ("neglect", "neglect"),
        ("iep", "special_education"),
        ("special education", "special_education"),
        ("domestic violence", "domestic_violence"),
    ];

    for (keyword, category) in &checks {
        if lower.contains(keyword) && !cats.contains(&category.to_string()) {
            cats.push(category.to_string());
        }
    }

    cats
}
