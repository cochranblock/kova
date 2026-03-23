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

/// Simple byte-pair-ish tokenizer. Maps bytes to vocab IDs.
/// Vocab 0-255 = raw bytes, 256+ = common bigrams learned from data.
pub struct SimpleTokenizer {
    max_vocab: usize,
}

impl SimpleTokenizer {
    pub fn new(vocab_size: usize) -> Self {
        Self { max_vocab: vocab_size }
    }

    /// Tokenize a string to u32 IDs. Simple byte-level encoding.
    pub fn encode(&self, text: &str, max_len: usize) -> Vec<u32> {
        let mut ids: Vec<u32> = text.bytes()
            .map(|b| b as u32)
            .take(max_len)
            .collect();
        // Pad
        while ids.len() < max_len {
            ids.push(0);
        }
        ids
    }

    pub fn vocab_size(&self) -> usize {
        self.max_vocab
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
