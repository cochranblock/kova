// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! KovaCommand — every kernel capability as a variant.
//! Surfaces build commands, kernel dispatches them.

use std::path::PathBuf;

/// Every capability the kernel can execute.
#[derive(Debug, Clone)]
pub enum KovaCommand {
    /// Run code gen pipeline (local inference).
    CodeGen {
        prompt: String,
        project_dir: PathBuf,
    },
    /// Run factory pipeline (cluster).
    Factory {
        prompt: String,
        project_dir: PathBuf,
    },
    /// Run MoE pipeline (cluster fan-out).
    Moe {
        prompt: String,
        num_experts: usize,
    },
    /// Run academy (autonomous agent).
    Academy {
        task: String,
        project_dir: PathBuf,
    },
    /// Run gauntlet (stress test).
    Gauntlet {
        phases: Option<Vec<u8>>,
    },
    /// Chat (single turn or REPL).
    Chat {
        prompt: String,
        project_dir: PathBuf,
    },
    /// Cargo command (tokenized x0-x9).
    Cargo {
        cmd: String,
        project: Option<String>,
    },
    /// Git command (tokenized g0-g9).
    Git {
        cmd: String,
    },
    /// Node command (tokenized c1-c9).
    Node {
        cmd: String,
    },
    /// Cluster status.
    ClusterStatus,
    /// Tool execution (from agent loop).
    ToolExec {
        tool: String,
        args: std::collections::HashMap<String, String>,
        project_dir: PathBuf,
    },
    /// Bootstrap ~/.kova.
    Bootstrap,
}
