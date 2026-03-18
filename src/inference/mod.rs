//! Inference — unified facade. InferenceRouter picks backend based on task + config.
//!
//! Submodules:
//!   local.rs     — Kalosm GGUF (was inference.rs)
//!   cluster.rs   — IRONHIVE distributed dispatch (was top-level cluster.rs)
//!   providers.rs — Multi-provider client (was top-level providers.rs)
// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

pub mod cluster;
pub mod local;
pub mod providers;

// Re-export everything from local for backward compat (callers use crate::inference::f76 etc).
pub use local::*;