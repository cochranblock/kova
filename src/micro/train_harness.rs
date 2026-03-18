// Unlicense — cochranblock.org
//! train_harness — Wraps mlx_lm.lora for kova micro training.
//!
//! Does NOT touch train.rs or tournament.rs. Uses exported data from
//! ~/.kova/micro/training/ (dpo_chatml.jsonl, sft_chatml.jsonl).
//!
//! Prereqs: pip install "mlx-lm[train]"
//! Data: kova micro export --format all (then tournament must have run)

use std::process::Command;

const DEFAULT_MODEL: &str = "mlx-community/Qwen2.5-Coder-0.5B-Instruct-4bit";
const DEFAULT_ITERS: u32 = 600;

/// T172=TrainFormat
/// Training format: SFT (supervised) or DPO (preference pairs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum T172 {
    Sft,
    Dpo,
}

/// f262=run_train
/// Run mlx_lm.lora fine-tuning.
///
/// Expects ~/.kova/micro/training/{sft,dpo}_chatml.jsonl.
/// Copies to train.jsonl (mlx_lm expects that name) then runs:
///   mlx_lm.lora --model <model> --train --data <dir> --adapter-path <out> --iters N --mask-prompt
pub fn f262(
    format: T172,
    iters: Option<u32>,
    dry_run: bool,
) -> Result<(), String> {
    let training_dir = super::train::f261();
    let adapters_dir = training_dir.join("adapters");

    let source = match format {
        T172::Sft => training_dir.join("sft_chatml.jsonl"),
        T172::Dpo => training_dir.join("dpo_chatml.jsonl"),
    };

    if !source.exists() {
        return Err(format!(
            "training data not found: {}\n  Run: kova micro export --format {}",
            source.display(),
            match format {
                T172::Sft => "sft",
                T172::Dpo => "dpo",
            }
        ));
    }

    // mlx_lm expects train.jsonl in the data dir
    let train_jsonl = training_dir.join("train.jsonl");
    std::fs::copy(&source, &train_jsonl).map_err(|e| format!("copy {} -> train.jsonl: {}", source.display(), e))?;

    std::fs::create_dir_all(&adapters_dir).map_err(|e| format!("create adapters dir: {}", e))?;

    let iters = iters.unwrap_or(DEFAULT_ITERS);

    let mut cmd = Command::new("python");
    cmd.args([
        "-m", "mlx_lm.lora",
        "--model", DEFAULT_MODEL,
        "--train",
        "--data", training_dir.to_str().unwrap(),
        "--adapter-path", adapters_dir.to_str().unwrap(),
        "--iters", &iters.to_string(),
        "--mask-prompt",
    ]);

    if dry_run {
        eprintln!("[dry-run] would run: {:?}", cmd);
        return Ok(());
    }

    eprintln!("[micro train] {} format, {} iters", match format { T172::Sft => "SFT", T172::Dpo => "DPO" }, iters);
    eprintln!("  data: {}", training_dir.display());
    eprintln!("  adapters: {}", adapters_dir.display());

    let status = cmd.status().map_err(|e| format!("run mlx_lm.lora: {}", e))?;
    if !status.success() {
        return Err(format!("mlx_lm.lora exited with {}", status));
    }

    eprintln!("[micro train] adapters saved to {}", adapters_dir.display());
    Ok(())
}
