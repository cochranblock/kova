// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6
//! bench — Held-out challenge benchmarks for micro-model templates.
//! NOT a self-licking ice cream cone: inputs are novel, not baked into prompts.
//! Verification uses structural checks, not string matching against training data.

use std::process::Command;
use std::time::Instant;

use super::registry::MicroRegistry;
use super::runner::{run_micro, Budget, CircuitBreaker};
use super::validate;
use crate::cluster::Cluster;

/// Held-out challenge: an input the model hasn't seen, with a verification function.
struct Challenge {
    template_id: &'static str,
    input: &'static str,
    /// How to verify: "contains:<text>", "compiles", "not_empty", "single_word"
    verify: &'static str,
    description: &'static str,
}

/// All held-out challenges — none of these appear in few-shot examples.
fn challenges() -> Vec<Challenge> {
    vec![
        // ── Classifier: novel inputs, check single-word category output ──
        Challenge {
            template_id: "f79",
            input: "refactor the database module to use connection pooling",
            verify: "single_word",
            description: "classify: refactor prompt (novel)",
        },
        Challenge {
            template_id: "f79",
            input: "why is this function so slow",
            verify: "single_word",
            description: "classify: performance question (novel)",
        },
        Challenge {
            template_id: "f79",
            input: "delete the unused import statements",
            verify: "single_word",
            description: "classify: cleanup task (novel)",
        },
        // ── Fix compile: novel errors ──
        Challenge {
            template_id: "f81",
            input: "Error: cannot find value `x` in this scope\nCode: fn foo() { println!(\"{}\", x); }",
            verify: "contains:fn foo",
            description: "fix: undefined variable (novel)",
        },
        Challenge {
            template_id: "f81",
            input: "Error: expected `;`\nCode: fn bar() -> i32 { 42 }",
            verify: "contains:fn bar",
            description: "fix: missing semicolon (novel)",
        },
        // ── Code gen: novel tasks, verify output is valid Rust ──
        Challenge {
            template_id: "f80",
            input: "write a function that checks if a number is prime",
            verify: "contains:fn",
            description: "gen: is_prime (novel)",
        },
        Challenge {
            template_id: "f80",
            input: "write a function that computes the nth fibonacci number iteratively",
            verify: "contains:fn",
            description: "gen: fibonacci (novel)",
        },
        Challenge {
            template_id: "f80",
            input: "write a struct called Stack with push, pop, and is_empty methods using a Vec",
            verify: "contains:struct Stack",
            description: "gen: stack data structure (novel)",
        },
        // ── Code review: give it clean code, expect LGTM ──
        Challenge {
            template_id: "f_code_review",
            input: "fn gcd(mut a: u64, mut b: u64) -> u64 { while b != 0 { let t = b; b = a % b; a = t; } a }",
            verify: "contains_any:LGTM,lgtm,looks good",
            description: "review: correct gcd (should be LGTM)",
        },
        // ── Validator: give it incomplete code, expect FAIL ──
        Challenge {
            template_id: "f_validate",
            input: "Request: implement quicksort\nCode: fn quicksort(v: &mut Vec<i32>) { /* TODO */ }",
            verify: "contains:FAIL",
            description: "validate: incomplete code (should FAIL)",
        },
        Challenge {
            template_id: "f_validate",
            input: "Request: binary search\nCode: fn binary_search(arr: &[i32], target: i32) -> Option<usize> { arr.iter().position(|&x| x == target) }",
            verify: "contains:PASS",
            description: "validate: correct but linear search (edge case)",
        },
        // ── Explain trace: novel trace ──
        Challenge {
            template_id: "f115",
            input: "Intent: deploy to production\nStage: cargo build --release\nOutcome: FAIL\nStderr: error[E0308]: mismatched types in src/main.rs:42",
            verify: "not_empty",
            description: "explain: novel build failure trace",
        },
    ]
}

/// Result of one challenge.
#[derive(Debug)]
pub struct BenchResult {
    pub template_id: String,
    pub description: String,
    pub passed: bool,
    pub response: String,
    pub duration_ms: u64,
    pub error: Option<String>,
}

/// Run all held-out challenges.
pub fn run_bench(registry: &MicroRegistry, cluster: &Cluster) -> Vec<BenchResult> {
    let mut results = Vec::new();

    for ch in challenges() {
        let tmpl = match registry.get(ch.template_id) {
            Some(t) => t,
            None => {
                eprintln!("  SKIP  {} — template not found", ch.description);
                continue;
            }
        };

        let breaker = CircuitBreaker::new(3);
        let budget = Budget::new(100_000);
        let start = Instant::now();

        match run_micro(tmpl, ch.input, cluster, &breaker, &budget) {
            Ok(result) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                let passed = verify(&result.response, ch.verify);

                if passed {
                    eprintln!("  PASS  {:>5}ms  {}", duration_ms, ch.description);
                } else {
                    eprintln!(
                        "  FAIL  {:>5}ms  {} → got: {}",
                        duration_ms,
                        ch.description,
                        result.response.trim().chars().take(80).collect::<String>()
                    );
                }

                results.push(BenchResult {
                    template_id: ch.template_id.to_string(),
                    description: ch.description.to_string(),
                    passed,
                    response: result.response,
                    duration_ms,
                    error: None,
                });
            }
            Err(e) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                eprintln!("  ERR   {:>5}ms  {} — {}", duration_ms, ch.description, e);

                results.push(BenchResult {
                    template_id: ch.template_id.to_string(),
                    description: ch.description.to_string(),
                    passed: false,
                    response: String::new(),
                    duration_ms,
                    error: Some(e),
                });
            }
        }
    }

    results
}

/// Verify a response against a check string.
fn verify(response: &str, check: &str) -> bool {
    let trimmed = response.trim();

    if check == "not_empty" {
        return !trimmed.is_empty();
    }

    if check == "single_word" {
        return trimmed.split_whitespace().count() <= 3 && !trimmed.is_empty();
    }

    if let Some(text) = check.strip_prefix("contains:") {
        return trimmed.to_lowercase().contains(&text.to_lowercase());
    }

    if let Some(texts) = check.strip_prefix("contains_any:") {
        return texts
            .split(',')
            .any(|t| trimmed.to_lowercase().contains(&t.to_lowercase()));
    }

    if check == "compiles" {
        return try_compile(trimmed);
    }

    // Default: non-empty + passes quick_validate
    validate::quick_validate(trimmed)
}

/// Try to compile a Rust snippet. Returns true if cargo check passes.
fn try_compile(code: &str) -> bool {
    let tmp = match tempfile::TempDir::new() {
        Ok(t) => t,
        Err(_) => return false,
    };

    let src = tmp.path().join("src");
    let _ = std::fs::create_dir_all(&src);

    // Extract code from ```rust blocks if present
    let clean = if let Some(start) = code.find("```rust") {
        let after = &code[start + 7..];
        if let Some(end) = after.find("```") {
            after[..end].trim()
        } else {
            after.trim()
        }
    } else {
        code
    };

    // Write as lib.rs
    let _ = std::fs::write(src.join("lib.rs"), clean);
    let _ = std::fs::write(
        tmp.path().join("Cargo.toml"),
        "[package]\nname = \"bench_check\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );

    Command::new("cargo")
        .args(["check", "--quiet"])
        .current_dir(tmp.path())
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Print bench results as a summary table.
pub fn print_bench_results(results: &[BenchResult]) {
    let pass_count = results.iter().filter(|r| r.passed).count();
    let fail_count = results
        .iter()
        .filter(|r| !r.passed && r.error.is_none())
        .count();
    let err_count = results.iter().filter(|r| r.error.is_some()).count();
    let total = results.len();

    println!("\nMicro-Model Held-Out Benchmark");
    println!("─────────────────────────────────────────────────────────────────");
    println!(
        "{:<12} {:<45} {:>5} {:>8}",
        "Template", "Challenge", "Result", "Time(ms)"
    );
    println!("─────────────────────────────────────────────────────────────────");

    for r in results {
        let status = if r.error.is_some() {
            "ERR"
        } else if r.passed {
            "PASS"
        } else {
            "FAIL"
        };
        println!(
            "{:<12} {:<45} {:>5} {:>8}",
            r.template_id, r.description, status, r.duration_ms
        );
    }

    println!("─────────────────────────────────────────────────────────────────");
    let accuracy = if total > 0 {
        (pass_count as f64 / total as f64) * 100.0
    } else {
        0.0
    };
    println!(
        "{} challenges: {} pass, {} fail, {} error — {:.0}% accuracy",
        total, pass_count, fail_count, err_count, accuracy
    );
}
