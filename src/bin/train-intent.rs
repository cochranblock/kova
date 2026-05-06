//! train-intent — train the banking77 intent classifier subatomic model.
//!
//! Reads `assets/datasets/banking77/{categories.json,train.csv}`, trains a
//! trigram-hash → linear classifier at FEATURE_DIM=4096 over 77 classes
//! (~315K params, ~1.25 MB on disk), and writes weights to
//! `assets/models/intent_classifier/`.
//!
//! Hash dim 4096 is a deliberate compromise between the gap-analysis-flagged
//! 256 (Weinberger 2009 collision floor ~98%) and a fully unconstrained 8192+
//! (which would dominate the embedded starter at 2.5 MB). 4096 brings the
//! collision rate to ~77% — still bad by FastText standards (default 2M
//! buckets), but a 21-percentage-point improvement that we can actually
//! afford to ship today. Subsequent retrains at higher dims, or a learned
//! feature extractor, are documented in docs/GAP_ANALYSIS_SIM_2026-05-06.md.
//!
//! Run:
//!   cargo run --release --bin train-intent
//
// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6

use std::collections::HashMap;
use std::path::Path;

use kova::swarm::train::{Example, SubatomicConfig, f389};

const FEATURE_DIM: usize = 4096;
const EPOCHS: usize = 30;
const LR: f64 = 0.05;

fn main() {
    if let Err(e) = run() {
        eprintln!("train-intent: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let project = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dataset = project.join("assets/datasets/banking77");
    let model_root = project.join("assets/models");

    // 1. Load class names (ordered → label index).
    let cats_path = dataset.join("categories.json");
    let cats_bytes =
        std::fs::read(&cats_path).map_err(|e| format!("read {}: {e}", cats_path.display()))?;
    let class_names: Vec<String> = serde_json::from_slice(&cats_bytes)
        .map_err(|e| format!("parse {}: {e}", cats_path.display()))?;
    if class_names.len() != 77 {
        return Err(format!(
            "expected 77 banking77 categories, got {}",
            class_names.len()
        ));
    }
    let label_idx: HashMap<&str, usize> = class_names
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i))
        .collect();

    // 2. Load training set.
    let train_csv = dataset.join("train.csv");
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(&train_csv)
        .map_err(|e| format!("open {}: {e}", train_csv.display()))?;

    let mut examples: Vec<Example> = Vec::new();
    let mut skipped = 0usize;
    for (i, rec) in rdr.records().enumerate() {
        let rec = rec.map_err(|e| format!("csv row {i}: {e}"))?;
        if rec.len() != 2 {
            skipped += 1;
            continue;
        }
        let text = rec.get(0).unwrap().to_string();
        let cat = rec.get(1).unwrap();
        let Some(&label) = label_idx.get(cat) else {
            return Err(format!("row {i}: unknown category {cat:?}"));
        };
        examples.push(Example { text, label });
    }
    if examples.is_empty() {
        return Err("no training examples".into());
    }
    eprintln!(
        "[train-intent] loaded {} examples, {} classes (skipped {} malformed)",
        examples.len(),
        class_names.len(),
        skipped
    );

    // 3. Train.
    let config = SubatomicConfig {
        name: "intent_classifier".into(),
        num_classes: class_names.len(),
        class_names: class_names.clone(),
        feature_dim: FEATURE_DIM,
        epochs: EPOCHS,
        lr: LR,
    };
    let saved = f389(&config, &examples, &model_root)
        .map_err(|e| format!("training failed: {e}"))?;

    eprintln!("[train-intent] saved to {}", saved.display());
    Ok(())
}
