//! pack-starter — consolidate trained subatomic models into assets/starter.nanobyte.
//!
//! Reads each model dir under assets/models/{name}/{config.json,weights.bin,bias.bin},
//! concatenates `[W | b]` per model into one f32 blob, and writes a single .nanobyte
//! file. The packed weights blob length per model is `num_classes * feature_dim + num_classes`.
//! Reader splits via `feature_dim` and `num_classes` from the manifest.
//!
//! Run:
//!   cargo run --bin pack-starter
//
// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6

use std::path::{Path, PathBuf};

use kova::nanobyte::{self, Nanobyte, PackInput};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct ModelConfig {
    name: String,
    feature_dim: u32,
    num_classes: u32,
    #[serde(default)]
    total_params: Option<u64>,
}

const MODELS: &[&str] = &[
    "slop_detector",
    "code_vs_english",
    "lang_detector",
    "intent_classifier",
];

fn main() {
    if let Err(e) = run() {
        eprintln!("pack-starter: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let project = Path::new(env!("CARGO_MANIFEST_DIR"));
    let assets = project.join("assets");
    let out = assets.join("starter.nanobyte");

    let mut owned: Vec<(ModelConfig, Vec<f32>)> = Vec::with_capacity(MODELS.len());
    for name in MODELS {
        let dir = assets.join("models").join(name);
        let (cfg, packed) = load_model(&dir).map_err(|e| format!("load {name}: {e}"))?;
        if cfg.name != *name {
            return Err(format!(
                "{name}: config.json name field is {:?}, expected {:?}",
                cfg.name, name
            ));
        }
        owned.push((cfg, packed));
    }

    let inputs: Vec<PackInput<'_>> = owned
        .iter()
        .map(|(cfg, packed)| PackInput {
            name: &cfg.name,
            tier: 1,
            num_classes: cfg.num_classes,
            feature_dim: cfg.feature_dim,
            weights: packed,
            routing: None,
        })
        .collect();

    nanobyte::consolidate(&inputs, &out).map_err(|e| format!("consolidate: {e}"))?;

    let nb = Nanobyte::load(&out).map_err(|e| format!("load roundtrip: {e}"))?;

    let file_size = std::fs::metadata(&out)
        .map_err(|e| e.to_string())?
        .len();
    println!(
        "wrote {} ({} bytes, {} models)",
        out.display(),
        file_size,
        nb.manifests().len()
    );
    for m in nb.manifests() {
        let w = nb
            .weights(&m.name)
            .map_err(|e| format!("read {}: {e}", m.name))?;
        let expected_len = (m.num_classes as usize) * (m.feature_dim as usize)
            + m.num_classes as usize;
        if w.len() != expected_len {
            return Err(format!(
                "{}: packed len {} != expected {}",
                m.name,
                w.len(),
                expected_len
            ));
        }
        println!(
            "  {:24} tier={} classes={} feat={} params={}",
            m.name, m.tier, m.num_classes, m.feature_dim, expected_len
        );
    }

    Ok(())
}

fn load_model(dir: &Path) -> Result<(ModelConfig, Vec<f32>), String> {
    let cfg_path = dir.join("config.json");
    let cfg_bytes = std::fs::read(&cfg_path)
        .map_err(|e| format!("read {}: {e}", cfg_path.display()))?;
    let cfg: ModelConfig = serde_json::from_slice(&cfg_bytes)
        .map_err(|e| format!("parse {}: {e}", cfg_path.display()))?;

    let weights = read_f32_le(&dir.join("weights.bin"))?;
    let bias = read_f32_le(&dir.join("bias.bin"))?;

    let nc = cfg.num_classes as usize;
    let fd = cfg.feature_dim as usize;
    if weights.len() != nc * fd {
        return Err(format!(
            "weights.bin len {} != num_classes * feature_dim ({} * {} = {})",
            weights.len(),
            nc,
            fd,
            nc * fd
        ));
    }
    if bias.len() != nc {
        return Err(format!(
            "bias.bin len {} != num_classes ({})",
            bias.len(),
            nc
        ));
    }
    if let Some(total) = cfg.total_params
        && total as usize != weights.len() + bias.len()
    {
        return Err(format!(
            "total_params {total} != weights ({}) + bias ({}) = {}",
            weights.len(),
            bias.len(),
            weights.len() + bias.len()
        ));
    }

    let mut packed = Vec::with_capacity(weights.len() + bias.len());
    packed.extend_from_slice(&weights);
    packed.extend_from_slice(&bias);
    Ok((cfg, packed))
}

fn read_f32_le(path: &PathBuf) -> Result<Vec<f32>, String> {
    let bytes =
        std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    if !bytes.len().is_multiple_of(4) {
        return Err(format!(
            "{}: length {} not divisible by 4",
            path.display(),
            bytes.len()
        ));
    }
    Ok(bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect())
}
