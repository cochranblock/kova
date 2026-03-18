// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! Code generation — shared infrastructure for all strategies.
//!
//! Submodules:
//!   helpers.rs    — extract_rust_block, f311, truncate (one copy)
//!   fix_loop.rs   — Shared fix logic (one copy)
//!   strategies/   — CodeGenStrategy trait + factory, moe, academy, gauntlet impls

pub mod fix_loop;
pub mod helpers;
pub mod strategies;

pub use helpers::{f327 as extract_rust_block, f311, f330 as truncate};
