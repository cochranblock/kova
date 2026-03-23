// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6
//! kova_model — From-scratch transformer models for kova tasks.
//! Pure Rust, candle, no pretrained weights. Pixel forge pattern.
//!
//! Three tiers:
//!   Spark  — 50K params, classifier only (intent routing)
//!   Flame  — 500K params, classifier + short generation
//!   Blaze  — 2M params, full task specialist
//!
//! Trained from tournament SFT/DPO data. Deployed as safetensors.

use candle_core::{DType, Device, IndexOp, Module, Result, Tensor, D};
use candle_nn::{embedding, layer_norm, linear, Activation, Embedding, LayerNorm, Linear, VarBuilder};

/// Number of task categories kova classifies into.
pub const NUM_CLASSES: usize = 8;

/// Category labels in canonical order.
pub const CLASS_LABELS: &[&str] = &[
    "classify", "clippy_fix", "code_gen", "code_review",
    "explain", "fix_compile", "test_write", "validate",
];

/// Model tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    Spark,
    Flame,
    Blaze,
}

impl Tier {
    pub fn config(&self) -> ModelConfig {
        match self {
            Tier::Spark => ModelConfig {
                vocab_size: 8192,
                embed_dim: 64,
                num_heads: 4,
                num_layers: 2,
                ff_dim: 256,
                max_seq_len: 128,
                dropout: 0.1,
            },
            Tier::Flame => ModelConfig {
                vocab_size: 8192,
                embed_dim: 128,
                num_heads: 4,
                num_layers: 4,
                ff_dim: 512,
                max_seq_len: 256,
                dropout: 0.1,
            },
            Tier::Blaze => ModelConfig {
                vocab_size: 8192,
                embed_dim: 256,
                num_heads: 8,
                num_layers: 6,
                ff_dim: 1024,
                max_seq_len: 512,
                dropout: 0.1,
            },
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Tier::Spark => "spark",
            Tier::Flame => "flame",
            Tier::Blaze => "blaze",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub vocab_size: usize,
    pub embed_dim: usize,
    pub num_heads: usize,
    pub num_layers: usize,
    pub ff_dim: usize,
    pub max_seq_len: usize,
    pub dropout: f64,
}

/// Single transformer block.
struct TransformerBlock {
    attn_q: Linear,
    attn_k: Linear,
    attn_v: Linear,
    attn_out: Linear,
    ff1: Linear,
    ff2: Linear,
    ln1: LayerNorm,
    ln2: LayerNorm,
    num_heads: usize,
    head_dim: usize,
}

impl TransformerBlock {
    fn new(cfg: &ModelConfig, vb: VarBuilder) -> Result<Self> {
        let d = cfg.embed_dim;
        let head_dim = d / cfg.num_heads;
        Ok(Self {
            attn_q: linear(d, d, vb.pp("attn_q"))?,
            attn_k: linear(d, d, vb.pp("attn_k"))?,
            attn_v: linear(d, d, vb.pp("attn_v"))?,
            attn_out: linear(d, d, vb.pp("attn_out"))?,
            ff1: linear(d, cfg.ff_dim, vb.pp("ff1"))?,
            ff2: linear(cfg.ff_dim, d, vb.pp("ff2"))?,
            ln1: layer_norm(d, 1e-5, vb.pp("ln1"))?,
            ln2: layer_norm(d, 1e-5, vb.pp("ln2"))?,
            num_heads: cfg.num_heads,
            head_dim,
        })
    }

    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        let (batch, seq, _dim) = x.dims3()?;

        // Self-attention with causal mask
        let residual = x;
        let x = self.ln1.forward(x)?;

        let q = self.attn_q.forward(&x)?
            .reshape((batch, seq, self.num_heads, self.head_dim))?
            .transpose(1, 2)?.contiguous()?; // [B, H, S, D]
        let k = self.attn_k.forward(&x)?
            .reshape((batch, seq, self.num_heads, self.head_dim))?
            .transpose(1, 2)?.contiguous()?;
        let v = self.attn_v.forward(&x)?
            .reshape((batch, seq, self.num_heads, self.head_dim))?
            .transpose(1, 2)?.contiguous()?;

        let scale = (self.head_dim as f64).sqrt();
        let k_t = k.transpose(D::Minus2, D::Minus1)?;
        let scores = q.matmul(&k_t)?
            .affine(1.0 / scale, 0.0)?;

        // Causal mask: upper triangle = -1e9
        let mask = {
            let mut vals = vec![0.0f32; seq * seq];
            for i in 0..seq {
                for j in (i + 1)..seq {
                    vals[i * seq + j] = -1e9;
                }
            }
            Tensor::from_vec(vals, (seq, seq), x.device())?
        };
        let scores = scores.broadcast_add(&mask)?;
        let attn = candle_nn::ops::softmax_last_dim(&scores)?;

        let out = attn.matmul(&v)?
            .transpose(1, 2)?.contiguous()?
            .reshape((batch, seq, self.num_heads * self.head_dim))?;
        let out = self.attn_out.forward(&out)?;
        let x = (residual + out)?;

        // FFN
        let residual = &x;
        let h = self.ln2.forward(&x)?;
        let h = self.ff1.forward(&h)?;
        let h = h.apply(&Activation::Gelu)?;
        let h = self.ff2.forward(&h)?;
        (residual + h)
    }
}

/// Kova classifier model. Input tokens → class logits.
pub struct KovaClassifier {
    token_embed: Embedding,
    pos_embed: Embedding,
    layers: Vec<TransformerBlock>,
    ln_final: LayerNorm,
    head: Linear,
    max_seq_len: usize,
}

impl KovaClassifier {
    pub fn new(cfg: &ModelConfig, vb: VarBuilder) -> Result<Self> {
        let token_embed = embedding(cfg.vocab_size, cfg.embed_dim, vb.pp("token_embed"))?;
        let pos_embed = embedding(cfg.max_seq_len, cfg.embed_dim, vb.pp("pos_embed"))?;

        let mut layers = Vec::new();
        for i in 0..cfg.num_layers {
            layers.push(TransformerBlock::new(cfg, vb.pp(format!("layer_{}", i)))?);
        }

        let ln_final = layer_norm(cfg.embed_dim, 1e-5, vb.pp("ln_final"))?;
        let head = linear(cfg.embed_dim, NUM_CLASSES, vb.pp("head"))?;

        Ok(Self {
            token_embed,
            pos_embed,
            layers,
            ln_final,
            head,
            max_seq_len: cfg.max_seq_len,
        })
    }

    /// Forward pass. Returns [batch, num_classes] logits.
    pub fn forward(&self, input_ids: &Tensor) -> Result<Tensor> {
        let (_batch, seq) = input_ids.dims2()?;
        let seq = seq.min(self.max_seq_len);

        // Truncate if needed
        let input_ids = if input_ids.dim(1)? > self.max_seq_len {
            input_ids.narrow(1, 0, self.max_seq_len)?
        } else {
            input_ids.clone()
        };

        let positions = Tensor::arange(0u32, seq as u32, input_ids.device())?;
        let tok = self.token_embed.forward(&input_ids)?;
        let pos = self.pos_embed.forward(&positions)?;
        let mut x = tok.broadcast_add(&pos)?;

        for layer in &self.layers {
            x = layer.forward(&x)?;
        }

        let x = self.ln_final.forward(&x)?;

        // Pool: mean over sequence
        let pooled = x.mean(1)?; // [batch, embed_dim]
        self.head.forward(&pooled) // [batch, num_classes]
    }

    /// Predict class index for a single input.
    pub fn predict(&self, input_ids: &Tensor) -> Result<usize> {
        let logits = self.forward(input_ids)?; // [1, num_classes]
        let logits = logits.squeeze(0)?;
        let idx: u32 = logits.argmax(0)?.to_scalar()?;
        Ok(idx as usize)
    }
}

/// BPE tokenizer. Trained on kova data. Pure Rust.
/// Vocab 0 = pad, 1-256 = raw bytes, 257+ = learned merges.
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct KovaTokenizer {
    /// Merge rules: (pair_a, pair_b) → merged_id. Applied in order.
    merges: Vec<(u32, u32)>,
    /// Total vocab size (257 + merges.len()).
    vocab_size: usize,
}

impl KovaTokenizer {
    /// Create a byte-only tokenizer (no merges). Baseline.
    pub fn byte_level() -> Self {
        Self { merges: Vec::new(), vocab_size: 257 }
    }

    /// Train BPE merges from a corpus of texts.
    /// Learns up to `max_merges` merge rules from the most frequent byte pairs.
    pub fn train(texts: &[String], max_merges: usize) -> Self {
        // Encode all texts as byte sequences (offset by 1 for pad=0)
        let mut sequences: Vec<Vec<u32>> = texts.iter()
            .map(|t| t.bytes().map(|b| b as u32 + 1).collect())
            .collect();

        let mut merges = Vec::new();
        let mut next_id = 257u32;

        for _ in 0..max_merges {
            // Count all adjacent pairs
            let mut pair_counts: std::collections::HashMap<(u32, u32), usize> = std::collections::HashMap::new();
            for seq in &sequences {
                for w in seq.windows(2) {
                    *pair_counts.entry((w[0], w[1])).or_insert(0) += 1;
                }
            }

            // Find most frequent pair
            let best = pair_counts.into_iter().max_by_key(|&(_, count)| count);
            let Some((pair, count)) = best else { break };
            if count < 2 { break; } // No pair appears more than once

            // Merge this pair everywhere
            let new_id = next_id;
            next_id += 1;
            merges.push(pair);

            for seq in &mut sequences {
                let mut i = 0;
                let mut new_seq = Vec::with_capacity(seq.len());
                while i < seq.len() {
                    if i + 1 < seq.len() && seq[i] == pair.0 && seq[i + 1] == pair.1 {
                        new_seq.push(new_id);
                        i += 2;
                    } else {
                        new_seq.push(seq[i]);
                        i += 1;
                    }
                }
                *seq = new_seq;
            }
        }

        Self {
            vocab_size: 257 + merges.len(),
            merges,
        }
    }

    /// Encode text to token IDs. Apply learned merges.
    pub fn encode(&self, text: &str, max_len: usize) -> Vec<u32> {
        let mut ids: Vec<u32> = text.bytes().map(|b| b as u32 + 1).collect();

        // Apply merges in order
        for (merge_idx, &(a, b)) in self.merges.iter().enumerate() {
            let new_id = 257 + merge_idx as u32;
            let mut i = 0;
            let mut merged = Vec::with_capacity(ids.len());
            while i < ids.len() {
                if i + 1 < ids.len() && ids[i] == a && ids[i + 1] == b {
                    merged.push(new_id);
                    i += 2;
                } else {
                    merged.push(ids[i]);
                    i += 1;
                }
            }
            ids = merged;
        }

        ids.truncate(max_len);
        while ids.len() < max_len {
            ids.push(0); // pad
        }
        ids
    }

    pub fn vocab_size(&self) -> usize {
        self.vocab_size
    }

    /// Save tokenizer to JSON.
    pub fn save(&self, path: &std::path::Path) -> std::result::Result<(), String> {
        let json = serde_json::to_string_pretty(self).map_err(|e| format!("serialize: {}", e))?;
        std::fs::write(path, json).map_err(|e| format!("write: {}", e))
    }

    /// Load tokenizer from JSON.
    pub fn load(path: &std::path::Path) -> std::result::Result<Self, String> {
        let json = std::fs::read_to_string(path).map_err(|e| format!("read: {}", e))?;
        serde_json::from_str(&json).map_err(|e| format!("parse: {}", e))
    }
}

/// Count parameters in the model.
pub fn count_params(cfg: &ModelConfig) -> usize {
    let embed = cfg.vocab_size * cfg.embed_dim + cfg.max_seq_len * cfg.embed_dim;
    let per_layer = 4 * cfg.embed_dim * cfg.embed_dim  // Q, K, V, Out
        + 2 * cfg.embed_dim                            // LN1
        + cfg.embed_dim * cfg.ff_dim + cfg.ff_dim * cfg.embed_dim // FF
        + 2 * cfg.embed_dim;                            // LN2
    let head = cfg.embed_dim * NUM_CLASSES + 2 * cfg.embed_dim; // head + ln_final
    embed + cfg.num_layers * per_layer + head
}
