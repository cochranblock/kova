// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6

//! assembler — Layer 1. Combines generator outputs into complete modules.
//! Each assembler understands how multiple experts' outputs fit together.

use super::{Subtask, SubtaskKind};

/// Assembler kind — which domain this assembler handles.
#[derive(Debug, Clone, PartialEq)]
pub enum AssemblerKind {
    /// Web assembler: routes + pages + assets + state
    Web,
    /// CLI assembler: main.rs + clap + subcommands
    Cli,
    /// Infrastructure assembler: Cargo.toml + CLAUDE.md + test binary + TOI + POA
    Infra,
    /// Inference assembler: model loading + forward pass + NanoSign
    Inference,
}

/// An assembler that combines expert outputs for a domain.
pub struct Assembler {
    pub kind: AssemblerKind,
}

impl Assembler {
    /// Select the right assembler for a subtask.
    pub fn for_subtask(subtask: &Subtask) -> Self {
        let kind = match subtask.kind {
            SubtaskKind::Web => AssemblerKind::Web,
            SubtaskKind::Cli => AssemblerKind::Cli,
            SubtaskKind::Scaffold | SubtaskKind::Tests => AssemblerKind::Infra,
            SubtaskKind::Inference => AssemblerKind::Inference,
            // Types, Errors, Storage route through Infra
            SubtaskKind::Types | SubtaskKind::Errors | SubtaskKind::Storage => AssemblerKind::Infra,
        };
        Self { kind }
    }

    /// Build the system prompt for this assembler's domain.
    pub fn system_prompt(&self) -> String {
        match self.kind {
            AssemblerKind::Web => {
                "You are a Rust web module assembler. You combine axum route handlers, \
                HTML page functions, asset serving, and shared state into a complete web module. \
                Use axum 0.7, tower-http for compression/headers, Arc<State> for shared state. \
                Every route handler is a pub async fn. Router uses .route() chaining. \
                No JavaScript unless absolutely required.".to_string()
            }
            AssemblerKind::Cli => {
                "You are a Rust CLI assembler. You combine clap Parser structs, \
                Subcommand enums, and handler functions into a complete main.rs. \
                Use clap 4 with derive. Every subcommand maps to a handler function. \
                main() parses args and dispatches. Error handling via anyhow::Result.".to_string()
            }
            AssemblerKind::Infra => {
                "You are a Rust project infrastructure assembler. You generate Cargo.toml \
                (edition 2024, MSRV 1.85, Unlicense, release profile with opt-level z, lto, \
                strip, panic abort), CLAUDE.md, test binary pattern (same binary for prod + test, \
                feature-gated via exopack), types, error enums (thiserror), and storage modules \
                (sled + bincode + zstd).".to_string()
            }
            AssemblerKind::Inference => {
                "You are a Rust AI inference assembler. You generate candle model loading \
                (GGUF format), NanoSign verification (BLAKE3 36-byte signature check before load), \
                forward pass execution, and MoE routing (select model tier based on hardware). \
                Use candle-core, candle-nn, candle-transformers.".to_string()
            }
        }
    }
}
