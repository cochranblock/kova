//! retrain-starters — re-train slop_detector / code_vs_english / lang_detector
//! at FEATURE_DIM=8192 (gap analysis 12.1).
//!
//! The original 3 starters were trained at hash dim 256, which Weinberger 2009's
//! tail bound shows produces ~98% trigram collision rate against typical English
//! vocab — a hard quality cap. 8192 brings collision rate to ~58% per the same
//! bound. Cost: starter.nanobyte grows by ~290 KB; per-model training is ~30s
//! on bt CPU.
//!
//! Run:
//!   cargo run --release --bin retrain-starters
//
// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6

use std::path::Path;

use kova::swarm::train::f395;

const FEATURE_DIM: usize = 8192;

fn main() {
    if let Err(e) = run() {
        eprintln!("retrain-starters: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let project = Path::new(env!("CARGO_MANIFEST_DIR"));
    let model_root = project.join("assets/models");
    eprintln!(
        "[retrain-starters] FEATURE_DIM={} (was 256), output={}",
        FEATURE_DIM,
        model_root.display()
    );
    f395(project, &model_root, FEATURE_DIM)?;
    Ok(())
}
