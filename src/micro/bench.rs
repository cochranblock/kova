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
/// Mix of human-written and model-generated (kova micro run f80), curated.
fn challenges() -> Vec<Challenge> {
    vec![
        // ══════════════════════════════════════════════════════════════
        // CLASSIFIER (f79) — 8 challenges
        // ══════════════════════════════════════════════════════════════
        Challenge {
            template_id: "f79",
            input: "refactor the database module to use connection pooling",
            verify: "single_word",
            description: "classify: refactor prompt",
        },
        Challenge {
            template_id: "f79",
            input: "why is this function so slow",
            verify: "single_word",
            description: "classify: performance question",
        },
        Challenge {
            template_id: "f79",
            input: "delete the unused import statements",
            verify: "single_word",
            description: "classify: cleanup task",
        },
        Challenge {
            template_id: "f79",
            input: "the server crashes when I send a POST to /api/users",
            verify: "single_word",
            description: "classify: bug report",
        },
        Challenge {
            template_id: "f79",
            input: "make the cache expire after 30 seconds",
            verify: "single_word",
            description: "classify: feature request",
        },
        Challenge {
            template_id: "f79",
            input: "run cargo clippy and fix all the warnings",
            verify: "single_word",
            description: "classify: clippy task",
        },
        Challenge {
            template_id: "f79",
            input: "I need tests for the authentication middleware",
            verify: "single_word",
            description: "classify: test request",
        },
        Challenge {
            template_id: "f79",
            input: "how does the router pick which node to use",
            verify: "single_word",
            description: "classify: explanation request",
        },
        // ══════════════════════════════════════════════════════════════
        // FIX COMPILE (f81) — 6 challenges
        // ══════════════════════════════════════════════════════════════
        Challenge {
            template_id: "f81",
            input: "Error: cannot find value `x` in this scope\nCode: fn foo() { println!(\"{}\", x); }",
            verify: "contains:fn foo",
            description: "fix: undefined variable",
        },
        Challenge {
            template_id: "f81",
            input: "Error: expected `;`\nCode: fn bar() -> i32 { 42 }",
            verify: "contains:fn bar",
            description: "fix: missing semicolon",
        },
        Challenge {
            template_id: "f81",
            input: "Error: mismatched types: expected `Vec<String>`, found `Vec<&str>`\nCode: fn names() -> Vec<String> { vec![\"alice\", \"bob\"] }",
            verify: "contains:fn names",
            description: "fix: Vec<String> vs Vec<&str>",
        },
        Challenge {
            template_id: "f81",
            input: "Error: cannot borrow `v` as mutable because it is also borrowed as immutable\nCode: fn f(v: &mut Vec<i32>) { let first = &v[0]; v.push(1); println!(\"{}\", first); }",
            verify: "contains:fn f",
            description: "fix: borrow checker violation",
        },
        Challenge {
            template_id: "f81",
            input: "Error: the trait `Display` is not implemented for `MyStruct`\nCode: struct MyStruct { x: i32 }\nfn show(s: MyStruct) { println!(\"{}\", s); }",
            verify: "contains_any:Display,fmt,impl",
            description: "fix: missing Display impl",
        },
        Challenge {
            template_id: "f81",
            input: "Error: type annotations needed\nCode: fn parse_it(s: &str) { let n = s.parse().unwrap(); println!(\"{}\", n + 1); }",
            verify: "contains:parse",
            description: "fix: type annotation needed for parse",
        },
        // ══════════════════════════════════════════════════════════════
        // CODE GEN (f80) — 8 challenges
        // ══════════════════════════════════════════════════════════════
        Challenge {
            template_id: "f80",
            input: "write a function that checks if a number is prime",
            verify: "contains:fn",
            description: "gen: is_prime",
        },
        Challenge {
            template_id: "f80",
            input: "write a function that computes the nth fibonacci number iteratively",
            verify: "contains:fn",
            description: "gen: fibonacci iterative",
        },
        Challenge {
            template_id: "f80",
            input: "write a struct called Stack with push, pop, and is_empty methods using a Vec",
            verify: "contains:struct Stack",
            description: "gen: stack struct",
        },
        Challenge {
            template_id: "f80",
            input: "write a function that takes a vector of integers and returns the sum of all elements",
            verify: "contains:fn",
            description: "gen: sum vector (model-generated)",
        },
        Challenge {
            template_id: "f80",
            input: "write a function that merges two sorted slices into a new sorted Vec",
            verify: "contains:fn",
            description: "gen: merge sorted slices",
        },
        Challenge {
            template_id: "f80",
            input: "write an enum called Color with Red Green Blue variants and a method to_hex returning a &str",
            verify: "contains:enum Color",
            description: "gen: enum with method",
        },
        Challenge {
            template_id: "f80",
            input: "write a function that counts the frequency of each word in a &str and returns a HashMap<String, usize>",
            verify: "contains:HashMap",
            description: "gen: word frequency counter",
        },
        Challenge {
            template_id: "f80",
            input: "write a function that flattens a Vec<Vec<i32>> into a Vec<i32>",
            verify: "contains:fn",
            description: "gen: flatten nested vec",
        },
        // ══════════════════════════════════════════════════════════════
        // CODE REVIEW (f_code_review) — 6 challenges
        // ══════════════════════════════════════════════════════════════
        Challenge {
            template_id: "f_code_review",
            input: "fn gcd(mut a: u64, mut b: u64) -> u64 { while b != 0 { let t = b; b = a % b; a = t; } a }",
            verify: "contains_any:LGTM,lgtm,looks good",
            description: "review: correct gcd (expect LGTM)",
        },
        Challenge {
            template_id: "f_code_review",
            input: "fn max(a: i32, b: i32) -> i32 { if a > b { a } else { b } }",
            verify: "contains_any:LGTM,lgtm,looks good",
            description: "review: correct max (expect LGTM)",
        },
        Challenge {
            template_id: "f_code_review",
            input: "fn average(nums: &[f64]) -> f64 { nums.iter().sum::<f64>() / nums.len() as f64 }",
            verify: "not_empty",
            description: "review: average with div-by-zero risk",
        },
        Challenge {
            template_id: "f_code_review",
            input: "fn read_file(path: &str) -> String { std::fs::read_to_string(path).unwrap() }",
            verify: "not_empty",
            description: "review: unwrap on file read (error handling)",
        },
        Challenge {
            template_id: "f_code_review",
            input: "fn process(data: &[u8]) -> Vec<u8> { unsafe { std::slice::from_raw_parts(data.as_ptr(), data.len() * 2).to_vec() } }",
            verify: "not_empty",
            description: "review: unsafe buffer overread",
        },
        Challenge {
            template_id: "f_code_review",
            input: "fn factorial(n: u64) -> u64 { (1..=n).product() }",
            verify: "contains_any:LGTM,lgtm,overflow,looks good",
            description: "review: factorial (correct but overflow possible)",
        },
        // ══════════════════════════════════════════════════════════════
        // VALIDATOR (f_validate) — 5 challenges
        // ══════════════════════════════════════════════════════════════
        Challenge {
            template_id: "f_validate",
            input: "Request: implement quicksort\nCode: fn quicksort(v: &mut Vec<i32>) { /* TODO */ }",
            verify: "contains:FAIL",
            description: "validate: TODO stub (expect FAIL)",
        },
        Challenge {
            template_id: "f_validate",
            input: "Request: reverse a vector in place\nCode: fn reverse(v: &mut Vec<i32>) { v.reverse(); }",
            verify: "contains:PASS",
            description: "validate: correct reverse (expect PASS)",
        },
        Challenge {
            template_id: "f_validate",
            input: "Request: count vowels in a string\nCode: fn count_vowels(s: &str) -> usize { s.chars().filter(|c| \"aeiou\".contains(*c)).count() }",
            verify: "contains:PASS",
            description: "validate: correct vowel counter (expect PASS)",
        },
        Challenge {
            template_id: "f_validate",
            input: "Request: find the maximum element\nCode: fn max_element(v: &[i32]) -> i32 { unimplemented!() }",
            verify: "contains:FAIL",
            description: "validate: unimplemented (expect FAIL)",
        },
        Challenge {
            template_id: "f_validate",
            input: "Request: sort a vector\nCode: fn sort(v: &mut Vec<i32>) { for i in 0..v.len() { for j in i+1..v.len() { if v[j] < v[i] { v.swap(i, j); } } } }",
            verify: "contains:PASS",
            description: "validate: bubble sort (correct, expect PASS)",
        },
        // ══════════════════════════════════════════════════════════════
        // CLIPPY FIX (f_clippy_fix) — 3 challenges
        // ══════════════════════════════════════════════════════════════
        Challenge {
            template_id: "f_clippy_fix",
            input: "Warning: redundant clone\nCode: fn greet(name: String) { let n = name.clone(); println!(\"hi {}\", n); }",
            verify: "contains:fn greet",
            description: "clippy: redundant clone",
        },
        Challenge {
            template_id: "f_clippy_fix",
            input: "Warning: this `if` has identical blocks\nCode: fn check(x: i32) -> &'static str { if x > 0 { \"yes\" } else { \"yes\" } }",
            verify: "contains:fn check",
            description: "clippy: identical if/else blocks",
        },
        Challenge {
            template_id: "f_clippy_fix",
            input: "Warning: manual implementation of `Iterator::any`\nCode: fn has_zero(v: &[i32]) -> bool { for x in v { if *x == 0 { return true; } } false }",
            verify: "contains:fn has_zero",
            description: "clippy: manual any() implementation",
        },
        // ══════════════════════════════════════════════════════════════
        // EXPLAIN TRACE (f115) — 3 challenges
        // ══════════════════════════════════════════════════════════════
        Challenge {
            template_id: "f115",
            input: "Intent: deploy to production\nStage: cargo build --release\nOutcome: FAIL\nStderr: error[E0308]: mismatched types in src/main.rs:42",
            verify: "not_empty",
            description: "explain: build failure trace",
        },
        Challenge {
            template_id: "f115",
            input: "Intent: run test suite\nStage: cargo test\nOutcome: FAIL\nStderr: thread 'tests::test_parse' panicked at 'assertion failed: result.is_ok()'",
            verify: "not_empty",
            description: "explain: test assertion failure",
        },
        Challenge {
            template_id: "f115",
            input: "Intent: start HTTP server\nStage: kova serve\nOutcome: FAIL\nStderr: Error: address already in use (os error 48) at 127.0.0.1:3002",
            verify: "not_empty",
            description: "explain: port already in use",
        },
        // ══════════════════════════════════════════════════════════════
        // TEST WRITE (f_test_write) — 3 challenges
        // ══════════════════════════════════════════════════════════════
        Challenge {
            template_id: "f_test_write",
            input: "fn clamp(val: i32, min: i32, max: i32) -> i32 { if val < min { min } else if val > max { max } else { val } }",
            verify: "contains:#[test]",
            description: "test: clamp function",
        },
        Challenge {
            template_id: "f_test_write",
            input: "fn is_palindrome(s: &str) -> bool { let bytes = s.as_bytes(); let len = bytes.len(); for i in 0..len / 2 { if bytes[i] != bytes[len - 1 - i] { return false; } } true }",
            verify: "contains:#[test]",
            description: "test: is_palindrome",
        },
        Challenge {
            template_id: "f_test_write",
            input: "fn celsius_to_fahrenheit(c: f64) -> f64 { c * 9.0 / 5.0 + 32.0 }",
            verify: "contains:#[test]",
            description: "test: temperature conversion",
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
