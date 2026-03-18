// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6
//! bench — Held-out challenge benchmarks for micro-model templates.
//! NOT a self-licking ice cream cone: inputs are novel, not baked into prompts.
//! Verification uses structural checks, not string matching against training data.

use std::process::Command;
use std::time::Instant;

use super::registry::T149;
use super::runner::{f244, T156, T155};
use super::validate;
use crate::cluster::T193;

/// Held-out challenge: an input the model hasn't seen, with a verification function.
struct Challenge {
    template_id: &'static str,
    input: &'static str,
    /// How to verify: "contains:<text>", "compiles", "not_empty", "single_word"
    verify: &'static str,
    description: &'static str,
}

/// All held-out challenges — none of these appear in few-shot examples.
/// Mix of human-written, model-generated (kova micro run f80), and prompt-mined.
/// Curriculum tiers: EASY (warm-up), MEDIUM (working knowledge), HARD (differentiator).
fn challenges() -> Vec<Challenge> {
    vec![
        // ══════════════════════════════════════════════════════════════
        // CLASSIFIER (f79) — 12 challenges
        // Mined from: f79 few-shot patterns + real kova user inputs
        // ══════════════════════════════════════════════════════════════
        // EASY — clear single-category
        Challenge { template_id: "f79", input: "refactor the database module to use connection pooling", verify: "single_word", description: "classify: refactor prompt" },
        Challenge { template_id: "f79", input: "why is this function so slow", verify: "single_word", description: "classify: performance question" },
        Challenge { template_id: "f79", input: "delete the unused import statements", verify: "single_word", description: "classify: cleanup task" },
        Challenge { template_id: "f79", input: "the server crashes when I send a POST to /api/users", verify: "single_word", description: "classify: bug report" },
        // MEDIUM — less obvious category
        Challenge { template_id: "f79", input: "make the cache expire after 30 seconds", verify: "single_word", description: "classify: feature request" },
        Challenge { template_id: "f79", input: "run cargo clippy and fix all the warnings", verify: "single_word", description: "classify: clippy task" },
        Challenge { template_id: "f79", input: "I need tests for the authentication middleware", verify: "single_word", description: "classify: test request" },
        Challenge { template_id: "f79", input: "how does the router pick which node to use", verify: "single_word", description: "classify: explanation request" },
        // HARD — ambiguous, multi-category, or uncommon phrasing
        Challenge { template_id: "f79", input: "this function panics on empty input and also has a clippy warning about redundant clone", verify: "single_word", description: "classify: ambiguous bug+clippy" },
        Challenge { template_id: "f79", input: "split the monolithic handle_request into smaller functions and add tests for each", verify: "single_word", description: "classify: ambiguous refactor+test" },
        Challenge { template_id: "f79", input: "look at the diff between v0.1 and v0.2 and tell me what changed", verify: "single_word", description: "classify: diff review request" },
        Challenge { template_id: "f79", input: "the borrow checker says I can't do this but I think I should be able to", verify: "single_word", description: "classify: explain+fix ambiguity" },

        // ══════════════════════════════════════════════════════════════
        // FIX COMPILE (f81) — 10 challenges
        // Mined from: f81 few-shot (String/&str pattern) + real rustc errors
        // ══════════════════════════════════════════════════════════════
        // EASY — simple fixes (compile-verified: the fix must actually compile)
        Challenge { template_id: "f81", input: "Error: cannot find value `x` in this scope\nCode: fn foo() { println!(\"{}\", x); }", verify: "compiles_and:contains:fn foo", description: "fix: undefined variable" },
        Challenge { template_id: "f81", input: "Error: expected `;`\nCode: fn bar() -> i32 { 42 }", verify: "compiles_and:contains:fn bar", description: "fix: missing semicolon" },
        // MEDIUM — type system (compile-verified)
        Challenge { template_id: "f81", input: "Error: mismatched types: expected `Vec<String>`, found `Vec<&str>`\nCode: fn names() -> Vec<String> { vec![\"alice\", \"bob\"] }", verify: "compiles_and:contains:fn names", description: "fix: Vec<String> vs Vec<&str>" },
        Challenge { template_id: "f81", input: "Error: the trait `Display` is not implemented for `MyStruct`\nCode: struct MyStruct { x: i32 }\nfn show(s: MyStruct) { println!(\"{}\", s); }", verify: "compiles", description: "fix: missing Display impl" },
        Challenge { template_id: "f81", input: "Error: type annotations needed\nCode: fn parse_it(s: &str) { let n = s.parse().unwrap(); println!(\"{}\", n + 1); }", verify: "compiles_and:contains:parse", description: "fix: type annotation needed" },
        // HARD — ownership/lifetime puzzles (compile-verified)
        Challenge { template_id: "f81", input: "Error: cannot borrow `v` as mutable because it is also borrowed as immutable\nCode: fn f(v: &mut Vec<i32>) { let first = &v[0]; v.push(1); println!(\"{}\", first); }", verify: "compiles", description: "fix: borrow checker violation" },
        Challenge { template_id: "f81", input: "Error: `s` does not live long enough\nCode: fn get_first_word(text: &str) -> &str { let s = text.to_string(); &s[..s.find(' ').unwrap_or(s.len())] }", verify: "compiles", description: "fix: dangling reference" },
        Challenge { template_id: "f81", input: "Error: lifetime may not live long enough\nCode: struct Wrapper<'a> { data: &'a str }\nimpl Wrapper<'_> { fn get(&self) -> &str { self.data } }", verify: "compiles", description: "fix: lifetime elision in impl" },
        Challenge { template_id: "f81", input: "Error: the trait bound `T: Clone` is not satisfied\nCode: fn dup<T>(x: T) -> (T, T) { (x.clone(), x) }", verify: "compiles", description: "fix: missing trait bound" },
        Challenge { template_id: "f81", input: "Error: cannot move out of `*self` which is behind a shared reference\nCode: struct Node { val: String, next: Option<Box<Node>> }\nimpl Node { fn take_val(self) -> String { self.val } fn borrow_take(&self) -> String { self.take_val() } }", verify: "compiles", description: "fix: move behind shared ref" },

        // ══════════════════════════════════════════════════════════════
        // CODE GEN (f80) — 12 challenges
        // Mined from: real kova pipeline requests + escalating difficulty
        // ══════════════════════════════════════════════════════════════
        // EASY — single function (compile-verified)
        Challenge { template_id: "f80", input: "write a function that checks if a number is prime", verify: "compiles_and:contains:fn", description: "gen: is_prime" },
        Challenge { template_id: "f80", input: "write a function that computes the nth fibonacci number iteratively", verify: "compiles_and:contains:fn", description: "gen: fibonacci" },
        Challenge { template_id: "f80", input: "write a function that flattens a Vec<Vec<i32>> into a Vec<i32>", verify: "compiles_and:contains:fn", description: "gen: flatten nested vec" },
        // MEDIUM — structs, enums, traits (compile-verified)
        Challenge { template_id: "f80", input: "write a struct called Stack with push, pop, and is_empty methods using a Vec", verify: "compiles_and:contains:struct Stack", description: "gen: stack struct" },
        Challenge { template_id: "f80", input: "write a function that merges two sorted slices into a new sorted Vec", verify: "compiles_and:contains:fn", description: "gen: merge sorted" },
        Challenge { template_id: "f80", input: "write an enum called Color with Red Green Blue variants and a method to_hex returning a &str", verify: "compiles_and:contains:enum Color", description: "gen: enum with method" },
        Challenge { template_id: "f80", input: "write a function that counts the frequency of each word in a &str and returns a HashMap<String, usize>", verify: "compiles_and:contains:HashMap", description: "gen: word frequency" },
        // HARD — generics, traits, lifetimes, async (compile-verified)
        Challenge { template_id: "f80", input: "write a generic LRU cache struct with get and put methods. Use a HashMap and a VecDeque. Capacity set at construction.", verify: "compiles_and:contains_any:struct,LRU,Lru,Cache,cache", description: "gen: LRU cache" },
        Challenge { template_id: "f80", input: "write a trait called Summarize with a method summary() -> String, then implement it for a struct Article with title and body fields", verify: "compiles_and:contains_any:trait Summarize,impl Summarize", description: "gen: trait + impl" },
        Challenge { template_id: "f80", input: "write a function that takes &[&str] and returns the longest common prefix as a String", verify: "compiles_and:contains:fn", description: "gen: longest common prefix" },
        Challenge { template_id: "f80", input: "write a binary search function that returns Result<usize, usize> like the standard library's binary_search", verify: "compiles_and:contains_any:Result,binary", description: "gen: binary search with Result" },
        Challenge { template_id: "f80", input: "write an iterator adapter struct called StepBy that wraps any iterator and yields every nth element. Implement Iterator for it.", verify: "compiles_and:contains_any:struct StepBy,impl Iterator,impl<", description: "gen: custom iterator adapter" },

        // ══════════════════════════════════════════════════════════════
        // CODE REVIEW (f_code_review) — 10 challenges
        // Mined from: real code patterns in kova codebase + common pitfalls
        // ══════════════════════════════════════════════════════════════
        // EASY — clearly correct or clearly wrong
        Challenge { template_id: "f_code_review", input: "fn gcd(mut a: u64, mut b: u64) -> u64 { while b != 0 { let t = b; b = a % b; a = t; } a }", verify: "contains_any:LGTM,lgtm,looks good", description: "review: correct gcd" },
        Challenge { template_id: "f_code_review", input: "fn max(a: i32, b: i32) -> i32 { if a > b { a } else { b } }", verify: "contains_any:LGTM,lgtm,looks good", description: "review: correct max" },
        Challenge { template_id: "f_code_review", input: "fn read_file(path: &str) -> String { std::fs::read_to_string(path).unwrap() }", verify: "not_empty", description: "review: unwrap on file read" },
        // MEDIUM — subtle issues
        Challenge { template_id: "f_code_review", input: "fn average(nums: &[f64]) -> f64 { nums.iter().sum::<f64>() / nums.len() as f64 }", verify: "not_empty", description: "review: div-by-zero on empty slice" },
        Challenge { template_id: "f_code_review", input: "fn factorial(n: u64) -> u64 { (1..=n).product() }", verify: "contains_any:LGTM,lgtm,overflow,looks good", description: "review: factorial overflow" },
        Challenge { template_id: "f_code_review", input: "fn process(data: &[u8]) -> Vec<u8> { unsafe { std::slice::from_raw_parts(data.as_ptr(), data.len() * 2).to_vec() } }", verify: "not_empty", description: "review: unsafe buffer overread" },
        // HARD — real-world patterns that trip up models
        Challenge { template_id: "f_code_review", input: "fn find_dup(nums: &[i32]) -> Option<i32> { let mut seen = std::collections::HashSet::new(); for &n in nums { if !seen.insert(n) { return Some(n); } } None }", verify: "contains_any:LGTM,lgtm,looks good", description: "review: correct HashSet dedup" },
        Challenge { template_id: "f_code_review", input: "fn parse_kv(input: &str) -> HashMap<String, String> { let mut map = HashMap::new(); for line in input.lines() { let parts: Vec<&str> = line.splitn(2, '=').collect(); map.insert(parts[0].to_string(), parts[1].to_string()); } map }", verify: "not_empty", description: "review: index panic on bad input" },
        Challenge { template_id: "f_code_review", input: "use std::sync::{Arc, Mutex};\nfn spawn_workers(data: Arc<Mutex<Vec<i32>>>) { for i in 0..4 { let d = data.clone(); std::thread::spawn(move || { let mut v = d.lock().unwrap(); v.push(i); }); } }", verify: "not_empty", description: "review: mutex poison + join" },
        Challenge { template_id: "f_code_review", input: "fn truncate_utf8(s: &str, max_bytes: usize) -> &str { if s.len() <= max_bytes { s } else { &s[..max_bytes] } }", verify: "not_empty", description: "review: slicing mid-codepoint" },

        // ══════════════════════════════════════════════════════════════
        // VALIDATOR (f_validate) — 8 challenges
        // Mined from: f_validate few-shot patterns (PASS/FAIL on code quality)
        // ══════════════════════════════════════════════════════════════
        Challenge { template_id: "f_validate", input: "Request: implement quicksort\nCode: fn quicksort(v: &mut Vec<i32>) { /* TODO */ }", verify: "contains:FAIL", description: "validate: TODO stub" },
        Challenge { template_id: "f_validate", input: "Request: reverse a vector in place\nCode: fn reverse(v: &mut Vec<i32>) { v.reverse(); }", verify: "contains:PASS", description: "validate: correct reverse" },
        Challenge { template_id: "f_validate", input: "Request: count vowels in a string\nCode: fn count_vowels(s: &str) -> usize { s.chars().filter(|c| \"aeiou\".contains(*c)).count() }", verify: "contains:PASS", description: "validate: correct vowel counter" },
        Challenge { template_id: "f_validate", input: "Request: find the maximum element\nCode: fn max_element(v: &[i32]) -> i32 { unimplemented!() }", verify: "contains:FAIL", description: "validate: unimplemented" },
        Challenge { template_id: "f_validate", input: "Request: sort a vector\nCode: fn sort(v: &mut Vec<i32>) { for i in 0..v.len() { for j in i+1..v.len() { if v[j] < v[i] { v.swap(i, j); } } } }", verify: "contains:PASS", description: "validate: bubble sort correct" },
        // HARD — subtle correctness issues
        Challenge { template_id: "f_validate", input: "Request: binary search returning index\nCode: fn bsearch(v: &[i32], target: i32) -> Option<usize> { let mut lo = 0; let mut hi = v.len(); while lo < hi { let mid = (lo + hi) / 2; if v[mid] == target { return Some(mid); } else if v[mid] < target { lo = mid + 1; } else { hi = mid; } } None }", verify: "contains:PASS", description: "validate: correct bsearch" },
        Challenge { template_id: "f_validate", input: "Request: safe division returning Result\nCode: fn safe_div(a: f64, b: f64) -> Result<f64, String> { Ok(a / b) }", verify: "contains:FAIL", description: "validate: missing zero check" },
        Challenge { template_id: "f_validate", input: "Request: capitalize first letter of each word\nCode: fn title_case(s: &str) -> String { s.split_whitespace().map(|w| { let mut c = w.chars(); match c.next() { None => String::new(), Some(f) => f.to_uppercase().to_string() + c.as_str() } }).collect::<Vec<_>>().join(\" \") }", verify: "contains:PASS", description: "validate: title_case correct" },

        // ══════════════════════════════════════════════════════════════
        // CLIPPY FIX (f_clippy_fix) — 6 challenges
        // Mined from: real clippy warnings in kova + cochranblock
        // ══════════════════════════════════════════════════════════════
        Challenge { template_id: "f_clippy_fix", input: "Warning: redundant clone\nCode: fn greet(name: String) { let n = name.clone(); println!(\"hi {}\", n); }", verify: "compiles_and:contains:fn greet", description: "clippy: redundant clone" },
        Challenge { template_id: "f_clippy_fix", input: "Warning: this `if` has identical blocks\nCode: fn check(x: i32) -> &'static str { if x > 0 { \"yes\" } else { \"yes\" } }", verify: "compiles_and:contains:fn check", description: "clippy: identical if/else" },
        Challenge { template_id: "f_clippy_fix", input: "Warning: manual implementation of `Iterator::any`\nCode: fn has_zero(v: &[i32]) -> bool { for x in v { if *x == 0 { return true; } } false }", verify: "compiles_and:contains:fn has_zero", description: "clippy: manual any()" },
        // HARD — less common warnings (compile-verified)
        Challenge { template_id: "f_clippy_fix", input: "Warning: useless conversion to the same type: `String`\nCode: fn ident(s: String) -> String { String::from(s) }", verify: "compiles_and:contains:fn ident", description: "clippy: useless conversion" },
        Challenge { template_id: "f_clippy_fix", input: "Warning: called `.iter().nth(0)` on a Vec\nCode: fn first(v: &Vec<i32>) -> Option<&i32> { v.iter().nth(0) }", verify: "compiles_and:contains:fn first", description: "clippy: iter().nth(0)" },
        Challenge { template_id: "f_clippy_fix", input: "Warning: length comparison to zero\nCode: fn is_nonempty(v: &Vec<String>) -> bool { v.len() > 0 }", verify: "compiles_and:contains:fn is_nonempty", description: "clippy: len() > 0 vs is_empty" },

        // ══════════════════════════════════════════════════════════════
        // EXPLAIN TRACE (f115) — 6 challenges
        // Mined from: real kova pipeline failures + CI output
        // ══════════════════════════════════════════════════════════════
        Challenge { template_id: "f115", input: "Intent: deploy to production\nStage: cargo build --release\nOutcome: FAIL\nStderr: error[E0308]: mismatched types in src/main.rs:42", verify: "not_empty", description: "explain: build failure" },
        Challenge { template_id: "f115", input: "Intent: run test suite\nStage: cargo test\nOutcome: FAIL\nStderr: thread 'tests::test_parse' panicked at 'assertion failed: result.is_ok()'", verify: "not_empty", description: "explain: test panic" },
        Challenge { template_id: "f115", input: "Intent: start HTTP server\nStage: kova serve\nOutcome: FAIL\nStderr: Error: address already in use (os error 48) at 127.0.0.1:3002", verify: "not_empty", description: "explain: port in use" },
        // HARD — multi-stage, less obvious
        Challenge { template_id: "f115", input: "Intent: run clippy\nStage: cargo clippy\nOutcome: FAIL\nStderr: error: could not compile `kova` (lib) due to 1 previous error\nerror[E0599]: no method named `as_secs_f32` found for struct `Duration`", verify: "not_empty", description: "explain: clippy blocked by compile error" },
        Challenge { template_id: "f115", input: "Intent: push to remote\nStage: git push origin main\nOutcome: FAIL\nStderr: ! [rejected] main -> main (non-fast-forward)\nhint: Updates were rejected because the tip of your current branch is behind", verify: "not_empty", description: "explain: git push rejected" },
        Challenge { template_id: "f115", input: "Intent: run integration tests\nStage: cargo test --features tests\nOutcome: FAIL\nStderr: thread 'main' panicked at 'connection refused (os error 61)'\nnote: test requires running sled instance", verify: "not_empty", description: "explain: missing test dependency" },

        // ══════════════════════════════════════════════════════════════
        // TEST WRITE (f_test_write) — 6 challenges
        // Mined from: real kova functions + escalating complexity
        // ══════════════════════════════════════════════════════════════
        Challenge { template_id: "f_test_write", input: "fn clamp(val: i32, min: i32, max: i32) -> i32 { if val < min { min } else if val > max { max } else { val } }", verify: "compiles_and:contains:#[test]", description: "test: clamp" },
        Challenge { template_id: "f_test_write", input: "fn is_palindrome(s: &str) -> bool { let bytes = s.as_bytes(); let len = bytes.len(); for i in 0..len / 2 { if bytes[i] != bytes[len - 1 - i] { return false; } } true }", verify: "compiles_and:contains:#[test]", description: "test: is_palindrome" },
        Challenge { template_id: "f_test_write", input: "fn celsius_to_fahrenheit(c: f64) -> f64 { c * 9.0 / 5.0 + 32.0 }", verify: "compiles_and:contains:#[test]", description: "test: temperature conversion" },
        // HARD — functions with edge cases that matter (compile-verified)
        Challenge { template_id: "f_test_write", input: "fn chunk<T: Clone>(v: &[T], size: usize) -> Vec<Vec<T>> { v.chunks(size).map(|c| c.to_vec()).collect() }", verify: "compiles_and:contains:#[test]", description: "test: chunk (empty, uneven)" },
        Challenge { template_id: "f_test_write", input: "fn safe_div(a: i64, b: i64) -> Option<i64> { if b == 0 { None } else { Some(a / b) } }", verify: "compiles_and:contains:#[test]", description: "test: safe_div (zero, overflow)" },
        Challenge { template_id: "f_test_write", input: "fn levenshtein(a: &str, b: &str) -> usize { let (a, b) = (a.as_bytes(), b.as_bytes()); let mut dp = (0..=b.len()).collect::<Vec<_>>(); for i in 1..=a.len() { let mut prev = dp[0]; dp[0] = i; for j in 1..=b.len() { let tmp = dp[j]; dp[j] = if a[i-1] == b[j-1] { prev } else { 1 + prev.min(dp[j]).min(dp[j-1]) }; prev = tmp; } } dp[b.len()] }", verify: "compiles_and:contains:#[test]", description: "test: levenshtein (hard algo)" },
    ]
}

/// T145=BenchResult
/// Result of one challenge.
#[derive(Debug)]
pub struct T145 {
    pub template_id: String,
    pub description: String,
    pub passed: bool,
    pub response: String,
    pub duration_ms: u64,
    pub error: Option<String>,
}

/// f233=run_bench
/// Run all held-out challenges.
pub fn f233(registry: &T149, cluster: &T193) -> Vec<T145> {
    let mut results = Vec::new();

    for ch in challenges() {
        let tmpl = match registry.get(ch.template_id) {
            Some(t) => t,
            None => {
                eprintln!("  SKIP  {} — template not found", ch.description);
                continue;
            }
        };

        let breaker = T155::new(3);
        let budget = T156::new(100_000);
        let start = Instant::now();

        match f244(tmpl, ch.input, cluster, &breaker, &budget) {
            Ok(result) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                let passed = f234(&result.response, ch.verify);

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

                results.push(T145 {
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

                results.push(T145 {
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

/// f234=verify_response
/// Verify a response against a check string.
pub fn f234(response: &str, check: &str) -> bool {
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

    // Combined: must compile AND pass a secondary check.
    // Format: "compiles_and:<sub-check>" e.g. "compiles_and:contains:fn"
    if let Some(sub) = check.strip_prefix("compiles_and:") {
        return try_compile(trimmed) && f234(response, sub);
    }

    // Combined: must compile AND be slop-free
    if check == "compiles_no_slop" {
        return try_compile(trimmed) && !f235(trimmed);
    }

    // P12 anti-slop: response must not contain any banned words
    if check == "no_slop" {
        return !trimmed.is_empty() && !f235(trimmed);
    }

    // Combined: must contain something AND be slop-free
    if let Some(text) = check.strip_prefix("contains_no_slop:") {
        return trimmed.to_lowercase().contains(&text.to_lowercase()) && !f235(trimmed);
    }

    // Default: non-empty + passes quick_validate
    validate::f264(trimmed)
}

/// P12 banned words — AI slop that must never appear in generated output.
const SLOP_WORDS: &[&str] = &[
    "utilize", "leverage", "optimize", "comprehensive", "robust",
    "seamlessly", "scalable", "paradigm", "synergy", "cutting-edge",
    "streamline", "empower", "utilizing", "leveraging", "optimizing",
    "empowering", "streamlining", "leveraged", "optimized",
];

/// f235=contains_slop
/// Returns true if the text contains any P12 banned slop words.
pub fn f235(text: &str) -> bool {
    let lower = text.to_lowercase();
    SLOP_WORDS.iter().any(|w| lower.contains(w))
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
        "[package]\nname = \"bench_check\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    );

    Command::new("cargo")
        .args(["check", "--quiet"])
        .current_dir(tmp.path())
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// f236=print_bench_results
/// Print bench results as a summary table.
pub fn f236(results: &[T145]) {
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
