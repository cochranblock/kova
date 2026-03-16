// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! moe — Mixture of Experts code generation. Fan-out to multiple IRONHIVE nodes,
//! compile all variants in parallel, triple-sim validate, score, pick the winner.
//!
//! Pipeline:
//!   1. Fan-out prompt to N expert nodes in parallel
//!   2. Compile each variant locally (cargo check + clippy)
//!   3. Run tests on survivors
//!   4. Score survivors (compile speed, code size, review score)
//!   5. Pick the winner
//!   6. Optionally save to ~/.kova/experts/

use crate::cluster::{Cluster, ModelTier, TaskKind};
use crate::providers::{self, Provider};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::time::Instant;

/// Result from a single expert variant.
#[derive(Debug, Clone)]
pub struct ExpertVariant {
    pub node_id: String,
    pub code: String,
    pub gen_ms: u64,
    pub compile_ok: bool,
    pub clippy_ok: bool,
    pub tests_ok: bool,
    pub compile_ms: u64,
    pub review_score: Option<u8>,
    pub review_text: String,
    pub total_score: u32,
}

/// MoE pipeline result.
#[derive(Debug)]
pub struct MoeResult {
    pub variants: Vec<ExpertVariant>,
    pub winner: Option<usize>,
    pub prompt: String,
}

impl MoeResult {
    pub fn winner_code(&self) -> Option<&str> {
        self.winner.map(|i| self.variants[i].code.as_str())
    }
}

/// MoE configuration.
pub struct MoeConfig {
    pub num_experts: usize,
    pub run_clippy: bool,
    pub run_tests: bool,
    pub run_review: bool,
    pub num_ctx: u32,
    pub save_winner: bool,
}

impl Default for MoeConfig {
    fn default() -> Self {
        Self {
            num_experts: 3,
            run_clippy: true,
            run_tests: true,
            run_review: true,
            num_ctx: 8192,
            save_winner: false,
        }
    }
}

/// Run MoE pipeline: fan-out → compile → score → pick winner.
pub fn run_moe(prompt: &str, config: MoeConfig) -> MoeResult {
    let cluster = Cluster::default_hive();

    println!("[moe] IRONHIVE Mixture of Experts");
    println!("[moe] prompt: {}", truncate(prompt, 80));
    println!("[moe] experts: {}", config.num_experts);
    println!();

    let wants_binary = prompt_wants_binary(prompt);
    let system = build_system_prompt(wants_binary);

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

    // ── Stage 1: Fan-out to expert nodes ──
    println!(
        "[moe] stage 1: fan-out generation to {} experts...",
        config.num_experts
    );

    let expert_nodes = pick_expert_nodes(&cluster, config.num_experts);
    if expert_nodes.is_empty() {
        eprintln!("[moe] no online nodes available");
        return MoeResult {
            variants: vec![],
            winner: None,
            prompt: prompt.to_string(),
        };
    }

    println!(
        "[moe] dispatching to: {}",
        expert_nodes
            .iter()
            .map(|(id, _)| id.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );

    // Parallel generation across nodes
    let (tx, rx) = mpsc::channel();
    let gen_threads: Vec<_> = expert_nodes
        .iter()
        .map(|(node_id, base_url)| {
            let tx = tx.clone();
            let node_id = node_id.clone();
            let base_url = base_url.clone();
            let system = system.clone();
            let gen_prompt = gen_prompt.clone();
            let _num_ctx = config.num_ctx;

            // Pick the right model for this node
            let node = cluster.nodes.iter().find(|n| n.id == node_id).unwrap();
            let model = node.model.clone();

            std::thread::spawn(move || {
                let start = Instant::now();
                let provider = Provider::OpenAiCompat {
                    url: base_url,
                    api_key: String::new(),
                    model: model.clone(),
                };
                let result = providers::provider_generate(&provider, &model, &system, &gen_prompt)
                    .map(|r| r.text);
                let elapsed = start.elapsed().as_millis() as u64;
                let _ = tx.send((node_id, result, elapsed));
            })
        })
        .collect();
    drop(tx);

    let mut variants: Vec<ExpertVariant> = Vec::new();

    for (node_id, result, gen_ms) in rx {
        match result {
            Ok(response) => {
                let code = extract_rust_block(&response).unwrap_or(response);
                println!(
                    "[moe] {} generated {} chars in {:.1}s",
                    node_id,
                    code.len(),
                    gen_ms as f64 / 1000.0
                );
                variants.push(ExpertVariant {
                    node_id,
                    code,
                    gen_ms,
                    compile_ok: false,
                    clippy_ok: false,
                    tests_ok: false,
                    compile_ms: 0,
                    review_score: None,
                    review_text: String::new(),
                    total_score: 0,
                });
            }
            Err(e) => {
                eprintln!("[moe] {} failed: {}", node_id, e);
            }
        }
    }

    // Wait for threads
    for t in gen_threads {
        let _ = t.join();
    }

    if variants.is_empty() {
        eprintln!("[moe] all experts failed to generate");
        return MoeResult {
            variants,
            winner: None,
            prompt: prompt.to_string(),
        };
    }

    println!("\n[moe] stage 2: compile {} variants...", variants.len());

    // ── Stage 2: Compile each variant ──
    // Parallel compilation in temp crates
    let compile_results: Vec<(bool, bool, bool, u64)> = {
        let handles: Vec<_> = variants
            .iter()
            .map(|v| {
                let code = v.code.clone();
                let run_clippy = config.run_clippy;
                let run_tests = config.run_tests;
                let wb = wants_binary;
                std::thread::spawn(move || {
                    let start = Instant::now();
                    let tmp = match tempfile::TempDir::new() {
                        Ok(d) => d,
                        Err(_) => return (false, false, false, 0u64),
                    };
                    write_temp_project(tmp.path(), &code, wb);

                    let (check_ok, _) = cargo_check(tmp.path());
                    if !check_ok {
                        return (false, false, false, start.elapsed().as_millis() as u64);
                    }

                    let clippy_ok = if run_clippy {
                        let (ok, _) = cargo_clippy(tmp.path());
                        ok
                    } else {
                        true
                    };

                    let tests_ok = if run_tests {
                        let (ok, _) = cargo_test(tmp.path());
                        ok
                    } else {
                        true
                    };

                    (
                        check_ok,
                        clippy_ok,
                        tests_ok,
                        start.elapsed().as_millis() as u64,
                    )
                })
            })
            .collect();

        handles
            .into_iter()
            .map(|h| h.join().unwrap_or((false, false, false, 0)))
            .collect()
    };

    for (i, (check_ok, clippy_ok, tests_ok, compile_ms)) in compile_results.into_iter().enumerate()
    {
        variants[i].compile_ok = check_ok;
        variants[i].clippy_ok = clippy_ok;
        variants[i].tests_ok = tests_ok;
        variants[i].compile_ms = compile_ms;

        let status = if !check_ok {
            "FAIL"
        } else if !clippy_ok {
            "clippy fail"
        } else if !tests_ok {
            "test fail"
        } else {
            "PASS"
        };

        println!(
            "[moe] {} — {} ({:.1}s)",
            variants[i].node_id,
            status,
            compile_ms as f64 / 1000.0
        );
    }

    let survivors: Vec<usize> = variants
        .iter()
        .enumerate()
        .filter(|(_, v)| v.compile_ok)
        .map(|(i, _)| i)
        .collect();

    println!(
        "\n[moe] {} of {} variants compiled",
        survivors.len(),
        variants.len()
    );

    if survivors.is_empty() {
        eprintln!("[moe] no variants compiled successfully");
        return MoeResult {
            variants,
            winner: None,
            prompt: prompt.to_string(),
        };
    }

    // ── Stage 3: Review survivors ──
    if config.run_review && survivors.len() > 1 {
        println!("[moe] stage 3: reviewing {} survivors...", survivors.len());

        for &idx in &survivors {
            let score = review_variant(&cluster, &variants[idx].code, config.num_ctx);
            variants[idx].review_score = Some(score.0);
            variants[idx].review_text = score.1;
            println!(
                "[moe] {} — review score: {}/10",
                variants[idx].node_id, score.0
            );
        }
    }

    // ── Stage 4: Score and pick winner ──
    println!("\n[moe] stage 4: scoring...");

    for v in &mut variants {
        if !v.compile_ok {
            v.total_score = 0;
            continue;
        }

        let mut score: u32 = 100; // base for compiling

        // Clippy clean: +30
        if v.clippy_ok {
            score += 30;
        }

        // Tests pass: +30
        if v.tests_ok {
            score += 30;
        }

        // Review score: 0-50 (scaled from 0-10)
        if let Some(rs) = v.review_score {
            score += rs as u32 * 5;
        }

        // Code size bonus: shorter is better (within reason). +0-20
        let lines = v.code.lines().count();
        if lines > 0 && lines < 200 {
            score += (20u32).saturating_sub(lines as u32 / 10);
        }

        // Speed bonus: faster generation = +0-20
        if v.gen_ms > 0 && v.gen_ms < 300_000 {
            let speed_bonus = 20u32.saturating_sub((v.gen_ms / 15_000) as u32);
            score += speed_bonus;
        }

        v.total_score = score;
    }

    // Pick highest scoring variant
    let winner = variants
        .iter()
        .enumerate()
        .filter(|(_, v)| v.compile_ok)
        .max_by_key(|(_, v)| v.total_score)
        .map(|(i, _)| i);

    // ── Output ──
    println!("\n── MoE Results ──");
    println!(
        "{:<6} {:<8} {:<8} {:<8} {:<8} {:<8} {:<6} Status",
        "Node", "Gen(s)", "Compile", "Clippy", "Tests", "Review", "Score"
    );
    println!("{}", "─".repeat(70));

    for (i, v) in variants.iter().enumerate() {
        let is_winner = winner == Some(i);
        let compile = if v.compile_ok { "ok" } else { "FAIL" };
        let clippy = if v.clippy_ok { "ok" } else { "FAIL" };
        let tests = if v.tests_ok { "ok" } else { "FAIL" };
        let review = v
            .review_score
            .map(|s| format!("{}/10", s))
            .unwrap_or("-".into());

        println!(
            "{:<6} {:<8.1} {:<8} {:<8} {:<8} {:<8} {:<6} {}",
            v.node_id,
            v.gen_ms as f64 / 1000.0,
            compile,
            clippy,
            tests,
            review,
            v.total_score,
            if is_winner { "WINNER" } else { "" }
        );
    }

    if let Some(w) = winner {
        println!(
            "\n[moe] winner: {} (score {})\n",
            variants[w].node_id, variants[w].total_score
        );

        if config.save_winner && let Some(path) = save_expert(prompt, &variants[w].code) {
            println!("[moe] saved to {}", path.display());
        }

        println!("```rust");
        println!("{}", variants[w].code);
        println!("```");
    } else {
        println!("\n[moe] no winner — all variants failed");
    }

    MoeResult {
        variants,
        winner,
        prompt: prompt.to_string(),
    }
}

/// Pick expert nodes for fan-out. Prefers heavy + mid nodes, avoids coordinator for gen.
fn pick_expert_nodes(cluster: &Cluster, max: usize) -> Vec<(String, String)> {
    let mut nodes: Vec<(String, String)> = Vec::new();

    // Heavy nodes first (32B — best code gen)
    for node in &cluster.nodes {
        if nodes.len() >= max {
            break;
        }
        if matches!(node.tier, ModelTier::Heavy) && providers::provider_health(&node.provider()) {
            nodes.push((node.id.clone(), node.base_url()));
        }
    }

    // Mid nodes next (14B — different perspective)
    for node in &cluster.nodes {
        if nodes.len() >= max {
            break;
        }
        if matches!(node.tier, ModelTier::Mid)
            && providers::provider_health(&node.provider())
            && !nodes.iter().any(|(id, _)| id == &node.id)
        {
            nodes.push((node.id.clone(), node.base_url()));
        }
    }

    // Light/Router only if we still need more
    for node in &cluster.nodes {
        if nodes.len() >= max {
            break;
        }
        if matches!(node.tier, ModelTier::Light | ModelTier::Router)
            && providers::provider_health(&node.provider())
            && !nodes.iter().any(|(id, _)| id == &node.id)
        {
            nodes.push((node.id.clone(), node.base_url()));
        }
    }

    nodes
}

/// Review a variant using a mid-tier node. Returns (score 0-10, review text).
fn review_variant(cluster: &Cluster, code: &str, num_ctx: u32) -> (u8, String) {
    let review_prompt = format!(
        "Rate this Rust code from 0-10. Consider:\n\
        - Correctness (does it do what it should?)\n\
        - Idiomatic Rust (proper error handling, ownership, iterators)\n\
        - Code clarity (readable, well-structured)\n\
        - Edge cases handled\n\n\
        Reply with ONLY a number 0-10 on the first line, then brief notes.\n\n\
        ```rust\n{}\n```",
        code
    );

    let system =
        "You are a Rust code reviewer. Rate code 0-10. First line must be the number only.";

    match cluster.dispatch(TaskKind::CodeReview, system, &review_prompt, Some(num_ctx)) {
        Ok((_, response)) => {
            let first_line = response.lines().next().unwrap_or("5");
            let score: u8 = first_line
                .trim()
                .chars()
                .filter(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse()
                .unwrap_or(5)
                .min(10);
            (score, response)
        }
        Err(_) => (5, "review unavailable".into()),
    }
}

/// Save winning expert code to ~/.kova/experts/
fn save_expert(prompt: &str, code: &str) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let experts_dir = home.join(".kova").join("experts");
    std::fs::create_dir_all(&experts_dir).ok()?;

    // Generate a slug from the prompt
    let slug: String = prompt
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ')
        .take(40)
        .collect::<String>()
        .trim()
        .replace(' ', "_")
        .to_lowercase();

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let filename = format!("{}_{}.rs", slug, ts);
    let path = experts_dir.join(&filename);
    std::fs::write(&path, code).ok()?;
    Some(path)
}

// ── Helpers (shared with factory) ──

fn extract_rust_block(s: &str) -> Option<String> {
    let (start_tag, tag_len) = if let Some(pos) = s.find("```rust") {
        (pos, 7)
    } else if let Some(pos) = s.find("```\n") {
        (pos, 4)
    } else {
        return None;
    };
    let after_start = &s[start_tag + tag_len..];
    let end = after_start.find("```")?;
    Some(after_start[..end].trim().to_string())
}

fn prompt_wants_binary(prompt: &str) -> bool {
    let lower = prompt.to_lowercase();
    lower.contains("cli ")
        || lower.contains("command line")
        || lower.contains("command-line")
        || lower.contains("executable")
        || lower.contains("binary")
        || lower.contains("tool that")
        || lower.contains("program that")
        || lower.contains("app that")
        || lower.contains("main()")
        || lower.contains("fn main")
        || lower.contains("takes a ")
        || lower.contains("prints ")
        || lower.contains("reads from")
        || lower.contains("accept")
}

fn build_system_prompt(wants_binary: bool) -> String {
    let code_type = if wants_binary {
        "Write a complete program with `fn main()`. The code will be compiled as src/main.rs."
    } else {
        "Write library code. The code will be compiled as src/lib.rs."
    };

    format!(
        "You are a Rust systems programming expert.\n\
        {}\n\
        Write clean, idiomatic Rust. No filler. No slop words.\n\
        IMPORTANT: Use only the Rust standard library. No external crates.\n\
        The code will be compiled in an isolated crate with zero dependencies.\n\
        IMPORTANT: All string types must match — don't mix &str with String in if/else or match arms.\n\
        Use `.to_string()` or `String::from()` to convert &str to String where needed.\n\
        Put all code in a single ```rust block. No text before or after the block.",
        code_type
    )
}

fn write_temp_project(dir: &Path, code: &str, is_binary: bool) {
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"gen\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .ok();
    std::fs::create_dir_all(dir.join("src")).ok();

    let file_name = if is_binary { "main.rs" } else { "lib.rs" };
    let content = if is_binary {
        code.to_string()
    } else {
        format!("#![allow(dead_code)]\n{}", code)
    };
    std::fs::write(dir.join("src").join(file_name), content).ok();
}

fn cargo_check(dir: &Path) -> (bool, String) {
    match Command::new("cargo")
        .args(["check"])
        .current_dir(dir)
        .output()
    {
        Ok(o) => (
            o.status.success(),
            String::from_utf8_lossy(&o.stderr).into(),
        ),
        Err(e) => (false, e.to_string()),
    }
}

fn cargo_clippy(dir: &Path) -> (bool, String) {
    match Command::new("cargo")
        .args(["clippy", "--", "-D", "warnings"])
        .current_dir(dir)
        .output()
    {
        Ok(o) => (
            o.status.success(),
            String::from_utf8_lossy(&o.stderr).into(),
        ),
        Err(e) => (false, e.to_string()),
    }
}

fn cargo_test(dir: &Path) -> (bool, String) {
    match Command::new("cargo")
        .args(["test"])
        .current_dir(dir)
        .output()
    {
        Ok(o) => (
            o.status.success(),
            String::from_utf8_lossy(&o.stderr).into(),
        ),
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
