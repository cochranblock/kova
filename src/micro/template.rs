// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, Mattbusel (micro-library pattern)
//! template — Micro-model template definition.
//! Each template defines a single-purpose AI unit: one function, one model, one job.
//! Inspired by Mattbusel's llm-* single-header pattern: each lib does ONE thing.

use std::path::Path;

/// A micro-model template. One per kova function that needs AI.
/// Like Mattbusel's single-header libs — self-contained, zero coupling.
#[derive(Debug, Clone)]
pub struct MicroTemplate {
    /// Compression token (f79, f80, f81, etc).
    pub id: String,
    /// Human name.
    pub name: String,
    /// What this micro-model does (one sentence).
    pub purpose: String,
    /// Minimum model tier: "router" (0.5B), "light" (1B), "mid" (3B), "heavy" (7B+).
    pub tier: String,
    /// Preferred ollama model tag.
    pub model: String,
    /// System prompt — baked into the binary.
    pub system_prompt: String,
    /// Few-shot examples: (input, expected_output) pairs.
    pub few_shot: Vec<(String, String)>,
    /// Input schema description.
    pub input_schema: String,
    /// Output schema description.
    pub output_schema: String,
    /// Context window size for this task.
    pub num_ctx: u32,
    /// Max tokens in response.
    pub max_tokens: u32,
    /// Temperature (lower = more deterministic).
    pub temperature: f32,
}

impl MicroTemplate {
    /// Load template from TOML file.
    pub fn from_toml(path: &Path) -> Result<Self, String> {
        let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let table: toml::Table = content
            .parse()
            .map_err(|e: toml::de::Error| e.to_string())?;

        let get_str = |key: &str| -> String {
            table
                .get(key)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        };

        let get_u32 = |key: &str, default: u32| -> u32 {
            table
                .get(key)
                .and_then(|v| v.as_integer())
                .map(|v| v as u32)
                .unwrap_or(default)
        };

        let get_f32 = |key: &str, default: f32| -> f32 {
            table
                .get(key)
                .and_then(|v| v.as_float())
                .map(|v| v as f32)
                .unwrap_or(default)
        };

        // Parse few-shot examples
        let few_shot = table
            .get("few_shot")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| {
                        let t = item.as_table()?;
                        let input = t.get("input")?.as_str()?.to_string();
                        let output = t.get("output")?.as_str()?.to_string();
                        Some((input, output))
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(MicroTemplate {
            id: get_str("id"),
            name: get_str("name"),
            purpose: get_str("purpose"),
            tier: get_str("tier"),
            model: get_str("model"),
            system_prompt: get_str("system_prompt"),
            few_shot,
            input_schema: get_str("input_schema"),
            output_schema: get_str("output_schema"),
            num_ctx: get_u32("num_ctx", 2048),
            max_tokens: get_u32("max_tokens", 512),
            temperature: get_f32("temperature", 0.1),
        })
    }

    /// Build the full prompt with few-shot examples baked in.
    pub fn build_prompt(&self, input: &str) -> String {
        let mut prompt = String::new();

        if !self.few_shot.is_empty() {
            prompt.push_str("Examples:\n\n");
            for (i, (inp, out)) in self.few_shot.iter().enumerate() {
                prompt.push_str(&format!(
                    "Input {}: {}\nOutput {}: {}\n\n",
                    i + 1,
                    inp,
                    i + 1,
                    out
                ));
            }
            prompt.push_str("Now process this input:\n\n");
        }

        prompt.push_str(input);
        prompt
    }
}

/// All built-in micro-templates for kova functions.
/// Each maps a compression token to its AI task definition.
pub fn builtin_templates() -> Vec<MicroTemplate> {
    vec![
        // ── Deterministic (no model needed, but template exists for the registry) ──

        // ── Router tier (0.5B-1B) ──
        MicroTemplate {
            id: "f79".into(),
            name: "classify_intent".into(),
            purpose: "Classify user input into one task category".into(),
            tier: "router".into(),
            model: "qwen2.5:0.5b".into(),
            system_prompt: "Classify the input into exactly one category. Reply with ONLY the category name, nothing else.\nCategories: code_gen, code_review, test_write, fix_compile, clippy_fix, explain, refactor, general\n\nRules:\n- compiler error, type mismatch, borrow error, semicolon, missing import = fix_compile\n- check for bugs, review, audit = code_review\n- write, create, add, implement, build = code_gen\n- write tests, add tests = test_write\n- clippy warning, lint = clippy_fix\n- what does, how does, explain, why = explain\n- rename, extract, move, restructure = refactor\n- everything else = general".into(),
            few_shot: vec![
                ("add exponential backoff to compute.rs".into(), "code_gen".into()),
                ("fix the bug in parser".into(), "fix_compile".into()),
                ("review this function for correctness".into(), "code_review".into()),
                ("what does f79 do?".into(), "explain".into()),
                ("write tests for the LRU cache".into(), "test_write".into()),
                ("compiler error expected semicolon on line 5".into(), "fix_compile".into()),
                ("the borrow checker says cannot move out of".into(), "fix_compile".into()),
                ("check this code for bugs".into(), "code_review".into()),
                ("extract this into a helper function".into(), "refactor".into()),
                ("type mismatch: expected i32 found &str".into(), "fix_compile".into()),
            ],
            input_schema: "User prompt text".into(),
            output_schema: "Single category name".into(),
            num_ctx: 512,
            max_tokens: 16,
            temperature: 0.0,
        },

        // ── Light tier (1B-3B) ──
        MicroTemplate {
            id: "f81".into(),
            name: "fix_compile".into(),
            purpose: "Fix a Rust compilation error given code and error message".into(),
            tier: "light".into(),
            model: "qwen2.5-coder:3b".into(),
            system_prompt: "You fix Rust compilation errors. You receive code and a compiler error. Return ONLY the fixed code in a ```rust block. No explanation. No narration. Fix the exact error reported.".into(),
            few_shot: vec![
                (
                    "Error: mismatched types: expected `String`, found `&str`\nCode: let x: String = \"hello\";".into(),
                    "```rust\nlet x: String = \"hello\".to_string();\n```".into(),
                ),
                (
                    "Error: cannot borrow `x` as mutable because it is also borrowed as immutable\nCode: let r = &x; x.push(1); println!(\"{}\", r);".into(),
                    "```rust\nlet r_len = x.len();\nx.push(1);\nprintln!(\"{}\", r_len);\n```".into(),
                ),
                (
                    "Error: expected `i32`, found `&str`\nCode: fn greet(name: i32) {} fn main() { greet(\"hello\"); }".into(),
                    "```rust\nfn greet(name: &str) {}\nfn main() { greet(\"hello\"); }\n```".into(),
                ),
            ],
            input_schema: "Error: <compiler error>\nCode: ```rust\n<code>\n```".into(),
            output_schema: "```rust\n<fixed code>\n```".into(),
            num_ctx: 4096,
            max_tokens: 2048,
            temperature: 0.1,
        },
        MicroTemplate {
            id: "f_clippy_fix".into(),
            name: "clippy_fix".into(),
            purpose: "Fix a Rust clippy warning given code and warning message".into(),
            tier: "light".into(),
            model: "qwen2.5-coder:3b".into(),
            system_prompt: "You fix Rust clippy warnings. Return ONLY the fixed code in a ```rust block. No explanation.".into(),
            few_shot: vec![
                (
                    "Warning: unnecessary `let` binding\nCode: fn area(w: f64, h: f64) -> f64 { let result = w * h; result }".into(),
                    "```rust\nfn area(w: f64, h: f64) -> f64 { w * h }\n```".into(),
                ),
                (
                    "Warning: this expression creates a reference which is immediately dereferenced\nCode: fn first(v: &Vec<i32>) -> i32 { *&v[0] }".into(),
                    "```rust\nfn first(v: &Vec<i32>) -> i32 { v[0] }\n```".into(),
                ),
            ],
            input_schema: "Warning: <clippy warning>\nCode: ```rust\n<code>\n```".into(),
            output_schema: "```rust\n<fixed code>\n```".into(),
            num_ctx: 4096,
            max_tokens: 2048,
            temperature: 0.1,
        },
        MicroTemplate {
            id: "f115".into(),
            name: "explain_trace".into(),
            purpose: "Explain a pipeline execution trace in plain English".into(),
            tier: "light".into(),
            model: "qwen2.5:3b".into(),
            system_prompt: "Explain this execution trace. What did the user want? What failed? Why? How to fix it? Be concise. 2-3 sentences max.".into(),
            few_shot: vec![
                (
                    "Intent: build project\nStage: cargo build\nOutcome: FAIL\nStderr: error[E0433]: failed to resolve: use of undeclared crate `serde`".into(),
                    "User tried to build but serde is missing. Add `serde = { version = \"1\", features = [\"derive\"] }` to Cargo.toml dependencies.".into(),
                ),
            ],
            input_schema: "Intent: <intent>\nStage: <stage>\nOutcome: <outcome>\nStderr: <error>".into(),
            output_schema: "Plain English explanation, 2-3 sentences".into(),
            num_ctx: 2048,
            max_tokens: 256,
            temperature: 0.3,
        },

        // ── Mid tier (7B) ──
        MicroTemplate {
            id: "f_code_review".into(),
            name: "code_review".into(),
            purpose: "Review Rust code for correctness, idiom violations, and bugs".into(),
            tier: "mid".into(),
            model: "qwen2.5-coder:7b".into(),
            system_prompt: "You are a senior Rust code reviewer. Flag real issues: correctness bugs, memory safety (including raw pointer deref, unsafe without null checks), logic errors, panics on bad input, missing error handling at boundaries. If the code is safe and correct, reply: LGTM. No style nits. No slop words (utilize/leverage/optimize/comprehensive/robust/seamlessly).".into(),
            few_shot: vec![
                (
                    "fn add(a: i32, b: i32) -> i32 { a + b }".into(),
                    "LGTM".into(),
                ),
                (
                    "fn get_index(v: &[i32], i: usize) -> i32 { v[i] }".into(),
                    "Panics if i >= v.len(). Return Option<i32> or check bounds.".into(),
                ),
                (
                    "fn get_val(ptr: *const i32) -> i32 { unsafe { *ptr } }".into(),
                    "Undefined behavior if ptr is null or dangling. Add null check: if ptr.is_null() { return 0; }".into(),
                ),
            ],
            input_schema: "```rust\n<code to review>\n```".into(),
            output_schema: "LGTM or list of real issues".into(),
            num_ctx: 8192,
            max_tokens: 1024,
            temperature: 0.2,
        },
        MicroTemplate {
            id: "f_test_write".into(),
            name: "test_write".into(),
            purpose: "Generate unit tests for a Rust function".into(),
            tier: "mid".into(),
            model: "qwen2.5-coder:7b".into(),
            system_prompt: "Write Rust unit tests for the given function. Use #[test] functions in a #[cfg(test)] mod tests block. Test: happy path, edge cases, error cases. Use assert_eq! and assert!. No external test frameworks.".into(),
            few_shot: vec![
                (
                    "fn abs(x: i32) -> i32 { if x < 0 { -x } else { x } }".into(),
                    "```rust\n#[cfg(test)]\nmod tests {\n    use super::*;\n    #[test]\n    fn test_positive() { assert_eq!(abs(5), 5); }\n    #[test]\n    fn test_negative() { assert_eq!(abs(-3), 3); }\n    #[test]\n    fn test_zero() { assert_eq!(abs(0), 0); }\n}\n```".into(),
                ),
            ],
            input_schema: "```rust\n<function to test>\n```".into(),
            output_schema: "```rust\n#[cfg(test)]\nmod tests { ... }\n```".into(),
            num_ctx: 4096,
            max_tokens: 2048,
            temperature: 0.2,
        },

        // ── Heavy tier (14B+) ──
        MicroTemplate {
            id: "f80".into(),
            name: "code_gen".into(),
            purpose: "Generate Rust code from a natural language description".into(),
            tier: "heavy".into(),
            model: "qwen2.5-coder:14b".into(),
            system_prompt: "You are a Rust systems programming expert. Write clean, idiomatic Rust. Use only the standard library unless told otherwise. All string types must match in if/else and match arms. Put all code in a single ```rust block. No explanation. No narration. NEVER use these words: utilize, leverage, optimize, comprehensive, robust, seamlessly, scalable, paradigm, synergy.".into(),
            few_shot: vec![],
            input_schema: "Natural language description of what to build".into(),
            output_schema: "```rust\n<complete code>\n```".into(),
            num_ctx: 8192,
            max_tokens: 4096,
            temperature: 0.2,
        },

        // ── Validation (inspired by Mattbusel/LLM-Hallucination-Detection-Script) ──
        MicroTemplate {
            id: "f_validate".into(),
            name: "validate_output".into(),
            purpose: "Check if generated code is valid, complete, and matches the request".into(),
            tier: "light".into(),
            model: "qwen2.5:3b".into(),
            system_prompt: "You validate generated Rust code. Check:\n1. Does it match the request?\n2. Is it complete (no TODO/unimplemented)?\n3. Are there obvious logic errors?\n4. Does it handle edge cases?\nReply: PASS or FAIL with one-line reason.".into(),
            few_shot: vec![
                (
                    "Request: reverse a string\nCode: fn reverse(s: &str) -> String { s.chars().rev().collect() }".into(),
                    "PASS".into(),
                ),
                (
                    "Request: sort a list\nCode: fn sort(v: &mut Vec<i32>) { todo!() }".into(),
                    "FAIL — unimplemented".into(),
                ),
            ],
            input_schema: "Request: <original prompt>\nCode: <generated code>".into(),
            output_schema: "PASS or FAIL with reason".into(),
            num_ctx: 4096,
            max_tokens: 64,
            temperature: 0.0,
        },
    ]
}
