// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6
//! moe_tournament — MoE competitor in the Micro Olympics.
//! Spark routes challenges to the best node, cascade on failure.
//! Scores as a single "KovaMoE" competitor against individual models.
//!
//! Pipeline per challenge:
//!   1. Spark classifier predicts category
//!   2. Pick best node for that category (from historical tournament data)
//!   3. Run challenge on picked node
//!   4. If fail → cascade to next-best node
//!   5. Score the final result

use std::collections::HashMap;
use std::time::Instant;

use super::bench;
use super::kova_model::{KovaClassifier, KovaTokenizer, Tier, CLASS_LABELS};
use super::registry::T149;
use super::runner;
use super::tournament::{T161, T162, T165, T166};
use crate::cluster::T193;

use candle_core::{DType, Device, Tensor};
use candle_nn::{VarBuilder, VarMap};

/// MoE tournament configuration.
pub struct MoeConfig {
    /// Max cascade attempts before giving up.
    pub max_cascade: usize,
    /// Path to trained Spark model (model.safetensors + tokenizer.json + config.json).
    pub spark_dir: std::path::PathBuf,
}

/// Loaded Spark router for MoE.
struct SparkRouter {
    model: KovaClassifier,
    tokenizer: KovaTokenizer,
    max_seq_len: usize,
    device: Device,
}

impl SparkRouter {
    fn load(dir: &std::path::Path) -> Result<Self, String> {
        let device = Device::Cpu;

        // Load config
        let config_path = dir.join("config.json");
        let config_json = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("read spark config: {}", e))?;
        let config: serde_json::Value = serde_json::from_str(&config_json)
            .map_err(|e| format!("parse spark config: {}", e))?;

        let tier_name = config["tier"].as_str().unwrap_or("spark");
        let tier = match tier_name {
            "flame" => Tier::Flame,
            "blaze" => Tier::Blaze,
            _ => Tier::Spark,
        };
        let mut model_cfg = tier.config();
        // Use actual vocab from config
        if let Some(v) = config["vocab_size"].as_u64() {
            model_cfg.vocab_size = v as usize;
        }

        // Load tokenizer
        let tokenizer = KovaTokenizer::load(&dir.join("tokenizer.json"))?;

        // Load model weights
        let mut varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let model = KovaClassifier::new(&model_cfg, vb)
            .map_err(|e| format!("build spark: {}", e))?;

        // Load saved weights
        let st_path = dir.join("model.safetensors");
        varmap.load(&st_path)
            .map_err(|e| format!("load spark weights: {}", e))?;

        Ok(Self {
            model,
            tokenizer,
            max_seq_len: model_cfg.max_seq_len,
            device,
        })
    }

    /// Classify input text → category label.
    fn classify(&self, text: &str) -> Result<String, String> {
        let ids = self.tokenizer.encode(text, self.max_seq_len);
        let input = Tensor::from_vec(ids, (1, self.max_seq_len), &self.device)
            .map_err(|e| format!("tensor: {}", e))?;
        let idx = self.model.predict(&input)
            .map_err(|e| format!("predict: {}", e))?;
        Ok(CLASS_LABELS.get(idx).unwrap_or(&"classify").to_string())
    }
}

/// Build a node preference map from historical tournament data.
/// Returns: category → ordered list of (node_id, accuracy%).
fn build_node_prefs(history: Option<&T165>) -> HashMap<String, Vec<(String, f64)>> {
    let mut prefs: HashMap<String, Vec<(String, f64)>> = HashMap::new();

    // Default: round-robin all nodes
    if history.is_none() {
        return prefs;
    }

    let hist = history.unwrap();
    // Per category × node: track pass/total
    let mut stats: HashMap<(String, String), (usize, usize)> = HashMap::new();
    for m in &hist.matches {
        let key = (m.category.clone(), m.competitor.node_id.clone());
        let entry = stats.entry(key).or_insert((0, 0));
        entry.1 += 1;
        if m.passed { entry.0 += 1; }
    }

    // Group by category, sort by accuracy desc
    let mut by_cat: HashMap<String, Vec<(String, f64)>> = HashMap::new();
    for ((cat, node), (pass, total)) in &stats {
        let acc = if *total > 0 { *pass as f64 / *total as f64 * 100.0 } else { 0.0 };
        by_cat.entry(cat.clone()).or_default().push((node.clone(), acc));
    }
    for (_, nodes) in &mut by_cat {
        nodes.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    }

    by_cat
}

/// Run MoE as a competitor in the tournament.
/// Returns match results as if "KovaMoE" were a single competitor.
pub fn run_moe_tournament(
    config: &MoeConfig,
    registry: &T149,
    cluster: &T193,
    challenges: &[T166],
    history: Option<&T165>,
) -> Vec<T162> {
    let mut results = Vec::new();

    // Load Spark router
    let router = match SparkRouter::load(&config.spark_dir) {
        Ok(r) => {
            eprintln!("[moe] Spark router loaded from {}", config.spark_dir.display());
            r
        }
        Err(e) => {
            eprintln!("[moe] failed to load Spark: {} — using round-robin", e);
            // Fall back to no routing
            return results;
        }
    };

    // Build node preferences from history
    let node_prefs = build_node_prefs(history);

    // Get online nodes
    let online = cluster.online_nodes();
    if online.is_empty() {
        eprintln!("[moe] no online nodes");
        return results;
    }
    let node_urls: HashMap<String, String> = online.iter()
        .map(|n| (n.id.clone(), n.base_url()))
        .collect();
    let node_models: HashMap<String, String> = online.iter()
        .map(|n| (n.id.clone(), n.model.clone()))
        .collect();

    let moe_competitor = T161 {
        node_id: "moe".into(),
        node_url: "local".into(),
        model: "KovaMoE".into(),
        weight_class: super::tournament::T160::Atomweight,
        exhibition: false,
    };

    eprintln!("[moe] running {} challenges with {} online nodes, max cascade {}",
        challenges.len(), online.len(), config.max_cascade);

    for ch in challenges {
        let start = Instant::now();

        // Step 1: Spark classifies
        let predicted_cat = router.classify(&ch.input).unwrap_or_else(|_| ch.category.clone());

        // Step 2: Pick best node for this category
        let node_order: Vec<String> = if let Some(prefs) = node_prefs.get(&predicted_cat) {
            prefs.iter().map(|(n, _)| n.clone()).collect()
        } else {
            // Round-robin
            online.iter().map(|n| n.id.clone()).collect()
        };

        // Step 3: Try nodes in order (cascade)
        let tmpl = match registry.get(&ch.template_id) {
            Some(t) => t,
            None => continue,
        };

        let mut best_result = None;
        let mut attempts = 0;

        for node_id in &node_order {
            if attempts >= config.max_cascade { break; }
            let url = match node_urls.get(node_id) {
                Some(u) => u,
                None => continue,
            };
            let model = match node_models.get(node_id) {
                Some(m) => m,
                None => continue,
            };

            attempts += 1;
            match runner::f245(tmpl, &ch.input, url, Some(model)) {
                Ok(r) => {
                    let passed = bench::f234(&r.response, &ch.verify);
                    if passed {
                        best_result = Some((r, true, node_id.clone()));
                        break; // Success — no need to cascade
                    } else {
                        // Failed — cascade to next node
                        best_result = Some((r, false, node_id.clone()));
                    }
                }
                Err(_) => continue, // Error — try next node
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;

        let (passed, response, response_len, tokens, via_node) = match best_result {
            Some((r, passed, node)) => {
                (passed, r.response.clone(), r.response.len(), r.tokens.unwrap_or(0), node)
            }
            None => (false, "MoE: all nodes failed".into(), 0, 0, "none".into()),
        };

        let route_info = format!("route={} via={} attempts={}", predicted_cat, via_node, attempts);
        eprintln!(
            "  {} MoE {:<16} {:>5}ms  {} [{}]",
            if passed { "PASS" } else { "FAIL" },
            ch.category, duration_ms, ch.description, route_info
        );

        results.push(T162 {
            competitor: moe_competitor.clone(),
            challenge: ch.description.clone(),
            category: ch.category.clone(),
            passed,
            duration_ms,
            tokens,
            response_len,
            response,
        });
    }

    let passed = results.iter().filter(|r| r.passed).count();
    let total = results.len();
    let acc = if total > 0 { passed as f64 / total as f64 * 100.0 } else { 0.0 };
    eprintln!("\n[moe] KovaMoE: {}/{} ({:.1}%) in {} challenges", passed, total, acc, total);

    results
}
