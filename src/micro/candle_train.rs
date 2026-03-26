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

/// Load samples from multiple JSONL files, deduplicating by text.
fn load_all_samples(paths: &[&Path]) -> Result<Vec<Sample>, String> {
    let mut all = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for path in paths {
        if !path.exists() { continue; }
        let samples = load_samples(path)?;
        for s in samples {
            if seen.insert(s.text.clone()) {
                all.push(s);
            }
        }
    }
    Ok(all)
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

    // Load training data from all sources (sft_chatml + classifier_sft)
    let classifier_path = config.data_path.parent()
        .map(|p| p.join("classifier_sft.jsonl"))
        .unwrap_or_default();
    let sources: Vec<&Path> = vec![config.data_path.as_path(), classifier_path.as_path()]
        .into_iter()
        .filter(|p| p.exists())
        .collect();
    let samples = load_all_samples(&sources)?;
    if samples.is_empty() {
        return Err("no training samples found".into());
    }

    // Print class distribution
    let mut dist = vec![0usize; NUM_CLASSES];
    for s in &samples { dist[s.label] += 1; }
    for (i, count) in dist.iter().enumerate() {
        if *count > 0 {
            eprintln!("  {}: {}", CLASS_LABELS[i], count);
        }
    }

    // Train/val split: stratified 80/20
    let mut rng_seed: u64 = 42;
    let mut xorshift = |seed: &mut u64| -> u64 {
        *seed ^= *seed << 13;
        *seed ^= *seed >> 7;
        *seed ^= *seed << 17;
        *seed
    };

    let mut train_idx: Vec<usize> = Vec::new();
    let mut val_idx: Vec<usize> = Vec::new();
    // Group by class, split each
    for class in 0..NUM_CLASSES {
        let class_indices: Vec<usize> = samples.iter().enumerate()
            .filter(|(_, s)| s.label == class)
            .map(|(i, _)| i)
            .collect();
        let n_val = (class_indices.len() as f64 * 0.2).ceil() as usize;
        let mut shuffled = class_indices.clone();
        for i in (1..shuffled.len()).rev() {
            let j = (xorshift(&mut rng_seed) as usize) % (i + 1);
            shuffled.swap(i, j);
        }
        val_idx.extend_from_slice(&shuffled[..n_val.min(shuffled.len())]);
        train_idx.extend_from_slice(&shuffled[n_val.min(shuffled.len())..]);
    }

    let n_train = train_idx.len();
    let n_val = val_idx.len();
    eprintln!("[train] {} total samples → {} train, {} val", samples.len(), n_train, n_val);

    // Train BPE tokenizer on all data (train + val)
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

    // Track best val accuracy for checkpoint
    let mut best_val_acc = 0.0f64;
    let mut best_epoch = 0u32;

    // Cosine LR schedule
    let lr_max = config.lr;
    let lr_min = lr_max * 0.01;
    let total_epochs = config.epochs as f64;

    // Helper: evaluate accuracy on a set of indices
    let eval_acc = |indices: &[usize]| -> (f64, usize, usize) {
        let mut correct = 0usize;
        let mut total = 0usize;
        // Evaluate in batches of 64
        for chunk in indices.chunks(64) {
            let bs = chunk.len();
            let mut flat = Vec::with_capacity(bs * max_len);
            let mut labs = Vec::with_capacity(bs);
            for &idx in chunk {
                flat.extend_from_slice(&all_ids[idx]);
                labs.push(all_labels[idx]);
            }
            let input = Tensor::from_vec(flat, (bs, max_len), &device).unwrap();
            let logits = model.forward(&input).unwrap();
            let preds: Vec<u32> = logits.argmax(1).unwrap().to_vec1().unwrap();
            for (i, &pred) in preds.iter().enumerate() {
                if pred == labs[i] { correct += 1; }
                total += 1;
            }
        }
        let acc = if total > 0 { correct as f64 / total as f64 * 100.0 } else { 0.0 };
        (acc, correct, total)
    };

    // Training loop
    for epoch in 0..config.epochs {
        // Cosine LR decay
        let lr = lr_min + 0.5 * (lr_max - lr_min) * (1.0 + (std::f64::consts::PI * epoch as f64 / total_epochs).cos());
        optimizer.set_learning_rate(lr);

        // Shuffle training indices
        for i in (1..n_train).rev() {
            let j = (xorshift(&mut rng_seed) as usize) % (i + 1);
            train_idx.swap(i, j);
        }

        let mut total_loss = 0.0f64;
        let mut correct = 0usize;
        let mut total = 0usize;

        for batch_start in (0..n_train).step_by(config.batch_size) {
            let batch_end = (batch_start + config.batch_size).min(n_train);
            let bs = batch_end - batch_start;

            let mut flat_ids = Vec::with_capacity(bs * max_len);
            let mut batch_labels = Vec::with_capacity(bs);
            for &idx in &train_idx[batch_start..batch_end] {
                flat_ids.extend_from_slice(&all_ids[idx]);
                batch_labels.push(all_labels[idx]);
            }
            let input = Tensor::from_vec(flat_ids, (bs, max_len), &device)
                .map_err(|e| format!("input tensor: {}", e))?;
            let labels = Tensor::from_vec(batch_labels.clone(), (bs,), &device)
                .map_err(|e| format!("label tensor: {}", e))?;

            let logits = model.forward(&input)
                .map_err(|e| format!("forward: {}", e))?;
            let loss = candle_nn::loss::cross_entropy(&logits, &labels)
                .map_err(|e| format!("loss: {}", e))?;
            let loss_val: f64 = loss.to_dtype(DType::F64)
                .and_then(|t| t.to_scalar())
                .map_err(|e| format!("loss scalar: {}", e))?;

            let preds: Vec<u32> = logits.argmax(1)
                .map_err(|e| format!("argmax: {}", e))?
                .to_vec1()
                .map_err(|e| format!("to_vec: {}", e))?;
            for (i, &pred) in preds.iter().enumerate() {
                if pred == batch_labels[i] { correct += 1; }
                total += 1;
            }

            optimizer.backward_step(&loss)
                .map_err(|e| format!("backward: {}", e))?;
            total_loss += loss_val * bs as f64;
        }

        let avg_loss = total_loss / n_train as f64;
        let train_acc = if total > 0 { correct as f64 / total as f64 * 100.0 } else { 0.0 };

        // Evaluate on validation set every 10 epochs (or last epoch)
        let is_eval = epoch % 10 == 9 || epoch == config.epochs - 1;
        if is_eval && n_val > 0 {
            let (val_acc, _, _) = eval_acc(&val_idx);
            let marker = if val_acc > best_val_acc { best_val_acc = val_acc; best_epoch = epoch + 1; " *best*" } else { "" };
            eprintln!("[train] epoch {}/{}: loss={:.4} train={:.1}% val={:.1}% lr={:.6}{}",
                epoch + 1, config.epochs, avg_loss, train_acc, val_acc, lr, marker);
        } else {
            eprintln!("[train] epoch {}/{}: loss={:.4} train={:.1}% lr={:.6}",
                epoch + 1, config.epochs, avg_loss, train_acc, lr);
        }
    }

    if n_val > 0 {
        eprintln!("[train] best val acc: {:.1}% at epoch {}", best_val_acc, best_epoch);
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

/// Train SFT with default config for a given tier.
/// Used by GUI micro_train panel.
pub fn train_sft(config: &TrainConfig) -> Result<PathBuf, String> {
    train(config)
}

/// Train all specialists (all tiers) from a directory.
/// Used by GUI micro_train panel.
pub fn train_all_specialists(
    training_dir: &Path,
    output_dir: &Path,
    epochs: u32,
    lr: f64,
    batch_size: usize,
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
            epochs,
            lr,
            batch_size,
        };
        match train(&config) {
            Ok(path) => outputs.push(path),
            Err(e) => eprintln!("[train] {} failed: {}", tier.name(), e),
        }
    }
    Ok(outputs)
}

/// Generate synthetic classifier training data for all 8 categories.
/// Returns the path to the augmented SFT file.
pub fn generate_synthetic_data(training_dir: &Path) -> Result<PathBuf, String> {
    let output = training_dir.join("sft_chatml.jsonl");

    // Load existing data to avoid duplicates
    let existing = if output.exists() {
        std::fs::read_to_string(&output).unwrap_or_default()
    } else {
        String::new()
    };
    let existing_count = existing.lines().filter(|l| !l.trim().is_empty()).count();

    let sys = "Classify the input into exactly one category. Reply with only the category name.\nCategories: classify, clippy_fix, code_gen, code_review, explain, fix_compile, test_write, validate";

    // Per-category example inputs — realistic Rust task descriptions
    let examples: &[(&str, &[&str])] = &[
        ("classify", &[
            "what kind of task is this: add a new endpoint to the API",
            "categorize: the build is broken after the last merge",
            "is this a bug fix or a feature request",
            "sort this request: write unit tests for the parser",
            "what type of work: refactor the error handling",
            "classify this ticket: add pagination to the list endpoint",
            "what category: the CI pipeline is failing on lint",
            "determine the task type: explain how the router works",
            "triage: users report slow response times on /api/search",
            "categorize this issue: add dark mode to the GUI",
            "what kind of change: rename all instances of foo to bar",
            "classify: implement retry logic for HTTP requests",
            "sort: update the README with new build instructions",
            "what type: add a health check endpoint",
            "triage this: memory leak in the connection pool",
            "determine: should this be a refactor or a rewrite",
            "classify this work item: add CORS headers to responses",
            "what category does this fall into: fix the broken tests",
            "categorize: add compression to the response pipeline",
            "is this a bug, feature, or maintenance task: update dependencies",
            "classify: implement WebSocket support for live updates",
            "what type of issue: the database migrations are failing",
            "triage: add rate limiting to the API",
            "categorize this request: write documentation for the CLI",
            "classify: switch from JSON to protobuf serialization",
            "determine task type: add logging to the auth middleware",
            "what kind of change is this: fix off-by-one in pagination",
            "sort this: add metrics collection for request latency",
            "classify: implement a caching layer for frequent queries",
            "categorize: set up automated deployment pipeline",
        ]),
        ("clippy_fix", &[
            "fix clippy warning: unnecessary clone on line 42",
            "resolve clippy lint: use of deprecated function",
            "clippy says: redundant closure, just pass the function directly",
            "fix lint: manual implementation of map",
            "clippy warning: this match expression can be simplified",
            "resolve: clippy reports needless borrow",
            "fix clippy: single-character string used as pattern",
            "clippy: len without is_empty implementation",
            "fix lint warning: collapsible if statements",
            "clippy says: use Option::map_or instead of match",
            "fix: clippy reports unused variable in loop",
            "resolve clippy: this could be a const fn",
            "clippy warning: manual swap, use std::mem::swap",
            "fix lint: unnecessary unwrap, pattern match instead",
            "clippy: iter().map().collect() can be simplified",
            "fix clippy warning: redundant field names in struct init",
            "clippy reports: use of to_string() on a string literal",
            "resolve lint: bool comparison, just use the bool directly",
            "fix clippy: explicit return at end of function",
            "clippy says: manual implementation of Option::map",
            "fix: use if let instead of match with single pattern",
            "clippy warning: casting u32 to f64 can use f64::from",
            "resolve: clippy says clone on copy type",
            "fix lint: unneeded return statement",
            "clippy: use contains instead of manual iteration",
            "fix clippy warning: use eprintln instead of writeln to stderr",
            "clippy reports: useless conversion, remove .into()",
            "fix: clippy warns about large enum variant",
            "resolve clippy: wildcard match arm not needed",
            "clippy: use is_some() instead of != None",
        ]),
        ("code_gen", &[
            "write a function that finds the longest common subsequence",
            "generate a Rust struct for a binary tree with insert and search",
            "implement an LRU cache with O(1) get and put",
            "write a parser for a simple arithmetic expression language",
            "create a thread pool implementation in Rust",
            "generate a rate limiter using token bucket algorithm",
            "write a function to serialize a struct to bincode",
            "implement a simple HTTP router with path matching",
            "create a function that compresses data with zstd",
            "write an iterator adapter that deduplicates consecutive elements",
            "generate a circular buffer implementation",
            "implement a trie data structure for string prefix lookup",
            "write a function that converts markdown to HTML",
            "create a simple key-value store backed by a file",
            "generate a Bloom filter implementation",
            "write a concurrent queue using atomic operations",
            "implement a topological sort for a DAG",
            "create a function that generates a UUID v4",
            "write a state machine for parsing CSV records",
            "generate a connection pool with max connections and timeout",
            "implement a simple regex engine for basic patterns",
            "write a function that merges two sorted iterators",
            "create a retry wrapper with exponential backoff",
            "generate a ring buffer for a fixed-size log",
            "implement an event emitter with typed callbacks",
            "write a function to walk a directory tree recursively",
            "create a builder pattern for a complex config struct",
            "generate a diff algorithm for two text files",
            "implement a skip list data structure",
            "write a function that parses CLI arguments into a struct",
        ]),
        ("code_review", &[
            "review this function for potential panics",
            "check this code for race conditions",
            "is this error handling correct or are we swallowing errors",
            "review the API design of this module",
            "does this implementation handle edge cases properly",
            "check if this code follows Rust idioms",
            "review this unsafe block, is it sound",
            "are there any memory safety issues in this code",
            "review the public API surface of this crate",
            "check this concurrent code for deadlock potential",
            "is this use of unwrap justified here",
            "review the error types, are they too broad",
            "check if this code handles backpressure correctly",
            "review this trait design for extensibility",
            "are there any performance issues in this hot path",
            "review this serialization code for correctness",
            "check if the lifetimes are correct in this struct",
            "review this match statement for exhaustiveness",
            "is this clone necessary or can we borrow instead",
            "review the thread safety of this shared state",
            "check if this async code properly handles cancellation",
            "review this parsing code for injection attacks",
            "is the error propagation in this chain correct",
            "review this HashMap usage for potential DoS via hash flooding",
            "check if this file I/O properly handles permissions",
            "review the drop implementation for resource cleanup",
            "is this use of transmute safe and justified",
            "review the public/private boundary of this module",
            "check if this iterator chain is lazy or collecting unnecessarily",
            "review this config parsing for missing field handling",
        ]),
        ("explain", &[
            "explain how the borrow checker works in this context",
            "what does this lifetime annotation mean",
            "how does the router pick which handler to call",
            "explain the difference between Box and Rc",
            "what is the purpose of the PhantomData in this struct",
            "how does this async executor work under the hood",
            "explain why this code needs Pin",
            "what does the where clause do in this trait bound",
            "how does serde deserialize into this enum",
            "explain the memory layout of this struct",
            "what is the purpose of the Drop trait here",
            "how does this macro expand",
            "explain the difference between &str and String in this API",
            "what does #[repr(C)] do on this struct",
            "how does the type inference work in this closure",
            "explain why we need Send + Sync on this type",
            "what is the difference between into_iter and iter",
            "how does this pattern matching with @ work",
            "explain the orphan rule and why this impl fails",
            "what does the turbofish syntax do here",
            "how does the deref coercion work in this call",
            "explain why this closure captures by move",
            "what is the difference between Fn and FnOnce here",
            "how does the trait object dispatch work",
            "explain the coherence rules for this impl",
            "what does the question mark operator expand to",
            "how does this const generic parameter work",
            "explain why this associated type is needed",
            "what is the purpose of the Cow type here",
            "how does the GAT in this trait work",
        ]),
        ("fix_compile", &[
            "fix: cannot borrow as mutable because it is also borrowed as immutable",
            "resolve: expected type A found type B",
            "fix compile error: lifetime mismatch in function signature",
            "the code fails with: cannot move out of borrowed content",
            "fix: trait X is not implemented for type Y",
            "resolve: mismatched types, expected &str got String",
            "fix build error: use of undeclared type or module",
            "cannot infer type, need type annotation on this let binding",
            "fix: value used after being moved",
            "resolve: temporary value dropped while borrowed",
            "fix compile: method not found for this type",
            "error: cannot return reference to temporary value",
            "fix: conflicting implementations of trait for type",
            "resolve: missing lifetime specifier on return type",
            "fix build: unused import warning treated as error",
            "error: closure may outlive the current function",
            "fix: private type in public interface",
            "resolve: size not known at compile time, needs Box",
            "fix compile: pattern has wrong number of fields",
            "error: cannot borrow self as mutable more than once",
            "fix: associated function not found for struct",
            "resolve: binary operation cannot be applied to these types",
            "fix build error: recursive type has infinite size",
            "error: type annotations needed for this binding",
            "fix: module not found in the current scope",
            "resolve: trait bound not satisfied for generic parameter",
            "fix compile: wrong number of type arguments",
            "error: cannot assign to immutable field",
            "fix: the trait From is not implemented for this conversion",
            "resolve: use of partially moved value",
        ]),
        ("test_write", &[
            "write tests for the parse_config function",
            "add unit tests for the LRU cache implementation",
            "create integration tests for the HTTP API endpoints",
            "write property-based tests for the serialization round-trip",
            "add test coverage for edge cases in the router",
            "write tests that verify error handling paths",
            "create a test for concurrent access to shared state",
            "add regression test for the off-by-one bug",
            "write tests for the CLI argument parser",
            "create benchmark tests for the hot path",
            "add tests for the database migration scripts",
            "write a test that verifies the config file loading",
            "create tests for the authentication middleware",
            "add test coverage for the WebSocket handler",
            "write tests for the rate limiter behavior",
            "create a test that exercises the retry logic",
            "add tests verifying the error messages are correct",
            "write tests for boundary conditions in pagination",
            "create integration tests for the file upload endpoint",
            "add tests for the caching layer TTL behavior",
            "write tests for the event emitter subscription lifecycle",
            "create a test for graceful shutdown behavior",
            "add tests for the connection pool under load",
            "write tests for the log rotation logic",
            "create tests for the permission checking middleware",
            "add tests that verify the metrics collection",
            "write a test for the streaming response handler",
            "create tests for the config validation rules",
            "add tests for the job queue retry behavior",
            "write tests for the search index update pipeline",
        ]),
        ("validate", &[
            "validate that this function returns the correct result",
            "check if the output matches the expected format",
            "verify the response contains all required fields",
            "validate the generated code compiles and runs",
            "check that the migration produces the expected schema",
            "verify the serialization output matches the spec",
            "validate the error response has the correct status code",
            "check if the output JSON matches the expected structure",
            "verify the function handles null input correctly",
            "validate that the config file is well-formed",
            "check the output against the golden file",
            "verify the generated SQL is syntactically correct",
            "validate that the API response time is within SLA",
            "check if the transformed data matches expectations",
            "verify the encryption output can be decrypted back",
            "validate the parsed AST matches the expected tree",
            "check that the generated docs contain all public items",
            "verify the output encoding is valid UTF-8",
            "validate the computed hash matches the expected value",
            "check if the rendered HTML passes validation",
            "verify the log output contains the expected fields",
            "validate that the test results are deterministic",
            "check the binary output matches the reference file",
            "verify the config merge produces correct precedence",
            "validate the generated protobuf matches the schema",
            "check if the response headers are correct",
            "verify the state machine transitions are valid",
            "validate the dependency graph has no cycles",
            "check that the build artifacts are reproducible",
            "verify the generated types implement the required traits",
        ]),
    ];

    let mut lines: Vec<String> = existing.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.to_string())
        .collect();

    let mut added = 0;
    for (label, inputs) in examples {
        for input in *inputs {
            let entry = serde_json::json!({
                "messages": [
                    {"role": "system", "content": sys},
                    {"role": "user", "content": input},
                    {"role": "assistant", "content": label}
                ]
            });
            let line = serde_json::to_string(&entry).unwrap();
            if !existing.contains(input) {
                lines.push(line);
                added += 1;
            }
        }
    }

    std::fs::write(&output, lines.join("\n") + "\n")
        .map_err(|e| format!("write augmented data: {}", e))?;

    eprintln!("[synth] added {} synthetic examples to {} (total {})",
        added, output.display(), lines.len());
    eprintln!("[synth] existing: {}, new: {}", existing_count, added);

    Ok(output)
}

/// Training data directory.
pub fn training_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".kova").join("micro").join("training")
}
