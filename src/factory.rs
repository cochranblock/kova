//! factory — Rust Binary T181. Distributed code gen pipeline across IRONHIVE cluster.
//!
//! Pipeline stages:
//!   1. Classify (c2, 3B) — what kind of task?
//!   2. Generate (lf/bt, 32B) — produce Rust code
//!   3. Compile (local) — cargo check + clippy + test
//!   4. Review (gd, 14B) — code quality check
//!   5. Fix (lf/bt, 32B) — fix compile/review errors, retry
//!   6. Output — final binary or code
// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use crate::cluster::{T193, T191};
use std::path::Path;

/// T181 pipeline result.
#[derive(Debug, Clone)]
pub struct T178 {
    pub code: String,
    pub stages: Vec<T179>,
    pub success: bool,
    pub binary_path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct T179 {
    pub stage: String,
    pub node: String,
    pub duration_ms: u64,
    pub success: bool,
    pub output: String,
}

/// T181 configuration.
pub struct T180 {
    pub max_fix_retries: u32,
    pub run_clippy: bool,
    pub run_tests: bool,
    pub run_review: bool,
    pub compile_node: Option<String>,
    pub num_ctx: u32,
    /// Allow external deps (adds them to temp Cargo.toml).
    pub allow_deps: bool,
}

impl Default for T180 {
    fn default() -> Self {
        Self {
            max_fix_retries: 4,
            run_clippy: true,
            run_tests: true,
            run_review: true,
            compile_node: None,
            num_ctx: 8192,
            allow_deps: false,
        }
    }
}

/// The Rust Binary T181. Orchestrates code gen across the IRONHIVE cluster.
pub struct T181 {
    cluster: T193,
    config: T180,
}

impl T181 {
    pub fn new(config: T180) -> Self {
        Self {
            cluster: T193::default_hive(),
            config,
        }
    }
}

impl Default for T181 {
    fn default() -> Self {
        Self::new(T180::default())
    }
}

impl T181 {
    /// Run the full factory pipeline: classify → generate → compile → review → fix → output.
    pub fn run(&self, prompt: &str, _project_dir: &Path) -> T178 {
        let mut result = T178 {
            code: String::new(),
            stages: Vec::new(),
            success: false,
            binary_path: None,
        };

        // ── Stage 1: Classify ──
        let task_kind = self.classify(prompt, &mut result);

        // Detect if the prompt wants a binary (CLI tool, main(), executable)
        let wants_binary = f310(prompt);

        // ── Stage 2: Generate ──
        let system = self.f311(wants_binary);
        let gen_prompt = format!(
            "{}\n\nGenerate Rust code. Put all code in a single ```rust ... ``` block.\n\
            {}Be concise. No explanation outside the code block.",
            prompt,
            if wants_binary {
                "Include a `fn main()` entry point.\n"
            } else {
                ""
            }
        );

        let code = match self.generate(&system, &gen_prompt, task_kind, &mut result) {
            Some(c) => c,
            None => return result,
        };
        result.code = code.clone();

        // ── Stage 3: Compile loop ──
        let (compiled_code, compile_ok) =
            self.compile_loop(&code, &system, wants_binary, &mut result);
        result.code = compiled_code.clone();

        if !compile_ok {
            return result;
        }

        // ── Stage 4: Review ──
        if self.config.run_review {
            let review_result = self.review(&compiled_code, &mut result);
            if let Some(issues) = review_result
                && let Some(fixed) =
                    self.fix_from_review(&compiled_code, &issues, &system, &mut result)
            {
                    // Re-verify the review fix compiles
                    let tmp = match tempfile::TempDir::new() {
                        Ok(d) => d,
                        Err(_) => {
                            result.code = fixed;
                            result.success = true;
                            return result;
                        }
                    };
                    f312(tmp.path(), &fixed, wants_binary);
                    let (ok, _) = cargo_check_local(tmp.path());
                    if ok {
                        result.code = fixed;
                    }
                    // If review fix broke compilation, keep the pre-review code
            }
        }

        result.success = true;
        result
    }

    /// Stage 1: Classify the task using the coordinator's fast model.
    fn classify(&self, prompt: &str, result: &mut T178) -> T191 {
        let start = std::time::Instant::now();

        let classify_prompt = format!(
            "Classify this task into exactly one category. Reply with only the category name.\n\
            Categories: code_gen, code_review, test_write, fix_compile, clippy_fix, general\n\n\
            Task: {}",
            prompt
        );

        let (node, response) = match self.cluster.dispatch(
            T191::Classify,
            "You are a task classifier. Reply with exactly one word: the category.",
            &classify_prompt,
            Some(256),
        ) {
            Ok(r) => r,
            Err(_) => {
                result.stages.push(T179 {
                    stage: "classify".into(),
                    node: "?".into(),
                    duration_ms: start.elapsed().as_millis() as u64,
                    success: false,
                    output: "classification failed, defaulting to code_gen".into(),
                });
                return T191::CodeGen;
            }
        };

        let kind = match response.trim().to_lowercase().as_str() {
            s if s.contains("code_gen") => T191::CodeGen,
            s if s.contains("code_review") || s.contains("review") => T191::CodeReview,
            s if s.contains("test") => T191::TestWrite,
            s if s.contains("fix_compile") || s.contains("fix") => T191::FixCompile,
            s if s.contains("clippy") => T191::ClippyFix,
            _ => T191::CodeGen,
        };

        result.stages.push(T179 {
            stage: "classify".into(),
            node,
            duration_ms: start.elapsed().as_millis() as u64,
            success: true,
            output: format!("{:?}", kind),
        });

        eprintln!("[factory] classified as {:?}", kind);
        kind
    }

    /// Stage 2: Generate code on a heavy node.
    fn generate(
        &self,
        system: &str,
        prompt: &str,
        task_kind: T191,
        result: &mut T178,
    ) -> Option<String> {
        let start = std::time::Instant::now();
        eprintln!("[factory] generating...");

        let (node, response) =
            match self
                .cluster
                .dispatch(task_kind, system, prompt, Some(self.config.num_ctx))
            {
                Ok(r) => r,
                Err(e) => {
                    result.stages.push(T179 {
                        stage: "generate".into(),
                        node: "?".into(),
                        duration_ms: start.elapsed().as_millis() as u64,
                        success: false,
                        output: format!("generation failed: {}", e),
                    });
                    return None;
                }
            };

        let code = extract_rust_block(&response).unwrap_or_else(|| response.clone());

        result.stages.push(T179 {
            stage: "generate".into(),
            node: node.clone(),
            duration_ms: start.elapsed().as_millis() as u64,
            success: true,
            output: format!("{} chars generated on {}", code.len(), node),
        });

        eprintln!("[factory] generated {} chars on {}", code.len(), node);
        Some(code)
    }

    /// Stage 3: Compile loop — check, clippy, test, fix on failure.
    fn compile_loop(
        &self,
        initial_code: &str,
        system: &str,
        wants_binary: bool,
        result: &mut T178,
    ) -> (String, bool) {
        let mut code = initial_code.to_string();
        let mut attempt = 0u32;
        let mut prev_errors: Vec<String> = Vec::new();

        loop {
            let start = std::time::Instant::now();

            let tmp = match tempfile::TempDir::new() {
                Ok(d) => d,
                Err(e) => {
                    result.stages.push(T179 {
                        stage: "compile".into(),
                        node: "local".into(),
                        duration_ms: 0,
                        success: false,
                        output: format!("temp dir: {}", e),
                    });
                    return (code, false);
                }
            };

            f312(tmp.path(), &code, wants_binary);

            // cargo check
            eprintln!("[factory] checking (attempt {})...", attempt + 1);
            let (ok, stderr) = cargo_check_local(tmp.path());
            if !ok {
                attempt += 1;
                let error_key = f307(&stderr);
                result.stages.push(T179 {
                    stage: format!("compile-{}", attempt),
                    node: "local".into(),
                    duration_ms: start.elapsed().as_millis() as u64,
                    success: false,
                    output: truncate(&stderr, 500),
                });

                if attempt > self.config.max_fix_retries {
                    eprintln!("[factory] compile failed after {} attempts", attempt);
                    return (code, false);
                }

                // Check if we're stuck on the same error
                let stuck = prev_errors.last().map(|e| e == &error_key).unwrap_or(false);
                prev_errors.push(error_key);

                eprintln!("[factory] compile failed, fixing on cluster...");
                match self.fix_code(&code, &stderr, system, stuck, &prev_errors, result) {
                    Some(fixed) => {
                        code = fixed;
                        continue;
                    }
                    None => return (code, false),
                }
            }

            // clippy
            if self.config.run_clippy {
                eprintln!("[factory] clippy...");
                let (ok, stderr) = cargo_clippy_local(tmp.path());
                if !ok {
                    attempt += 1;
                    let error_key = f307(&stderr);
                    result.stages.push(T179 {
                        stage: format!("clippy-{}", attempt),
                        node: "local".into(),
                        duration_ms: start.elapsed().as_millis() as u64,
                        success: false,
                        output: truncate(&stderr, 500),
                    });

                    if attempt > self.config.max_fix_retries {
                        eprintln!("[factory] clippy failed after {} attempts", attempt);
                        return (code, false);
                    }

                    let stuck = prev_errors.last().map(|e| e == &error_key).unwrap_or(false);
                    prev_errors.push(error_key);

                    match self.fix_code(&code, &stderr, system, stuck, &prev_errors, result) {
                        Some(fixed) => {
                            code = fixed;
                            continue;
                        }
                        None => return (code, false),
                    }
                }
            }

            // tests
            if self.config.run_tests {
                eprintln!("[factory] testing...");
                let (ok, stderr) = cargo_test_local(tmp.path());
                if !ok {
                    attempt += 1;
                    let error_key = f307(&stderr);
                    result.stages.push(T179 {
                        stage: format!("test-{}", attempt),
                        node: "local".into(),
                        duration_ms: start.elapsed().as_millis() as u64,
                        success: false,
                        output: truncate(&stderr, 500),
                    });

                    if attempt > self.config.max_fix_retries {
                        return (code, false);
                    }

                    let stuck = prev_errors.last().map(|e| e == &error_key).unwrap_or(false);
                    prev_errors.push(error_key);

                    match self.fix_code(&code, &stderr, system, stuck, &prev_errors, result) {
                        Some(fixed) => {
                            code = fixed;
                            continue;
                        }
                        None => return (code, false),
                    }
                }
            }

            result.stages.push(T179 {
                stage: "compile".into(),
                node: "local".into(),
                duration_ms: start.elapsed().as_millis() as u64,
                success: true,
                output: format!("passed (attempt {})", attempt + 1),
            });

            eprintln!("[factory] compile passed");
            return (code, true);
        }
    }

    /// Fix code using a heavy node. Tracks previous errors to avoid loops.
    fn fix_code(
        &self,
        code: &str,
        error: &str,
        system: &str,
        stuck: bool,
        prev_errors: &[String],
        result: &mut T178,
    ) -> Option<String> {
        let start = std::time::Instant::now();

        // Build a smarter fix prompt based on context
        let fix_prompt = if stuck {
            // We're hitting the same error — escalate with more context
            format!(
                "IMPORTANT: Your previous fix attempt did NOT resolve this error. The same error occurred again.\n\
                You must use a DIFFERENT approach this time.\n\n\
                The compiler error is:\n```\n{}\n```\n\n\
                Previous error history ({} attempts):\n{}\n\n\
                Current code:\n```rust\n{}\n```\n\n\
                Think carefully about the root cause. The error type and line number are exact.\n\
                Return ONLY the complete fixed code in a ```rust block.",
                error,
                prev_errors.len(),
                prev_errors.iter().enumerate()
                    .map(|(i, e)| format!("  attempt {}: {}", i + 1, truncate(e, 100)))
                    .collect::<Vec<_>>().join("\n"),
                code
            )
        } else {
            format!(
                "Fix this Rust code. The compiler error is:\n```\n{}\n```\n\n\
                Code:\n```rust\n{}\n```\n\n\
                Return ONLY the complete fixed code in a ```rust block. No explanation.",
                error, code
            )
        };

        // If stuck, try speculative dispatch (race multiple nodes) for a different answer
        let dispatch_result = if stuck {
            self.cluster.speculative_dispatch(
                T191::FixCompile,
                system,
                &fix_prompt,
                Some(self.config.num_ctx),
            )
        } else {
            self.cluster.dispatch(
                T191::FixCompile,
                system,
                &fix_prompt,
                Some(self.config.num_ctx),
            )
        };

        let (node, response) = match dispatch_result {
            Ok(r) => r,
            Err(e) => {
                result.stages.push(T179 {
                    stage: "fix".into(),
                    node: "?".into(),
                    duration_ms: start.elapsed().as_millis() as u64,
                    success: false,
                    output: format!("fix dispatch failed: {}", e),
                });
                return None;
            }
        };

        let fixed = extract_rust_block(&response).unwrap_or(response);

        result.stages.push(T179 {
            stage: "fix".into(),
            node: node.clone(),
            duration_ms: start.elapsed().as_millis() as u64,
            success: true,
            output: format!(
                "fixed on {}{}",
                node,
                if stuck { " (escalated)" } else { "" }
            ),
        });

        eprintln!(
            "[factory] fixed on {}{}",
            node,
            if stuck { " (escalated)" } else { "" }
        );
        Some(fixed)
    }

    /// Stage 4: Review code using a mid-tier node.
    fn review(&self, code: &str, result: &mut T178) -> Option<String> {
        let start = std::time::Instant::now();
        eprintln!("[factory] reviewing...");

        let review_prompt = format!(
            "Review this Rust code. Report only real issues:\n\
            - Correctness bugs\n\
            - Memory safety issues\n\
            - Logic errors\n\
            - Missing error handling at boundaries\n\n\
            If the code is good, reply: LGTM\n\n\
            ```rust\n{}\n```",
            code
        );

        let system = "You are a senior Rust code reviewer. Be direct. Only flag real problems. If the code is fine, say LGTM.";

        let (node, response) = match self.cluster.dispatch(
            T191::CodeReview,
            system,
            &review_prompt,
            Some(self.config.num_ctx),
        ) {
            Ok(r) => r,
            Err(e) => {
                result.stages.push(T179 {
                    stage: "review".into(),
                    node: "?".into(),
                    duration_ms: start.elapsed().as_millis() as u64,
                    success: false,
                    output: format!("review failed: {}", e),
                });
                return None;
            }
        };

        let clean = response.trim().to_uppercase().contains("LGTM");

        result.stages.push(T179 {
            stage: "review".into(),
            node: node.clone(),
            duration_ms: start.elapsed().as_millis() as u64,
            success: true,
            output: if clean {
                "LGTM".into()
            } else {
                truncate(&response, 300)
            },
        });

        if clean {
            eprintln!("[factory] review: LGTM on {}", node);
            None
        } else {
            eprintln!("[factory] review: issues found on {}", node);
            Some(response)
        }
    }

    /// Fix code based on review feedback.
    fn fix_from_review(
        &self,
        code: &str,
        review: &str,
        system: &str,
        result: &mut T178,
    ) -> Option<String> {
        let start = std::time::Instant::now();
        eprintln!("[factory] fixing from review...");

        let fix_prompt = format!(
            "A code reviewer found these issues:\n{}\n\n\
            Original code:\n```rust\n{}\n```\n\n\
            Fix the code. Return only the fixed code in a ```rust block.",
            review, code
        );

        let (node, response) = match self.cluster.dispatch(
            T191::FixCompile,
            system,
            &fix_prompt,
            Some(self.config.num_ctx),
        ) {
            Ok(r) => r,
            Err(e) => {
                result.stages.push(T179 {
                    stage: "fix-review".into(),
                    node: "?".into(),
                    duration_ms: start.elapsed().as_millis() as u64,
                    success: false,
                    output: format!("fix from review failed: {}", e),
                });
                return None;
            }
        };

        let fixed = extract_rust_block(&response).unwrap_or(response);

        result.stages.push(T179 {
            stage: "fix-review".into(),
            node,
            duration_ms: start.elapsed().as_millis() as u64,
            success: true,
            output: "applied review fixes".into(),
        });

        Some(fixed)
    }

    /// Build system prompt.
    fn f311(&self, wants_binary: bool) -> String {
        crate::cargo::f311(wants_binary)
    }
}

/// Run the factory from CLI.
pub fn f297(prompt: &str, project_dir: &Path, config: T180) {
    let factory = T181::new(config);

    println!("[factory] IRONHIVE Rust Binary T181");
    println!("[factory] prompt: {}", truncate(prompt, 80));
    println!();

    let result = factory.run(prompt, project_dir);

    // Print stage summary
    println!("\n── Pipeline ──");
    for stage in &result.stages {
        let icon = if stage.success { "ok" } else { "XX" };
        println!(
            "  [{}] {} ({}, {}ms) — {}",
            icon, stage.stage, stage.node, stage.duration_ms, stage.output
        );
    }

    println!();
    if result.success {
        println!("[factory] SUCCESS\n");
        println!("```rust");
        println!("{}", result.code);
        println!("```");
    } else {
        println!("[factory] FAILED");
        if !result.code.is_empty() {
            println!("\nLast code attempt:");
            println!("```rust");
            println!("{}", result.code);
            println!("```");
        }
    }
}

// ── Helpers (delegated to crate::cargo) ──

fn extract_rust_block(s: &str) -> Option<String> {
    crate::cargo::f309(s)
}

fn f310(prompt: &str) -> bool {
    crate::cargo::f310(prompt)
}

fn f312(dir: &Path, code: &str, is_binary: bool) {
    crate::cargo::sandbox::f312(dir, code, is_binary);
}

fn cargo_check_local(dir: &Path) -> (bool, String) {
    crate::cargo::cargo_check(dir)
}

fn cargo_clippy_local(dir: &Path) -> (bool, String) {
    crate::cargo::cargo_clippy(dir)
}

fn cargo_test_local(dir: &Path) -> (bool, String) {
    crate::cargo::cargo_test(dir)
}

fn f307(stderr: &str) -> String {
    crate::cargo::f307(stderr)
}

fn truncate(s: &str, max: usize) -> String {
    crate::cargo::f308(s, max)
}