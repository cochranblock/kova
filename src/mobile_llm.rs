//! Mobile LLM inference via llama.cpp. On-device GGUF, no kalosm.
//! Works on Android (aarch64-linux-android) and desktop for testing.
//! Feature: mobile-llm.

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::sampling::LlamaSampler;
use llama_cpp_2::token::data_array::LlamaTokenDataArray;
use std::num::NonZeroU32;
use std::path::Path;
use std::sync::mpsc;

/// Generate text from a GGUF model. Blocking. Returns full response.
pub fn generate(model_path: &Path, system: &str, prompt: &str) -> Result<String, String> {
    let backend = LlamaBackend::init().map_err(|e| format!("backend: {}", e))?;

    let model_params = LlamaModelParams::default();
    let model = LlamaModel::load_from_file(&backend, model_path, &model_params)
        .map_err(|e| format!("load model: {}", e))?;

    let ctx_params = LlamaContextParams::default()
        .with_n_ctx(NonZeroU32::new(4096));

    let mut ctx = model
        .new_context(&backend, ctx_params)
        .map_err(|e| format!("context: {}", e))?;

    // Format as ChatML template (Qwen2.5-Coder uses this)
    let input = format!(
        "<|im_start|>system\n{}<|im_end|>\n<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n",
        system, prompt
    );

    // Tokenize
    let tokens = model
        .str_to_token(&input, llama_cpp_2::model::AddBos::Always)
        .map_err(|e| format!("tokenize: {}", e))?;

    // Create batch and evaluate prompt tokens
    let mut batch = LlamaBatch::new(4096, 1);
    for (i, &token) in tokens.iter().enumerate() {
        let is_last = i == tokens.len() - 1;
        batch.add(token, i as i32, &[0], is_last)
            .map_err(|e| format!("batch add: {}", e))?;
    }

    ctx.decode(&mut batch).map_err(|e| format!("decode prompt: {}", e))?;

    // Build sampler chain: temp → top_k → top_p → dist
    let sampler = LlamaSampler::chain_simple(vec![
        LlamaSampler::temp(0.2),
        LlamaSampler::top_k(40),
        LlamaSampler::top_p(0.9, 1),
        LlamaSampler::dist(42),
    ]);

    // Generate tokens
    let mut output = String::new();
    let mut n_cur = tokens.len() as i32;
    let max_tokens = 2048;
    let mut decoder = encoding_rs::UTF_8.new_decoder();

    for _ in 0..max_tokens {
        let candidates = ctx.candidates_ith(batch.n_tokens() - 1);
        let mut candidates_data = LlamaTokenDataArray::from_iter(candidates, false);

        candidates_data.apply_sampler(&sampler);
        let new_token = candidates_data.selected_token()
            .ok_or("failed to select token")?;

        // Check end of generation
        if model.is_eog_token(new_token) {
            break;
        }

        let piece = model.token_to_piece(new_token, &mut decoder, true, None)
            .map_err(|e| format!("detokenize: {}", e))?;

        output.push_str(&piece);

        // Stop on end-of-turn markers
        if output.ends_with("<|im_end|>") {
            output.truncate(output.len() - "<|im_end|>".len());
            break;
        }

        // Prepare next batch
        batch.clear();
        batch.add(new_token, n_cur, &[0], true)
            .map_err(|e| format!("batch add gen: {}", e))?;
        n_cur += 1;

        ctx.decode(&mut batch).map_err(|e| format!("decode gen: {}", e))?;
    }

    Ok(output.trim().to_string())
}

/// Streaming generate. Returns receiver for token chunks.
#[allow(dead_code)]
pub fn generate_stream(
    model_path: &Path,
    system: &str,
    prompt: &str,
) -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel();
    let model_path = model_path.to_path_buf();
    let system = system.to_string();
    let prompt = prompt.to_string();

    std::thread::spawn(move || {
        match generate(&model_path, &system, &prompt) {
            Ok(response) => {
                for chunk in response.as_bytes().chunks(20) {
                    let s = String::from_utf8_lossy(chunk).to_string();
                    if tx.send(s).is_err() {
                        break;
                    }
                }
            }
            Err(e) => {
                let _ = tx.send(format!("Error: {}", e));
            }
        }
    });

    rx
}

/// Find bundled GGUF model. Checks:
/// 1. KOVA_MODEL env var
/// 2. ~/.kova/models/*.gguf (first match)
pub fn find_model() -> Option<std::path::PathBuf> {
    if let Ok(p) = std::env::var("KOVA_MODEL") {
        let path = std::path::PathBuf::from(p);
        if path.exists() {
            return Some(path);
        }
    }

    let models_dir = crate::config::models_dir();
    if models_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&models_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("gguf") {
                    return Some(path);
                }
            }
        }
    }

    None
}
