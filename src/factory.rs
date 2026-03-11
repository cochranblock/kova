// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! factory — Rust Binary Factory. Distributed code gen pipeline across IRONHIVE cluster.
//!
//! Pipeline stages:
//!   1. Classify (c2, 3B) — what kind of task?
//!   2. Generate (lf/bt, 32B) — produce Rust code
//!   3. Compile (any node) — cargo check + clippy
//!   4. Review (gd, 14B) — code quality check
//!   5. Fix (lf/bt, 32B) — fix compile/review errors, retry
//!   6. Output — final binary or code

use crate::cluster::{Cluster, TaskKind};
use crate::ollama;
use std::path::Path;
use std::process::Command;

/// Factory pipeline result.
#[derive(Debug, Clone)]
pub struct FactoryResult {
    pub code: String,
    pub stages: Vec<StageResult>,
    pub success: bool,
    pub binary_path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StageResult {
    pub stage: String,
    pub node: String,
    pub duration_ms: u64,
    pub success: bool,
    pub output: String,
}

/// Factory configuration.
pub struct FactoryConfig {
    pub max_fix_retries: u32,
    pub run_clippy: bool,
    pub run_tests: bool,
    pub run_review: bool,
    pub compile_node: Option<String>,
    pub num_ctx: u32,
    /// Allow external deps (adds them to temp Cargo.toml).
    pub allow_deps: bool,
}

impl Default for FactoryConfig {
    fn default() -> Self {
        Self {
            max_fix_retries: 2,
            run_clippy: true,
            run_tests: true,
            run_review: true,
            compile_node: None,
            num_ctx: 8192,
            allow_deps: false,
        }
    }
}

/// The Rust Binary Factory. Orchestrates code gen across the IRONHIVE cluster.
pub struct Factory {
    cluster: Cluster,
    config: FactoryConfig,
}

impl Factory {
    pub fn new(config: FactoryConfig) -> Self {
        Self {
            cluster: Cluster::default_hive(),
            config,
        }
    }

    pub fn default() -> Self {
        Self::new(FactoryConfig::default())
    }

    /// Run the full factory pipeline: classify → generate → compile → review → fix → output.
    pub fn run(&self, prompt: &str, project_dir: &Path) -> FactoryResult {
        let mut result = FactoryResult {
            code: String::new(),
            stages: Vec::new(),
            success: false,
            binary_path: None,
        };

        // ── Stage 1: Classify ──
        let task_kind = self.classify(prompt, &mut result);

        // ── Stage 2: Generate ──
        let system = self.build_system_prompt(project_dir);
        let gen_prompt = format!(
            "{}\n\nGenerate Rust code. Put code in ```rust ... ``` blocks. Be concise. No explanation.",
            prompt
        );

        let code = match self.generate(&system, &gen_prompt, task_kind, &mut result) {
            Some(c) => c,
            None => return result,
        };
        result.code = code.clone();

        // ── Stage 3: Compile loop ──
        let (compiled_code, compile_ok) = self.compile_loop(&code, &system, project_dir, &mut result);
        result.code = compiled_code.clone();

        if !compile_ok {
            return result;
        }

        // ── Stage 4: Review ──
        if self.config.run_review {
            let review_result = self.review(&compiled_code, &mut result);
            if let Some(issues) = review_result {
                // If review found issues, try one fix pass
                if let Some(fixed) = self.fix_from_review(&compiled_code, &issues, &system, &mut result) {
                    result.code = fixed;
                }
            }
        }

        result.success = true;
        result
    }

    /// Stage 1: Classify the task using the coordinator's fast model.
    fn classify(&self, prompt: &str, result: &mut FactoryResult) -> TaskKind {
        let start = std::time::Instant::now();

        let classify_prompt = format!(
            "Classify this task into exactly one category. Reply with only the category name.\n\
            Categories: code_gen, code_review, test_write, fix_compile, clippy_fix, general\n\n\
            Task: {}",
            prompt
        );

        let (node, response) = match self.cluster.dispatch(
            TaskKind::Classify,
            "You are a task classifier. Reply with exactly one word: the category.",
            &classify_prompt,
            Some(256),
        ) {
            Ok(r) => r,
            Err(_) => {
                result.stages.push(StageResult {
                    stage: "classify".into(),
                    node: "?".into(),
                    duration_ms: start.elapsed().as_millis() as u64,
                    success: false,
                    output: "classification failed, defaulting to code_gen".into(),
                });
                return TaskKind::CodeGen;
            }
        };

        let kind = match response.trim().to_lowercase().as_str() {
            s if s.contains("code_gen") => TaskKind::CodeGen,
            s if s.contains("code_review") || s.contains("review") => TaskKind::CodeReview,
            s if s.contains("test") => TaskKind::TestWrite,
            s if s.contains("fix_compile") || s.contains("fix") => TaskKind::FixCompile,
            s if s.contains("clippy") => TaskKind::ClippyFix,
            _ => TaskKind::CodeGen,
        };

        result.stages.push(StageResult {
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
        task_kind: TaskKind,
        result: &mut FactoryResult,
    ) -> Option<String> {
        let start = std::time::Instant::now();
        eprintln!("[factory] generating...");

        let (node, response) = match self.cluster.dispatch(
            task_kind,
            system,
            prompt,
            Some(self.config.num_ctx),
        ) {
            Ok(r) => r,
            Err(e) => {
                result.stages.push(StageResult {
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

        result.stages.push(StageResult {
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
        project_dir: &Path,
        result: &mut FactoryResult,
    ) -> (String, bool) {
        let mut code = initial_code.to_string();
        let mut attempt = 0u32;

        loop {
            let start = std::time::Instant::now();

            // Create temp project
            let tmp = match tempfile::TempDir::new() {
                Ok(d) => d,
                Err(e) => {
                    result.stages.push(StageResult {
                        stage: "compile".into(),
                        node: "local".into(),
                        duration_ms: 0,
                        success: false,
                        output: format!("temp dir: {}", e),
                    });
                    return (code, false);
                }
            };

            write_temp_project(tmp.path(), &code);

            // cargo check
            eprintln!("[factory] checking (attempt {})...", attempt + 1);
            let (ok, stderr) = cargo_check_local(tmp.path());
            if !ok {
                attempt += 1;
                result.stages.push(StageResult {
                    stage: format!("compile-{}", attempt),
                    node: "local".into(),
                    duration_ms: start.elapsed().as_millis() as u64,
                    success: false,
                    output: truncate(&stderr, 200),
                });

                if attempt > self.config.max_fix_retries {
                    eprintln!("[factory] compile failed after {} attempts", attempt);
                    return (code, false);
                }

                eprintln!("[factory] compile failed, fixing on cluster...");
                match self.fix_code(&code, &stderr, system, result) {
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
                    result.stages.push(StageResult {
                        stage: format!("clippy-{}", attempt),
                        node: "local".into(),
                        duration_ms: start.elapsed().as_millis() as u64,
                        success: false,
                        output: truncate(&stderr, 200),
                    });

                    if attempt > self.config.max_fix_retries {
                        eprintln!("[factory] clippy failed after {} attempts", attempt);
                        return (code, false);
                    }

                    match self.fix_code(&code, &stderr, system, result) {
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
                    result.stages.push(StageResult {
                        stage: format!("test-{}", attempt),
                        node: "local".into(),
                        duration_ms: start.elapsed().as_millis() as u64,
                        success: false,
                        output: truncate(&stderr, 200),
                    });

                    if attempt > self.config.max_fix_retries {
                        return (code, false);
                    }

                    match self.fix_code(&code, &stderr, system, result) {
                        Some(fixed) => {
                            code = fixed;
                            continue;
                        }
                        None => return (code, false),
                    }
                }
            }

            result.stages.push(StageResult {
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

    /// Fix code using a 32B node.
    fn fix_code(
        &self,
        code: &str,
        error: &str,
        system: &str,
        result: &mut FactoryResult,
    ) -> Option<String> {
        let start = std::time::Instant::now();

        let fix_prompt = format!(
            "Fix this Rust code. The error is:\n```\n{}\n```\n\nCode:\n```rust\n{}\n```\n\n\
            Return only the fixed code in a ```rust block. No explanation.",
            truncate(error, 500), code
        );

        let (node, response) = match self.cluster.dispatch(
            TaskKind::FixCompile,
            system,
            &fix_prompt,
            Some(self.config.num_ctx),
        ) {
            Ok(r) => r,
            Err(e) => {
                result.stages.push(StageResult {
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

        result.stages.push(StageResult {
            stage: "fix".into(),
            node: node.clone(),
            duration_ms: start.elapsed().as_millis() as u64,
            success: true,
            output: format!("fixed on {}", node),
        });

        eprintln!("[factory] fixed on {}", node);
        Some(fixed)
    }

    /// Stage 4: Review code using a mid-tier node.
    fn review(&self, code: &str, result: &mut FactoryResult) -> Option<String> {
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
            TaskKind::CodeReview,
            system,
            &review_prompt,
            Some(self.config.num_ctx),
        ) {
            Ok(r) => r,
            Err(e) => {
                result.stages.push(StageResult {
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

        result.stages.push(StageResult {
            stage: "review".into(),
            node: node.clone(),
            duration_ms: start.elapsed().as_millis() as u64,
            success: true,
            output: if clean {
                "LGTM".into()
            } else {
                truncate(&response, 200)
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
        result: &mut FactoryResult,
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
            TaskKind::FixCompile,
            system,
            &fix_prompt,
            Some(self.config.num_ctx),
        ) {
            Ok(r) => r,
            Err(e) => {
                result.stages.push(StageResult {
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

        result.stages.push(StageResult {
            stage: "fix-review".into(),
            node,
            duration_ms: start.elapsed().as_millis() as u64,
            success: true,
            output: "applied review fixes".into(),
        });

        Some(fixed)
    }

    /// Build system prompt with project context.
    fn build_system_prompt(&self, project_dir: &Path) -> String {
        let project_name = project_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        format!(
            "You are a Rust systems programming expert working on the {} project.\n\
            Write clean, idiomatic Rust. No filler. No slop words (utilize/leverage/optimize/comprehensive/robust/seamlessly).\n\
            IMPORTANT: Use only the Rust standard library. No external crates (no tokio, no serde, no thiserror, etc).\n\
            The code will be compiled in an isolated crate with zero dependencies.\n\
            Put all code in ```rust blocks.",
            project_name
        )
    }
}

/// Run the factory from CLI.
pub fn run_factory(prompt: &str, project_dir: &Path, config: FactoryConfig) {
    let factory = Factory::new(config);

    println!("[factory] IRONHIVE Rust Binary Factory");
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

// ── Helpers ──

fn extract_rust_block(s: &str) -> Option<String> {
    let start = s.find("```rust")?;
    let after_start = &s[start + 7..];
    let end = after_start.find("```")?;
    Some(after_start[..end].trim().to_string())
}

fn write_temp_project(dir: &Path, code: &str) {
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"gen\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    ).ok();
    std::fs::create_dir_all(dir.join("src")).ok();
    // Prepend #![allow(dead_code)] so clippy doesn't fail on non-pub items in lib crate
    let code_with_allow = format!("#![allow(dead_code)]\n{}", code);
    std::fs::write(dir.join("src/lib.rs"), code_with_allow).ok();
}

fn cargo_check_local(dir: &Path) -> (bool, String) {
    let output = Command::new("cargo")
        .args(["check"])
        .current_dir(dir)
        .output();
    match output {
        Ok(o) => (o.status.success(), String::from_utf8_lossy(&o.stderr).into()),
        Err(e) => (false, e.to_string()),
    }
}

fn cargo_clippy_local(dir: &Path) -> (bool, String) {
    let output = Command::new("cargo")
        .args(["clippy", "--", "-D", "warnings"])
        .current_dir(dir)
        .output();
    match output {
        Ok(o) => (o.status.success(), String::from_utf8_lossy(&o.stderr).into()),
        Err(e) => (false, e.to_string()),
    }
}

fn cargo_test_local(dir: &Path) -> (bool, String) {
    let output = Command::new("cargo")
        .args(["test"])
        .current_dir(dir)
        .output();
    match output {
        Ok(o) => (o.status.success(), String::from_utf8_lossy(&o.stderr).into()),
        Err(e) => (false, e.to_string()),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max])
    }
}
