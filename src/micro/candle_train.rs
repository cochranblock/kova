//! candle_train — Pure Rust fine-tuning via candle. No Python, no MLX.
//! Trains kova specialist models from tournament DPO/SFT data.
//! Output: safetensors models loadable by mobile_llm and MoE pipeline.
//!
//! Training happens on IRONHIVE nodes (bt/st) or Mac Mini.
//! Models deploy to ~/.kova/models/{specialist_name}/ as safetensors.

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use candle_core::{DType, Device, Tensor};
use candle_nn::{Optimizer, VarBuilder, VarMap};
use candle_transformers::models::qwen2::{Config as QwenConfig, ModelForCausalLM};
use std::path::{Path, PathBuf};
use tokenizers::Tokenizer;

/// Training configuration.
pub struct TrainConfig {
    /// Base model directory (safetensors + tokenizer.json + config.json).
    pub base_model: PathBuf,
    /// Training data (JSONL — SFT or DPO ChatML format).
    pub data_path: PathBuf,
    /// Output directory for fine-tuned model.
    pub output_dir: PathBuf,
    /// Specialist name (e.g. "kova-rustfix", "kova-tokenizer").
    pub name: String,
    /// Number of training epochs.
    pub epochs: u32,
    /// Learning rate.
    pub lr: f64,
    /// Max sequence length for training.
    pub max_seq_len: usize,
    /// Batch size (1 for small models on limited RAM).
    pub batch_size: usize,
}

impl Default for TrainConfig {
    fn default() -> Self {
        Self {
            base_model: PathBuf::new(),
            data_path: PathBuf::new(),
            output_dir: PathBuf::new(),
            name: "kova-specialist".into(),
            epochs: 3,
            lr: 1e-5,
            max_seq_len: 512,
            batch_size: 1,
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

/// Load SFT examples from ChatML JSONL file.
fn load_sft_data(path: &Path) -> Result<Vec<SftExample>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("read training data: {}", e))?;
    let mut examples = Vec::new();
    for line in content.lines() {
        if line.trim().is_empty() { continue; }
        let ex: SftExample = serde_json::from_str(line)
            .map_err(|e| format!("parse JSONL line: {}", e))?;
        examples.push(ex);
    }
    Ok(examples)
}

/// Format SFT example as ChatML string for tokenization.
fn format_chatml(ex: &SftExample) -> String {
    let mut out = String::new();
    for msg in &ex.messages {
        out.push_str(&format!("<|im_start|>{}\n{}<|im_end|>\n", msg.role, msg.content));
    }
    out
}

/// Tokenize a batch of strings, pad to max_len.
fn tokenize_batch(
    tokenizer: &Tokenizer,
    texts: &[String],
    max_len: usize,
    device: &Device,
) -> Result<(Tensor, Tensor), String> {
    let pad_id = tokenizer.token_to_id("<|endoftext|>").unwrap_or(0);
    let mut all_ids = Vec::new();
    let mut all_labels = Vec::new();

    for text in texts {
        let encoding = tokenizer.encode(text.as_str(), true)
            .map_err(|e| format!("tokenize: {}", e))?;
        let mut ids: Vec<u32> = encoding.get_ids().to_vec();
        ids.truncate(max_len);

        // Labels = shifted input (next-token prediction)
        let mut labels = ids[1..].to_vec();
        labels.push(pad_id);

        // Pad
        while ids.len() < max_len {
            ids.push(pad_id);
            labels.push(pad_id); // pad labels too
        }

        all_ids.extend_from_slice(&ids);
        all_labels.extend_from_slice(&labels);
    }

    let batch_size = texts.len();
    let input = Tensor::from_vec(all_ids, (batch_size, max_len), device)
        .map_err(|e| format!("input tensor: {}", e))?;
    let labels = Tensor::from_vec(all_labels, (batch_size, max_len), device)
        .map_err(|e| format!("labels tensor: {}", e))?;

    Ok((input, labels))
}

/// Cross-entropy loss for language modeling.
fn cross_entropy_loss(logits: &Tensor, labels: &Tensor) -> Result<Tensor, String> {
    let (batch, seq_len, vocab) = logits.dims3()
        .map_err(|e| format!("logits dims: {}", e))?;
    let logits_flat = logits.reshape((batch * seq_len, vocab))
        .map_err(|e| format!("reshape logits: {}", e))?;
    let labels_flat = labels.reshape(batch * seq_len)
        .map_err(|e| format!("reshape labels: {}", e))?;
    candle_nn::loss::cross_entropy(&logits_flat, &labels_flat)
        .map_err(|e| format!("cross_entropy: {}", e))
}

/// Train a specialist model from SFT data.
/// Loads base model weights, fine-tunes on training data, saves as safetensors.
pub fn train_sft(config: &TrainConfig) -> Result<PathBuf, String> {
    eprintln!("[train] specialist: {}", config.name);
    eprintln!("[train] base model: {}", config.base_model.display());
    eprintln!("[train] data: {}", config.data_path.display());
    eprintln!("[train] epochs: {}, lr: {}, max_seq: {}", config.epochs, config.lr, config.max_seq_len);

    let device = Device::Cpu;

    // Load training data
    let examples = load_sft_data(&config.data_path)?;
    if examples.is_empty() {
        return Err("no training examples found".into());
    }
    eprintln!("[train] loaded {} examples", examples.len());

    // Load model config
    let config_path = config.base_model.join("config.json");
    let model_config: QwenConfig = if config_path.exists() {
        let json = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("read config: {}", e))?;
        serde_json::from_str(&json).map_err(|e| format!("parse config: {}", e))?
    } else {
        crate::mobile_llm::qwen_05b_config()
    };

    // Load tokenizer
    let tok_path = config.base_model.join("tokenizer.json");
    let tokenizer = Tokenizer::from_file(&tok_path)
        .map_err(|e| format!("load tokenizer: {}", e))?;

    // Load weights into VarMap (trainable)
    let varmap = VarMap::new();
    let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);

    // Find safetensors files
    let st_files: Vec<PathBuf> = std::fs::read_dir(&config.base_model)
        .map_err(|e| format!("read model dir: {}", e))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("safetensors"))
        .collect();

    if st_files.is_empty() {
        return Err("no safetensors files in base model dir".into());
    }

    // Load pre-trained weights into varmap
    for st_file in &st_files {
        varmap.load(st_file)
            .map_err(|e| format!("load weights from {}: {}", st_file.display(), e))?;
    }

    eprintln!("[train] loaded {} weight files", st_files.len());

    // Build model from varmap
    let mut model = ModelForCausalLM::new(&model_config, vb)
        .map_err(|e| format!("build model: {}", e))?;

    // Setup optimizer (AdamW)
    let params = varmap.all_vars();
    let mut optimizer = candle_nn::AdamW::new(
        params,
        candle_nn::ParamsAdamW {
            lr: config.lr,
            weight_decay: 0.01,
            ..Default::default()
        },
    ).map_err(|e| format!("optimizer: {}", e))?;

    // Format all examples
    let formatted: Vec<String> = examples.iter().map(|ex| format_chatml(ex)).collect();

    // Training loop
    for epoch in 0..config.epochs {
        let mut total_loss = 0.0f64;
        let mut n_batches = 0;

        for batch_start in (0..formatted.len()).step_by(config.batch_size) {
            let batch_end = (batch_start + config.batch_size).min(formatted.len());
            let batch_texts: Vec<String> = formatted[batch_start..batch_end].to_vec();

            let (input_ids, labels) = tokenize_batch(
                &tokenizer,
                &batch_texts,
                config.max_seq_len,
                &device,
            )?;

            // Forward pass
            let logits = model.forward(&input_ids, 0)
                .map_err(|e| format!("forward: {}", e))?;

            // Loss
            let loss = cross_entropy_loss(&logits, &labels)?;
            let loss_val: f64 = loss.to_scalar()
                .map_err(|e| format!("loss scalar: {}", e))?;

            // Backward + step
            optimizer.backward_step(&loss)
                .map_err(|e| format!("backward: {}", e))?;

            total_loss += loss_val;
            n_batches += 1;
        }

        let avg_loss = if n_batches > 0 { total_loss / n_batches as f64 } else { 0.0 };
        eprintln!("[train] epoch {}/{}: loss={:.4}", epoch + 1, config.epochs, avg_loss);
    }

    // Save fine-tuned model
    let out_dir = config.output_dir.join(&config.name);
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("create output: {}", e))?;

    let out_path = out_dir.join("model.safetensors");
    varmap.save(&out_path).map_err(|e| format!("save model: {}", e))?;

    // Copy tokenizer + config to output
    let _ = std::fs::copy(
        config.base_model.join("tokenizer.json"),
        out_dir.join("tokenizer.json"),
    );
    let _ = std::fs::copy(
        config.base_model.join("config.json"),
        out_dir.join("config.json"),
    );

    eprintln!("[train] saved to {}", out_dir.display());
    eprintln!("[train] model: {}", out_path.display());

    Ok(out_dir)
}

/// Train all four kova specialists from exported tournament data.
pub fn train_all_specialists(
    base_model: &Path,
    training_dir: &Path,
    output_dir: &Path,
) -> Result<Vec<PathBuf>, String> {
    let specialists = [
        ("kova-rustfix", "dpo_chatml.jsonl"),
        ("kova-tokenizer", "sft_chatml.jsonl"),
        ("kova-architect", "sft_chatml.jsonl"),
        ("kova-reviewer", "dpo_chatml.jsonl"),
    ];

    let mut outputs = Vec::new();

    for (name, data_file) in &specialists {
        let data_path = training_dir.join(data_file);
        if !data_path.exists() {
            eprintln!("[train] skipping {} — no data at {}", name, data_path.display());
            continue;
        }

        let config = TrainConfig {
            base_model: base_model.to_path_buf(),
            data_path,
            output_dir: output_dir.to_path_buf(),
            name: name.to_string(),
            epochs: 3,
            lr: 1e-5,
            max_seq_len: 512,
            batch_size: 1,
        };

        match train_sft(&config) {
            Ok(path) => outputs.push(path),
            Err(e) => eprintln!("[train] {} failed: {}", name, e),
        }
    }

    Ok(outputs)
}
