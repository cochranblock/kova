//! complaints — MSDE special education complaint letter ingest for AACPS.
//!
//! Source: marylandpublicschools.org complaint letters
//! Strategy: scrape fiscal year index pages → filter AACPS PDF links → download → extract text

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use crate::legal::store::{self, ComplaintLetter};

const BASE_URL: &str = "https://marylandpublicschools.org";
const INDEX_PATTERN: &str = "/programs/Pages/Special-Education/FSDR/ComplaintLetters/{YEAR}/index.aspx";

// Fiscal years to scrape
const FISCAL_YEARS: &[&str] = &["2022", "2023", "2024", "2025"];

// Known AACPS complaint PDF URLs (discovered from web search, verified accessible)
const KNOWN_AACPS_PDFS: &[(&str, &str)] = &[
    ("25-105AACPS-A", "/programs/Documents/Special-Ed/FSDR/ComplaintLetters/2025/2/25-105AACPS-A.pdf"),
    ("24-247AACPS-A", "/programs/Documents/Special-Ed/FSDR/ComplaintLetters/2024/4/24-247AACPS-A.pdf"),
    ("24-210AACPS-A", "/programs/Documents/Special-Ed/FSDR/ComplaintLetters/2024/4/24-210AACPS-A.pdf"),
    ("23-267AACPS", "/programs/Documents/Special-Ed/FSDR/ComplaintLetters/2023/4/23-267AACPS.pdf"),
    ("23-253AACPS", "/programs/Documents/Special-Ed/FSDR/ComplaintLetters/2023/4/23-253AACPS.pdf"),
    ("13-100AACPS2", "/msde/divisions/earlyinterv/complaint_investigation/complaint_letters/2013/docs_4/13-100%20AACPS2.pdf"),
    ("12-033AACPS", "/msde/divisions/earlyinterv/complaint_investigation/complaint_letters/2012/docs/12033AACPS.pdf"),
];

/// Ingest MSDE complaint letters for AACPS.
pub fn ingest(db: &sled::Db) -> anyhow::Result<u64> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()?;

    let mut total: u64 = 0;

    for year in FISCAL_YEARS {
        let index_url = format!("{}{}", BASE_URL, INDEX_PATTERN.replace("{YEAR}", year));
        println!("  [complaints] scraping FY{} index: {}", year, index_url);

        let html = match client.get(&index_url).send() {
            Ok(resp) if resp.status().is_success() => resp.text()?,
            Ok(resp) => {
                println!("  [complaints] FY{}: HTTP {}, skipping", year, resp.status());
                continue;
            }
            Err(e) => {
                println!("  [complaints] FY{}: {}, skipping", year, e);
                continue;
            }
        };

        // Extract PDF links containing "AACPS"
        let mut pdf_urls: Vec<String> = Vec::new();
        for chunk in html.split("href=\"") {
            if let Some(end) = chunk.find('"') {
                let href = &chunk[..end];
                if href.contains("AACPS") && href.ends_with(".pdf") {
                    let full_url = if href.starts_with("http") {
                        href.to_string()
                    } else if href.starts_with('/') {
                        format!("{}{}", BASE_URL, href)
                    } else {
                        format!("{}/{}", BASE_URL, href)
                    };
                    if !pdf_urls.contains(&full_url) {
                        pdf_urls.push(full_url);
                    }
                }
            }
        }

        println!("  [complaints] FY{}: found {} AACPS complaint PDFs", year, pdf_urls.len());

        for pdf_url in &pdf_urls {
            // Extract complaint ID from URL (e.g., "24-247AACPS-A")
            let complaint_id = extract_complaint_id(pdf_url);

            // Check if already ingested
            let key = format!("AACPS:{}", complaint_id);
            if store::get::<ComplaintLetter>(db, store::TREE_COMPLAINTS, &key)?.is_some() {
                continue;
            }

            // Download PDF
            let pdf_bytes = match client.get(pdf_url).send() {
                Ok(resp) if resp.status().is_success() => resp.bytes()?.to_vec(),
                _ => {
                    println!("  [complaints] failed to download {}", pdf_url);
                    continue;
                }
            };

            // Extract text from PDF
            let full_text = extract_pdf_text(&pdf_bytes);

            // Parse violations and corrective actions from text
            let violations = extract_violations(&full_text);
            let corrective_actions = extract_corrective_actions(&full_text);

            let letter = ComplaintLetter {
                complaint_id: complaint_id.clone(),
                fiscal_year: year.to_string(),
                school_system: "AACPS".into(),
                filing_date: String::new(), // Would need text parsing
                violations_found: violations,
                corrective_actions,
                full_text,
                pdf_url: pdf_url.clone(),
            };

            store::put(db, store::TREE_COMPLAINTS, &key, &letter)?;
            total += 1;
            println!("  [complaints] ingested {}", complaint_id);
        }
    }

    // Fallback: download known AACPS complaint PDFs directly
    if total == 0 {
        println!("  [complaints] index scrape found 0 — using {} known AACPS PDFs", KNOWN_AACPS_PDFS.len());
        for (complaint_id, path) in KNOWN_AACPS_PDFS {
            let key = format!("AACPS:{}", complaint_id);
            if store::get::<ComplaintLetter>(db, store::TREE_COMPLAINTS, &key)?.is_some() {
                continue;
            }

            let url = if path.starts_with("/msde/") {
                format!("https://archives.marylandpublicschools.org{}", path)
            } else {
                format!("{}{}", BASE_URL, path)
            };

            let pdf_bytes = match client.get(&url).send() {
                Ok(resp) if resp.status().is_success() => resp.bytes()?.to_vec(),
                _ => {
                    println!("  [complaints] failed: {}", complaint_id);
                    continue;
                }
            };

            let full_text = extract_pdf_text(&pdf_bytes);
            let violations = extract_violations(&full_text);
            let corrective_actions = extract_corrective_actions(&full_text);

            let letter = ComplaintLetter {
                complaint_id: complaint_id.to_string(),
                fiscal_year: format!("20{}", &complaint_id[..2]),
                school_system: "AACPS".into(),
                filing_date: String::new(),
                violations_found: violations,
                corrective_actions,
                full_text,
                pdf_url: url,
            };

            store::put(db, store::TREE_COMPLAINTS, &key, &letter)?;
            total += 1;
            println!("  [complaints] ingested {}", complaint_id);
        }
    }

    super::ingest::record_ingest(db, "msde_complaints", total, None)?;
    Ok(total)
}

fn extract_complaint_id(url: &str) -> String {
    // URL like: .../ComplaintLetters/2024/4/24-247AACPS-A.pdf
    url.rsplit('/')
        .next()
        .unwrap_or("unknown")
        .trim_end_matches(".pdf")
        .to_string()
}

fn extract_pdf_text(bytes: &[u8]) -> String {
    // Try pdf-extract if available, otherwise return empty
    // For now, use a basic approach
    match pdf_extract::extract_text_from_mem(bytes) {
        Ok(text) => text,
        Err(_) => String::new(),
    }
}

fn extract_violations(text: &str) -> Vec<String> {
    let mut violations = Vec::new();
    let markers = [
        "violation", "did not ensure", "failed to", "did not provide",
        "was not implemented", "did not comply", "not in compliance",
    ];

    for line in text.lines() {
        let line_lower = line.to_lowercase();
        if markers.iter().any(|m| line_lower.contains(m)) {
            let trimmed = line.trim();
            if trimmed.len() > 10 && trimmed.len() < 500 {
                violations.push(trimmed.to_string());
            }
        }
    }

    violations
}

fn extract_corrective_actions(text: &str) -> Vec<String> {
    let mut actions = Vec::new();

    let markers = [
        "corrective action", "shall ensure", "must provide", "is required to",
        "within 30", "within 60", "within 90", "shall develop",
    ];

    for line in text.lines() {
        let line_lower = line.to_lowercase();
        if markers.iter().any(|m| line_lower.contains(m)) {
            let trimmed = line.trim();
            if trimmed.len() > 10 && trimmed.len() < 500 {
                actions.push(trimmed.to_string());
            }
        }
    }

    actions
}
