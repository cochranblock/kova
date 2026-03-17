// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! CLI surface — REPL, agent loop, tokenized commands.
//! Thin adapter. Business logic lives in kernel/capabilities.

#[cfg(feature = "inference")]
pub mod agent_loop;
pub mod cargo_cmd;
pub mod git_cmd;
pub mod node_cmd;
#[cfg(feature = "inference")]
pub mod repl;
