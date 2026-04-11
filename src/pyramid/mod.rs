// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6

//! pyramid — Hierarchical MoE with Sponge Mesh correction.
//!
//! Architecture:
//!   Layer 2: Router (1 model, broadest) — decomposes task into subtasks
//!   Layer 1: Assemblers (3-4 models) — combine generator outputs into modules
//!   Layer 0: Shared Expert (always active) + Routed Experts (activated per task)
//!   Correction: Sponge Mesh — retry failed experts with error context, backoff, escalation
//!   Flywheel: Compiler Teacher — save (bad, error, good) pairs from mesh corrections
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
pub mod compiler_teacher;

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
    Cli,
    Web,
    Lib,
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
    Scaffold,
    Types,
    Storage,
    Web,
    Cli,
    Errors,
    Tests,
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
    let mesh = SpongeMesh::new(3, std::time::Duration::from_secs(2));

    for subtask in &subtasks {
        let assembler = Assembler::for_subtask(subtask);
        println!("\n[pyramid] assembler: {:?} → {:?}", assembler.kind, subtask.kind);

        // Shared expert generates boilerplate (always runs)
        let shared = SharedExpert::generate(subtask, provider);
        println!("[pyramid]   shared expert: {} bytes", shared.code.len());

        // Phase 6: Parallel expert execution via mpsc
        let experts = Expert::for_subtask(subtask);
        let (tx, rx) = std::sync::mpsc::channel::<LayerOutput>();

        let handles: Vec<_> = experts
            .into_iter()
            .map(|expert| {
                let tx = tx.clone();
                let shared_clone = shared.clone();
                let subtask_clone = subtask.clone();
                let provider_clone = provider.clone();
                let mesh_clone = SpongeMesh::new(3, std::time::Duration::from_secs(2));

                std::thread::spawn(move || {
                    println!("[pyramid]   routing to: {:?}", expert.kind);

                    // Phase 3: dispatch_with_context — expert sees its own errors
                    let expert_kind = expert.kind.clone();
                    let result = mesh_clone.dispatch_with_context(|err_ctx| {
                        expert.generate_with_context(
                            &subtask_clone,
                            &shared_clone,
                            &provider_clone,
                            err_ctx,
                        )
                    });

                    match result {
                        MeshResult::Success(mut output) => {
                            println!(
                                "[pyramid]   {:?}: {} bytes ({}ms, {} retries)",
                                expert_kind,
                                output.code.len(),
                                output.generation_ms,
                                output.retry_count
                            );
                            output.code = format!("{}\n\n{}", shared_clone.code, output.code);
                            let _ = tx.send(output);
                        }
                        MeshResult::Exhausted {
                            retries,
                            last_error,
                        } => {
                            eprintln!(
                                "[pyramid]   {:?} FAILED after {} retries: {}",
                                expert_kind, retries, last_error
                            );
                        }
                    }
                })
            })
            .collect();

        drop(tx);
        for output in rx {
            all_outputs.push(output);
        }
        for h in handles {
            let _ = h.join();
        }
    }

    // Phase 2: Validation gate — cargo check on assembled output
    let compile_ok = if !all_outputs.is_empty() {
        println!("\n[pyramid] validation gate: cargo check");
        validate_outputs(&all_outputs, &task.project_name)
    } else {
        false
    };

    let total_retries: u32 = all_outputs.iter().map(|o| o.retry_count).sum();
    let total_ms = start.elapsed().as_millis() as u64;

    println!(
        "\n[pyramid] complete: {} files, {}ms, {} retries, compile={}",
        all_outputs.len(),
        total_ms,
        total_retries,
        compile_ok
    );

    PyramidResult {
        task,
        outputs: all_outputs,
        total_ms,
        total_retries,
        compile_ok,
    }
}

/// Phase 2: Validation gate — write outputs to temp crate, run cargo check.
fn validate_outputs(outputs: &[LayerOutput], project_name: &str) -> bool {
    let tmp = match tempfile::TempDir::new() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("[pyramid] validation: temp dir failed: {}", e);
            return false;
        }
    };

    let base = tmp.path();

    // Write each output file
    for output in outputs {
        if output.file_path.is_empty() {
            continue;
        }
        let dest = base.join(&output.file_path);
        if let Some(parent) = dest.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        // Merge outputs targeting the same file
        if dest.exists() {
            let existing = std::fs::read_to_string(&dest).unwrap_or_default();
            let _ = std::fs::write(&dest, format!("{}\n\n{}", existing, output.code));
        } else {
            let _ = std::fs::write(&dest, &output.code);
        }
    }

    // Generate Cargo.toml if not present
    let cargo_path = base.join("Cargo.toml");
    if !cargo_path.exists() {
        let cargo_toml = format!(
            "[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
            project_name
        );
        let _ = std::fs::write(&cargo_path, cargo_toml);
    }

    // Ensure src dir exists
    let _ = std::fs::create_dir_all(base.join("src"));
    let lib_path = base.join("src/lib.rs");
    if !lib_path.exists() && !base.join("src/main.rs").exists() {
        let _ = std::fs::write(&lib_path, "");
    }

    // Run cargo check
    let result = std::process::Command::new("cargo")
        .arg("check")
        .current_dir(base)
        .output();

    match result {
        Ok(out) => {
            let ok = out.status.success();
            if !ok {
                let stderr = String::from_utf8_lossy(&out.stderr);
                eprintln!(
                    "[pyramid] validation FAILED:\n{}",
                    truncate(&stderr, 500)
                );
            } else {
                println!("[pyramid] validation PASSED");
            }
            ok
        }
        Err(e) => {
            eprintln!("[pyramid] validation: cargo check failed to run: {}", e);
            false
        }
    }
}

/// Phase 4: Write validated outputs to project directory.
pub fn write_outputs(result: &PyramidResult, out_dir: &std::path::Path) -> std::io::Result<()> {
    // Group outputs by file_path, merge same-file outputs
    let mut by_file: std::collections::HashMap<&str, Vec<&str>> = std::collections::HashMap::new();
    for output in &result.outputs {
        if !output.file_path.is_empty() {
            by_file
                .entry(&output.file_path)
                .or_default()
                .push(&output.code);
        }
    }

    for (file_path, code_parts) in &by_file {
        let dest = out_dir.join(file_path);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let merged = code_parts.join("\n\n");
        std::fs::write(&dest, &merged)?;
        println!("[pyramid] wrote {}", dest.display());
    }

    // Run cargo fmt if available
    let _ = std::process::Command::new("cargo")
        .arg("fmt")
        .current_dir(out_dir)
        .output();

    Ok(())
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..max]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn router_decompose_cli_target() {
        let task = Task {
            description: "build a CLI tool".to_string(),
            project_name: "test".to_string(),
            target: TaskTarget::Cli,
        };
        let subtasks = Router::decompose(&task);
        assert!(!subtasks.is_empty());
        assert!(subtasks.iter().any(|s| s.kind == SubtaskKind::Scaffold));
        assert!(subtasks.iter().any(|s| s.kind == SubtaskKind::Cli));
    }

    #[test]
    fn expert_for_subtask_returns_nonempty() {
        let subtask = Subtask {
            kind: SubtaskKind::Storage,
            context: "sled storage".to_string(),
            parent_task: "test".to_string(),
        };
        let experts = Expert::for_subtask(&subtask);
        assert!(!experts.is_empty());
        assert!(experts.iter().any(|e| e.kind == ExpertKind::SledRead));
    }

    #[test]
    fn mesh_validates_empty_as_failure() {
        let mesh = SpongeMesh::new(1, std::time::Duration::from_millis(10));
        let result = mesh.dispatch(|| LayerOutput {
            code: "   ".to_string(),
            file_path: String::new(),
            source_expert: "test".to_string(),
            generation_ms: 0,
            compile_ok: None,
            error_output: None,
            retry_count: 0,
        });
        assert!(matches!(result, MeshResult::Exhausted { .. }));
    }

    #[test]
    fn mesh_success_on_valid_output() {
        let mesh = SpongeMesh::new(1, std::time::Duration::from_millis(10));
        let result = mesh.dispatch(|| LayerOutput {
            code: "fn main() {}".to_string(),
            file_path: "src/main.rs".to_string(),
            source_expert: "test".to_string(),
            generation_ms: 5,
            compile_ok: None,
            error_output: None,
            retry_count: 0,
        });
        assert!(matches!(result, MeshResult::Success(_)));
    }

    #[test]
    fn mesh_dispatch_with_context_passes_error() {
        let mesh = SpongeMesh::new(2, std::time::Duration::from_millis(10));
        let call_count = std::sync::atomic::AtomicU32::new(0);
        let result = mesh.dispatch_with_context(|err_ctx| {
            let n = call_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if n == 0 {
                LayerOutput {
                    code: String::new(),
                    file_path: String::new(),
                    source_expert: "test".to_string(),
                    generation_ms: 0,
                    compile_ok: Some(false),
                    error_output: Some("expected struct".to_string()),
                    retry_count: 0,
                }
            } else {
                assert!(err_ctx.is_some());
                LayerOutput {
                    code: "fn main() {}".to_string(),
                    file_path: "src/main.rs".to_string(),
                    source_expert: "test".to_string(),
                    generation_ms: 1,
                    compile_ok: None,
                    error_output: None,
                    retry_count: 0,
                }
            }
        });
        assert!(matches!(result, MeshResult::Success(_)));
    }
}
