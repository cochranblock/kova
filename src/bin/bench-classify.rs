//! bench-classify — measure starter classifiers against held-out test sets.
//!
//! Anchors gap 12.2 (no standard-benchmark validation) in real numbers.
//! Runs `intent_classifier` against the kova_intents test split (33 examples,
//! 10 classes). Adding more (model, dataset) pairs is a one-line edit to BENCHES.
//!
//! Output: total accuracy, macro-precision, macro-recall, macro-F1, the
//! 10 lowest-F1 classes, and the top confusion pairs.
//
// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, Claude Sonnet 4.6

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use kova::nanobyte::{self, Nanobyte};

/// (model_name_in_nanobyte, dataset_name, csv_path_relative_to_assets, text_col, label_col).
struct Bench {
    model: &'static str,
    dataset: &'static str,
    csv_path: &'static str,
}

const BENCHES: &[Bench] = &[Bench {
    model: "intent_classifier",
    dataset: "kova_intents (test split)",
    csv_path: "datasets/kova_intents/test.csv",
}];

fn main() {
    if let Err(e) = run() {
        eprintln!("bench-classify: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let project = Path::new(env!("CARGO_MANIFEST_DIR"));
    let assets = project.join("assets");

    let nb = nanobyte::starter().map_err(|e| format!("load starter.nanobyte: {e}"))?;

    for b in BENCHES {
        run_bench(&nb, b, &assets)?;
    }
    Ok(())
}

fn run_bench(nb: &Nanobyte, b: &Bench, assets: &Path) -> Result<(), String> {
    let class_names: &[String] = nb
        .manifests()
        .iter()
        .find(|m| m.name == b.model)
        .map(|m| m.class_names.as_slice())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("model {:?} not found or has no class names", b.model))?;
    let label_idx: HashMap<&str, usize> = class_names
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i))
        .collect();

    let csv_path: PathBuf = assets.join(b.csv_path);
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(&csv_path)
        .map_err(|e| format!("open {}: {e}", csv_path.display()))?;

    let nc = class_names.len();
    let mut tp = vec![0u32; nc];
    let mut fp = vec![0u32; nc];
    let mut fn_ = vec![0u32; nc];
    let mut total = 0u32;
    let mut correct = 0u32;
    let mut confusion: HashMap<(usize, usize), u32> = HashMap::new();
    let mut unknown_label = 0u32;

    for rec in rdr.records() {
        let rec = rec.map_err(|e| format!("csv: {e}"))?;
        if rec.len() != 2 {
            continue;
        }
        let text = rec.get(0).unwrap();
        let true_name = rec.get(1).unwrap();
        let Some(&true_idx) = label_idx.get(true_name) else {
            unknown_label += 1;
            continue;
        };
        let (pred_idx, _conf) = nb
            .infer(b.model, text)
            .map_err(|e| format!("infer: {e}"))?;
        total += 1;
        if pred_idx == true_idx {
            correct += 1;
            tp[true_idx] += 1;
        } else {
            fp[pred_idx] += 1;
            fn_[true_idx] += 1;
            *confusion.entry((true_idx, pred_idx)).or_default() += 1;
        }
    }

    if total == 0 {
        return Err(format!("{}: zero rows scored", b.dataset));
    }

    println!("\n=== {} on {} ===", b.model, b.dataset);
    println!("rows scored: {total}, unknown labels skipped: {unknown_label}");

    let acc = correct as f64 / total as f64;
    println!("accuracy:    {:.2}% ({}/{})", acc * 100.0, correct, total);

    // Per-class precision / recall / F1.
    let mut per_class: Vec<(usize, f64, f64, f64, u32)> = Vec::with_capacity(nc);
    for c in 0..nc {
        let tpc = tp[c] as f64;
        let fpc = fp[c] as f64;
        let fnc = fn_[c] as f64;
        let p = if tpc + fpc > 0.0 { tpc / (tpc + fpc) } else { 0.0 };
        let r = if tpc + fnc > 0.0 { tpc / (tpc + fnc) } else { 0.0 };
        let f1 = if p + r > 0.0 { 2.0 * p * r / (p + r) } else { 0.0 };
        let support = tp[c] + fn_[c];
        per_class.push((c, p, r, f1, support));
    }

    let macro_p: f64 = per_class.iter().map(|x| x.1).sum::<f64>() / nc as f64;
    let macro_r: f64 = per_class.iter().map(|x| x.2).sum::<f64>() / nc as f64;
    let macro_f1: f64 = per_class.iter().map(|x| x.3).sum::<f64>() / nc as f64;
    println!(
        "macro:       precision={:.2}%  recall={:.2}%  f1={:.2}%",
        macro_p * 100.0,
        macro_r * 100.0,
        macro_f1 * 100.0
    );

    // 10 lowest-F1 classes.
    let mut sorted = per_class.clone();
    sorted.sort_by(|a, b| a.3.partial_cmp(&b.3).unwrap_or(std::cmp::Ordering::Equal));
    println!("\n10 lowest-F1 classes:");
    println!("  {:<48} {:>7} {:>7} {:>7} {:>7}", "class", "P", "R", "F1", "n");
    for (idx, p, r, f1, n) in sorted.iter().take(10) {
        println!(
            "  {:<48} {:>6.2}% {:>6.2}% {:>6.2}% {:>7}",
            class_names[*idx],
            p * 100.0,
            r * 100.0,
            f1 * 100.0,
            n
        );
    }

    // 10 most frequent confusion pairs.
    let mut conf_pairs: Vec<((usize, usize), u32)> =
        confusion.into_iter().collect();
    conf_pairs.sort_by(|a, b| b.1.cmp(&a.1));
    println!("\n10 most frequent confusions (true -> predicted):");
    for ((t, p), n) in conf_pairs.iter().take(10) {
        println!(
            "  {:>4}× {:<36} -> {}",
            n, class_names[*t], class_names[*p]
        );
    }

    Ok(())
}
