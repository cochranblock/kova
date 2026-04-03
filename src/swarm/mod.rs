//! swarm — Subatomic model training and inference.
//! Tiny classifiers (<10K params) trained on CPU via candle.
//! Architecture: character n-gram hash → embedding bag → linear → output.
// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6

pub mod train;
