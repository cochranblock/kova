//! Local LLM inference. Candle + GGUF. Streams tokens.
//! f76=chat_stream, f80=chat_complete.
//! Replaces Kalosm — direct candle for auditability + shared engine with pixel-forge.

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};
use std::thread;

#[cfg(feature = "inference")]
use std::num::NonZeroUsize;
#[cfg(feature = "inference")]
use std::sync::{LazyLock, Mutex};

#[cfg(feature = "inference")]
use candle_core::{quantized::gguf_file, Device, Tensor};
#[cfg(feature = "inference")]
use candle_transformers::models::quantized_llama::ModelWeights;
#[cfg(feature = "inference")]
use tokenizers::Tokenizer;

#[cfg(feature = "inference")]
struct CachedModel {
    model: ModelWeights,
    tokenizer: Tokenizer,
    device: Device,
}

#[cfg(feature = "inference")]
static MODEL_CACHE: LazyLock<Mutex<lru::LruCache<PathBuf, Arc<CachedModel>>>> =
    LazyLock::new(|| {
        let cap = NonZeroUsize::new(crate::config::model_cache_size()).unwrap_or(NonZeroUsize::MIN);
        Mutex::new(lru::LruCache::new(cap))
    });

#[cfg(feature = "inference")]
pub(crate) fn get_or_load_model(model_path: &Path) -> anyhow::Result<Arc<CachedModel>> {
    let path_buf = model_path.to_path_buf();

    {
        let mut cache = MODEL_CACHE.lock().unwrap();
        if let Some(cached) = cache.get(&path_buf) {
            return Ok(Arc::clone(cached));
        }
    }

    let device = Device::Cpu;
    let mut file = std::fs::File::open(model_path)?;
    let content = gguf_file::Content::read(&mut file)
        .map_err(|e| anyhow::anyhow!("GGUF read: {}", e))?;
    let model = ModelWeights::from_gguf(content, &mut file, &device)
        .map_err(|e| anyhow::anyhow!("model load: {}", e))?;

    // Tokenizer: look for tokenizer.json next to model, or in ~/.kova/models/
    let tok_path = model_path
        .parent()
        .map(|p| p.join("tokenizer.json"))
        .filter(|p| p.exists())
        .unwrap_or_else(|| crate::models_dir().join("tokenizer.json"));

    let tokenizer = Tokenizer::from_file(&tok_path)
        .map_err(|e| anyhow::anyhow!("tokenizer load from {}: {}", tok_path.display(), e))?;

    let cached = Arc::new(CachedModel {
        model,
        tokenizer,
        device,
    });
    {
        let mut cache = MODEL_CACHE.lock().unwrap();
        cache.put(path_buf, Arc::clone(&cached));
    }
    Ok(cached)
}

/// f76=chat_stream. Spawn inference in a thread. Returns receiver for streamed tokens.
pub fn f76(
    model_path: &Path,
    system_prompt: &str,
    _history: &[(String, String)],
    user_input: &str,
) -> mpsc::Receiver<Arc<str>> {
    let (tx, rx) = mpsc::channel();
    let path = model_path.to_path_buf();
    let system = system_prompt.to_string();
    let input = user_input.to_string();

    thread::spawn(move || {
        if let Err(e) = run_inference(&path, &system, &input, &tx) {
            let _ = tx.send(Arc::from(format!("Error: {}", e)));
        }
    });

    rx
}

#[cfg(feature = "inference")]
fn run_inference(
    model_path: &Path,
    system_prompt: &str,
    user_input: &str,
    tx: &mpsc::Sender<Arc<str>>,
) -> anyhow::Result<()> {
    use candle_transformers::generation::LogitsProcessor;

    let cached = get_or_load_model(model_path)?;

    let prompt = format!(
        "<|im_start|>system\n{}<|im_end|>\n<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n",
        system_prompt, user_input
    );

    let tokens = cached
        .tokenizer
        .encode(prompt.as_str(), true)
        .map_err(|e| anyhow::anyhow!("tokenize: {}", e))?;
    let input_ids = tokens.get_ids();

    let mut logits_proc = LogitsProcessor::new(42, Some(0.7), Some(0.9));
    let mut all_tokens: Vec<u32> = input_ids.to_vec();
    let eos = cached
        .tokenizer
        .token_to_id("<|im_end|>")
        .unwrap_or(151643);

    let mut model = cached.model.clone();
    let max_tokens: usize = 2048;

    // Process prompt
    let input_tensor =
        Tensor::new(input_ids, &cached.device)?.unsqueeze(0)?;
    let logits = model.forward(&input_tensor, 0)?;
    let logits = logits.squeeze(0)?.to_dtype(candle_core::DType::F32)?;
    let next_token = logits_proc.sample(&logits)?;
    all_tokens.push(next_token);

    if next_token != eos {
        if let Some(text) = decode_token(&cached.tokenizer, next_token) {
            let _ = tx.send(Arc::from(text));
        }
    }

    // Generate tokens
    for i in 0..max_tokens {
        let input = Tensor::new(&[next_token], &cached.device)?.unsqueeze(0)?;
        let logits = model.forward(&input, input_ids.len() + i + 1)?;
        let logits = logits.squeeze(0)?.to_dtype(candle_core::DType::F32)?;
        let next_token = logits_proc.sample(&logits)?;
        all_tokens.push(next_token);

        if next_token == eos {
            break;
        }
        if let Some(text) = decode_token(&cached.tokenizer, next_token) {
            let _ = tx.send(Arc::from(text));
        }
    }

    Ok(())
}

#[cfg(not(feature = "inference"))]
fn run_inference(
    _model_path: &Path,
    _system_prompt: &str,
    _user_input: &str,
    tx: &mpsc::Sender<Arc<str>>,
) -> anyhow::Result<()> {
    let _ = tx.send(Arc::from("Error: build with --features inference"));
    Ok(())
}

#[cfg(feature = "inference")]
fn decode_token(tokenizer: &Tokenizer, token: u32) -> Option<String> {
    tokenizer.decode(&[token], true).ok()
}

/// f80=chat_complete. Run inference, return full response. No streaming.
pub fn f80(
    model_path: &Path,
    system_prompt: &str,
    user_input: &str,
) -> anyhow::Result<String> {
    let rx = f76(model_path, system_prompt, &[], user_input);
    let mut out = String::new();
    for token in rx {
        out.push_str(&token);
    }
    Ok(out)
}

/// f80_code_gen_structured. Run inference, extract code block from response.
#[cfg(feature = "inference")]
pub fn f80_code_gen_structured(
    model_path: &Path,
    system_prompt: &str,
    user_input: &str,
) -> anyhow::Result<String> {
    let full_system = format!(
        "{}\n\nGenerate Rust code only. Wrap in ```rust ... ``` fences.",
        system_prompt
    );
    let response = f80(model_path, &full_system, user_input)?;

    // Extract code between ```rust and ```
    if let Some(start) = response.find("```rust") {
        let code_start = start + 7;
        if let Some(end) = response[code_start..].find("```") {
            return Ok(response[code_start..code_start + end].trim().to_string());
        }
    }
    // Fallback: return raw response
    Ok(response)
}
