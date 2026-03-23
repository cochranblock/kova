// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! candle_train — Train kova's own models from scratch. Pure Rust, candle.
//! No pretrained weights. No Python. No HuggingFace dependency.
//!
//! Pixel forge pattern: define architecture, train on our data, deploy safetensors.
//! Models: Spark (50K), Flame (500K), Blaze (2M).
//!
//! Training data: tournament SFT/DPO exports from ~/.kova/micro/training/
//! Output: ~/.kova/models/kova-{tier}/ as safetensors.

use candle_core::{DType, Device, Tensor};
use candle_nn::{Optimizer, VarBuilder, VarMap};
use std::path::{Path, PathBuf};

use super::kova_model::{self, KovaClassifier, KovaTokenizer, Tier, CLASS_LABELS, NUM_CLASSES};

/// Training configuration.
pub struct TrainConfig {
    /// Model tier (Spark, Flame, Blaze).
    pub tier: Tier,
    /// Training data (JSONL — SFT ChatML format).
    pub data_path: PathBuf,
    /// Output directory for trained model.
    pub output_dir: PathBuf,
    /// Number of training epochs.
    pub epochs: u32,
    /// Learning rate.
    pub lr: f64,
    /// Batch size.
    pub batch_size: usize,
}

impl Default for TrainConfig {
    fn default() -> Self {
        Self {
            tier: Tier::Spark,
            data_path: PathBuf::new(),
            output_dir: PathBuf::new(),
            epochs: 10,
            lr: 3e-4,
            batch_size: 16,
        }
    }
}

/// SFT training example (loaded from JSONL).
#[derive(serde::Deserialize)]
struct SftExample {
    messages: Vec<ChatMsg>,
}

#[derive(serde::Deserialize)]
struct ChatMsg {
    role: String,
    content: String,
}

/// DPO example (prompt + chosen/rejected message lists).
#[derive(serde::Deserialize)]
struct DpoExample {
    prompt: Vec<ChatMsg>,
    chosen: Vec<ChatMsg>,
    #[allow(dead_code)]
    rejected: Vec<ChatMsg>,
}

/// Parsed training sample: input text + class label.
struct Sample {
    text: String,
    label: usize,
}

/// Load and parse training data into (text, label) pairs.
fn load_samples(path: &Path) -> Result<Vec<Sample>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("read training data: {}", e))?;
    let mut samples = Vec::new();

    for line in content.lines() {
        if line.trim().is_empty() { continue; }

        // Try SFT format
        if let Ok(ex) = serde_json::from_str::<SftExample>(line) {
            if let Some(sample) = extract_sample_sft(&ex) {
                samples.push(sample);
                continue;
            }
        }
        // Try DPO format — use prompt + chosen
        if let Ok(dpo) = serde_json::from_str::<DpoExample>(line) {
            let mut messages = dpo.prompt;
            messages.extend(dpo.chosen);
            let ex = SftExample { messages };
            if let Some(sample) = extract_sample_sft(&ex) {
                samples.push(sample);
                continue;
            }
        }
    }
    Ok(samples)
}

/// Extract (input_text, class_label) from an SFT example.
/// The user message is the input, the assistant message is the class label.
fn extract_sample_sft(ex: &SftExample) -> Option<Sample> {
    let user_msg = ex.messages.iter().find(|m| m.role == "user")?;
    let asst_msg = ex.messages.iter().find(|m| m.role == "assistant")?;

    let label_str = asst_msg.content.trim().to_lowercase();
    let label = CLASS_LABELS.iter().position(|&l| label_str == l || label_str.starts_with(l))?;

    Some(Sample {
        text: user_msg.content.clone(),
        label,
    })
}

/// Train a kova model from scratch.
pub fn train(config: &TrainConfig) -> Result<PathBuf, String> {
    let tier = config.tier;
    let model_cfg = tier.config();
    let params = kova_model::count_params(&model_cfg);

    eprintln!("[train] tier: {} ({} params)", tier.name(), params);
    eprintln!("[train] data: {}", config.data_path.display());
    eprintln!("[train] epochs: {}, lr: {}, batch: {}", config.epochs, config.lr, config.batch_size);

    let device = Device::Cpu;

    // Load training data
    let samples = load_samples(&config.data_path)?;
    if samples.is_empty() {
        return Err("no training samples found".into());
    }
    eprintln!("[train] {} samples, {} classes", samples.len(), NUM_CLASSES);

    // Print class distribution
    let mut dist = vec![0usize; NUM_CLASSES];
    for s in &samples { dist[s.label] += 1; }
    for (i, count) in dist.iter().enumerate() {
        if *count > 0 {
            eprintln!("  {}: {}", CLASS_LABELS[i], count);
        }
    }

    // Train BPE tokenizer on the training data
    let texts: Vec<String> = samples.iter().map(|s| s.text.clone()).collect();
    let max_merges = model_cfg.vocab_size.saturating_sub(257);
    eprintln!("[train] training BPE tokenizer ({} merges from {} texts)...", max_merges, texts.len());
    let tokenizer = KovaTokenizer::train(&texts, max_merges);
    eprintln!("[train] tokenizer vocab: {}", tokenizer.vocab_size());

    // Update model config to match actual vocab
    let mut model_cfg = model_cfg;
    model_cfg.vocab_size = tokenizer.vocab_size();

    // Build model (random init)
    let varmap = VarMap::new();
    let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
    let model = KovaClassifier::new(&model_cfg, vb)
        .map_err(|e| format!("build model: {}", e))?;

    // Optimizer
    let all_vars = varmap.all_vars();
    let mut optimizer = candle_nn::AdamW::new(
        all_vars,
        candle_nn::ParamsAdamW {
            lr: config.lr,
            weight_decay: 0.01,
            ..Default::default()
        },
    ).map_err(|e| format!("optimizer: {}", e))?;

    // Tokenize all samples
    let max_len = model_cfg.max_seq_len;
    let all_ids: Vec<Vec<u32>> = samples.iter()
        .map(|s| tokenizer.encode(&s.text, max_len))
        .collect();
    let all_labels: Vec<u32> = samples.iter().map(|s| s.label as u32).collect();

    // Training loop
    for epoch in 0..config.epochs {
        let mut total_loss = 0.0f64;
        let mut correct = 0usize;
        let mut total = 0usize;

        for batch_start in (0..samples.len()).step_by(config.batch_size) {
            let batch_end = (batch_start + config.batch_size).min(samples.len());
            let bs = batch_end - batch_start;

            // Build input tensor [batch, seq_len]
            let mut flat_ids = Vec::with_capacity(bs * max_len);
            for i in batch_start..batch_end {
                flat_ids.extend_from_slice(&all_ids[i]);
            }
            let input = Tensor::from_vec(flat_ids, (bs, max_len), &device)
                .map_err(|e| format!("input tensor: {}", e))?;

            // Build label tensor [batch]
            let labels = Tensor::from_vec(
                all_labels[batch_start..batch_end].to_vec(),
                (bs,),
                &device,
            ).map_err(|e| format!("label tensor: {}", e))?;

            // Forward
            let logits = model.forward(&input)
                .map_err(|e| format!("forward: {}", e))?;

            // Cross-entropy loss
            let loss = candle_nn::loss::cross_entropy(&logits, &labels)
                .map_err(|e| format!("loss: {}", e))?;
            let loss_val: f64 = loss.to_dtype(DType::F64)
                .and_then(|t| t.to_scalar())
                .map_err(|e| format!("loss scalar: {}", e))?;

            // Accuracy
            let preds: Vec<u32> = logits.argmax(1)
                .map_err(|e| format!("argmax: {}", e))?
                .to_vec1()
                .map_err(|e| format!("to_vec: {}", e))?;
            for (i, &pred) in preds.iter().enumerate() {
                if pred == all_labels[batch_start + i] {
                    correct += 1;
                }
                total += 1;
            }

            // Backward + step
            optimizer.backward_step(&loss)
                .map_err(|e| format!("backward: {}", e))?;

            total_loss += loss_val * bs as f64;
        }

        let avg_loss = total_loss / samples.len() as f64;
        let acc = if total > 0 { correct as f64 / total as f64 * 100.0 } else { 0.0 };
        eprintln!("[train] epoch {}/{}: loss={:.4} acc={:.1}%", epoch + 1, config.epochs, avg_loss, acc);
    }

    // Save model
    let out_dir = config.output_dir.join(format!("kova-{}", tier.name()));
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("create output: {}", e))?;

    let model_path = out_dir.join("model.safetensors");
    varmap.save(&model_path).map_err(|e| format!("save: {}", e))?;

    // Save tokenizer
    tokenizer.save(&out_dir.join("tokenizer.json"))
        .map_err(|e| format!("save tokenizer: {}", e))?;

    // Save config
    let config_json = serde_json::json!({
        "tier": tier.name(),
        "params": params,
        "vocab_size": model_cfg.vocab_size,
        "embed_dim": model_cfg.embed_dim,
        "num_heads": model_cfg.num_heads,
        "num_layers": model_cfg.num_layers,
        "ff_dim": model_cfg.ff_dim,
        "max_seq_len": model_cfg.max_seq_len,
        "classes": CLASS_LABELS,
    });
    std::fs::write(
        out_dir.join("config.json"),
        serde_json::to_string_pretty(&config_json).unwrap(),
    ).map_err(|e| format!("save config: {}", e))?;

    eprintln!("[train] saved {} to {}", tier.name(), out_dir.display());
    eprintln!("[train] {} params, {:.1} KB", params, model_path.metadata().map(|m| m.len()).unwrap_or(0) as f64 / 1024.0);

    Ok(out_dir)
}

/// Train all three tiers.
pub fn train_all_tiers(
    training_dir: &Path,
    output_dir: &Path,
) -> Result<Vec<PathBuf>, String> {
    let data_path = training_dir.join("sft_chatml.jsonl");
    if !data_path.exists() {
        return Err(format!("no SFT data at {}", data_path.display()));
    }

    let mut outputs = Vec::new();

    for tier in [Tier::Spark, Tier::Flame, Tier::Blaze] {
        let config = TrainConfig {
            tier,
            data_path: data_path.clone(),
            output_dir: output_dir.to_path_buf(),
            epochs: match tier {
                Tier::Spark => 20,
                Tier::Flame => 15,
                Tier::Blaze => 10,
            },
            lr: 3e-4,
            batch_size: match tier {
                Tier::Spark => 32,
                Tier::Flame => 16,
                Tier::Blaze => 8,
            },
        };

        match train(&config) {
            Ok(path) => outputs.push(path),
            Err(e) => eprintln!("[train] {} failed: {}", tier.name(), e),
        }
    }

    Ok(outputs)
}

/// Training data directory.
pub fn training_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".kova").join("micro").join("training")
}
