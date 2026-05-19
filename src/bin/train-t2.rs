//! train-t2 — train the T2 molecular coordinator on T1 output features.
//!
//! Loads starter.nanobyte (T1 models), runs each kova_intents train.csv example
//! through all 4 T1 models to build 8-float feature vectors, assigns routing labels
//! (code_responder vs prose_responder), then trains a linear classifier via SGD.
//! Saves weights.bin + bias.bin + config.json to assets/models/t2_coordinator/.
//!
//! Run:
//!   cargo run --release --bin train-t2
//
// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Sonnet 4.6

use std::path::Path;

use kova::nanobyte;
use kova::swarm::t2::{T2_FEATURE_DIM, f410};

const N_CLASSES: usize = 2;
const EPOCHS: usize = 300;
const LR: f64 = 0.05;

// code_responder (0) intents — action-oriented, produces code
const CODE_INTENTS: &[&str] = &[
    "fix_bug", "add_feature", "write_tests", "review_code", "run_command", "search_code",
];

fn intent_to_label(intent: &str) -> usize {
    if CODE_INTENTS.contains(&intent) { 0 } else { 1 }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("train-t2: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let project = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dataset  = project.join("assets/datasets/kova_intents");
    let model_out = project.join("assets/models/t2_coordinator");

    let nb = nanobyte::starter().map_err(|e| format!("load starter: {e}"))?;

    let train_csv = dataset.join("train.csv");
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(&train_csv)
        .map_err(|e| format!("open {}: {e}", train_csv.display()))?;

    let mut examples: Vec<([f32; T2_FEATURE_DIM], usize)> = Vec::new();
    let mut skipped = 0usize;
    for (i, rec) in rdr.records().enumerate() {
        let rec = rec.map_err(|e| format!("csv row {i}: {e}"))?;
        if rec.len() != 2 { skipped += 1; continue; }
        let text   = rec.get(0).unwrap().to_string();
        let intent = rec.get(1).unwrap().to_string();
        let label  = intent_to_label(&intent);
        let feat   = f410(&nb, &text);
        examples.push((feat, label));
    }
    eprintln!(
        "[train-t2] {} examples ({} skipped) — {} code, {} prose",
        examples.len(), skipped,
        examples.iter().filter(|(_, l)| *l == 0).count(),
        examples.iter().filter(|(_, l)| *l == 1).count(),
    );

    // Weights W[c][d] and bias b[c]; f64 for stable gradient accumulation.
    let mut w = vec![vec![0.0f64; T2_FEATURE_DIM]; N_CLASSES];
    let mut b = vec![0.0f64; N_CLASSES];

    for epoch in 0..EPOCHS {
        let mut total_loss = 0.0f64;
        let mut correct = 0usize;

        for (feat, label) in &examples {
            // Forward: logits[c] = dot(W[c], feat) + b[c]
            let mut logits = vec![0.0f64; N_CLASSES];
            for c in 0..N_CLASSES {
                logits[c] = b[c];
                for d in 0..T2_FEATURE_DIM {
                    logits[c] += w[c][d] * feat[d] as f64;
                }
            }

            // Softmax
            let max_l = logits.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let exp: Vec<f64> = logits.iter().map(|&l| (l - max_l).exp()).collect();
            let sum_exp: f64  = exp.iter().sum();
            let probs: Vec<f64> = exp.iter().map(|&e| e / sum_exp).collect();

            total_loss -= probs[*label].ln().max(-100.0);
            let pred = probs.iter().enumerate()
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
                .map(|(i, _)| i).unwrap_or(0);
            if pred == *label { correct += 1; }

            // Backward: dL/dlogit[c] = probs[c] - 1{c == label}
            for c in 0..N_CLASSES {
                let g = probs[c] - if c == *label { 1.0 } else { 0.0 };
                b[c] -= LR * g;
                for d in 0..T2_FEATURE_DIM {
                    w[c][d] -= LR * g * feat[d] as f64;
                }
            }
        }

        if epoch % 60 == 0 || epoch == EPOCHS - 1 {
            let acc = 100.0 * correct as f64 / examples.len() as f64;
            eprintln!(
                "[train-t2] epoch {}/{}: loss={:.4} acc={:.1}%",
                epoch + 1, EPOCHS,
                total_loss / examples.len() as f64, acc
            );
        }
    }

    // Save
    std::fs::create_dir_all(&model_out)
        .map_err(|e| format!("mkdir: {e}"))?;

    let w_flat: Vec<f32> = w.iter().flat_map(|row| row.iter().map(|&v| v as f32)).collect();
    let b_flat: Vec<f32> = b.iter().map(|&v| v as f32).collect();

    std::fs::write(
        model_out.join("weights.bin"),
        w_flat.iter().flat_map(|f| f.to_le_bytes()).collect::<Vec<u8>>(),
    ).map_err(|e| format!("write weights: {e}"))?;
    std::fs::write(
        model_out.join("bias.bin"),
        b_flat.iter().flat_map(|f| f.to_le_bytes()).collect::<Vec<u8>>(),
    ).map_err(|e| format!("write bias: {e}"))?;

    let config = serde_json::json!({
        "name":         "t2_coordinator",
        "tier":         2,
        "feature_dim":  T2_FEATURE_DIM,
        "num_classes":  N_CLASSES,
        "total_params": N_CLASSES * T2_FEATURE_DIM + N_CLASSES
    });
    std::fs::write(
        model_out.join("config.json"),
        serde_json::to_string_pretty(&config).unwrap(),
    ).map_err(|e| format!("write config: {e}"))?;

    std::fs::write(
        model_out.join("categories.json"),
        serde_json::to_string_pretty(&["code_responder", "prose_responder"]).unwrap(),
    ).map_err(|e| format!("write categories: {e}"))?;

    eprintln!(
        "[train-t2] saved to {} ({} params)",
        model_out.display(),
        N_CLASSES * T2_FEATURE_DIM + N_CLASSES
    );
    Ok(())
}
