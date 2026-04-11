// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6

//! expert — Layer 0. Shared expert (always active) + routed experts (task-specific).
//!
//! DeepSeek-MoE insight: one shared expert handles common patterns (Rust syntax,
//! imports, derives). Routed experts handle specialized patterns (sled ops, axum routes).
//! More smaller experts > fewer larger experts.

use super::{Subtask, SubtaskKind, LayerOutput};
use crate::providers::T129;
use std::time::Instant;

/// Expert specialization — what pattern this expert generates.
#[derive(Debug, Clone, PartialEq)]
pub enum ExpertKind {
    // ── Shared (always active) ──
    RustBoilerplate,

    // ── Routed (activated per subtask) ──
    CargoToml,
    ClaudeConfig,
    StructDef,
    EnumDef,
    ThiserrorEnum,
    AxumRouter,
    AxumHandler,
    ClapParser,
    ClapSubcommand,
    SledRead,
    SledWrite,
    BincodeSerialize,
    ZstdCompress,
    ReqwestClient,
    CandleModelLoad,
    CandleForward,
    NanosignVerify,
    TestUnit,
    TestIntegration,
    ExopackGate,
}

/// Shared expert — always active, generates boilerplate every file needs.
pub struct SharedExpert;

impl SharedExpert {
    /// Generate the common boilerplate for a subtask.
    pub fn generate(subtask: &Subtask, provider: &T129) -> LayerOutput {
        let start = Instant::now();

        let system = "You are a Rust shared expert. Generate ONLY the common boilerplate: \
            Unlicense header comment, use statements, module-level doc comments. \
            Edition 2024 idioms. No business logic — just the skeleton. \
            Output raw Rust code in a ```rust block.";

        let prompt = format!(
            "Generate the boilerplate (header, imports, doc comment) for a Rust file that will contain: {}",
            subtask.context
        );

        let code = match crate::providers::f199(provider, "", system, &prompt) {
            Ok(r) => extract_rust(&r.text),
            Err(e) => format!("// shared expert error: {}", e),
        };

        LayerOutput {
            code,
            file_path: String::new(),
            source_expert: "shared".to_string(),
            generation_ms: start.elapsed().as_millis() as u64,
            compile_ok: None,
            error_output: None,
            retry_count: 0,
        }
    }
}

/// A routed expert — activated for specific subtask types.
pub struct Expert {
    pub kind: ExpertKind,
}

impl Expert {
    /// Select which experts handle a subtask. May return multiple.
    /// DeepSeek-MoE: fine-grained experts. Each subtask activates 2-3 specialists.
    pub fn for_subtask(subtask: &Subtask) -> Vec<Self> {
        match subtask.kind {
            SubtaskKind::Scaffold => vec![
                Self { kind: ExpertKind::CargoToml },
                Self { kind: ExpertKind::ClaudeConfig },
            ],
            SubtaskKind::Types => vec![
                Self { kind: ExpertKind::StructDef },
                Self { kind: ExpertKind::EnumDef },
            ],
            SubtaskKind::Errors => vec![
                Self { kind: ExpertKind::ThiserrorEnum },
            ],
            SubtaskKind::Web => vec![
                Self { kind: ExpertKind::AxumRouter },
                Self { kind: ExpertKind::AxumHandler },
            ],
            SubtaskKind::Cli => vec![
                Self { kind: ExpertKind::ClapParser },
                Self { kind: ExpertKind::ClapSubcommand },
            ],
            SubtaskKind::Storage => vec![
                Self { kind: ExpertKind::SledRead },
                Self { kind: ExpertKind::SledWrite },
                Self { kind: ExpertKind::BincodeSerialize },
                Self { kind: ExpertKind::ZstdCompress },
            ],
            SubtaskKind::Inference => vec![
                Self { kind: ExpertKind::CandleModelLoad },
                Self { kind: ExpertKind::CandleForward },
                Self { kind: ExpertKind::NanosignVerify },
            ],
            SubtaskKind::Tests => vec![
                Self { kind: ExpertKind::TestUnit },
                Self { kind: ExpertKind::TestIntegration },
                Self { kind: ExpertKind::ExopackGate },
            ],
        }
    }

    /// Generate code for this expert's specialization.
    pub fn generate(
        &self,
        subtask: &Subtask,
        shared: &LayerOutput,
        provider: &T129,
    ) -> LayerOutput {
        self.generate_with_context(subtask, shared, provider, None)
    }

    /// Generate with optional error context from a previous failed attempt.
    /// Phase 3: Sponge Mesh feeds error back so the expert can correct.
    pub fn generate_with_context(
        &self,
        subtask: &Subtask,
        shared: &LayerOutput,
        provider: &T129,
        error_ctx: Option<&str>,
    ) -> LayerOutput {
        let start = Instant::now();
        let system = self.system_prompt();

        let mut prompt = format!(
            "Context: {}\n\nShared boilerplate already generated:\n```rust\n{}\n```\n\n\
            Generate ONLY the {} code. Do not repeat the boilerplate. \
            Output raw Rust code in a ```rust block.",
            subtask.context,
            shared.code,
            self.description()
        );

        // Phase 3: Append error context from previous failed attempt
        if let Some(err) = error_ctx {
            prompt.push_str(&format!(
                "\n\nPrevious attempt failed:\n{}\nFix the issue. Do not repeat the same mistake.",
                err
            ));
        }

        // Flywheel: Check sled for past failures matching this expert type
        if let Some(hint) = super::compiler_teacher::lookup_hint(&self.kind) {
            prompt.push_str(&format!(
                "\n\nKnown pitfall for {:?}: previously generated code failed with: {}\nThe fix was to: {}",
                self.kind, hint.error, truncate_str(&hint.good_code, 200)
            ));
        }

        let code = match crate::providers::f199(provider, "", &system, &prompt) {
            Ok(r) => extract_rust(&r.text),
            Err(e) => format!("// {:?} expert error: {}", self.kind, e),
        };

        LayerOutput {
            code,
            file_path: self.default_file_path(),
            source_expert: format!("{:?}", self.kind),
            generation_ms: start.elapsed().as_millis() as u64,
            compile_ok: None,
            error_output: None,
            retry_count: 0,
        }
    }

    fn system_prompt(&self) -> String {
        match self.kind {
            ExpertKind::CargoToml => "Generate a Cargo.toml. Edition 2024. Unlicense. \
                Release profile: opt-level z, lto true, codegen-units 1, panic abort, strip true. \
                Only include dependencies needed for the described functionality.".to_string(),
            ExpertKind::ClaudeConfig => "Generate a CLAUDE.md file for a Rust project. \
                Include project name, build command, test command, key modules.".to_string(),
            ExpertKind::StructDef => "Generate Rust struct definitions with serde Serialize/Deserialize \
                derives. Use P13 compressed naming (t0, t1, etc.) for internal types. \
                Public API types get readable names.".to_string(),
            ExpertKind::EnumDef => "Generate Rust enum definitions. Use serde derives. \
                Variants should be descriptive. Include Display impl if user-facing.".to_string(),
            ExpertKind::ThiserrorEnum => "Generate a thiserror error enum. Cover all failure modes. \
                Each variant has a descriptive #[error(\"...\")] message. \
                Name: E0 for the primary error type.".to_string(),
            ExpertKind::AxumRouter => "Generate an axum Router with typed routes. \
                Use .route() chaining. Add CompressionLayer, security headers. \
                State via Arc<AppState>.".to_string(),
            ExpertKind::AxumHandler => "Generate axum route handler functions. \
                Each is pub async fn with State extractor. Return Html<String> or Json. \
                Named f0, f1, etc. per P13.".to_string(),
            ExpertKind::ClapParser => "Generate a clap Parser derive struct. \
                Include global args, version, about. Use value_parser where needed.".to_string(),
            ExpertKind::ClapSubcommand => "Generate a clap Subcommand derive enum. \
                Each variant has its own args. Match in main() to dispatch.".to_string(),
            ExpertKind::SledRead => "Generate sled read operations. Open tree, get by key, \
                deserialize via bincode, decompress via zstd. Return Result.".to_string(),
            ExpertKind::SledWrite => "Generate sled write operations. Serialize via bincode, \
                compress via zstd, insert into tree. Flush after critical writes.".to_string(),
            ExpertKind::BincodeSerialize => "Generate bincode 2.0 serialization helpers. \
                Use bincode::serde::encode_to_vec and decode_from_slice.".to_string(),
            ExpertKind::ZstdCompress => "Generate zstd compression/decompression helpers. \
                Compression level 3 for speed. Wrap around bincode output.".to_string(),
            ExpertKind::ReqwestClient => "Generate reqwest HTTP client code. Use rustls-tls. \
                Include rate limiting, retry logic, error handling.".to_string(),
            ExpertKind::CandleModelLoad => "Generate candle GGUF model loading. \
                Load from file path, verify with NanoSign before loading weights. \
                Return the model struct ready for inference.".to_string(),
            ExpertKind::CandleForward => "Generate candle forward pass. \
                Tokenize input, run through model, sample output. \
                Handle device selection (CPU vs GPU).".to_string(),
            ExpertKind::NanosignVerify => "Generate NanoSign BLAKE3 verification. \
                Read last 36 bytes of model file (4-byte magic 'NSIG' + 32-byte hash). \
                Hash the file excluding the signature. Compare. Reject on mismatch.".to_string(),
            ExpertKind::TestUnit => "Generate #[test] functions. One per public function. \
                Use assert_eq!, assert!(). Temp dirs for file operations. \
                No mocks — use real resources.".to_string(),
            ExpertKind::TestIntegration => "Generate integration tests. Test the full pipeline \
                end-to-end. Use real data, real storage, real operations.".to_string(),
            ExpertKind::ExopackGate => "Generate exopack triple_sims test gate. \
                Feature-gated behind #[cfg(feature = \"tests\")]. \
                Run all tests 3x, verify identical results.".to_string(),
            ExpertKind::RustBoilerplate => "Generate Rust boilerplate: Unlicense header, \
                use statements, module doc comment.".to_string(),
        }
    }

    fn description(&self) -> &str {
        match self.kind {
            ExpertKind::CargoToml => "Cargo.toml",
            ExpertKind::ClaudeConfig => "CLAUDE.md",
            ExpertKind::StructDef => "struct definitions",
            ExpertKind::EnumDef => "enum definitions",
            ExpertKind::ThiserrorEnum => "thiserror error enum",
            ExpertKind::AxumRouter => "axum router",
            ExpertKind::AxumHandler => "axum handlers",
            ExpertKind::ClapParser => "clap Parser",
            ExpertKind::ClapSubcommand => "clap Subcommand",
            ExpertKind::SledRead => "sled read operations",
            ExpertKind::SledWrite => "sled write operations",
            ExpertKind::BincodeSerialize => "bincode serialization",
            ExpertKind::ZstdCompress => "zstd compression",
            ExpertKind::ReqwestClient => "reqwest HTTP client",
            ExpertKind::CandleModelLoad => "candle model loading",
            ExpertKind::CandleForward => "candle forward pass",
            ExpertKind::NanosignVerify => "NanoSign verification",
            ExpertKind::TestUnit => "unit tests",
            ExpertKind::TestIntegration => "integration tests",
            ExpertKind::ExopackGate => "exopack triple_sims gate",
            ExpertKind::RustBoilerplate => "boilerplate",
        }
    }

    fn default_file_path(&self) -> String {
        match self.kind {
            ExpertKind::CargoToml => "Cargo.toml".to_string(),
            ExpertKind::ClaudeConfig => "CLAUDE.md".to_string(),
            ExpertKind::StructDef | ExpertKind::EnumDef => "src/types.rs".to_string(),
            ExpertKind::ThiserrorEnum => "src/error.rs".to_string(),
            ExpertKind::AxumRouter => "src/web/router.rs".to_string(),
            ExpertKind::AxumHandler => "src/web/pages.rs".to_string(),
            ExpertKind::ClapParser | ExpertKind::ClapSubcommand => "src/main.rs".to_string(),
            ExpertKind::SledRead | ExpertKind::SledWrite => "src/storage.rs".to_string(),
            ExpertKind::BincodeSerialize | ExpertKind::ZstdCompress => "src/storage.rs".to_string(),
            ExpertKind::ReqwestClient => "src/client.rs".to_string(),
            ExpertKind::CandleModelLoad | ExpertKind::CandleForward => "src/inference.rs".to_string(),
            ExpertKind::NanosignVerify => "src/nanosign.rs".to_string(),
            ExpertKind::TestUnit | ExpertKind::TestIntegration | ExpertKind::ExopackGate => "src/tests.rs".to_string(),
            ExpertKind::RustBoilerplate => "src/lib.rs".to_string(),
        }
    }
}

fn truncate_str(s: &str, max: usize) -> &str {
    if s.len() <= max { s } else { &s[..max] }
}

/// Extract Rust code from a ```rust ... ``` block.
fn extract_rust(text: &str) -> String {
    if let Some(start) = text.find("```rust") {
        let after = &text[start + 7..];
        if let Some(end) = after.find("```") {
            return after[..end].trim().to_string();
        }
    }
    if let Some(start) = text.find("```") {
        let after = &text[start + 3..];
        if let Some(end) = after.find("```") {
            return after[..end].trim().to_string();
        }
    }
    text.to_string()
}
