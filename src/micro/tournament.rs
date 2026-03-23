//! tournament — Olympic-style model competition across cluster nodes.
//!
//! Weight classes (wrestling):
//!   Atomweight   ≤1B   — sub-billion, the tiniest contenders
//!   Flyweight    1-3B   — fast, light, sprint events
//!   Bantamweight 3-7B  — balanced speed/quality
//!   Middleweight 7-15B — quality-focused
//!
//! Arenas (nodes as venues with weight restrictions):
//!   c2 (local)  — Flyweight/Bantamweight arena (≤7B only)
//!   n0-n3       — Open weight arenas
//!
//! Event types:
//!   Sprint     — classifier (f79), fastest correct wins
//!   Technical  — fix_compile (f81), precision matters
//!   Freestyle  — code_gen (f80), creativity + correctness
//!   Endurance  — test_write, long-form generation
//!   Doping     — anti-slop (P12), penalizes AI filler words
//!   Exhibition — non-coder models doing Rust (cross-weight)
// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

/// (key, model, node_id, duration_sum, count, passed_count, _)
type CatBestEntry = (String, String, String, f64, u64, usize, usize);

use super::bench;
use super::registry::T149;
use super::runner;
use crate::cluster::T193;
use crate::providers;

// ── Weight Classes ──────────────────────────────────────────────

/// T160=WeightClass
/// Weight class for a model based on parameter count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum T160 {
    /// ≤1B parameters — sub-billion tinies
    Atomweight,
    /// 1-3B parameters
    Flyweight,
    /// 3-7B parameters
    Bantamweight,
    /// 7-15B parameters
    Middleweight,
}

impl T160 {
    /// Classify a model by its name tag.
    pub fn from_model(model: &str) -> Self {
        let lower = model.to_lowercase();
        // Extract size from model name patterns like ":0.5b", ":1b", ":3b", ":7b", ":14b"
        if let Some(size) = extract_param_size(&lower) {
            if size <= 1.0 {
                T160::Atomweight
            } else if size <= 3.0 {
                T160::Flyweight
            } else if size <= 7.5 {
                T160::Bantamweight
            } else {
                T160::Middleweight
            }
        } else {
            // Models with no explicit size tag — check known families
            if lower.contains("tinyllama") || lower.contains("smollm2") {
                T160::Atomweight
            } else {
                T160::Bantamweight  // phi4-mini, etc. = 3-7B
            }
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            T160::Atomweight => "Atomweight (<=1B)",
            T160::Flyweight => "Flyweight (1-3B)",
            T160::Bantamweight => "Bantamweight (3-7B)",
            T160::Middleweight => "Middleweight (7-15B)",
        }
    }

    pub fn short(&self) -> &'static str {
        match self {
            T160::Atomweight => "ATM",
            T160::Flyweight => "FLY",
            T160::Bantamweight => "BAN",
            T160::Middleweight => "MID",
        }
    }

    /// Is this model a "coder" variant (trained on code)?
    /// Non-coder models in Rust events = exhibition match.
    pub fn is_exhibition(model: &str) -> bool {
        let lower = model.to_lowercase();
        // Code-trained model families
        let code_model = lower.contains("coder")
            || lower.contains("starcoder")
            || lower.contains("codellama")
            || lower.contains("codegemma")
            || lower.contains("granite-code")
            || lower.contains("codestral")
            || lower.contains("stable-code")
            || lower.contains("mellum")
            || lower.contains("tessa-rust")
            || lower.contains("rust-coder")
            || lower.contains("code-llama");
        // Everything else is exhibition: gemma2, tinyllama, llama3.2, phi4-mini,
        // smollm2, qwen2.5 (non-coder), mistral, orca-mini, etc.
        !code_model
    }
}

/// Extract parameter size in billions from model name.
fn extract_param_size(model: &str) -> Option<f64> {
    // Match patterns like ":3b", ":7b", ":14b", ":0.5b", ":7b-instruct-q5_K_M", ":135m", ":360m"
    for part in model.split(':') {
        let size_part = part.split('-').next().unwrap_or(part);
        // Check for billions (e.g. "3b", "0.5b", "14b")
        if let Some(num_str) = size_part.strip_suffix('b').or_else(|| size_part.strip_suffix('B'))
            && let Ok(n) = num_str.parse::<f64>()
        {
            return Some(n);
        }
        // Check for millions (e.g. "135m", "360m") — convert to billions
        if let Some(num_str) = size_part.strip_suffix('m').or_else(|| size_part.strip_suffix('M'))
            && let Ok(n) = num_str.parse::<f64>()
        {
            return Some(n / 1000.0); // 135m → 0.135B, 360m → 0.36B
        }
    }
    None
}

// ── Arena (node weight restrictions) ────────────────────────────

/// Max weight class allowed on a node (arena restriction).
/// Hardware-aware: nodes with weaker GPUs get weight caps to keep
/// generation times fast and avoid thermal/memory issues.
fn arena_max_weight(node_id: &str) -> T160 {
    match node_id {
        "c2" => T160::Flyweight,      // Local Mac — ≤3B only
        "n2" => T160::Middleweight,    // bt (RX 6700 XT, 150W muzzle) — open weight, hardware-limited by power cap
        _ => T160::Middleweight,       // gd, st, lf — open weight
    }
}

/// Check if a model is allowed in a node's arena.
fn allowed_in_arena(model: &str, node_id: &str) -> bool {
    T160::from_model(model) <= arena_max_weight(node_id)
}

// ── Core Types ──────────────────────────────────────────────────

/// T161=Competitor
/// A competitor: one model on one node, classified by weight.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct T161 {
    pub node_id: String,
    pub node_url: String,
    pub model: String,
    pub weight_class: T160,
    pub exhibition: bool,
}

/// T162=MatchResult
/// Result of one model running one challenge.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct T162 {
    pub competitor: T161,
    pub challenge: String,
    pub category: String,
    pub passed: bool,
    pub duration_ms: u64,
    pub tokens: u64,
    pub response_len: usize,
    /// Raw model response text (for training data export).
    #[serde(default)]
    pub response: String,
}

/// T163=ModelScore
/// Per-model aggregate score.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct T163 {
    pub model: String,
    pub node_id: String,
    pub weight_class: T160,
    pub exhibition: bool,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub errors: usize,
    pub total_duration_ms: u64,
    pub total_tokens: u64,
    pub score: f64,
}

impl T163 {
    pub fn accuracy(&self) -> f64 {
        if self.total == 0 { 0.0 } else { self.passed as f64 / self.total as f64 }
    }
    pub fn avg_ms(&self) -> u64 {
        if self.total == 0 { 0 } else { self.total_duration_ms / self.total as u64 }
    }
}

/// T164=CategoryWinner
/// Category winner: best model for a specific task type.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct T164 {
    pub category: String,
    pub model: String,
    pub node_id: String,
    pub node_url: String,
    pub weight_class: T160,
    pub accuracy: f64,
    pub avg_ms: u64,
    pub score: f64,
}

/// T165=TournamentResult
/// Full tournament results.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct T165 {
    pub timestamp: String,
    pub competitors: Vec<T161>,
    pub scores: Vec<T163>,
    pub category_winners: Vec<T164>,
    pub weight_class_winners: Vec<(T160, T163)>,
    pub exhibition_results: Vec<T163>,
    pub matches: Vec<T162>,
    pub easy_challenges: Vec<String>,
    pub impossible_challenges: Vec<String>,
}

/// T166=TournamentChallenge
/// Held-out challenge for the tournament.
pub struct T166 {
    pub template_id: String,
    pub category: String,
    pub event_type: &'static str,
    pub input: String,
    pub verify: String,
    pub description: String,
}

/// f248=get_challenges
/// Get all tournament challenges (for training data export).
pub fn f248(registry: &T149) -> Vec<T166> {
    tournament_challenges(registry)
}

// ── JIT Prequalification ─────────────────────────────────────────

/// Max time (ms) a model gets on the first challenge of an event.
/// If it exceeds this, it's DQ'd from the rest of that event —
/// "wasn't supposed to make it to the event."
fn prequal_cutoff_ms(weight_class: T160) -> u64 {
    match weight_class {
        T160::Atomweight   => 120_000,  // 2min — CPU inference via kova serve
        T160::Flyweight    => 180_000,  // 3min
        T160::Bantamweight => 300_000,  // 5min
        T160::Middleweight => 600_000,  // 10min
    }
}

// ── Discovery ───────────────────────────────────────────────────

/// Models too slow for tournament.
const EXCLUDED_MODELS: &[&str] = &["32b", "70b", "72b"];

/// f249=discover_competitors
/// Discover all competitors with weight classification and arena filtering.
pub fn f249(cluster: &T193) -> Vec<T161> {
    let mut competitors = Vec::new();
    eprintln!("[discovery] {} nodes in cluster", cluster.nodes.len());
    let online = cluster.online_nodes();
    eprintln!("[discovery] {} online", online.len());
    for node in &online {
        eprintln!("  {} ({}:{})", node.id, node.host, node.port);
    }

    for node in online {
        let url = node.base_url();
        eprintln!("[discovery] {} ({}) — listing models...", node.id, url);
        match providers::f336(&node.provider()) {
            Ok(ref models) => eprintln!("[discovery]   {} models found", models.len()),
            Err(ref e) => eprintln!("[discovery]   error: {}", e),
        }
        if let Ok(models) = providers::f336(&node.provider()) {
            for m in models {
                if EXCLUDED_MODELS.iter().any(|ex| m.name.contains(ex)) {
                    eprintln!("  SKIP {} on {} (too large)", m.name, node.id);
                    continue;
                }
                if !allowed_in_arena(&m.name, &node.id) {
                    eprintln!("  SKIP {} on {} (exceeds arena weight limit)", m.name, node.id);
                    continue;
                }
                let weight_class = T160::from_model(&m.name);
                let exhibition = T160::is_exhibition(&m.name);
                competitors.push(T161 {
                    node_id: node.id.clone(),
                    node_url: url.clone(),
                    model: m.name.clone(),
                    weight_class,
                    exhibition,
                });
            }
        }
    }

    competitors
}

// ── Challenges ──────────────────────────────────────────────────

fn tournament_challenges(_registry: &T149) -> Vec<T166> {
    vec![
        // SPRINT events — classifier
        tce("f79", "classify", "sprint", "refactor the database module to use connection pooling", "single_word", "classify: refactor"),
        tce("f79", "classify", "sprint", "the server crashes when I send a POST to /api/users", "single_word", "classify: bug report"),
        tce("f79", "classify", "sprint", "how does the router pick which node to use", "single_word", "classify: explain"),
        tce("f79", "classify", "sprint", "this function panics on empty input and also has a clippy warning about redundant clone", "single_word", "classify: ambiguous bug+clippy"),
        tce("f79", "classify", "sprint", "split the monolithic handle_request into smaller functions and add tests for each", "single_word", "classify: ambiguous refactor+test"),
        tce("f79", "classify", "sprint", "the borrow checker says I can't do this but I think I should be able to", "single_word", "classify: explain+fix ambiguity"),

        // TECHNICAL events — fix compile (compile-verified: the fix must actually compile)
        tce("f81", "fix_compile", "technical", "Error: mismatched types: expected `Vec<String>`, found `Vec<&str>`\nCode: fn names() -> Vec<String> { vec![\"alice\", \"bob\"] }", "compiles_and:contains:fn names", "fix: Vec<String> vs Vec<&str>"),
        tce("f81", "fix_compile", "technical", "Error: cannot borrow `v` as mutable because it is also borrowed as immutable\nCode: fn f(v: &mut Vec<i32>) { let first = &v[0]; v.push(1); println!(\"{}\", first); }", "compiles", "fix: borrow checker"),
        tce("f81", "fix_compile", "technical", "Error: `s` does not live long enough\nCode: fn get_first_word(text: &str) -> &str { let s = text.to_string(); &s[..s.find(' ').unwrap_or(s.len())] }", "compiles", "fix: dangling reference"),
        tce("f81", "fix_compile", "technical", "Error: lifetime may not live long enough\nCode: struct Wrapper<'a> { data: &'a str }\nimpl Wrapper<'_> { fn get(&self) -> &str { self.data } }", "compiles", "fix: lifetime elision"),
        tce("f81", "fix_compile", "technical", "Error: the trait bound `T: Clone` is not satisfied\nCode: fn dup<T>(x: T) -> (T, T) { (x.clone(), x) }", "compiles", "fix: missing trait bound"),
        tce("f81", "fix_compile", "technical", "Error: cannot move out of `*self` which is behind a shared reference\nCode: struct Node { val: String, next: Option<Box<Node>> }\nimpl Node { fn take_val(self) -> String { self.val } fn borrow_take(&self) -> String { self.take_val() } }", "compiles", "fix: move behind shared ref"),

        // FREESTYLE events — code gen (compile-verified: generated code must compile)
        tce("f80", "code_gen", "freestyle", "write a function that merges two sorted slices into a new sorted Vec", "compiles_and:contains:fn", "gen: merge sorted"),
        tce("f80", "code_gen", "freestyle", "write a function that counts the frequency of each word in a &str and returns a HashMap<String, usize>", "compiles_and:contains:HashMap", "gen: word freq"),
        tce("f80", "code_gen", "freestyle", "write a generic LRU cache struct with get and put methods. Use a HashMap and a VecDeque. Capacity set at construction.", "compiles_and:contains_any:struct,LRU,Lru,Cache,cache", "gen: LRU cache"),
        tce("f80", "code_gen", "freestyle", "write a trait called Summarize with a method summary() -> String, then implement it for a struct Article with title and body fields", "compiles_and:contains_any:trait Summarize,impl Summarize", "gen: trait + impl"),
        tce("f80", "code_gen", "freestyle", "write a function that takes &[&str] and returns the longest common prefix as a String", "compiles_and:contains:fn", "gen: longest common prefix"),
        tce("f80", "code_gen", "freestyle", "write a binary search function that returns Result<usize, usize> like the standard library's binary_search", "compiles_and:contains_any:Result,binary", "gen: binary search"),
        tce("f80", "code_gen", "freestyle", "write an iterator adapter struct called StepBy that wraps any iterator and yields every nth element. Implement Iterator for it.", "compiles_and:contains_any:struct StepBy,impl Iterator,impl<", "gen: custom iterator"),

        // JUDGED events — code review (subjective quality)
        tce("f_code_review", "code_review", "judged", "fn find_dup(nums: &[i32]) -> Option<i32> { let mut seen = std::collections::HashSet::new(); for &n in nums { if !seen.insert(n) { return Some(n); } } None }", "contains_any:LGTM,lgtm,looks good", "review: correct HashSet dedup"),
        tce("f_code_review", "code_review", "judged", "fn parse_kv(input: &str) -> HashMap<String, String> { let mut map = HashMap::new(); for line in input.lines() { let parts: Vec<&str> = line.splitn(2, '=').collect(); map.insert(parts[0].to_string(), parts[1].to_string()); } map }", "not_empty", "review: index panic on bad input"),
        tce("f_code_review", "code_review", "judged", "use std::sync::{Arc, Mutex};\nfn spawn_workers(data: Arc<Mutex<Vec<i32>>>) { for i in 0..4 { let d = data.clone(); std::thread::spawn(move || { let mut v = d.lock().unwrap(); v.push(i); }); } }", "not_empty", "review: mutex poison + join"),
        tce("f_code_review", "code_review", "judged", "fn truncate_utf8(s: &str, max_bytes: usize) -> &str { if s.len() <= max_bytes { s } else { &s[..max_bytes] } }", "not_empty", "review: slicing mid-codepoint"),

        // VALIDATOR events
        tce("f_validate", "validate", "judged", "Request: implement quicksort\nCode: fn quicksort(v: &mut Vec<i32>) { /* TODO */ }", "contains:FAIL", "validate: TODO stub"),
        tce("f_validate", "validate", "judged", "Request: binary search returning index\nCode: fn bsearch(v: &[i32], target: i32) -> Option<usize> { let mut lo = 0; let mut hi = v.len(); while lo < hi { let mid = (lo + hi) / 2; if v[mid] == target { return Some(mid); } else if v[mid] < target { lo = mid + 1; } else { hi = mid; } } None }", "contains:PASS", "validate: correct bsearch"),
        tce("f_validate", "validate", "judged", "Request: safe division returning Result\nCode: fn safe_div(a: f64, b: f64) -> Result<f64, String> { Ok(a / b) }", "contains:FAIL", "validate: missing zero check"),

        // CLIPPY events (compile-verified)
        tce("f_clippy_fix", "clippy_fix", "technical", "Warning: redundant clone\nCode: fn greet(name: String) { let n = name.clone(); println!(\"hi {}\", n); }", "compiles_and:contains:fn greet", "clippy: redundant clone"),
        tce("f_clippy_fix", "clippy_fix", "technical", "Warning: called `.iter().nth(0)` on a Vec\nCode: fn first(v: &Vec<i32>) -> Option<&i32> { v.iter().nth(0) }", "compiles_and:contains:fn first", "clippy: iter().nth(0)"),
        tce("f_clippy_fix", "clippy_fix", "technical", "Warning: length comparison to zero\nCode: fn is_nonempty(v: &Vec<String>) -> bool { v.len() > 0 }", "compiles_and:contains:fn is_nonempty", "clippy: len() > 0"),

        // EXPLAIN events
        tce("f115", "explain", "judged", "Intent: run test suite\nStage: cargo test\nOutcome: FAIL\nStderr: thread 'tests::test_parse' panicked at 'assertion failed: result.is_ok()'", "not_empty", "explain: test panic"),
        tce("f115", "explain", "judged", "Intent: push to remote\nStage: git push origin main\nOutcome: FAIL\nStderr: ! [rejected] main -> main (non-fast-forward)\nhint: Updates were rejected because the tip of your current branch is behind", "not_empty", "explain: git push rejected"),
        tce("f115", "explain", "judged", "Intent: run integration tests\nStage: cargo test --features tests\nOutcome: FAIL\nStderr: thread 'main' panicked at 'connection refused (os error 61)'\nnote: test requires running sled instance", "not_empty", "explain: missing dependency"),

        // DOPING TEST events — P12 anti-slop (banned words = performance enhancing drugs)
        // Models must generate useful output WITHOUT slop words.
        // "Doping" = using AI filler (utilize, leverage, optimize, etc.) to pad responses.
        // Pass = correct output + zero banned words. Fail = slop detected.

        // Code gen: write code + comments, must compile AND be slop-free
        tce("f80", "code_gen", "doping", "write a function that retries an HTTP request with exponential backoff. Add doc comments explaining the approach.", "compiles_no_slop", "doping: code+comments no slop"),
        tce("f80", "code_gen", "doping", "write a connection pool struct with get and release methods. Include doc comments on each method.", "compiles_no_slop", "doping: connection pool docs"),
        tce("f80", "code_gen", "doping", "write a rate limiter using a token bucket algorithm. Comment each section of the implementation.", "compiles_no_slop", "doping: rate limiter commentary"),

        // Code review: review must flag real issues without slop language
        tce("f_code_review", "code_review", "doping", "fn process_all(items: &[String]) -> Vec<String> { items.iter().map(|s| s.to_uppercase()).collect() }", "no_slop", "doping: review simple code"),
        tce("f_code_review", "code_review", "doping", "use std::fs;\nfn read_config(path: &str) -> String { fs::read_to_string(path).unwrap() }", "no_slop", "doping: review unwrap in config"),
        tce("f_code_review", "code_review", "doping", "struct Cache { data: std::collections::HashMap<String, Vec<u8>>, max_size: usize }\nimpl Cache { fn insert(&mut self, key: String, val: Vec<u8>) { self.data.insert(key, val); } fn get(&self, key: &str) -> Option<&Vec<u8>> { self.data.get(key) } }", "no_slop", "doping: review cache no eviction"),

        // Explain: plain English explanation, zero filler
        tce("f115", "explain", "doping", "Intent: deploy new version\nStage: cargo build --release\nOutcome: SUCCESS\nStage: scp target/release/app server:/opt/\nOutcome: FAIL\nStderr: Permission denied (publickey)", "no_slop", "doping: explain deploy failure"),
        tce("f115", "explain", "doping", "Intent: add caching layer\nStage: implement LRU cache\nOutcome: SUCCESS\nStage: cargo test\nOutcome: FAIL\nStderr: thread 'cache::tests::test_eviction' panicked at 'assertion `left == right` failed\n  left: Some(\"old\")\n  right: None'", "no_slop", "doping: explain cache test fail"),

        // Rewrite challenge: given sloppy text, clean it up (uses code_gen template as proxy)
        tce("f80", "code_gen", "doping", "Rewrite this doc comment to remove AI slop words while keeping the meaning:\n/// This function leverages advanced optimization techniques to seamlessly\n/// streamline the data processing pipeline for robust and scalable throughput.\nfn process(data: &[u8]) -> Vec<u8> { data.to_vec() }\nReturn the full function with a clean doc comment.", "compiles_no_slop", "doping: rewrite sloppy docs"),
        tce("f80", "code_gen", "doping", "Rewrite this doc comment to remove AI slop words while keeping the meaning:\n/// This module utilizes a comprehensive paradigm to empower users\n/// with cutting-edge capabilities that leverage synergy between components.\nReturn just the cleaned doc comment, no code.", "no_slop", "doping: rewrite sloppy module doc"),

        // ENDURANCE events — test_write (compile-verified: tests must compile)
        tce("f_test_write", "test_write", "endurance", "fn chunk<T: Clone>(v: &[T], size: usize) -> Vec<Vec<T>> { v.chunks(size).map(|c| c.to_vec()).collect() }", "compiles_and:contains:#[test]", "test: chunk (empty, uneven)"),
        tce("f_test_write", "test_write", "endurance", "fn safe_div(a: i64, b: i64) -> Option<i64> { if b == 0 { None } else { Some(a / b) } }", "compiles_and:contains:#[test]", "test: safe_div"),
        tce("f_test_write", "test_write", "endurance", "fn levenshtein(a: &str, b: &str) -> usize { let (a, b) = (a.as_bytes(), b.as_bytes()); let mut dp = (0..=b.len()).collect::<Vec<_>>(); for i in 1..=a.len() { let mut prev = dp[0]; dp[0] = i; for j in 1..=b.len() { let tmp = dp[j]; dp[j] = if a[i-1] == b[j-1] { prev } else { 1 + prev.min(dp[j]).min(dp[j-1]) }; prev = tmp; } } dp[b.len()] }", "compiles_and:contains:#[test]", "test: levenshtein"),
    ]
}

fn tce(tid: &str, cat: &str, event: &'static str, input: &str, verify: &str, desc: &str) -> T166 {
    T166 {
        template_id: tid.to_string(),
        category: cat.to_string(),
        event_type: event,
        input: input.to_string(),
        verify: verify.to_string(),
        description: desc.to_string(),
    }
}

// ── Tournament Execution ────────────────────────────────────────

/// f250=run_tournament
/// Run the full Olympic-style tournament.
pub fn f250(registry: &T149, cluster: &T193) -> T165 {
    let competitors = f249(cluster);
    let challenges = tournament_challenges(registry);

    // Group competitors by weight class
    let mut by_weight: HashMap<&str, Vec<&T161>> = HashMap::new();
    for c in &competitors {
        by_weight.entry(c.weight_class.short()).or_default().push(c);
    }

    eprintln!("KOVA MICRO OLYMPICS");
    eprintln!("═══════════════════════════════════════════════════════════════════");
    eprintln!("{} competitors, {} events, {} challenges", competitors.len(), 6, challenges.len());
    eprintln!();
    for (wc, members) in &by_weight {
        eprintln!("  {} division:", wc);
        for c in members {
            let tag = if c.exhibition { " [EXHIBITION]" } else { "" };
            eprintln!("    {} on {} ({}){}", c.model, c.node_id, c.node_url, tag);
        }
    }
    eprintln!("═══════════════════════════════════════════════════════════════════");

    // Resume from checkpoint if available
    let mut checkpoint = load_checkpoint().unwrap_or(Checkpoint {
        matches: Vec::new(),
        completed: Vec::new(),
    });
    let mut all_matches: Vec<T162> = checkpoint.matches.clone();
    let resuming = !checkpoint.completed.is_empty();
    if resuming {
        eprintln!("RESUMING — {} matches from checkpoint, {} competitor/events done",
            all_matches.len(), checkpoint.completed.len());
    }

    // Run events grouped by type for better output
    let event_order = ["sprint", "technical", "freestyle", "judged", "endurance", "doping"];
    let event_names = ["SPRINT", "TECHNICAL", "FREESTYLE", "JUDGED", "ENDURANCE", "DOPING TEST"];

    for (event, event_name) in event_order.iter().zip(event_names.iter()) {
        let event_challenges: Vec<&T166> =
            challenges.iter().filter(|c| c.event_type == *event).collect();
        if event_challenges.is_empty() { continue; }

        eprintln!("\n--- {} EVENT ({} challenges) ---", event_name, event_challenges.len());

        // Per-event DQ list: errors or too-slow models skip rest of this event
        let mut dqd: std::collections::HashSet<String> = std::collections::HashSet::new();

        for competitor in &competitors {
            let comp_key = format!("{}@{}", competitor.model, competitor.node_id);

            // Skip if DQ'd from this event
            if dqd.contains(&comp_key) { continue; }

            // Skip if already completed in checkpoint
            if checkpoint.is_done(&competitor.model, &competitor.node_id, event) {
                continue;
            }

            for ch in event_challenges.iter() {
                let tmpl = match registry.get(&ch.template_id) {
                    Some(t) => t,
                    None => continue,
                };

                let start = Instant::now();
                let result = runner::f245(
                    tmpl,
                    &ch.input,
                    &competitor.node_url,
                    Some(&competitor.model),
                );
                let duration_ms = start.elapsed().as_millis() as u64;

                let exh = if competitor.exhibition { "[EXH] " } else { "" };

                match result {
                    Ok(r) => {
                        let passed = bench::f234(&r.response, &ch.verify);
                        let tokens = r.tokens.unwrap_or(0);
                        let status = if passed { "PASS" } else { "FAIL" };
                        eprintln!(
                            "  {} {}{:<3} {:<24} {:>5}ms  {}",
                            status, exh, competitor.weight_class.short(),
                            competitor.model, duration_ms, ch.description
                        );
                        all_matches.push(T162 {
                            competitor: competitor.clone(),
                            challenge: ch.description.clone(),
                            category: ch.category.clone(),
                            passed, duration_ms, tokens,
                            response_len: r.response.len(),
                            response: r.response.clone(),
                        });

                        // WIRE-2: Record failures into feedback loop for challenge mining.
                        if !passed {
                            crate::feedback::f194(crate::feedback::T126 {
                                challenge_desc: ch.description.clone(),
                                input: ch.input.clone(),
                                expected_verify: ch.verify.clone(),
                                actual_response: r.response.clone(),
                                model: competitor.model.clone(),
                                event_type: ch.event_type.to_string(),
                                ts: 0,
                            });
                        }

                        // JIT prequal: too slow = DQ from this event
                        if duration_ms > prequal_cutoff_ms(competitor.weight_class) {
                            eprintln!(
                                "  DQ   {}{:<3} {:<24} — too slow ({}ms > {}ms limit)",
                                exh, competitor.weight_class.short(), competitor.model,
                                duration_ms, prequal_cutoff_ms(competitor.weight_class)
                            );
                            dqd.insert(comp_key.clone());
                            break;
                        }
                    }
                    Err(e) => {
                        eprintln!(
                            "  ERR  {}{:<3} {:<24} {:>5}ms  {} - {}",
                            exh, competitor.weight_class.short(),
                            competitor.model, duration_ms, ch.description, e
                        );
                        all_matches.push(T162 {
                            competitor: competitor.clone(),
                            challenge: ch.description.clone(),
                            category: ch.category.clone(),
                            passed: false, duration_ms, tokens: 0, response_len: 0,
                            response: format!("ERROR: {}", e),
                        });

                        // WIRE-2: Record errors into feedback loop.
                        crate::feedback::f194(crate::feedback::T126 {
                            challenge_desc: ch.description.clone(),
                            input: ch.input.clone(),
                            expected_verify: ch.verify.clone(),
                            actual_response: format!("ERROR: {}", e),
                            model: competitor.model.clone(),
                            event_type: ch.event_type.to_string(),
                            ts: 0,
                        });

                        // Error = DQ from this event
                        eprintln!(
                            "  DQ   {}{:<3} {:<24} — error, skipping rest of event",
                            exh, competitor.weight_class.short(), competitor.model
                        );
                        dqd.insert(comp_key.clone());
                        break;
                    }
                }
            }

            // Checkpoint after each competitor finishes an event
            checkpoint.mark_done(&competitor.model, &competitor.node_id, event);
            checkpoint.matches = all_matches.clone();
            save_checkpoint(&checkpoint);
        }

        if !dqd.is_empty() {
            eprintln!("  --- {} competitors DQ'd from {} event ---", dqd.len(), event_name);
        }
    }

    // Tournament complete — clear checkpoint
    clear_checkpoint();

    // Aggregate scores
    let mut score_map: HashMap<String, T163> = HashMap::new();
    for m in &all_matches {
        let key = format!("{}@{}", m.competitor.model, m.competitor.node_id);
        let entry = score_map.entry(key).or_insert_with(|| T163 {
            model: m.competitor.model.clone(),
            node_id: m.competitor.node_id.clone(),
            weight_class: m.competitor.weight_class,
            exhibition: m.competitor.exhibition,
            total: 0, passed: 0, failed: 0, errors: 0,
            total_duration_ms: 0, total_tokens: 0, score: 0.0,
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

    // Compute composite score
    let max_avg_ms = score_map.values().map(|s| s.avg_ms()).max().unwrap_or(1).max(1);
    for s in score_map.values_mut() {
        let speed_bonus = 20.0 * (1.0 - (s.avg_ms() as f64 / max_avg_ms as f64));
        s.score = s.accuracy() * 100.0 + speed_bonus;
    }

    let mut scores: Vec<T163> = score_map.into_values().collect();
    scores.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    // Weight class winners (best non-exhibition model per weight class)
    let mut weight_class_winners = Vec::new();
    for wc in [T160::Atomweight, T160::Flyweight, T160::Bantamweight, T160::Middleweight] {
        if let Some(best) = scores.iter().filter(|s| s.weight_class == wc && !s.exhibition).max_by(|a, b| {
            a.score.partial_cmp(&b.score).unwrap_or(std::cmp::Ordering::Equal)
        }) {
            weight_class_winners.push((wc, best.clone()));
        }
    }

    // Exhibition results (non-coder models doing code tasks)
    let exhibition_results: Vec<T163> = scores.iter().filter(|s| s.exhibition).cloned().collect();

    // Category winners (best overall per task type, excluding exhibition)
    let mut cat_best: HashMap<String, CatBestEntry> = HashMap::new();
    for m in &all_matches {
        if m.competitor.exhibition { continue; }
        let key = format!("{}@{}", m.competitor.model, m.competitor.node_id);
        let cat_key = format!("{}:{}", m.category, key);
        let entry = cat_best.entry(cat_key).or_insert_with(|| {
            (key.clone(), m.competitor.model.clone(), m.competitor.node_id.clone(), 0.0, 0, 0, 0)
        });
        entry.4 += m.duration_ms;
        entry.5 += 1;
        if m.passed { entry.6 += 1; }
    }

    let mut category_winners = Vec::new();
    let categories: Vec<String> = all_matches.iter().map(|m| m.category.clone()).collect::<std::collections::HashSet<_>>().into_iter().collect();
    for cat in &categories {
        let cat_entries: Vec<_> = cat_best.iter()
            .filter(|(k, _)| k.starts_with(&format!("{}:", cat)))
            .map(|(_, v)| v)
            .collect();
        if let Some(best) = cat_entries.iter().max_by(|a, b| {
            let acc_a = a.6 as f64 / a.5.max(1) as f64;
            let acc_b = b.6 as f64 / b.5.max(1) as f64;
            let sa = acc_a * 100.0 + 20.0 * (1.0 - (a.4 / a.5.max(1) as u64) as f64 / max_avg_ms as f64);
            let sb = acc_b * 100.0 + 20.0 * (1.0 - (b.4 / b.5.max(1) as u64) as f64 / max_avg_ms as f64);
            sa.partial_cmp(&sb).unwrap_or(std::cmp::Ordering::Equal)
        }) {
            let acc = best.6 as f64 / best.5.max(1) as f64;
            let avg = best.4 / best.5.max(1) as u64;
            let wc = T160::from_model(&best.1);
            let node_url = competitors.iter()
                .find(|c| c.node_id == best.2 && c.model == best.1)
                .map(|c| c.node_url.clone()).unwrap_or_default();
            category_winners.push(T164 {
                category: cat.clone(), model: best.1.clone(), node_id: best.2.clone(),
                node_url, weight_class: wc, accuracy: acc, avg_ms: avg,
                score: acc * 100.0 + 20.0 * (1.0 - avg as f64 / max_avg_ms as f64),
            });
        }
    }
    category_winners.sort_by(|a, b| a.category.cmp(&b.category));

    // Challenge analysis
    let mut ch_results: HashMap<String, (usize, usize)> = HashMap::new();
    for m in &all_matches {
        let e = ch_results.entry(m.challenge.clone()).or_insert((0, 0));
        e.0 += 1;
        if m.passed { e.1 += 1; }
    }
    let easy_challenges: Vec<String> = ch_results.iter()
        .filter(|(_, (t, p))| *t > 1 && *p == *t).map(|(k, _)| k.clone()).collect();
    let impossible_challenges: Vec<String> = ch_results.iter()
        .filter(|(_, (t, p))| *t > 1 && *p == 0).map(|(k, _)| k.clone()).collect();

    T165 {
        timestamp: chrono_now(),
        competitors, scores, category_winners, weight_class_winners,
        exhibition_results, matches: all_matches,
        easy_challenges, impossible_challenges,
    }
}

// ── Display ─────────────────────────────────────────────────────

/// f251=print_results
/// Print Olympic-style tournament results.
pub fn f251(r: &T165) {
    println!("\nKOVA MICRO OLYMPICS — RESULTS ({})", r.timestamp);
    println!("═══════════════════════════════════════════════════════════════════════");

    // Overall medal table
    println!("\nOVERALL STANDINGS");
    println!("───────────────────────────────────────────────────────────────────────");
    println!(
        "{:<4} {:<3} {:<28} {:<5} {:>5} {:>5} {:>5} {:>8} {:>7}",
        "Rank", "WC", "Model", "Node", "Total", "Pass", "Fail", "Avg(ms)", "Score"
    );
    println!("───────────────────────────────────────────────────────────────────────");
    for (i, s) in r.scores.iter().filter(|s| !s.exhibition).enumerate() {
        println!(
            "{:<4} {:<3} {:<28} {:<5} {:>5} {:>5} {:>5} {:>8} {:>7.1}",
            i + 1, s.weight_class.short(), s.model, s.node_id,
            s.total, s.passed, s.failed + s.errors, s.avg_ms(), s.score
        );
    }

    // Weight class champions
    if !r.weight_class_winners.is_empty() {
        println!("\nWEIGHT CLASS CHAMPIONS");
        println!("───────────────────────────────────────────────────────────────────────");
        for (wc, winner) in &r.weight_class_winners {
            println!(
                "  {} — {} on {} ({:.0}% acc, {}ms avg, score {:.1})",
                wc.label(), winner.model, winner.node_id,
                winner.accuracy() * 100.0, winner.avg_ms(), winner.score
            );
        }
    }

    // Category gold medals
    println!("\nEVENT GOLD MEDALS");
    println!("───────────────────────────────────────────────────────────────────────");
    println!(
        "{:<14} {:<3} {:<28} {:<5} {:>6} {:>8}",
        "Event", "WC", "Model", "Node", "Acc%", "Avg(ms)"
    );
    println!("───────────────────────────────────────────────────────────────────────");
    for w in &r.category_winners {
        println!(
            "{:<14} {:<3} {:<28} {:<5} {:>5.0}% {:>8}",
            w.category, w.weight_class.short(), w.model, w.node_id,
            w.accuracy * 100.0, w.avg_ms
        );
    }

    // Exhibition results
    if !r.exhibition_results.is_empty() {
        println!("\nEXHIBITION MATCHES (non-coder models doing Rust)");
        println!("───────────────────────────────────────────────────────────────────────");
        for s in &r.exhibition_results {
            println!(
                "  {} {:<3} {:<28} {:<5} {:>5}/{:<5} {:.0}% acc  {}ms avg",
                if s.accuracy() >= 0.8 { "***" } else if s.accuracy() >= 0.5 { " * " } else { "   " },
                s.weight_class.short(), s.model, s.node_id,
                s.passed, s.total, s.accuracy() * 100.0, s.avg_ms()
            );
        }
    }

    // Cross-weight analysis
    println!("\nCROSS-WEIGHT ANALYSIS");
    println!("───────────────────────────────────────────────────────────────────────");
    let wc_order = [T160::Atomweight, T160::Flyweight, T160::Bantamweight, T160::Middleweight];
    for wc in &wc_order {
        let class_scores: Vec<&T163> = r.scores.iter()
            .filter(|s| s.weight_class == *wc && !s.exhibition).collect();
        if class_scores.is_empty() { continue; }
        let avg_acc = class_scores.iter().map(|s| s.accuracy()).sum::<f64>() / class_scores.len() as f64;
        let avg_ms = class_scores.iter().map(|s| s.avg_ms()).sum::<u64>() / class_scores.len().max(1) as u64;
        println!(
            "  {:<22} {} models  {:.0}% avg acc  {}ms avg speed",
            wc.label(), class_scores.len(), avg_acc * 100.0, avg_ms
        );
    }

    // Challenge retirement candidates
    if !r.easy_challenges.is_empty() {
        println!("\nRETIREMENT CANDIDATES (all models passed):");
        for c in &r.easy_challenges {
            println!("  {}", c);
        }
    }
    if !r.impossible_challenges.is_empty() {
        println!("\nBROKEN CHALLENGES (no model passed):");
        for c in &r.impossible_challenges {
            println!("  {}", c);
        }
    }

    println!("\n═══════════════════════════════════════════════════════════════════════");
}

/// f252=save_results
/// Save tournament results.
pub fn f252(r: &T165) -> Result<(), String> {
    let path = f253();
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

/// f253=tournament_path
pub fn f253() -> PathBuf {
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

/// f254=checkpoint_path
pub fn f254() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home)
        .join(".kova")
        .join("micro")
        .join("tournament_checkpoint.json")
}

/// In-progress tournament state for resume support.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct Checkpoint {
    /// Completed matches so far.
    matches: Vec<T162>,
    /// Set of "model@node::event" keys already finished.
    completed: Vec<String>,
}

impl Checkpoint {
    fn is_done(&self, model: &str, node_id: &str, event: &str) -> bool {
        let key = format!("{}@{}::{}", model, node_id, event);
        self.completed.contains(&key)
    }

    fn mark_done(&mut self, model: &str, node_id: &str, event: &str) {
        let key = format!("{}@{}::{}", model, node_id, event);
        if !self.completed.contains(&key) {
            self.completed.push(key);
        }
    }
}

fn load_checkpoint() -> Option<Checkpoint> {
    let path = f254();
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

fn save_checkpoint(cp: &Checkpoint) {
    let path = f254();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string(cp) {
        let _ = std::fs::write(&path, json);
    }
}

fn clear_checkpoint() {
    let path = f254();
    let _ = std::fs::remove_file(path);
}

fn chrono_now() -> String {
    // Simple timestamp without chrono crate
    use std::time::SystemTime;
    let dur = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", dur.as_secs())
}