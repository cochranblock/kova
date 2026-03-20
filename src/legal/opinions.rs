//! opinions — MD appellate family law opinion scraper.
//!
//! Source: mdcourts.gov/cgi-bin/indexlist.pl
//! Strategy: scrape yearly index tables → filter family law case names → download PDFs → extract text

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use crate::legal::store::{self, AppellateOpinion};

const SEARCH_BASE: &str = "https://www.mdcourts.gov/cgi-bin/indexlist.pl";

// Family law keywords to filter case names
const FAMILY_KEYWORDS: &[&str] = &[
    " v. ", // all cases have this, but combined with below
];

// Case name patterns that indicate family law
const FAMILY_NAME_PATTERNS: &[&str] = &[
    "custody", "visitation", "child", "minor", "adoption",
    "guardianship", "divorce", "domestic", "in re:",
    "in the matter of", "department of social",
    "dept. of social", "dss", "dhr",
];

/// Ingest MD appellate family law opinions.
pub fn ingest(db: &sled::Db) -> anyhow::Result<u64> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;

    let mut total: u64 = 0;

    for year in 2018..=2025 {
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

        // Parse HTML table rows: <TR><TD><A HREF='...pdf'>docket</a><TD>citation<TD>date<TD>judge<TD>case_name<TD>num
        let rows = parse_opinion_rows(&html);
        let family_rows: Vec<&OpinionRow> = rows.iter()
            .filter(|r| is_family_law(&r.case_name))
            .collect();

        println!("  [opinions] {}: {} total opinions, {} family law", year, rows.len(), family_rows.len());

        for row in &family_rows {
            let citation = row.citation.trim().to_string();
            if citation.is_empty() { continue; }

            // Check if already ingested
            if store::get::<AppellateOpinion>(db, store::TREE_OPINIONS, &citation)?.is_some() {
                continue;
            }

            let pdf_url = format!("https://www.mdcourts.gov{}", row.pdf_path);

            // Download PDF
            let pdf_bytes = match client.get(&pdf_url).send() {
                Ok(resp) if resp.status().is_success() => resp.bytes()?.to_vec(),
                _ => {
                    println!("  [opinions] failed to download: {}", row.case_name);
                    continue;
                }
            };

            // Extract text
            let full_text = match pdf_extract::extract_text_from_mem(&pdf_bytes) {
                Ok(t) => t,
                Err(_) => {
                    println!("  [opinions] PDF extract failed: {}", row.case_name);
                    continue;
                }
            };

            let holding = extract_holding(&full_text);
            let categories = categorize_opinion(&full_text);

            let opinion = AppellateOpinion {
                case_name: row.case_name.clone(),
                citation: citation.clone(),
                date: row.date.clone(),
                judges: vec![row.judge.clone()],
                holding,
                full_text,
                categories,
                pdf_url,
            };

            store::put(db, store::TREE_OPINIONS, &citation, &opinion)?;
            total += 1;
            println!("  [opinions] ingested: {} ({})", row.case_name, citation);
        }
    }

    super::ingest::record_ingest(db, "appellate", total, None)?;
    Ok(total)
}

struct OpinionRow {
    pdf_path: String,
    citation: String,
    date: String,
    judge: String,
    case_name: String,
}

/// Parse the mdcourts.gov opinion index table.
/// Format: <TR ...><TD><A HREF='/data/opinions/...pdf'>docket</a><TD>citation<TD>date<TD>judge<TD>case_name<TD>num
fn parse_opinion_rows(html: &str) -> Vec<OpinionRow> {
    let mut rows = Vec::new();

    for tr_chunk in html.split("<TR") {
        // Extract PDF href
        let pdf_path = match extract_between(tr_chunk, "HREF='", "'") {
            Some(p) if p.ends_with(".pdf") => p,
            _ => continue,
        };

        // Split on <TD to get columns
        let tds: Vec<&str> = tr_chunk.split("<TD").collect();
        if tds.len() < 6 { continue; }

        let citation = strip_td(tds.get(2).unwrap_or(&""));
        let date = strip_td(tds.get(3).unwrap_or(&""));
        let judge = strip_td(tds.get(4).unwrap_or(&""));
        let case_name = strip_td(tds.get(5).unwrap_or(&""));

        if case_name.is_empty() { continue; }

        rows.push(OpinionRow {
            pdf_path: pdf_path.to_string(),
            citation: citation.trim().to_string(),
            date: date.trim().to_string(),
            judge: judge.trim().to_string(),
            case_name: case_name.trim().to_string(),
        });
    }

    rows
}

fn extract_between<'a>(text: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let s = text.find(start)? + start.len();
    let e = text[s..].find(end)? + s;
    Some(&text[s..e])
}

/// Strip HTML tags and TD attributes (WIDTH="30%" etc) from table cell text.
fn strip_td(text: &str) -> String {
    // First strip everything before the first > (TD attributes)
    let content = if let Some(pos) = text.find('>') {
        &text[pos + 1..]
    } else {
        text
    };
    // Then strip HTML tags
    let mut result = String::new();
    let mut in_tag = false;
    for ch in content.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    result.trim().to_string()
}

fn is_family_law(case_name: &str) -> bool {
    let lower = case_name.to_lowercase();
    FAMILY_NAME_PATTERNS.iter().any(|p| lower.contains(p))
}

fn extract_holding(text: &str) -> String {
    let lower = text.to_lowercase();
    if let Some(pos) = lower.find("held:") {
        let start = pos + 5;
        let end = text[start..].find("\n\n").map(|p| start + p).unwrap_or((start + 500).min(text.len()));
        return text[start..end].trim().to_string();
    }
    // Fallback: first paragraph > 100 chars
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
        ("custody", "custody"), ("modification", "modification"),
        ("best interest", "best_interest"), ("alienation", "alienation"),
        ("visitation", "visitation"), ("child support", "child_support"),
        ("abuse", "abuse"), ("neglect", "neglect"),
        ("iep", "special_education"), ("special education", "special_education"),
        ("domestic violence", "domestic_violence"),
        ("guardianship", "guardianship"), ("adoption", "adoption"),
    ];
    for (keyword, category) in &checks {
        if lower.contains(keyword) && !cats.contains(&category.to_string()) {
            cats.push(category.to_string());
        }
    }
    cats
}
