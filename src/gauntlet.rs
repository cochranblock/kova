// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! gauntlet — Hell Week for the IRONHIVE AI pipeline.
//! Progressive difficulty challenges that test factory, MoE, and academy.
//! If it can't survive this, it's not worth deploying.
//!
//! Phases:
//!   Phase 1: Crawl (trivial code gen — must be 100%)
//!   Phase 2: Walk (medium complexity — algorithms, data structures)
//!   Phase 3: Run (hard — parsers, state machines, real-world tools)
//!   Phase 4: Fight (adversarial — ambiguous prompts, edge cases, traps)
//!   Phase 5: Survive (endurance — chained tasks, large context, multi-file)

use crate::factory::{Factory, FactoryConfig};
use crate::moe::{self, MoeConfig};
use std::time::Instant;

/// A single gauntlet challenge.
struct Challenge {
    phase: u8,
    name: &'static str,
    prompt: &'static str,
    /// Expected patterns in the output code (all must be present).
    must_contain: &'static [&'static str],
    /// Patterns that must NOT appear (traps).
    must_not_contain: &'static [&'static str],
    /// Use MoE instead of factory for this challenge.
    use_moe: bool,
    /// Maximum allowed time in seconds.
    max_secs: u64,
}

/// Result of a single challenge.
#[derive(Debug)]
#[allow(dead_code)]
struct ChallengeResult {
    phase: u8,
    name: String,
    passed: bool,
    compiled: bool,
    content_ok: bool,
    time_secs: f64,
    failure_reason: String,
}

/// Full gauntlet report.
#[derive(Debug)]
pub struct GauntletReport {
    results: Vec<ChallengeResult>,
    pub phase_scores: Vec<(u8, usize, usize)>, // (phase, passed, total)
    pub total_passed: usize,
    pub total_challenges: usize,
    pub total_time_secs: f64,
}

impl GauntletReport {
    pub fn grade(&self) -> &'static str {
        let pct = if self.total_challenges == 0 {
            0.0
        } else {
            self.total_passed as f64 / self.total_challenges as f64 * 100.0
        };
        match pct as u32 {
            95..=100 => "GREEN BERET — combat ready",
            85..=94 => "RANGER — field deployable",
            70..=84 => "INFANTRY — basic competence",
            50..=69 => "RECRUIT — needs training",
            25..=49 => "WASHOUT — serious deficiencies",
            _ => "REJECT — not fit for service",
        }
    }
}

// ── Phase 1: Crawl ──────────────────────────────────────────────

const PHASE1: &[Challenge] = &[
    Challenge {
        phase: 1,
        name: "fibonacci",
        prompt: "Write a function `fn fib(n: u64) -> u64` that returns the nth Fibonacci number. Include tests for fib(0)=0, fib(1)=1, fib(10)=55.",
        must_contain: &["fn fib", "fn ", "55"],
        must_not_contain: &["tokio", "async"],
        use_moe: false,
        max_secs: 300,
    },
    Challenge {
        phase: 1,
        name: "reverse_string",
        prompt: "Write a function `fn reverse(s: &str) -> String` that reverses a string. Handle unicode correctly. Include tests.",
        must_contain: &["fn reverse", "String"],
        must_not_contain: &[],
        use_moe: false,
        max_secs: 300,
    },
    Challenge {
        phase: 1,
        name: "max_element",
        prompt: "Write a function `fn find_max(nums: &[i32]) -> Option<i32>` that returns the maximum element. Return None for empty slices. Include tests.",
        must_contain: &["fn find_max", "Option<i32>", "None"],
        must_not_contain: &[],
        use_moe: false,
        max_secs: 300,
    },
    Challenge {
        phase: 1,
        name: "is_palindrome",
        prompt: "Write a function `fn is_palindrome(s: &str) -> bool` that checks if a string is a palindrome, ignoring case and non-alphanumeric characters. Include tests for 'racecar', 'A man a plan a canal Panama', and 'hello'.",
        must_contain: &["fn is_palindrome", "bool"],
        must_not_contain: &[],
        use_moe: false,
        max_secs: 300,
    },
    Challenge {
        phase: 1,
        name: "fizzbuzz",
        prompt: "Write a function `fn fizzbuzz(n: u32) -> Vec<String>` that returns FizzBuzz output for 1..=n. Divisible by 3: 'Fizz', by 5: 'Buzz', by both: 'FizzBuzz', otherwise the number as string. Include tests.",
        must_contain: &["fn fizzbuzz", "Fizz", "Buzz"],
        must_not_contain: &[],
        use_moe: false,
        max_secs: 300,
    },
];

// ── Phase 2: Walk ───────────────────────────────────────────────

const PHASE2: &[Challenge] = &[
    Challenge {
        phase: 2,
        name: "binary_search",
        prompt: "Write a function `fn binary_search(arr: &[i32], target: i32) -> Option<usize>` that performs binary search. Do not use the standard library's binary_search. Include tests for found, not found, empty array, single element.",
        must_contain: &["fn binary_search", "Option<usize>"],
        must_not_contain: &[".binary_search("],
        use_moe: false,
        max_secs: 300,
    },
    Challenge {
        phase: 2,
        name: "linked_list",
        prompt: "Implement a singly linked list with push_front, pop_front, len, and iter methods. Use `Option<Box<Node<T>>>` for links. Include tests.",
        must_contain: &["struct", "Box", "Option", "push_front", "pop_front"],
        must_not_contain: &["use std::collections::LinkedList"],
        use_moe: false,
        max_secs: 300,
    },
    Challenge {
        phase: 2,
        name: "matrix_multiply",
        prompt: "Write a function that multiplies two 2D matrices represented as Vec<Vec<f64>>. Return Err if dimensions don't match. Include tests for 2x3 * 3x2, identity matrix multiplication, and dimension mismatch error.",
        must_contain: &["Vec<Vec<f64>>", "Err"],
        must_not_contain: &[],
        use_moe: false,
        max_secs: 300,
    },
    Challenge {
        phase: 2,
        name: "lru_cache",
        prompt: "Implement an LRU cache with a fixed capacity. Methods: new(capacity), get(key) -> Option<value>, put(key, value). When capacity is exceeded, evict the least recently used item. Use only std HashMap and VecDeque. Include tests.",
        must_contain: &["HashMap", "capacity", "fn get", "fn put"],
        must_not_contain: &["use lru"],
        use_moe: false,
        max_secs: 300,
    },
    Challenge {
        phase: 2,
        name: "sieve_moe",
        prompt: "Write a function that finds all prime numbers up to N using the Sieve of Eratosthenes. Include comprehensive tests for N=0, N=1, N=2, N=10, N=100.",
        must_contain: &["fn ", "vec!", "true"],
        must_not_contain: &[],
        use_moe: true,
        max_secs: 300,
    },
];

// ── Phase 3: Run ────────────────────────────────────────────────

const PHASE3: &[Challenge] = &[
    Challenge {
        phase: 3,
        name: "json_parser",
        prompt: "Write a simple JSON value parser that can parse: null, booleans, numbers (integers), strings (with \\\" and \\\\ escapes), arrays, and objects. Define an enum JsonValue with variants for each type. Write a `fn parse(input: &str) -> Result<JsonValue, String>` function. Include tests for each JSON type and nested structures like {\"a\":[1,true,null]}.",
        must_contain: &["enum", "JsonValue", "parse", "Result"],
        must_not_contain: &["serde_json", "serde"],
        use_moe: true,
        max_secs: 600,
    },
    Challenge {
        phase: 3,
        name: "state_machine",
        prompt: "Implement a finite state machine for a simple traffic light: Red -> Green -> Yellow -> Red. States have minimum durations (Red: 30s, Green: 25s, Yellow: 5s). Methods: new(), tick(elapsed_secs), current_state(), can_transition(). Include tests that verify the full cycle and that early transitions are rejected.",
        must_contain: &["enum", "Red", "Green", "Yellow", "fn tick"],
        must_not_contain: &[],
        use_moe: true,
        max_secs: 600,
    },
    Challenge {
        phase: 3,
        name: "expression_evaluator",
        prompt: "Write a math expression evaluator that handles +, -, *, / with correct operator precedence and parentheses. Input is a string like \"2 + 3 * (4 - 1)\". Return f64 result or error for invalid input. Handle division by zero. Include tests.",
        must_contain: &["fn ", "f64", "Result"],
        must_not_contain: &[],
        use_moe: true,
        max_secs: 600,
    },
    Challenge {
        phase: 3,
        name: "thread_pool",
        prompt: "Implement a fixed-size thread pool. Methods: new(num_threads), execute(closure), shutdown(). Jobs are closures sent via channel. Threads pull from a shared job queue. Include tests that submit 10 jobs to a pool of 3 threads and verify all complete.",
        must_contain: &["struct", "thread", "Mutex", "fn execute", "fn shutdown"],
        must_not_contain: &["tokio", "rayon"],
        use_moe: true,
        max_secs: 600,
    },
];

// ── Phase 4: Fight ──────────────────────────────────────────────

const PHASE4: &[Challenge] = &[
    Challenge {
        phase: 4,
        name: "ambiguous_prompt",
        prompt: "Write a function that processes items",
        must_contain: &["fn "],
        must_not_contain: &[],
        use_moe: false,
        max_secs: 300,
    },
    Challenge {
        phase: 4,
        name: "type_trap",
        prompt: "Write a function that formats a table of data. Each row has a name (String) and values (Vec<f64>). Column widths should adapt to content. If a column header is wider than all values, use the header width. If a value is wider, use the value width. Return the formatted table as a String with proper alignment. Use only &str and String — no mixing &str with String in if/else branches without conversion.",
        must_contain: &["fn ", "String", "format!"],
        must_not_contain: &[],
        use_moe: true,
        max_secs: 600,
    },
    Challenge {
        phase: 4,
        name: "lifetime_challenge",
        prompt: "Write a struct `TextIndex` that takes a &str reference to a text document and builds a word index (HashMap<&str, Vec<usize>> mapping words to line numbers). Method `search(&self, word: &str) -> &[usize]` returns line numbers where a word appears. Lifetimes must be correct. Include tests.",
        must_contain: &["struct TextIndex", "HashMap", "fn search"],
        must_not_contain: &[],
        use_moe: true,
        max_secs: 600,
    },
    Challenge {
        phase: 4,
        name: "trait_objects",
        prompt: "Define a trait `Shape` with methods `area(&self) -> f64` and `name(&self) -> &str`. Implement it for Circle (radius), Rectangle (width, height), and Triangle (base, height). Write a function `fn largest(shapes: &[Box<dyn Shape>]) -> Option<&dyn Shape>` that returns the shape with the largest area. Include tests.",
        must_contain: &["trait Shape", "dyn Shape", "Circle", "Rectangle", "Triangle", "fn largest"],
        must_not_contain: &[],
        use_moe: true,
        max_secs: 600,
    },
];

// ── Phase 5: Survive ────────────────────────────────────────────

const PHASE5: &[Challenge] = &[
    Challenge {
        phase: 5,
        name: "cli_tool_moe",
        prompt: "Write a CLI tool that reads CSV from stdin or a file argument, and outputs a summary: row count, column names, min/max/avg for numeric columns, unique value count for string columns. Handle malformed rows by skipping them and printing a warning to stderr. Accept --delimiter to change the separator (default comma). Include a main function.",
        must_contain: &["fn main", "stdin", "delimiter"],
        must_not_contain: &["csv =", "clap ="],
        use_moe: true,
        max_secs: 600,
    },
    Challenge {
        phase: 5,
        name: "concurrent_map",
        prompt: "Implement a concurrent hash map safe for multi-threaded access. Use sharding: split the map into N shards, each protected by its own RwLock. Methods: new(shards), insert(key, value), get(key) -> Option<value>, remove(key), len(). The shard is determined by hashing the key. Include tests that spawn 4 threads doing concurrent inserts and reads.",
        must_contain: &["RwLock", "struct", "fn insert", "fn get", "thread::spawn"],
        must_not_contain: &["dashmap", "crossbeam"],
        use_moe: true,
        max_secs: 600,
    },
    Challenge {
        phase: 5,
        name: "mini_regex",
        prompt: "Implement a minimal regex engine that supports: literal characters, . (any char), * (zero or more of previous), + (one or more of previous), ? (zero or one of previous), ^ (start anchor), $ (end anchor). Write `fn matches(pattern: &str, text: &str) -> bool`. Include tests for each operator and combinations like \"a.*b\" matching \"aXYZb\".",
        must_contain: &["fn matches", "bool"],
        must_not_contain: &["use regex"],
        use_moe: true,
        max_secs: 600,
    },
];

/// Run the full gauntlet. Returns report.
pub fn run_gauntlet(phases: Option<Vec<u8>>) -> GauntletReport {
    let all_phases: Vec<&[Challenge]> = vec![PHASE1, PHASE2, PHASE3, PHASE4, PHASE5];
    let run_phases = phases.unwrap_or_else(|| vec![1, 2, 3, 4, 5]);

    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║          IRONHIVE GAUNTLET — HELL WEEK                  ║");
    println!("║          Survive or wash out. No mercy.                 ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();

    let start = Instant::now();
    let mut results: Vec<ChallengeResult> = Vec::new();

    for phase_num in &run_phases {
        let phase_idx = (*phase_num as usize).saturating_sub(1);
        if phase_idx >= all_phases.len() {
            continue;
        }
        let challenges = all_phases[phase_idx];
        let phase_name = match phase_num {
            1 => "CRAWL",
            2 => "WALK",
            3 => "RUN",
            4 => "FIGHT",
            5 => "SURVIVE",
            _ => "UNKNOWN",
        };

        println!(
            "═══ Phase {}: {} ({} challenges) ═══",
            phase_num,
            phase_name,
            challenges.len()
        );
        println!();

        for challenge in challenges {
            let result = run_challenge(challenge);
            let icon = if result.passed { "PASS" } else { "FAIL" };
            println!(
                "  [{}] {} — {:.1}s{}",
                icon,
                result.name,
                result.time_secs,
                if result.passed {
                    String::new()
                } else {
                    format!(" ({})", result.failure_reason)
                }
            );
            results.push(result);
        }
        println!();
    }

    let total_time = start.elapsed().as_secs_f64();

    // Compute phase scores
    let mut phase_scores: Vec<(u8, usize, usize)> = Vec::new();
    for phase_num in &run_phases {
        let phase_results: Vec<&ChallengeResult> =
            results.iter().filter(|r| r.phase == *phase_num).collect();
        let passed = phase_results.iter().filter(|r| r.passed).count();
        let total = phase_results.len();
        phase_scores.push((*phase_num, passed, total));
    }

    let total_passed = results.iter().filter(|r| r.passed).count();
    let total_challenges = results.len();

    let report = GauntletReport {
        results,
        phase_scores,
        total_passed,
        total_challenges,
        total_time_secs: total_time,
    };

    // Print final report
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║                    GAUNTLET REPORT                      ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();

    for (phase, passed, total) in &report.phase_scores {
        let phase_name = match phase {
            1 => "Crawl",
            2 => "Walk",
            3 => "Run",
            4 => "Fight",
            5 => "Survive",
            _ => "?",
        };
        let pct = if *total == 0 { 0 } else { passed * 100 / total };
        let bar = "█".repeat(*passed) + &"░".repeat(total - passed);
        println!(
            "  Phase {} ({}): {}/{} [{}] {}%",
            phase, phase_name, passed, total, bar, pct
        );
    }

    println!();
    println!(
        "  Total: {}/{} ({:.0}%)",
        report.total_passed,
        report.total_challenges,
        if report.total_challenges == 0 {
            0.0
        } else {
            report.total_passed as f64 / report.total_challenges as f64 * 100.0
        }
    );
    println!("  Time: {:.0}s", report.total_time_secs);
    println!();
    println!("  Grade: {}", report.grade());
    println!();

    // Failures detail
    let failures: Vec<&ChallengeResult> = report.results.iter().filter(|r| !r.passed).collect();
    if !failures.is_empty() {
        println!("  Failures:");
        for f in &failures {
            println!("    Phase {} / {} — {}", f.phase, f.name, f.failure_reason);
        }
        println!();
    }

    report
}

/// Run a single challenge through factory or MoE.
fn run_challenge(challenge: &Challenge) -> ChallengeResult {
    let start = Instant::now();

    let (code, compiled) = if challenge.use_moe {
        run_moe_challenge(challenge)
    } else {
        run_factory_challenge(challenge)
    };

    let elapsed = start.elapsed().as_secs_f64();

    // Time check
    if elapsed > challenge.max_secs as f64 {
        return ChallengeResult {
            phase: challenge.phase,
            name: challenge.name.to_string(),
            passed: false,
            compiled,
            content_ok: false,
            time_secs: elapsed,
            failure_reason: format!("timeout ({:.0}s > {}s)", elapsed, challenge.max_secs),
        };
    }

    // Compile check
    if !compiled {
        return ChallengeResult {
            phase: challenge.phase,
            name: challenge.name.to_string(),
            passed: false,
            compiled: false,
            content_ok: false,
            time_secs: elapsed,
            failure_reason: "did not compile".into(),
        };
    }

    // Content checks
    let code_lower = code.to_lowercase();
    for pattern in challenge.must_contain {
        if !code.contains(pattern) && !code_lower.contains(&pattern.to_lowercase()) {
            return ChallengeResult {
                phase: challenge.phase,
                name: challenge.name.to_string(),
                passed: false,
                compiled: true,
                content_ok: false,
                time_secs: elapsed,
                failure_reason: format!("missing required pattern: {}", pattern),
            };
        }
    }

    for pattern in challenge.must_not_contain {
        if code.contains(pattern) {
            return ChallengeResult {
                phase: challenge.phase,
                name: challenge.name.to_string(),
                passed: false,
                compiled: true,
                content_ok: false,
                time_secs: elapsed,
                failure_reason: format!("contains forbidden pattern: {}", pattern),
            };
        }
    }

    ChallengeResult {
        phase: challenge.phase,
        name: challenge.name.to_string(),
        passed: true,
        compiled: true,
        content_ok: true,
        time_secs: elapsed,
        failure_reason: String::new(),
    }
}

fn run_factory_challenge(challenge: &Challenge) -> (String, bool) {
    let config = FactoryConfig {
        max_fix_retries: 4,
        run_clippy: true,
        run_tests: true,
        run_review: false, // Skip review in gauntlet — we just need compile+test
        num_ctx: 8192,
        ..Default::default()
    };

    let factory = Factory::new(config);
    let project_dir = std::env::current_dir().unwrap_or_default();
    let result = factory.run(challenge.prompt, &project_dir);

    (result.code, result.success)
}

fn run_moe_challenge(challenge: &Challenge) -> (String, bool) {
    let config = MoeConfig {
        num_experts: 2,
        run_clippy: true,
        run_tests: true,
        run_review: false,
        num_ctx: 8192,
        save_winner: false,
    };

    let result = moe::run_moe(challenge.prompt, config);

    match result.winner {
        Some(idx) => (result.variants[idx].code.clone(), true),
        None => {
            // Check if any variant has code even if none compiled
            let code = result
                .variants
                .first()
                .map(|v| v.code.clone())
                .unwrap_or_default();
            (code, false)
        }
    }
}
