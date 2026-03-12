// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6
//! tournament — Model competition across cluster nodes.
//! Runs held-out challenges against every available model,
//! scores them, crowns winners per category, feeds the router bandit.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use super::bench;
use super::registry::MicroRegistry;
use super::runner;
use crate::cluster::Cluster;
use crate::ollama;

/// A competitor: one model on one node.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Competitor {
    pub node_id: String,
    pub node_url: String,
    pub model: String,
}

/// Result of one model running one challenge.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MatchResult {
    pub competitor: Competitor,
    pub challenge: String,
    pub category: String,
    pub passed: bool,
    pub duration_ms: u64,
    pub tokens: u64,
    pub response_len: usize,
}

/// Per-model aggregate score.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModelScore {
    pub model: String,
    pub node_id: String,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub errors: usize,
    pub total_duration_ms: u64,
    pub total_tokens: u64,
    /// Composite score: accuracy * 100 + speed_bonus
    pub score: f64,
}

impl ModelScore {
    pub fn accuracy(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.passed as f64 / self.total as f64
        }
    }

    pub fn avg_ms(&self) -> u64 {
        if self.total == 0 {
            0
        } else {
            self.total_duration_ms / self.total as u64
        }
    }
}

/// Category winner: best model for a specific task type.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CategoryWinner {
    pub category: String,
    pub model: String,
    pub node_id: String,
    pub node_url: String,
    pub accuracy: f64,
    pub avg_ms: u64,
    pub score: f64,
}

/// Full tournament results.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TournamentResult {
    pub timestamp: String,
    pub competitors: Vec<Competitor>,
    pub scores: Vec<ModelScore>,
    pub category_winners: Vec<CategoryWinner>,
    pub matches: Vec<MatchResult>,
    /// Challenges every model aced — candidates for retirement.
    pub easy_challenges: Vec<String>,
    /// Challenges no model passed — too hard or broken.
    pub impossible_challenges: Vec<String>,
}

/// Held-out challenge for the tournament (mirrors bench::Challenge but owned).
struct TournamentChallenge {
    template_id: String,
    category: String,
    input: String,
    verify: String,
    description: String,
}

/// Discover all competitors: every model on every online node.
pub fn discover_competitors(cluster: &Cluster) -> Vec<Competitor> {
    let mut competitors = Vec::new();
    let online = cluster.online_nodes();

    for node in online {
        let url = node.base_url();
        if let Ok(models) = ollama::list_models(&url) {
            for m in models {
                competitors.push(Competitor {
                    node_id: node.id.clone(),
                    node_url: url.clone(),
                    model: m.name.clone(),
                });
            }
        }
    }

    competitors
}

/// Build tournament challenges from the bench module's challenges.
/// We re-use the held-out set but need owned data.
fn tournament_challenges(_registry: &MicroRegistry) -> Vec<TournamentChallenge> {
    // We can't call bench::challenges() directly since it's private.
    // Instead, define the same challenges here, grouped by category.
    // This is intentional duplication — tournament may diverge from bench over time.
    vec![
        // Classifier
        tc("f79", "classify", "refactor the database module to use connection pooling", "single_word", "classify: refactor"),
        tc("f79", "classify", "the server crashes when I send a POST to /api/users", "single_word", "classify: bug report"),
        tc("f79", "classify", "run cargo clippy and fix all the warnings", "single_word", "classify: clippy"),
        tc("f79", "classify", "I need tests for the authentication middleware", "single_word", "classify: test request"),
        tc("f79", "classify", "how does the router pick which node to use", "single_word", "classify: explain"),
        // Fix compile
        tc("f81", "fix_compile", "Error: mismatched types: expected `Vec<String>`, found `Vec<&str>`\nCode: fn names() -> Vec<String> { vec![\"alice\", \"bob\"] }", "contains:fn names", "fix: Vec<String> vs Vec<&str>"),
        tc("f81", "fix_compile", "Error: cannot borrow `v` as mutable because it is also borrowed as immutable\nCode: fn f(v: &mut Vec<i32>) { let first = &v[0]; v.push(1); println!(\"{}\", first); }", "contains:fn f", "fix: borrow checker"),
        tc("f81", "fix_compile", "Error: type annotations needed\nCode: fn parse_it(s: &str) { let n = s.parse().unwrap(); println!(\"{}\", n + 1); }", "contains:parse", "fix: type annotation"),
        // Code gen
        tc("f80", "code_gen", "write a function that checks if a number is prime", "contains:fn", "gen: is_prime"),
        tc("f80", "code_gen", "write a function that merges two sorted slices into a new sorted Vec", "contains:fn", "gen: merge sorted"),
        tc("f80", "code_gen", "write a function that counts the frequency of each word in a &str and returns a HashMap<String, usize>", "contains:HashMap", "gen: word freq"),
        tc("f80", "code_gen", "write a function that flattens a Vec<Vec<i32>> into a Vec<i32>", "contains:fn", "gen: flatten"),
        // Code review
        tc("f_code_review", "code_review", "fn gcd(mut a: u64, mut b: u64) -> u64 { while b != 0 { let t = b; b = a % b; a = t; } a }", "contains_any:LGTM,lgtm,looks good", "review: gcd"),
        tc("f_code_review", "code_review", "fn read_file(path: &str) -> String { std::fs::read_to_string(path).unwrap() }", "not_empty", "review: unwrap file"),
        // Validate
        tc("f_validate", "validate", "Request: implement quicksort\nCode: fn quicksort(v: &mut Vec<i32>) { /* TODO */ }", "contains:FAIL", "validate: TODO"),
        tc("f_validate", "validate", "Request: reverse a vector in place\nCode: fn reverse(v: &mut Vec<i32>) { v.reverse(); }", "contains:PASS", "validate: correct"),
        // Clippy
        tc("f_clippy_fix", "clippy_fix", "Warning: redundant clone\nCode: fn greet(name: String) { let n = name.clone(); println!(\"hi {}\", n); }", "contains:fn greet", "clippy: redundant clone"),
        // Explain
        tc("f115", "explain", "Intent: run test suite\nStage: cargo test\nOutcome: FAIL\nStderr: thread 'tests::test_parse' panicked at 'assertion failed: result.is_ok()'", "not_empty", "explain: test panic"),
        // Test write
        tc("f_test_write", "test_write", "fn clamp(val: i32, min: i32, max: i32) -> i32 { if val < min { min } else if val > max { max } else { val } }", "contains:#[test]", "test: clamp"),
    ]
}

fn tc(tid: &str, cat: &str, input: &str, verify: &str, desc: &str) -> TournamentChallenge {
    TournamentChallenge {
        template_id: tid.to_string(),
        category: cat.to_string(),
        input: input.to_string(),
        verify: verify.to_string(),
        description: desc.to_string(),
    }
}

/// Run the full tournament.
pub fn run_tournament(registry: &MicroRegistry, cluster: &Cluster) -> TournamentResult {
    let competitors = discover_competitors(cluster);
    let challenges = tournament_challenges(registry);

    eprintln!(
        "TOURNAMENT: {} competitors, {} challenges",
        competitors.len(),
        challenges.len()
    );
    eprintln!("─────────────────────────────────────────────────────────────────");
    for c in &competitors {
        eprintln!("  {} on {} ({})", c.model, c.node_id, c.node_url);
    }
    eprintln!("─────────────────────────────────────────────────────────────────");

    let mut all_matches: Vec<MatchResult> = Vec::new();

    // Run every competitor through every challenge for its template's category
    for competitor in &competitors {
        eprintln!(
            "\n[tournament] {} on {}:",
            competitor.model, competitor.node_id
        );

        for ch in &challenges {
            let tmpl = match registry.get(&ch.template_id) {
                Some(t) => t,
                None => continue,
            };

            let start = Instant::now();
            let result = runner::run_micro_direct(
                tmpl,
                &ch.input,
                &competitor.node_url,
                Some(&competitor.model),
            );

            let duration_ms = start.elapsed().as_millis() as u64;

            match result {
                Ok(r) => {
                    let passed = bench::verify_response(&r.response, &ch.verify);
                    let tokens = r.tokens.unwrap_or(0);

                    if passed {
                        eprint!("  PASS ");
                    } else {
                        eprint!("  FAIL ");
                    }
                    eprintln!(" {:>5}ms  {}", duration_ms, ch.description);

                    all_matches.push(MatchResult {
                        competitor: competitor.clone(),
                        challenge: ch.description.clone(),
                        category: ch.category.clone(),
                        passed,
                        duration_ms,
                        tokens,
                        response_len: r.response.len(),
                    });
                }
                Err(e) => {
                    eprintln!("  ERR   {:>5}ms  {} — {}", duration_ms, ch.description, e);
                    all_matches.push(MatchResult {
                        competitor: competitor.clone(),
                        challenge: ch.description.clone(),
                        category: ch.category.clone(),
                        passed: false,
                        duration_ms,
                        tokens: 0,
                        response_len: 0,
                    });
                }
            }
        }
    }

    // Aggregate scores per model
    let mut score_map: HashMap<String, ModelScore> = HashMap::new();
    for m in &all_matches {
        let key = format!("{}@{}", m.competitor.model, m.competitor.node_id);
        let entry = score_map.entry(key).or_insert_with(|| ModelScore {
            model: m.competitor.model.clone(),
            node_id: m.competitor.node_id.clone(),
            total: 0,
            passed: 0,
            failed: 0,
            errors: 0,
            total_duration_ms: 0,
            total_tokens: 0,
            score: 0.0,
        });
        entry.total += 1;
        if m.passed {
            entry.passed += 1;
        } else if m.tokens == 0 && m.response_len == 0 {
            entry.errors += 1;
        } else {
            entry.failed += 1;
        }
        entry.total_duration_ms += m.duration_ms;
        entry.total_tokens += m.tokens;
    }

    // Compute composite score: accuracy * 100 + speed bonus (faster = higher)
    let max_avg_ms = score_map
        .values()
        .map(|s| s.avg_ms())
        .max()
        .unwrap_or(1)
        .max(1);
    for s in score_map.values_mut() {
        let accuracy = s.accuracy();
        let speed_bonus = 20.0 * (1.0 - (s.avg_ms() as f64 / max_avg_ms as f64));
        s.score = accuracy * 100.0 + speed_bonus;
    }

    let mut scores: Vec<ModelScore> = score_map.into_values().collect();
    scores.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Find category winners
    let mut cat_scores: HashMap<String, Vec<(String, String, String, f64, u64, usize, usize)>> =
        HashMap::new();
    for m in &all_matches {
        let entry = cat_scores.entry(m.category.clone()).or_default();
        // Find or create entry for this model in this category
        let key = format!("{}@{}", m.competitor.model, m.competitor.node_id);
        if let Some(existing) = entry.iter_mut().find(|e| e.0 == key) {
            existing.5 += 1; // total
            if m.passed {
                existing.6 += 1; // passed
            }
            existing.4 += m.duration_ms;
        } else {
            entry.push((
                key,
                m.competitor.model.clone(),
                m.competitor.node_id.clone(),
                0.0, // score (computed below)
                m.duration_ms,
                1,                            // total
                if m.passed { 1 } else { 0 }, // passed
            ));
        }
    }

    let mut category_winners = Vec::new();
    for (cat, entries) in &cat_scores {
        let best = entries.iter().max_by(|a, b| {
            let acc_a = a.6 as f64 / a.5.max(1) as f64;
            let acc_b = b.6 as f64 / b.5.max(1) as f64;
            let avg_a = a.4 / a.5.max(1) as u64;
            let avg_b = b.4 / b.5.max(1) as u64;
            let score_a = acc_a * 100.0 + 20.0 * (1.0 - avg_a as f64 / max_avg_ms as f64);
            let score_b = acc_b * 100.0 + 20.0 * (1.0 - avg_b as f64 / max_avg_ms as f64);
            score_a
                .partial_cmp(&score_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        if let Some(b) = best {
            let acc = b.6 as f64 / b.5.max(1) as f64;
            let avg = b.4 / b.5.max(1) as u64;
            let node_url = competitors
                .iter()
                .find(|c| c.node_id == b.2 && c.model == b.1)
                .map(|c| c.node_url.clone())
                .unwrap_or_default();
            category_winners.push(CategoryWinner {
                category: cat.clone(),
                model: b.1.clone(),
                node_id: b.2.clone(),
                node_url,
                accuracy: acc,
                avg_ms: avg,
                score: acc * 100.0 + 20.0 * (1.0 - avg as f64 / max_avg_ms as f64),
            });
        }
    }
    category_winners.sort_by(|a, b| a.category.cmp(&b.category));

    // Find easy (all pass) and impossible (all fail) challenges
    let mut challenge_results: HashMap<String, (usize, usize)> = HashMap::new();
    for m in &all_matches {
        let entry = challenge_results
            .entry(m.challenge.clone())
            .or_insert((0, 0));
        entry.0 += 1;
        if m.passed {
            entry.1 += 1;
        }
    }
    let easy_challenges: Vec<String> = challenge_results
        .iter()
        .filter(|(_, (total, passed))| *total > 1 && *passed == *total)
        .map(|(k, _)| k.clone())
        .collect();
    let impossible_challenges: Vec<String> = challenge_results
        .iter()
        .filter(|(_, (total, passed))| *total > 1 && *passed == 0)
        .map(|(k, _)| k.clone())
        .collect();

    let timestamp = chrono_now();

    TournamentResult {
        timestamp,
        competitors,
        scores,
        category_winners,
        matches: all_matches,
        easy_challenges,
        impossible_challenges,
    }
}

/// Print tournament results.
pub fn print_results(r: &TournamentResult) {
    println!("\nTOURNAMENT RESULTS ({})", r.timestamp);
    println!("═══════════════════════════════════════════════════════════════════");

    // Overall leaderboard
    println!("\nOVERALL LEADERBOARD");
    println!("─────────────────────────────────────────────────────────────────");
    println!(
        "{:<4} {:<28} {:<5} {:>5} {:>5} {:>5} {:>8} {:>7}",
        "Rank", "Model", "Node", "Total", "Pass", "Fail", "Avg(ms)", "Score"
    );
    println!("─────────────────────────────────────────────────────────────────");
    for (i, s) in r.scores.iter().enumerate() {
        println!(
            "{:<4} {:<28} {:<5} {:>5} {:>5} {:>5} {:>8} {:>7.1}",
            i + 1,
            s.model,
            s.node_id,
            s.total,
            s.passed,
            s.failed + s.errors,
            s.avg_ms(),
            s.score
        );
    }

    // Category winners
    println!("\nCATEGORY WINNERS");
    println!("─────────────────────────────────────────────────────────────────");
    println!(
        "{:<14} {:<28} {:<5} {:>6} {:>8}",
        "Category", "Model", "Node", "Acc%", "Avg(ms)"
    );
    println!("─────────────────────────────────────────────────────────────────");
    for w in &r.category_winners {
        println!(
            "{:<14} {:<28} {:<5} {:>5.0}% {:>8}",
            w.category,
            w.model,
            w.node_id,
            w.accuracy * 100.0,
            w.avg_ms
        );
    }

    // Challenge analysis
    if !r.easy_challenges.is_empty() {
        println!("\nEASY (all models passed — candidates for retirement):");
        for c in &r.easy_challenges {
            println!("  {}", c);
        }
    }
    if !r.impossible_challenges.is_empty() {
        println!("\nIMPOSSIBLE (no model passed — too hard or broken):");
        for c in &r.impossible_challenges {
            println!("  {}", c);
        }
    }

    println!("\n═══════════════════════════════════════════════════════════════════");
}

/// Save tournament results.
pub fn save_results(r: &TournamentResult) -> Result<(), String> {
    let path = tournament_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(r).map_err(|e| e.to_string())?;
    std::fs::write(&path, &json).map_err(|e| e.to_string())?;

    // Also append to history
    let history_path = history_path();
    let mut history: Vec<TournamentSummary> = if history_path.exists() {
        let content = std::fs::read_to_string(&history_path).unwrap_or_default();
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        Vec::new()
    };

    history.push(TournamentSummary {
        timestamp: r.timestamp.clone(),
        competitors: r.competitors.len(),
        challenges: r.matches.len() / r.competitors.len().max(1),
        winner: r
            .scores
            .first()
            .map(|s| s.model.clone())
            .unwrap_or_default(),
        winner_score: r.scores.first().map(|s| s.score).unwrap_or(0.0),
    });

    let json = serde_json::to_string_pretty(&history).map_err(|e| e.to_string())?;
    std::fs::write(&history_path, json).map_err(|e| e.to_string())
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct TournamentSummary {
    timestamp: String,
    competitors: usize,
    challenges: usize,
    winner: String,
    winner_score: f64,
}

fn tournament_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home)
        .join(".kova")
        .join("micro")
        .join("tournament.json")
}

fn history_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home)
        .join(".kova")
        .join("micro")
        .join("tournament_history.json")
}

fn chrono_now() -> String {
    // Simple timestamp without chrono crate
    use std::time::SystemTime;
    let dur = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", dur.as_secs())
}
