//! Inference — unified facade. InferenceRouter picks backend based on task + config.
//!
//! Submodules:
//!   local.rs     — Kalosm GGUF (was inference.rs)
//!   cluster.rs   — IRONHIVE distributed dispatch (was top-level cluster.rs)
//!   providers.rs — Multi-provider client (was top-level providers.rs)
//!
//! f382=dual_stream. Unified inference dispatcher. Reads KOVA_INFERENCE env:
//!   local  — Kalosm GGUF only (f76)
//!   remote — Anthropic API streaming (f381)
//!   auto   — local if model exists, else remote (default)
// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

pub mod cluster;
pub mod local;
pub mod providers;

// Re-export everything from local for backward compat (callers use crate::inference::f76 etc).
pub use local::*;

use std::path::Path;
use std::sync::{mpsc, Arc};

/// Default Anthropic model when using remote inference.
const REMOTE_MODEL: &str = "claude-sonnet-4-6";

/// f382=dual_stream. Unified inference — picks local or remote based on KOVA_INFERENCE env.
/// Same return type as f76: mpsc::Receiver<Arc<str>> for streamed tokens.
///
/// KOVA_INFERENCE values:
///   "local"  — Kalosm GGUF via f76. Fails if model_path doesn't exist.
///   "remote" — Anthropic Messages API via f381. Needs ANTHROPIC_API_KEY.
///   "auto"   — Local if model_path exists, else remote. (default)
pub fn f382(
    model_path: &Path,
    system_prompt: &str,
    history: &[(String, String)],
    user_input: &str,
) -> mpsc::Receiver<Arc<str>> {
    let mode = std::env::var("KOVA_INFERENCE")
        .unwrap_or_else(|_| "auto".into())
        .to_lowercase();

    match mode.as_str() {
        "local" => f76(model_path, system_prompt, history, user_input),
        "remote" => remote_stream(system_prompt, user_input),
        _ => {
            // Auto: local if model exists, remote otherwise.
            if model_path.exists() {
                f76(model_path, system_prompt, history, user_input)
            } else {
                eprintln!("\x1b[33m[inference: no local model, using remote]\x1b[0m");
                remote_stream(system_prompt, user_input)
            }
        }
    }
}

/// Stream from Anthropic API. Reads ANTHROPIC_API_KEY from env.
/// Uses KOVA_MODEL env to override the default model.
fn remote_stream(system_prompt: &str, user_input: &str) -> mpsc::Receiver<Arc<str>> {
    let api_key = match std::env::var("ANTHROPIC_API_KEY") {
        Ok(k) if !k.is_empty() => k,
        _ => {
            let (tx, rx) = mpsc::channel();
            let _ = tx.send(Arc::from(
                "Error: ANTHROPIC_API_KEY not set. Set it or use KOVA_INFERENCE=local.",
            ));
            return rx;
        }
    };

    let model = std::env::var("KOVA_MODEL").unwrap_or_else(|_| REMOTE_MODEL.into());

    providers::f381(&api_key, &model, system_prompt, user_input)
}