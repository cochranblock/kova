// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6

//! router — Layer 2. Decomposes a task into subtasks.
//! Broadest model in the pyramid. Understands all domains.
//! Routes to assemblers based on task target.

use super::{Task, TaskTarget, Subtask, SubtaskKind};

/// Top-level router. Decomposes tasks into typed subtasks.
pub struct Router;

impl Router {
    /// Decompose a task into subtasks based on its target type.
    /// This is the coarse-grained routing decision — "what kind of project is this?"
    /// Each subtask maps to an assembler + set of experts.
    pub fn decompose(task: &Task) -> Vec<Subtask> {
        let mut subtasks = Vec::new();
        let ctx = &task.description;
        let name = &task.project_name;

        // Every project needs scaffold + types + errors
        subtasks.push(Subtask {
            kind: SubtaskKind::Scaffold,
            context: format!("Create project scaffold for '{}': Cargo.toml with edition 2024, \
                release profile (opt-level z, lto, strip), Unlicense, repository url, \
                keywords, categories. Project: {}", name, ctx),
            parent_task: ctx.clone(),
        });

        subtasks.push(Subtask {
            kind: SubtaskKind::Types,
            context: format!("Define core types (structs, enums) for '{}'. \
                Use serde derives where needed. P13 compression naming. \
                Project: {}", name, ctx),
            parent_task: ctx.clone(),
        });

        subtasks.push(Subtask {
            kind: SubtaskKind::Errors,
            context: format!("Define error types using thiserror for '{}'. \
                Cover all failure modes. Project: {}", name, ctx),
            parent_task: ctx.clone(),
        });

        // Target-specific subtasks
        match task.target {
            TaskTarget::Cli => {
                subtasks.push(Subtask {
                    kind: SubtaskKind::Cli,
                    context: format!("Create CLI interface with clap derive for '{}'. \
                        Parser struct + Subcommand enum. Project: {}", name, ctx),
                    parent_task: ctx.clone(),
                });
            }
            TaskTarget::Web => {
                subtasks.push(Subtask {
                    kind: SubtaskKind::Web,
                    context: format!("Create axum web server for '{}'. \
                        Router with typed handlers, state via Arc. \
                        Compression + security headers. Project: {}", name, ctx),
                    parent_task: ctx.clone(),
                });
            }
            TaskTarget::Full => {
                subtasks.push(Subtask {
                    kind: SubtaskKind::Cli,
                    context: format!("Create CLI with clap + serve subcommand for '{}'. \
                        Project: {}", name, ctx),
                    parent_task: ctx.clone(),
                });
                subtasks.push(Subtask {
                    kind: SubtaskKind::Web,
                    context: format!("Create axum web layer for '{}'. Project: {}", name, ctx),
                    parent_task: ctx.clone(),
                });
            }
            TaskTarget::Lib => {}
        }

        // Storage if the task mentions data, database, cache, store, persist
        let lower = ctx.to_lowercase();
        if lower.contains("data") || lower.contains("store") || lower.contains("cache")
            || lower.contains("persist") || lower.contains("database") || lower.contains("save")
        {
            subtasks.push(Subtask {
                kind: SubtaskKind::Storage,
                context: format!("Implement sled storage layer for '{}'. \
                    bincode serialization, zstd compression. Project: {}", name, ctx),
                parent_task: ctx.clone(),
            });
        }

        // Inference if the task mentions AI, model, inference, predict, classify
        if lower.contains("ai") || lower.contains("model") || lower.contains("inference")
            || lower.contains("predict") || lower.contains("classify") || lower.contains("llm")
        {
            subtasks.push(Subtask {
                kind: SubtaskKind::Inference,
                context: format!("Implement candle inference for '{}'. \
                    GGUF model loading, NanoSign verification, forward pass. \
                    Project: {}", name, ctx),
                parent_task: ctx.clone(),
            });
        }

        // Tests always come last
        subtasks.push(Subtask {
            kind: SubtaskKind::Tests,
            context: format!("Write tests for '{}'. Unit tests for each module. \
                Integration test for the full pipeline. \
                exopack triple_sims gate. Project: {}", name, ctx),
            parent_task: ctx.clone(),
        });

        subtasks
    }
}
