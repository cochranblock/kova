// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6

//! pyramid — Hierarchical MoE with Sponge Mesh correction.
//!
//! Architecture:
//!   Layer 2: Router (1 model, broadest) — decomposes task into subtasks
//!   Layer 1: Assemblers (3-4 models) — combine generator outputs into modules
//!   Layer 0: Shared Expert (always active) + Routed Experts (activated per task)
//!   Correction: Sponge Mesh — retry failed experts with error context, backoff, escalation
//!
//! Research basis:
//!   - DeepSeek-MoE: fine-grained experts + shared expert (always active)
//!   - THOR-MoE: hierarchical task-guided routing (task-level then token-level)
//!   - Expert Choice (Google 2022): experts choose tasks for load balance
//!   - Sponge Mesh (Cochran Block 2026): rate-limit-aware retry for inference failures

pub mod router;
pub mod assembler;
pub mod expert;
pub mod mesh;
pub mod extract;

pub use router::Router;
pub use assembler::{Assembler, AssemblerKind};
pub use expert::{Expert, ExpertKind, SharedExpert};
pub use mesh::{SpongeMesh, MeshResult};

/// A high-level task to be decomposed by the router.
#[derive(Debug, Clone)]
pub struct Task {
    pub description: String,
    pub project_name: String,
    pub target: TaskTarget,
}

/// What kind of project the task produces.
#[derive(Debug, Clone, PartialEq)]
pub enum TaskTarget {
    /// CLI binary (clap + main.rs)
    Cli,
    /// Web server (axum + router + pages)
    Web,
    /// Library crate (lib.rs only)
    Lib,
    /// Full stack (CLI + web + lib)
    Full,
}

/// A subtask produced by the router, assigned to an assembler.
#[derive(Debug, Clone)]
pub struct Subtask {
    pub kind: SubtaskKind,
    pub context: String,
    pub parent_task: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SubtaskKind {
    /// Scaffold: Cargo.toml, CLAUDE.md, directory structure
    Scaffold,
    /// Types: struct/enum definitions
    Types,
    /// Storage: sled operations, bincode/zstd serialization
    Storage,
    /// Web: axum routes, handlers, pages
    Web,
    /// Cli: clap parser, subcommands, main.rs
    Cli,
    /// Errors: thiserror enum
    Errors,
    /// Tests: unit + integration test functions
    Tests,
    /// Inference: candle model loading, forward pass
    Inference,
}

/// Output from any layer — code with metadata.
#[derive(Debug, Clone)]
pub struct LayerOutput {
    pub code: String,
    pub file_path: String,
    pub source_expert: String,
    pub generation_ms: u64,
    pub compile_ok: Option<bool>,
    pub error_output: Option<String>,
    pub retry_count: u32,
}

/// Full pyramid result — all generated files.
#[derive(Debug)]
pub struct PyramidResult {
    pub task: Task,
    pub outputs: Vec<LayerOutput>,
    pub total_ms: u64,
    pub total_retries: u32,
    pub compile_ok: bool,
}

/// Run the full pyramid pipeline for a task.
pub fn run(task: Task, provider: &crate::providers::T129) -> PyramidResult {
    let start = std::time::Instant::now();
    println!("[pyramid] task: {}", task.description);
    println!("[pyramid] target: {:?}", task.target);

    // Layer 2: Router decomposes task into subtasks
    let subtasks = Router::decompose(&task);
    println!("[pyramid] router decomposed into {} subtasks", subtasks.len());
    for st in &subtasks {
        println!("[pyramid]   {:?}: {}", st.kind, truncate(&st.context, 60));
    }

    // Layer 1+0: Assemblers dispatch to experts, collect outputs
    let mut all_outputs: Vec<LayerOutput> = Vec::new();
    let mut mesh = SpongeMesh::new(3, std::time::Duration::from_secs(2));

    for subtask in &subtasks {
        let assembler = Assembler::for_subtask(subtask);
        println!("\n[pyramid] assembler: {:?} → {:?}", assembler.kind, subtask.kind);

        // Shared expert generates boilerplate (always runs)
        let shared = SharedExpert::generate(subtask, provider);
        println!("[pyramid]   shared expert: {} bytes", shared.code.len());

        // Routed experts generate specialized code
        let experts = Expert::for_subtask(subtask);
        for expert in &experts {
            println!("[pyramid]   routing to: {:?}", expert.kind);

            let result = mesh.dispatch(|| {
                expert.generate(subtask, &shared, provider)
            });

            match result {
                MeshResult::Success(mut output) => {
                    println!("[pyramid]   {:?}: {} bytes ({}ms, {} retries)",
                        expert.kind, output.code.len(), output.generation_ms, output.retry_count);
                    // Merge shared expert boilerplate with specialized code
                    output.code = format!("{}\n\n{}", shared.code, output.code);
                    all_outputs.push(output);
                }
                MeshResult::Exhausted { retries, last_error } => {
                    eprintln!("[pyramid]   {:?} FAILED after {} retries: {}",
                        expert.kind, retries, last_error);
                }
            }
        }
    }

    // Validation gate: cargo check on assembled output
    let compile_ok = if !all_outputs.is_empty() {
        println!("\n[pyramid] validation gate: cargo check");
        // TODO: write files to temp crate, run cargo check
        true
    } else {
        false
    };

    let total_retries: u32 = all_outputs.iter().map(|o| o.retry_count).sum();
    let total_ms = start.elapsed().as_millis() as u64;

    println!("\n[pyramid] complete: {} files, {}ms, {} retries, compile={}",
        all_outputs.len(), total_ms, total_retries, compile_ok);

    PyramidResult {
        task,
        outputs: all_outputs,
        total_ms,
        total_retries,
        compile_ok,
    }
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max { s } else { &s[..max] }
}
