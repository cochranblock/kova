//! Mobile LLM inference via candle + safetensors. Pure Rust, no C deps.
//! Works on Android, iOS, desktop. Loads Qwen2.5-Coder or custom kova specialists.
//! Feature: mobile-llm.

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::qwen2::{Config as QwenConfig, ModelForCausalLM};
use std::path::Path;
use std::sync::mpsc;
use tokenizers::Tokenizer;

/// Model + tokenizer loaded and ready for inference.
struct LoadedModel {
    model: ModelForCausalLM,
    tokenizer: Tokenizer,
    device: Device,
    config: QwenConfig,
}

/// Load Qwen2 model from safetensors + tokenizer.json in the same directory.
fn load_model(model_dir: &Path) -> Result<LoadedModel, String> {
    let device = Device::Cpu; // Mobile: CPU. Desktop with Metal: Device::new_metal(0)

    // Find safetensors files
    let st_files: Vec<std::path::PathBuf> = std::fs::read_dir(model_dir)
        .map_err(|e| format!("read dir: {}", e))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("safetensors"))
        .collect();

    if st_files.is_empty() {
        return Err(format!("No .safetensors files in {}", model_dir.display()));
    }

    // Load config.json
    let config_path = model_dir.join("config.json");
    let config: QwenConfig = if config_path.exists() {
        let json = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("read config: {}", e))?;
        serde_json::from_str(&json).map_err(|e| format!("parse config: {}", e))?
    } else {
        // Default Qwen2.5-Coder-0.5B config
        qwen_05b_config()
    };

    // Load weights
    let vb = unsafe {
        VarBuilder::from_mmaped_safetensors(&st_files, DType::F32, &device)
            .map_err(|e| format!("load weights: {}", e))?
    };

    let model = ModelForCausalLM::new(&config, vb)
        .map_err(|e| format!("build model: {}", e))?;

    // Load tokenizer
    let tok_path = model_dir.join("tokenizer.json");
    let tokenizer = Tokenizer::from_file(&tok_path)
        .map_err(|e| format!("load tokenizer: {}", e))?;

    Ok(LoadedModel { model, tokenizer, device, config })
}

/// Default config for Qwen2.5-Coder-0.5B-Instruct.
pub fn qwen_05b_config() -> QwenConfig {
    QwenConfig {
        vocab_size: 151936,
        hidden_size: 896,
        intermediate_size: 4864,
        num_hidden_layers: 24,
        num_attention_heads: 14,
        num_key_value_heads: 2,
        max_position_embeddings: 32768,
        sliding_window: 32768,
        rope_theta: 1000000.0,
        rms_norm_eps: 1e-6,
        tie_word_embeddings: true,
        use_sliding_window: false,
        hidden_act: candle_nn::Activation::Silu,
        max_window_layers: 0,
    }
}

/// Top-p (nucleus) sampling from logits tensor.
fn sample_top_p(logits: &Tensor, temp: f64, top_p: f64) -> Result<u32, String> {
    let logits = logits.to_dtype(DType::F32).map_err(|e| format!("dtype: {}", e))?;
    let logits = (&logits / temp).map_err(|e| format!("temp: {}", e))?;

    // Softmax
    let probs = candle_nn::ops::softmax_last_dim(&logits)
        .map_err(|e| format!("softmax: {}", e))?;
    let probs_vec: Vec<f32> = probs.to_vec1().map_err(|e| format!("to_vec: {}", e))?;

    // Sort by probability descending
    let mut indexed: Vec<(usize, f32)> = probs_vec.iter().copied().enumerate().collect();
    indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Accumulate until top_p threshold
    let mut cumsum = 0.0f32;
    let mut candidates = Vec::new();
    for (idx, prob) in &indexed {
        cumsum += prob;
        candidates.push((*idx, *prob));
        if cumsum as f64 >= top_p {
            break;
        }
    }

    // Weighted random selection
    let total: f32 = candidates.iter().map(|(_, p)| p).sum();
    let mut r = rand_f32() * total;
    for (idx, prob) in &candidates {
        r -= prob;
        if r <= 0.0 {
            return Ok(*idx as u32);
        }
    }

    // Fallback: highest prob
    Ok(candidates[0].0 as u32)
}

/// Simple RNG (no external crate needed).
fn rand_f32() -> f32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    // xorshift32
    let mut x = seed.wrapping_add(1);
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    (x as f32) / (u32::MAX as f32)
}

/// Generate text from a safetensors model directory. Blocking.
pub fn generate(model_dir: &Path, system: &str, prompt: &str) -> Result<String, String> {
    let mut loaded = load_model(model_dir)?;

    // Format as ChatML
    let input = format!(
        "<|im_start|>system\n{}<|im_end|>\n<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n",
        system, prompt
    );

    // Tokenize
    let encoding = loaded.tokenizer.encode(input.as_str(), true)
        .map_err(|e| format!("tokenize: {}", e))?;
    let token_ids: Vec<u32> = encoding.get_ids().to_vec();

    let mut tokens = token_ids.clone();
    let mut output = String::new();
    let max_tokens = 2048;

    // Get EOS token ID
    let eos_token = loaded.tokenizer.token_to_id("<|im_end|>").unwrap_or(151645);

    for i in 0..max_tokens {
        let input_tensor = Tensor::new(&tokens[..], &loaded.device)
            .map_err(|e| format!("tensor: {}", e))?
            .unsqueeze(0)
            .map_err(|e| format!("unsqueeze: {}", e))?;

        let logits = loaded.model.forward(&input_tensor, i)
            .map_err(|e| format!("forward: {}", e))?;

        // Get last token logits
        let seq_len = logits.dim(1).map_err(|e| format!("dim: {}", e))?;
        let last_logits = logits
            .narrow(1, seq_len - 1, 1)
            .map_err(|e| format!("narrow: {}", e))?
            .squeeze(0)
            .map_err(|e| format!("squeeze0: {}", e))?
            .squeeze(0)
            .map_err(|e| format!("squeeze1: {}", e))?;

        let next_token = sample_top_p(&last_logits, 0.2, 0.9)?;

        if next_token == eos_token {
            break;
        }

        // Decode token
        let piece = loaded.tokenizer.decode(&[next_token], false)
            .map_err(|e| format!("decode: {}", e))?;
        output.push_str(&piece);

        // Stop on im_end in output
        if output.contains("<|im_end|>") {
            output = output.replace("<|im_end|>", "");
            break;
        }

        tokens = vec![next_token];
    }

    Ok(output.trim().to_string())
}

/// Streaming generate. Returns receiver for token chunks.
#[allow(dead_code)]
pub fn generate_stream(
    model_dir: &Path,
    system: &str,
    prompt: &str,
) -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel();
    let model_dir = model_dir.to_path_buf();
    let system = system.to_string();
    let prompt = prompt.to_string();

    std::thread::spawn(move || {
        match generate(&model_dir, &system, &prompt) {
            Ok(response) => {
                for chunk in response.as_bytes().chunks(20) {
                    let s = String::from_utf8_lossy(chunk).to_string();
                    if tx.send(s).is_err() { break; }
                }
            }
            Err(e) => {
                let _ = tx.send(format!("Error: {}", e));
            }
        }
    });

    rx
}

/// Find bundled model directory. Checks:
/// 1. KOVA_MODEL_DIR env var
/// 2. ~/.kova/models/ (first dir containing .safetensors)
/// 3. Any .safetensors file in ~/.kova/models/ (treat parent as model dir)
pub fn find_model() -> Option<std::path::PathBuf> {
    if let Ok(p) = std::env::var("KOVA_MODEL_DIR") {
        let path = std::path::PathBuf::from(p);
        if path.is_dir() {
            return Some(path);
        }
    }

    let models_dir = crate::config::models_dir();
    if !models_dir.is_dir() {
        return None;
    }

    // Check subdirs first (e.g. ~/.kova/models/Qwen2.5-Coder-0.5B/)
    if let Ok(entries) = std::fs::read_dir(&models_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let has_st = std::fs::read_dir(&path)
                    .ok()
                    .map(|entries| {
                        entries.flatten().any(|e| {
                            e.path().extension().and_then(|x| x.to_str()) == Some("safetensors")
                        })
                    })
                    .unwrap_or(false);
                if has_st {
                    return Some(path);
                }
            }
        }
    }

    // Fallback: if .safetensors files are directly in models_dir
    if let Ok(entries) = std::fs::read_dir(&models_dir) {
        let has_st = entries.flatten().any(|e| {
            e.path().extension().and_then(|x| x.to_str()) == Some("safetensors")
        });
        if has_st {
            return Some(models_dir);
        }
    }

    None
}
