//! train-kova-intent — train the kova intent classifier on kova-domain prompts.
//!
//! Reads `assets/datasets/kova_intents/{categories.json,train.csv}`, trains a
//! trigram-hash → linear classifier at FEATURE_DIM=4096 over 10 kova intent
//! classes (fix_bug, add_feature, explain_code, refactor, write_tests,
//! write_docs, review_code, run_command, search_code, project_info), and
//! writes weights to `assets/models/intent_classifier/`.
//!
//! Replaces the wrong-domain banking77 model that shipped as a PoC.
//!
//! Run:
//!   cargo run --release --bin train-kova-intent
//
// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Sonnet 4.6

use std::collections::HashMap;
use std::path::Path;

use kova::swarm::train::{t216, t217, f389};

const FEATURE_DIM: usize = 4096;
const EPOCHS: usize = 50;
const LR: f64 = 0.05;

fn main() {
    if let Err(e) = run() {
        eprintln!("train-kova-intent: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let project = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dataset = project.join("assets/datasets/kova_intents");
    let model_root = project.join("assets/models");

    let cats_path = dataset.join("categories.json");
    let cats_bytes =
        std::fs::read(&cats_path).map_err(|e| format!("read {}: {e}", cats_path.display()))?;
    let class_names: Vec<String> = serde_json::from_slice(&cats_bytes)
        .map_err(|e| format!("parse {}: {e}", cats_path.display()))?;
    eprintln!("[train-kova-intent] {} classes: {:?}", class_names.len(), class_names);

    let label_idx: HashMap<&str, usize> = class_names
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i))
        .collect();

    let train_csv = dataset.join("train.csv");
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(&train_csv)
        .map_err(|e| format!("open {}: {e}", train_csv.display()))?;

    let mut examples: Vec<t216> = Vec::new();
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
        examples.push(t216 { text, label });
    }
    if examples.is_empty() {
        return Err("no training examples".into());
    }
    eprintln!(
        "[train-kova-intent] {} examples loaded ({} skipped)",
        examples.len(),
        skipped
    );

    let config = t217 {
        name: "intent_classifier".into(),
        num_classes: class_names.len(),
        class_names: class_names.clone(),
        feature_dim: FEATURE_DIM,
        epochs: EPOCHS,
        lr: LR,
    };
    let saved = f389(&config, &examples, &model_root)
        .map_err(|e| format!("training failed: {e}"))?;

    eprintln!("[train-kova-intent] model saved to {}", saved.display());

    // Print per-class example counts.
    for (i, name) in class_names.iter().enumerate() {
        let count = examples.iter().filter(|e| e.label == i).count();
        eprintln!("  {:20} {} examples", name, count);
    }

    Ok(())
}
