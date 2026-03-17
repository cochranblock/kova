// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! Surface adapters — thin layers that dispatch to kernel.
//! CLI, Serve (Axum), GUI (egui).

pub mod cli;

#[cfg(feature = "serve")]
pub mod serve;

#[cfg(feature = "gui")]
pub mod gui;
