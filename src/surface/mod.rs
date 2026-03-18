//! Surface adapters — thin layers that dispatch to kernel.
//! Serve (Axum), GUI (egui).

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

#[cfg(feature = "serve")]
pub mod serve;

#[cfg(feature = "gui")]
pub mod gui;