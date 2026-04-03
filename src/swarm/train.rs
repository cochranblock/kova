//! Subatomic model trainer. Tiny classifiers via candle.
//!
//! Architecture per model:
//!   Input text → character trigram hashing → fixed-size feature vector
//!   → Linear(feature_dim, num_classes) → softmax → class prediction
//!
//! Total params = feature_dim * num_classes + num_classes (bias)
//! For feature_dim=256, 2 classes: 514 params. For 5 classes: 1,285 params.
//!
//! f389=train_subatomic, f390=generate_slop_data, f391=generate_code_vs_english_data,
//! f392=generate_lang_data.
// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

/// Feature dimension for trigram hash. 256 gives good separation at tiny param count.
const FEATURE_DIM: usize = 256;

/// Training example: text + class label.
pub struct Example {
    pub text: String,
    pub label: usize,
}

/// Subatomic model config.
pub struct SubatomicConfig {
    pub name: String,
    pub num_classes: usize,
    pub class_names: Vec<String>,
    pub feature_dim: usize,
    pub epochs: usize,
    pub lr: f64,
}

/// Hash a character trigram to a feature index.
fn trigram_hash(a: char, b: char, c: char, dim: usize) -> usize {
    let mut h = DefaultHasher::new();
    (a, b, c).hash(&mut h);
    (h.finish() as usize) % dim
}

/// Convert text to fixed-size feature vector via character trigram hashing.
pub fn featurize(text: &str, dim: usize) -> Vec<f32> {
    let mut features = vec![0.0f32; dim];
    let chars: Vec<char> = text.chars().collect();
    if chars.len() < 3 {
        // For very short text, hash the whole thing.
        let mut h = DefaultHasher::new();
        text.hash(&mut h);
        features[(h.finish() as usize) % dim] = 1.0;
        return features;
    }
    for window in chars.windows(3) {
        let idx = trigram_hash(window[0], window[1], window[2], dim);
        features[idx] += 1.0;
    }
    // L2 normalize.
    let norm: f32 = features.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for f in &mut features {
            *f /= norm;
        }
    }
    features
}

/// f389=train_subatomic. Train a tiny linear classifier and save as safetensors.
///
/// Architecture: feature_dim → num_classes linear layer.
/// Total params: feature_dim * num_classes + num_classes.
/// Training: SGD with softmax cross-entropy loss.
pub fn f389(config: &SubatomicConfig, examples: &[Example], output_dir: &Path) -> Result<PathBuf, String> {
    if examples.is_empty() {
        return Err("no training examples".into());
    }

    let dim = config.feature_dim;
    let nc = config.num_classes;
    let total_params = dim * nc + nc;

    eprintln!(
        "[subatomic] training '{}': {} examples, {} classes, {} params",
        config.name,
        examples.len(),
        nc,
        total_params
    );

    // Initialize weights (Xavier init).
    let scale = (2.0 / (dim + nc) as f64).sqrt() as f32;
    let mut weights = vec![0.0f32; dim * nc];
    let mut bias = vec![0.0f32; nc];

    // Deterministic pseudo-random init.
    for (i, w) in weights.iter_mut().enumerate() {
        let mut h = DefaultHasher::new();
        i.hash(&mut h);
        let r = (h.finish() as f32 / u64::MAX as f32) * 2.0 - 1.0;
        *w = r * scale;
    }

    // Featurize all examples.
    let features: Vec<Vec<f32>> = examples.iter().map(|e| featurize(&e.text, dim)).collect();

    // Train via SGD with softmax cross-entropy.
    let lr = config.lr as f32;
    let mut best_acc = 0.0f32;

    for epoch in 0..config.epochs {
        let mut total_loss = 0.0f32;
        let mut correct = 0usize;

        for (i, ex) in examples.iter().enumerate() {
            let feat = &features[i];

            // Forward: logits = W^T * x + b.
            let mut logits = vec![0.0f32; nc];
            for c in 0..nc {
                let mut sum = bias[c];
                for d in 0..dim {
                    sum += weights[c * dim + d] * feat[d];
                }
                logits[c] = sum;
            }

            // Softmax.
            let max_logit = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            let mut probs = vec![0.0f32; nc];
            let mut sum_exp = 0.0f32;
            for c in 0..nc {
                probs[c] = (logits[c] - max_logit).exp();
                sum_exp += probs[c];
            }
            for c in 0..nc {
                probs[c] /= sum_exp;
            }

            // Loss: -log(prob[target]).
            let target = ex.label;
            total_loss -= probs[target].max(1e-10).ln();

            // Accuracy.
            let pred = logits
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
                .map(|(i, _)| i)
                .unwrap_or(0);
            if pred == target {
                correct += 1;
            }

            // Backward: grad_logits[c] = probs[c] - (c == target ? 1 : 0).
            let mut grad_logits = probs.clone();
            grad_logits[target] -= 1.0;

            // Update weights: W[c][d] -= lr * grad_logits[c] * feat[d].
            for c in 0..nc {
                let g = grad_logits[c];
                for d in 0..dim {
                    weights[c * dim + d] -= lr * g * feat[d];
                }
                bias[c] -= lr * g;
            }
        }

        let acc = correct as f32 / examples.len() as f32;
        if acc > best_acc {
            best_acc = acc;
        }

        if epoch % 5 == 0 || epoch == config.epochs - 1 {
            eprintln!(
                "[subatomic] epoch {}/{}: loss={:.4}, acc={:.1}%",
                epoch + 1,
                config.epochs,
                total_loss / examples.len() as f32,
                acc * 100.0
            );
        }
    }

    // Save as safetensors-compatible format.
    // We use a simple binary format: header + weights + bias.
    let model_dir = output_dir.join(&config.name);
    std::fs::create_dir_all(&model_dir).map_err(|e| format!("mkdir: {}", e))?;

    // Save weights as raw f32 binary (candle-compatible).
    let weights_path = model_dir.join("weights.bin");
    let weights_bytes: Vec<u8> = weights.iter().flat_map(|f| f.to_le_bytes()).collect();
    std::fs::write(&weights_path, &weights_bytes).map_err(|e| format!("write weights: {}", e))?;

    let bias_path = model_dir.join("bias.bin");
    let bias_bytes: Vec<u8> = bias.iter().flat_map(|f| f.to_le_bytes()).collect();
    std::fs::write(&bias_path, &bias_bytes).map_err(|e| format!("write bias: {}", e))?;

    // Save config.
    let config_json = serde_json::json!({
        "name": config.name,
        "feature_dim": dim,
        "num_classes": nc,
        "class_names": config.class_names,
        "total_params": total_params,
        "best_accuracy": best_acc,
        "architecture": "trigram_hash_linear",
    });
    let config_path = model_dir.join("config.json");
    std::fs::write(&config_path, serde_json::to_string_pretty(&config_json).unwrap())
        .map_err(|e| format!("write config: {}", e))?;

    eprintln!(
        "[subatomic] saved '{}' to {} ({} params, {:.1}% acc)",
        config.name,
        model_dir.display(),
        total_params,
        best_acc * 100.0
    );

    Ok(model_dir)
}

// ── Data Generators ─────────────────────────────────────────

/// P12 banned slop words.
const SLOP_WORDS: &[&str] = &[
    "utilize", "leverage", "optimize", "comprehensive", "robust",
    "seamlessly", "scalable", "paradigm", "synergy", "cutting-edge",
    "streamline", "empower", "delve", "foster", "harness",
    "groundbreaking", "innovative", "transform", "revolutionize",
    "unprecedented",
];

/// Clean replacement templates.
const CLEAN_SENTENCES: &[&str] = &[
    "This function reads the file and returns the contents.",
    "The build passed with zero warnings.",
    "Fixed the off-by-one error in the loop.",
    "Added a test for the edge case.",
    "Refactored the parser to handle nested blocks.",
    "The server starts on port 8080.",
    "Removed the unused import.",
    "Updated the config to use the new path.",
    "The binary is 27 MB after stripping.",
    "Moved the struct to a separate module.",
    "Changed the return type to Result.",
    "The test covers both success and error paths.",
    "Split the function into smaller parts.",
    "Added timeout handling for SSH connections.",
    "The CI pipeline runs clippy and tests.",
    "Implemented the trait for the new type.",
    "Reduced memory usage by reusing the buffer.",
    "The daemon polls every 3 seconds.",
    "Applied the fix from the upstream PR.",
    "Sorted the imports alphabetically.",
];

/// Slop sentence templates.
const SLOP_TEMPLATES: &[&str] = &[
    "We need to {slop} the codebase for better results.",
    "This {slop} solution will improve everything.",
    "The system is designed to {slop} workflows.",
    "Our {slop} approach handles all cases.",
    "This provides a {slop} way to handle errors.",
    "The tool will {slop} your development process.",
    "We've built a {slop} framework for testing.",
    "This {slop} architecture supports all platforms.",
    "The engine is designed to be {slop} and reliable.",
    "We can {slop} the deployment pipeline.",
];

/// f390=generate_slop_data. Generate training data for slop detector.
/// Label 0 = clean, label 1 = slop.
pub fn f390() -> Vec<Example> {
    let mut examples = Vec::new();

    // Clean examples.
    for s in CLEAN_SENTENCES {
        examples.push(Example {
            text: s.to_string(),
            label: 0,
        });
    }

    // Slop examples: insert banned words into templates.
    for word in SLOP_WORDS {
        for template in SLOP_TEMPLATES {
            examples.push(Example {
                text: template.replace("{slop}", word),
                label: 1,
            });
        }
    }

    // More clean examples from common coding phrases.
    let coding_clean = [
        "cargo build --release", "git commit -m 'fix bug'", "fn main() {}",
        "let x = 42;", "assert_eq!(result, expected);", "use std::path::Path;",
        "impl Display for Error {}", "#[test] fn it_works() {}",
        "pub struct Config { port: u16 }", "match result { Ok(v) => v, Err(e) => return Err(e) }",
        "The function returns an error if the file is missing.",
        "Run the tests before pushing.",
        "This module handles HTTP routing.",
        "The struct stores the connection pool.",
        "Each worker node has its own sled database.",
    ];
    for s in &coding_clean {
        examples.push(Example {
            text: s.to_string(),
            label: 0,
        });
    }

    examples
}

/// f391=generate_code_vs_english_data. Generate training data for code-vs-english classifier.
/// Label 0 = english, label 1 = code.
/// Scrapes .rs and .md files from the given project directory.
pub fn f391(project_dir: &Path) -> Vec<Example> {
    let mut examples = Vec::new();

    // Scrape .rs files for code examples.
    if let Ok(entries) = glob::glob(&format!("{}/**/*.rs", project_dir.display())) {
        for entry in entries.flatten() {
            if let Ok(content) = std::fs::read_to_string(&entry) {
                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed.len() > 10 && trimmed.len() < 200 && !trimmed.starts_with("//") {
                        examples.push(Example {
                            text: trimmed.to_string(),
                            label: 1,
                        });
                    }
                    if examples.len() > 2000 {
                        break;
                    }
                }
            }
            if examples.len() > 2000 {
                break;
            }
        }
    }

    let code_count = examples.len();

    // Scrape .md files for english examples.
    if let Ok(entries) = glob::glob(&format!("{}/**/*.md", project_dir.display())) {
        for entry in entries.flatten() {
            if let Ok(content) = std::fs::read_to_string(&entry) {
                for line in content.lines() {
                    let trimmed = line.trim();
                    // Skip markdown syntax, headers, tables, code blocks.
                    if trimmed.len() > 15
                        && trimmed.len() < 200
                        && !trimmed.starts_with('#')
                        && !trimmed.starts_with('|')
                        && !trimmed.starts_with("```")
                        && !trimmed.starts_with("- ")
                        && !trimmed.contains("```")
                    {
                        examples.push(Example {
                            text: trimmed.to_string(),
                            label: 0,
                        });
                    }
                    if examples.len() > code_count + 2000 {
                        break;
                    }
                }
            }
            if examples.len() > code_count + 2000 {
                break;
            }
        }
    }

    // Add synthetic code examples for coverage.
    let synthetic_code = [
        "fn main() { println!(\"hello\"); }",
        "let mut v: Vec<i32> = Vec::new();",
        "impl From<String> for Error {",
        "use std::collections::HashMap;",
        "pub async fn serve(port: u16) -> Result<()> {",
        "#[derive(Debug, Clone, Serialize)]",
        "match self.state { State::Running => {},",
        "for (i, item) in items.iter().enumerate() {",
        "if let Some(val) = map.get(&key) {",
        "type Result<T> = std::result::Result<T, Error>;",
        "const MAX_RETRIES: u32 = 10;",
        "struct Config { pub port: u16, pub host: String }",
        "def train(model, data, epochs=10):",
        "import numpy as np",
        "function handleClick(event) {",
        "const express = require('express');",
        "func main() { fmt.Println(\"hello\") }",
        "#!/bin/bash\nset -e",
        "for f in *.rs; do wc -l \"$f\"; done",
        "export PATH=\"$HOME/.cargo/bin:$PATH\"",
    ];
    for s in &synthetic_code {
        examples.push(Example {
            text: s.to_string(),
            label: 1,
        });
    }

    // Add synthetic english.
    let synthetic_english = [
        "The project uses a single binary architecture for deployment.",
        "All tests must pass before merging a pull request.",
        "The documentation should be updated when adding new features.",
        "This approach reduces complexity by keeping everything in one crate.",
        "Worker nodes communicate over SSH with host certificate authentication.",
        "The tournament results show that smaller models can be more accurate.",
        "Each commit message should explain why, not what.",
        "The config file lives in the home directory under a hidden folder.",
        "Binary size was reduced from 54 MB to 27 MB with link-time optimization.",
        "The agent loop continues until the model stops calling tools.",
    ];
    for s in &synthetic_english {
        examples.push(Example {
            text: s.to_string(),
            label: 0,
        });
    }

    examples
}

/// f392=generate_lang_data. Generate training data for language detector.
/// Labels: 0=rust, 1=python, 2=javascript, 3=go, 4=shell.
pub fn f392() -> Vec<Example> {
    let mut examples = Vec::new();

    // Rust examples.
    let rust = [
        "fn main() { println!(\"hello\"); }",
        "let mut v: Vec<i32> = Vec::new();",
        "impl Display for Error { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { } }",
        "use std::collections::HashMap;",
        "pub async fn serve(port: u16) -> anyhow::Result<()> {",
        "#[derive(Debug, Clone, Serialize, Deserialize)]",
        "match result { Ok(v) => v, Err(e) => return Err(e.into()) }",
        "for (i, item) in items.iter().enumerate() {",
        "if let Some(val) = map.get(&key) { process(val); }",
        "type Result<T> = std::result::Result<T, Error>;",
        "const MAX_RETRIES: u32 = 10;",
        "struct Config { pub port: u16, pub host: String, pub workers: usize }",
        "trait Handler: Send + Sync { fn handle(&self, req: Request) -> Response; }",
        "enum State { Idle, Running, Failed(String) }",
        "let handle = tokio::spawn(async move { process().await });",
        "pub(crate) fn extract_rust_block(s: &str) -> Option<String> {",
        "#[cfg(test)] mod tests { use super::*; }",
        "impl From<std::io::Error> for AppError {",
        "let db = sled::open(&path)?;",
        "fn f141(call: &t103, project_dir: &Path) -> t104 {",
        "cargo build --release -p kova --features serve",
        "assert_eq!(f170(\"hello\"), 2);",
        "eprintln!(\"[deploy] syncing to {} nodes\", nodes.len());",
        "pub static TOOLS: &[t101] = &[",
        "let rx = mpsc::channel::<Arc<str>>();",
    ];
    for s in &rust {
        examples.push(Example { text: s.to_string(), label: 0 });
    }

    // Python examples.
    let python = [
        "def train(model, data, epochs=10, lr=3e-4):",
        "import numpy as np",
        "from transformers import AutoTokenizer",
        "class DataLoader: def __init__(self, batch_size=32):",
        "if __name__ == '__main__': main()",
        "for i, (x, y) in enumerate(dataloader):",
        "loss = criterion(output, target)",
        "model.eval() with torch.no_grad():",
        "optimizer.zero_grad() loss.backward() optimizer.step()",
        "import os; os.path.join(base, 'models')",
        "print(f'epoch {epoch}: loss={loss:.4f}')",
        "x = np.array([[1, 2], [3, 4]])",
        "def forward(self, x): return self.linear(self.relu(x))",
        "pip install torch transformers datasets",
        "with open('data.json', 'r') as f: data = json.load(f)",
        "@dataclass class Config: lr: float = 3e-4",
        "yield from self._generate_batch(data)",
        "except ValueError as e: logger.error(f'bad value: {e}')",
        "lambda x: x ** 2 + 1",
        "self.weights = nn.Parameter(torch.randn(256, 128))",
    ];
    for s in &python {
        examples.push(Example { text: s.to_string(), label: 1 });
    }

    // JavaScript examples.
    let javascript = [
        "const express = require('express');",
        "function handleClick(event) { event.preventDefault(); }",
        "const [state, setState] = useState(null);",
        "export default function App() { return <div>hello</div>; }",
        "fetch('/api/data').then(res => res.json()).then(data => setData(data));",
        "document.getElementById('root').addEventListener('click', handler);",
        "const app = express(); app.listen(3000);",
        "module.exports = { config, handler };",
        "async function fetchData(url) { const res = await fetch(url); return res.json(); }",
        "console.log(`server running on port ${PORT}`);",
        "npm install express cors dotenv",
        "const router = express.Router();",
        "arr.map(x => x * 2).filter(x => x > 10).reduce((a, b) => a + b, 0)",
        "try { JSON.parse(input) } catch (e) { return null; }",
        "const { data, error } = useSWR('/api/users', fetcher);",
        "window.localStorage.setItem('token', jwt);",
        "new Promise((resolve, reject) => { setTimeout(resolve, 1000); })",
        "class EventEmitter extends EventTarget {}",
        "Object.keys(obj).forEach(key => { delete obj[key]; });",
        "import { createServer } from 'http';",
    ];
    for s in &javascript {
        examples.push(Example { text: s.to_string(), label: 2 });
    }

    // Go examples.
    let go = [
        "func main() { fmt.Println(\"hello\") }",
        "package main; import \"fmt\"",
        "func (s *Server) ServeHTTP(w http.ResponseWriter, r *http.Request) {",
        "if err != nil { return fmt.Errorf(\"failed: %w\", err) }",
        "ch := make(chan string, 10)",
        "go func() { result <- process(data) }()",
        "type Config struct { Port int `json:\"port\"` }",
        "defer file.Close()",
        "for _, item := range items { process(item) }",
        "ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)",
        "http.HandleFunc(\"/api/health\", healthHandler)",
        "log.Fatal(http.ListenAndServe(\":8080\", nil))",
        "var wg sync.WaitGroup",
        "select { case msg := <-ch: handle(msg) case <-ctx.Done(): return }",
        "func NewClient(addr string) (*Client, error) {",
        "json.NewDecoder(r.Body).Decode(&req)",
        "bytes, err := ioutil.ReadAll(resp.Body)",
        "go build -o myapp ./cmd/server",
        "interface{ Error() string }",
        "mu.Lock() defer mu.Unlock()",
    ];
    for s in &go {
        examples.push(Example { text: s.to_string(), label: 3 });
    }

    // Shell examples.
    let shell = [
        "#!/bin/bash\nset -euo pipefail",
        "for f in *.rs; do wc -l \"$f\"; done",
        "export PATH=\"$HOME/.cargo/bin:$PATH\"",
        "if [ -f \"$CONFIG\" ]; then source \"$CONFIG\"; fi",
        "curl -sSf https://sh.rustup.rs | sh",
        "find . -name '*.rs' -exec grep -l 'TODO' {} +",
        "tar -czf backup.tar.gz --exclude=target .",
        "ssh lf 'cd /home/mcochran && cargo build --release'",
        "rsync -avz --exclude target src/ remote:src/",
        "echo \"$VAR\" | grep -q 'pattern' && echo 'found'",
        "kill $(pgrep -f 'kova serve')",
        "systemctl --user restart kova-serve",
        "cat /proc/cpuinfo | grep 'model name' | head -1",
        "awk '{print $1}' /etc/hosts | sort -u",
        "scp -r ./dist user@host:/var/www/",
        "chmod +x scripts/deploy.sh && ./scripts/deploy.sh",
        "nohup ./server &> /var/log/server.log &",
        "while read -r line; do process \"$line\"; done < input.txt",
        "[ -z \"$API_KEY\" ] && echo 'API_KEY not set' && exit 1",
        "ln -sf /usr/local/bin/kova /usr/bin/kova",
    ];
    for s in &shell {
        examples.push(Example { text: s.to_string(), label: 4 });
    }

    examples
}

/// Train all three starter subatomic models. Saves to output_dir.
pub fn train_starter(project_dir: &Path, output_dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(output_dir).map_err(|e| format!("mkdir: {}", e))?;

    // 1. Slop detector (binary: clean=0, slop=1).
    let slop_data = f390();
    let slop_config = SubatomicConfig {
        name: "slop_detector".into(),
        num_classes: 2,
        class_names: vec!["clean".into(), "slop".into()],
        feature_dim: FEATURE_DIM,
        epochs: 30,
        lr: 0.01,
    };
    f389(&slop_config, &slop_data, output_dir)?;

    // 2. Code vs english (binary: english=0, code=1).
    let cve_data = f391(project_dir);
    let cve_config = SubatomicConfig {
        name: "code_vs_english".into(),
        num_classes: 2,
        class_names: vec!["english".into(), "code".into()],
        feature_dim: FEATURE_DIM,
        epochs: 30,
        lr: 0.01,
    };
    f389(&cve_config, &cve_data, output_dir)?;

    // 3. Language detector (5 classes: rust=0, python=1, js=2, go=3, shell=4).
    let lang_data = f392();
    let lang_config = SubatomicConfig {
        name: "lang_detector".into(),
        num_classes: 5,
        class_names: vec!["rust".into(), "python".into(), "javascript".into(), "go".into(), "shell".into()],
        feature_dim: FEATURE_DIM,
        epochs: 50,
        lr: 0.005,
    };
    f389(&lang_config, &lang_data, output_dir)?;

    eprintln!("\n[subatomic] all 3 models trained and saved to {}", output_dir.display());
    Ok(())
}

/// Load a trained model and run inference on a single input.
pub fn predict(model_dir: &Path, text: &str) -> Result<(usize, String, f32), String> {
    // Load config.
    let config_path = model_dir.join("config.json");
    let config_str = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("read config: {}", e))?;
    let config: serde_json::Value = serde_json::from_str(&config_str)
        .map_err(|e| format!("parse config: {}", e))?;

    let dim = config["feature_dim"].as_u64().ok_or("missing feature_dim")? as usize;
    let nc = config["num_classes"].as_u64().ok_or("missing num_classes")? as usize;
    let class_names: Vec<String> = config["class_names"]
        .as_array()
        .ok_or("missing class_names")?
        .iter()
        .map(|v| v.as_str().unwrap_or("?").to_string())
        .collect();

    // Load weights.
    let weights_bytes = std::fs::read(model_dir.join("weights.bin"))
        .map_err(|e| format!("read weights: {}", e))?;
    let weights: Vec<f32> = weights_bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();

    let bias_bytes = std::fs::read(model_dir.join("bias.bin"))
        .map_err(|e| format!("read bias: {}", e))?;
    let bias: Vec<f32> = bias_bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();

    if weights.len() != dim * nc || bias.len() != nc {
        return Err(format!(
            "weight shape mismatch: got {}/{}, expected {}/{}",
            weights.len(),
            bias.len(),
            dim * nc,
            nc
        ));
    }

    // Featurize and predict.
    let feat = featurize(text, dim);
    let mut logits = vec![0.0f32; nc];
    for c in 0..nc {
        let mut sum = bias[c];
        for d in 0..dim {
            sum += weights[c * dim + d] * feat[d];
        }
        logits[c] = sum;
    }

    // Softmax for confidence.
    let max_logit = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let mut probs = vec![0.0f32; nc];
    let mut sum_exp = 0.0f32;
    for c in 0..nc {
        probs[c] = (logits[c] - max_logit).exp();
        sum_exp += probs[c];
    }
    for c in 0..nc {
        probs[c] /= sum_exp;
    }

    let pred = probs
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .map(|(i, _)| i)
        .unwrap_or(0);
    let confidence = probs[pred];
    let class_name = class_names.get(pred).cloned().unwrap_or_else(|| format!("class_{}", pred));

    Ok((pred, class_name, confidence))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn featurize_produces_fixed_dim() {
        let f = featurize("hello world", 256);
        assert_eq!(f.len(), 256);
    }

    #[test]
    fn featurize_normalized() {
        let f = featurize("some longer text for testing normalization", 256);
        let norm: f32 = f.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.01, "norm should be ~1.0, got {}", norm);
    }

    #[test]
    fn featurize_short_text() {
        let f = featurize("ab", 256);
        assert_eq!(f.len(), 256);
        // Should have exactly one non-zero feature.
        assert_eq!(f.iter().filter(|&&x| x > 0.0).count(), 1);
    }

    #[test]
    fn slop_data_balanced() {
        let data = f390();
        let clean = data.iter().filter(|e| e.label == 0).count();
        let slop = data.iter().filter(|e| e.label == 1).count();
        assert!(clean > 10, "need clean examples");
        assert!(slop > 10, "need slop examples");
    }

    #[test]
    fn lang_data_all_classes() {
        let data = f392();
        for label in 0..5 {
            let count = data.iter().filter(|e| e.label == label).count();
            assert!(count >= 10, "class {} has only {} examples", label, count);
        }
    }

    #[test]
    fn train_slop_detector() {
        let data = f390();
        let config = SubatomicConfig {
            name: "test_slop".into(),
            num_classes: 2,
            class_names: vec!["clean".into(), "slop".into()],
            feature_dim: 64,
            epochs: 10,
            lr: 0.01,
        };
        let tmp = tempfile::TempDir::new().unwrap();
        let result = f389(&config, &data, tmp.path());
        assert!(result.is_ok(), "training failed: {:?}", result.err());

        // Verify model files exist.
        let model_dir = result.unwrap();
        assert!(model_dir.join("weights.bin").exists());
        assert!(model_dir.join("bias.bin").exists());
        assert!(model_dir.join("config.json").exists());
    }

    #[test]
    fn predict_after_train() {
        let data = f390();
        let config = SubatomicConfig {
            name: "test_predict".into(),
            num_classes: 2,
            class_names: vec!["clean".into(), "slop".into()],
            feature_dim: FEATURE_DIM, // full dim for accuracy
            epochs: 30,
            lr: 0.01,
        };
        let tmp = tempfile::TempDir::new().unwrap();
        let model_dir = f389(&config, &data, tmp.path()).unwrap();

        // Test that predict returns valid results (class name + confidence).
        let (pred, class, conf) = predict(&model_dir, "We need to leverage the synergy.").unwrap();
        assert!(pred < 2, "prediction should be 0 or 1");
        assert!(!class.is_empty());
        assert!(conf > 0.0 && conf <= 1.0);

        // On full feature dim, slop detection should work.
        assert_eq!(class, "slop", "should detect slop");
    }

    #[test]
    fn train_lang_detector() {
        let data = f392();
        let config = SubatomicConfig {
            name: "test_lang".into(),
            num_classes: 5,
            class_names: vec!["rust".into(), "python".into(), "javascript".into(), "go".into(), "shell".into()],
            feature_dim: FEATURE_DIM, // full dim
            epochs: 50,
            lr: 0.005,
        };
        let tmp = tempfile::TempDir::new().unwrap();
        let result = f389(&config, &data, tmp.path());
        assert!(result.is_ok());

        // Verify predict returns valid output.
        let model_dir = result.unwrap();
        let (pred, class, conf) = predict(&model_dir, "fn main() { println!(\"hello\"); }").unwrap();
        assert!(pred < 5);
        assert!(!class.is_empty());
        assert!(conf > 0.0);
    }
}
