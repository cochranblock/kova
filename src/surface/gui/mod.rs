//! GUI surface — egui native app.

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

mod gui_impl;
#[cfg(all(feature = "mobile-llm", feature = "inference"))]
pub mod micro_train;
pub mod pixel_forge;
pub mod products;
pub mod sprite_qc;
pub mod theme;

pub use gui_impl::*;