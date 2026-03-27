// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6
//! quantize — TurboQuant-inspired weight quantization for kova models.
//!
//! Techniques from vllm-turboquant adapted for post-training weight compression:
//!   - Fast Walsh-Hadamard Transform (FWHT): rotate weights to spread energy
//!   - Mixed-precision outlier/inlier split: more bits for high-norm rows
//!   - QJL residual recovery: 1-bit sign projections recover quantization error direction
//!
//! Target: compress Spark (50K params, ~200 KB FP32) to ~15 KB at 2.5 bits/weight.

use candle_core::{DType, Device, Tensor};
use std::path::Path;

// ── Fast Walsh-Hadamard Transform ──────────────────────────────

/// Apply in-place Fast Walsh-Hadamard Transform to a vector.
/// Input length must be a power of 2. If not, pad to next power of 2.
/// Normalizes by 1/sqrt(n) for orthogonality.
/// f366=fwht
pub fn f366(data: &mut [f32]) {
    let n = data.len();
    assert!(n.is_power_of_two(), "FWHT requires power-of-2 length, got {}", n);

    let mut h = 1;
    while h < n {
        for i in (0..n).step_by(h * 2) {
            for j in i..(i + h) {
                let x = data[j];
                let y = data[j + h];
                data[j] = x + y;
                data[j + h] = x - y;
            }
        }
        h *= 2;
    }

    // Normalize
    let scale = 1.0 / (n as f32).sqrt();
    for v in data.iter_mut() {
        *v *= scale;
    }
}

/// Inverse FWHT (same as forward for orthogonal Hadamard).
/// f367=ifwht
pub fn f367(data: &mut [f32]) {
    f366(data);
}

/// Apply FWHT with deterministic sign flipping (randomized Hadamard).
/// seed controls the sign pattern — different seeds give different rotations.
/// f368=fwht_signed
pub fn f368(data: &mut [f32], seed: u64) {
    let n = data.len();
    // Deterministic sign flip based on seed
    let mut rng = seed;
    for v in data.iter_mut() {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        if rng & 1 == 1 {
            *v = -*v;
        }
    }
    f366(data);
}

/// Inverse signed FWHT: undo FWHT then undo sign flips.
/// f369=ifwht_signed
pub fn f369(data: &mut [f32], seed: u64) {
    f367(data);
    // Undo sign flips (same pattern, applied in reverse order doesn't matter for signs)
    let n = data.len();
    let mut rng = seed;
    for v in data.iter_mut() {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        if rng & 1 == 1 {
            *v = -*v;
        }
    }
}

/// Pad a vector to the next power of 2, returning (padded_vec, original_len).
/// f370=pad_to_pow2
pub fn f370(data: &[f32]) -> (Vec<f32>, usize) {
    let n = data.len();
    let target = n.next_power_of_two();
    let mut padded = data.to_vec();
    padded.resize(target, 0.0);
    (padded, n)
}

// ── Mixed-Precision Quantization ───────────────────────────────

/// Quantize a weight vector to n_bits using uniform quantization.
/// Returns (quantized_indices, scale, zero_point).
fn uniform_quantize(data: &[f32], n_bits: u8) -> (Vec<u8>, f32, f32) {
    let levels = (1u32 << n_bits) - 1;
    let min_val = data.iter().cloned().fold(f32::INFINITY, f32::min);
    let max_val = data.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let range = max_val - min_val;
    let scale = if range > 0.0 { range / levels as f32 } else { 1.0 };

    let indices: Vec<u8> = data.iter().map(|&v| {
        let q = ((v - min_val) / scale).round() as u32;
        q.min(levels) as u8
    }).collect();

    (indices, scale, min_val)
}

/// Dequantize indices back to f32.
fn uniform_dequantize(indices: &[u8], scale: f32, zero_point: f32) -> Vec<f32> {
    indices.iter().map(|&i| i as f32 * scale + zero_point).collect()
}

/// Per-row L2 norms for a 2D weight matrix.
fn row_l2_norms(weights: &[f32], rows: usize, cols: usize) -> Vec<f32> {
    (0..rows).map(|r| {
        let start = r * cols;
        let end = start + cols;
        weights[start..end].iter().map(|v| v * v).sum::<f32>().sqrt()
    }).collect()
}

/// Split rows into outlier (high-norm) and inlier (low-norm) groups.
/// Returns (outlier_indices, inlier_indices).
fn split_outlier_inlier(norms: &[f32], outlier_frac: f32) -> (Vec<usize>, Vec<usize>) {
    let mut sorted_idx: Vec<usize> = (0..norms.len()).collect();
    sorted_idx.sort_by(|&a, &b| norms[b].partial_cmp(&norms[a]).unwrap_or(std::cmp::Ordering::Equal));

    let n_outlier = ((norms.len() as f32 * outlier_frac).ceil() as usize).max(1);
    let outliers = sorted_idx[..n_outlier].to_vec();
    let inliers = sorted_idx[n_outlier..].to_vec();
    (outliers, inliers)
}

/// Quantized weight layer with mixed precision.
#[derive(serde::Serialize, serde::Deserialize)]
/// T214=T214
pub struct T214 {
    /// Layer name (for matching during load).
    pub name: String,
    /// Original shape (rows, cols).
    pub shape: (usize, usize),
    /// Outlier row indices.
    pub outlier_rows: Vec<usize>,
    /// Outlier quantized data (4-bit indices, packed 2 per byte).
    pub outlier_data: Vec<u8>,
    pub outlier_scale: f32,
    pub outlier_zero: f32,
    /// Inlier quantized data (2-bit indices, packed 4 per byte).
    pub inlier_data: Vec<u8>,
    pub inlier_scale: f32,
    pub inlier_zero: f32,
    /// QJL residual signs (1 bit per element, packed 8 per byte).
    pub residual_signs: Vec<u8>,
    /// Per-row residual norms.
    pub residual_norms: Vec<f32>,
    /// Hadamard seed used for pre-rotation.
    pub hadamard_seed: u64,
}

/// Pack 4-bit indices (2 per byte).
fn pack_4bit(indices: &[u8]) -> Vec<u8> {
    indices.chunks(2).map(|pair| {
        let lo = pair[0] & 0x0F;
        let hi = if pair.len() > 1 { pair[1] & 0x0F } else { 0 };
        lo | (hi << 4)
    }).collect()
}

/// Unpack 4-bit indices.
fn unpack_4bit(packed: &[u8], count: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(count);
    for &byte in packed {
        out.push(byte & 0x0F);
        if out.len() < count {
            out.push((byte >> 4) & 0x0F);
        }
    }
    out.truncate(count);
    out
}

/// Pack 2-bit indices (4 per byte).
fn pack_2bit(indices: &[u8]) -> Vec<u8> {
    indices.chunks(4).map(|group| {
        let mut byte = 0u8;
        for (i, &val) in group.iter().enumerate() {
            byte |= (val & 0x03) << (i * 2);
        }
        byte
    }).collect()
}

/// Unpack 2-bit indices.
fn unpack_2bit(packed: &[u8], count: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(count);
    for &byte in packed {
        for i in 0..4 {
            out.push((byte >> (i * 2)) & 0x03);
            if out.len() >= count { break; }
        }
    }
    out.truncate(count);
    out
}

/// Pack sign bits (8 per byte). 1 = negative, 0 = non-negative.
fn pack_signs(data: &[f32]) -> Vec<u8> {
    data.chunks(8).map(|group| {
        let mut byte = 0u8;
        for (i, &val) in group.iter().enumerate() {
            if val < 0.0 {
                byte |= 1 << i;
            }
        }
        byte
    }).collect()
}

/// Unpack sign bits → +1.0 or -1.0.
fn unpack_signs(packed: &[u8], count: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(count);
    for &byte in packed {
        for i in 0..8 {
            out.push(if byte & (1 << i) != 0 { -1.0 } else { 1.0 });
            if out.len() >= count { break; }
        }
    }
    out.truncate(count);
    out
}

// ── QJL Residual Recovery ──────────────────────────────────────

/// Compute QJL residual encoding for a row.
/// After quantizing, the residual = original - dequantized.
/// Hadamard-transform the residual with a second seed, store only signs.
/// On dequant: reconstruct residual direction from signs, scale by stored norm.
fn encode_qjl_residual(original: &[f32], dequantized: &[f32], seed: u64) -> (Vec<u8>, f32) {
    let residual: Vec<f32> = original.iter().zip(dequantized.iter())
        .map(|(o, d)| o - d)
        .collect();

    let norm = residual.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm < 1e-10 {
        return (vec![0u8; (original.len() + 7) / 8], 0.0);
    }

    // Hadamard-transform the residual
    let (mut padded, orig_len) = f370(&residual);
    f368(&mut padded, seed);

    // Store only signs
    let signs = pack_signs(&padded[..orig_len]);
    (signs, norm)
}

/// Decode QJL residual: signs → Hadamard inverse → scaled residual approximation.
fn decode_qjl_residual(signs: &[u8], norm: f32, dim: usize, seed: u64) -> Vec<f32> {
    if norm < 1e-10 {
        return vec![0.0; dim];
    }

    let sign_vals = unpack_signs(signs, dim);
    let (mut padded, _) = f370(&sign_vals);
    f369(&mut padded, seed);

    // Scale: sqrt(pi/2) / dim * norm (JL scaling)
    let scale = (std::f32::consts::FRAC_PI_2).sqrt() / dim as f32 * norm;
    padded.iter().take(dim).map(|v| v * scale).collect()
}

// ── Full Layer Quantization ────────────────────────────────────

/// Quantize a weight matrix with mixed-precision + QJL residual.
/// outlier_frac: fraction of rows treated as outliers (e.g. 0.25).
/// outlier_bits: bits for outlier rows (e.g. 4).
/// inlier_bits: bits for inlier rows (e.g. 2).
/// f371=quantize_layer
pub fn f371(
    name: &str,
    weights: &[f32],
    rows: usize,
    cols: usize,
    outlier_frac: f32,
    outlier_bits: u8,
    inlier_bits: u8,
    hadamard_seed: u64,
) -> T214 {
    let qjl_seed = hadamard_seed.wrapping_add(0xDEAD_BEEF);

    // Step 1: Hadamard pre-rotation per row
    let mut rotated = weights.to_vec();
    let pad_cols = cols.next_power_of_two();
    let mut row_buf = vec![0.0f32; pad_cols];
    for r in 0..rows {
        let start = r * cols;
        row_buf[..cols].copy_from_slice(&rotated[start..start + cols]);
        row_buf[cols..].fill(0.0);
        f368(&mut row_buf, hadamard_seed);
        rotated[start..start + cols].copy_from_slice(&row_buf[..cols]);
    }

    // Step 2: Split outlier/inlier by row norms
    let norms = row_l2_norms(&rotated, rows, cols);
    let (outlier_idx, inlier_idx) = split_outlier_inlier(&norms, outlier_frac);

    // Step 3: Quantize outlier rows (more bits)
    let outlier_flat: Vec<f32> = outlier_idx.iter()
        .flat_map(|&r| rotated[r * cols..(r + 1) * cols].iter().cloned())
        .collect();
    let (outlier_q, outlier_scale, outlier_zero) = uniform_quantize(&outlier_flat, outlier_bits);
    let outlier_packed = pack_4bit(&outlier_q);

    // Step 4: Quantize inlier rows (fewer bits)
    let inlier_flat: Vec<f32> = inlier_idx.iter()
        .flat_map(|&r| rotated[r * cols..(r + 1) * cols].iter().cloned())
        .collect();
    let (inlier_q, inlier_scale, inlier_zero) = uniform_quantize(&inlier_flat, inlier_bits);
    let inlier_packed = pack_2bit(&inlier_q);

    // Step 5: QJL residual for all rows
    let outlier_deq = uniform_dequantize(&outlier_q, outlier_scale, outlier_zero);
    let inlier_deq = uniform_dequantize(&inlier_q, inlier_scale, inlier_zero);

    let mut all_signs = Vec::new();
    let mut all_norms = Vec::with_capacity(rows);

    // Process outlier rows
    for (i, &r) in outlier_idx.iter().enumerate() {
        let orig = &rotated[r * cols..(r + 1) * cols];
        let deq = &outlier_deq[i * cols..(i + 1) * cols];
        let (signs, norm) = encode_qjl_residual(orig, deq, qjl_seed);
        all_signs.extend_from_slice(&signs);
        all_norms.push(norm);
    }
    // Process inlier rows
    for (i, &r) in inlier_idx.iter().enumerate() {
        let orig = &rotated[r * cols..(r + 1) * cols];
        let deq = &inlier_deq[i * cols..(i + 1) * cols];
        let (signs, norm) = encode_qjl_residual(orig, deq, qjl_seed);
        all_signs.extend_from_slice(&signs);
        all_norms.push(norm);
    }

    T214 {
        name: name.to_string(),
        shape: (rows, cols),
        outlier_rows: outlier_idx,
        outlier_data: outlier_packed,
        outlier_scale,
        outlier_zero,
        inlier_data: inlier_packed,
        inlier_scale,
        inlier_zero,
        residual_signs: all_signs,
        residual_norms: all_norms,
        hadamard_seed,
    }
}

/// Dequantize a layer back to f32 weights.
/// f372=dequantize_layer
pub fn f372(layer: &T214) -> Vec<f32> {
    let (rows, cols) = layer.shape;
    let qjl_seed = layer.hadamard_seed.wrapping_add(0xDEAD_BEEF);
    let pad_cols = cols.next_power_of_two();

    let n_outlier = layer.outlier_rows.len();
    let n_inlier = rows - n_outlier;

    // Unpack quantized data
    let outlier_q = unpack_4bit(&layer.outlier_data, n_outlier * cols);
    let inlier_q = unpack_2bit(&layer.inlier_data, n_inlier * cols);
    let outlier_deq = uniform_dequantize(&outlier_q, layer.outlier_scale, layer.outlier_zero);
    let inlier_deq = uniform_dequantize(&inlier_q, layer.inlier_scale, layer.inlier_zero);

    // Reconstruct with QJL residuals
    let mut result = vec![0.0f32; rows * cols];
    let signs_per_row = (cols + 7) / 8;

    // Outlier rows
    for (i, &r) in layer.outlier_rows.iter().enumerate() {
        let base = &outlier_deq[i * cols..(i + 1) * cols];
        let sign_start = i * signs_per_row;
        let signs = &layer.residual_signs[sign_start..sign_start + signs_per_row];
        let norm = layer.residual_norms[i];
        let residual = decode_qjl_residual(signs, norm, cols, qjl_seed);

        let start = r * cols;
        for j in 0..cols {
            result[start + j] = base[j] + residual[j];
        }
    }

    // Inlier rows (indices not in outlier_rows)
    let outlier_set: std::collections::HashSet<usize> = layer.outlier_rows.iter().cloned().collect();
    let inlier_idx: Vec<usize> = (0..rows).filter(|r| !outlier_set.contains(r)).collect();
    for (i, &r) in inlier_idx.iter().enumerate() {
        let base = &inlier_deq[i * cols..(i + 1) * cols];
        let sign_start = (n_outlier + i) * signs_per_row;
        let signs = &layer.residual_signs[sign_start..sign_start + signs_per_row];
        let norm = layer.residual_norms[n_outlier + i];
        let residual = decode_qjl_residual(signs, norm, cols, qjl_seed);

        let start = r * cols;
        for j in 0..cols {
            result[start + j] = base[j] + residual[j];
        }
    }

    // Inverse Hadamard rotation per row
    let mut row_buf = vec![0.0f32; pad_cols];
    for r in 0..rows {
        let start = r * cols;
        row_buf[..cols].copy_from_slice(&result[start..start + cols]);
        row_buf[cols..].fill(0.0);
        f369(&mut row_buf, layer.hadamard_seed);
        result[start..start + cols].copy_from_slice(&row_buf[..cols]);
    }

    result
}

// ── Model Quantization ─────────────────────────────────────────

/// Quantized model: all layers packed.
#[derive(serde::Serialize, serde::Deserialize)]
/// T215=T215
pub struct T215 {
    pub layers: Vec<T214>,
    pub metadata: serde_json::Value,
}

/// Compute total size in bytes of a quantized model.
/// f373=model_size_bytes
pub fn f373(model: &T215) -> usize {
    model.layers.iter().map(|l| {
        l.outlier_data.len()
            + l.inlier_data.len()
            + l.residual_signs.len()
            + l.residual_norms.len() * 4 // f32
            + l.outlier_rows.len() * 8 // usize
            + 8 + 4 + 4 // scales, zeros
    }).sum()
}

/// Compute effective bits per weight.
/// f374=bits_per_weight
pub fn f374(model: &T215) -> f64 {
    let total_weights: usize = model.layers.iter()
        .map(|l| l.shape.0 * l.shape.1)
        .sum();
    let total_bits: usize = model.layers.iter().map(|l| {
        let (rows, cols) = l.shape;
        let n_outlier = l.outlier_rows.len();
        let n_inlier = rows - n_outlier;
        // Outlier: 4 bits/weight, Inlier: 2 bits/weight, QJL: 1 bit/weight
        n_outlier * cols * 4 + n_inlier * cols * 2 + rows * cols * 1
    }).sum();
    total_bits as f64 / total_weights as f64
}

/// Save quantized model to binary file.
/// f375=save_quantized
pub fn f375(model: &T215, path: &Path) -> Result<(), String> {
    let json = serde_json::to_vec(model).map_err(|e| format!("serialize: {}", e))?;
    std::fs::write(path, &json).map_err(|e| format!("write: {}", e))
}

/// Load quantized model from binary file.
/// f376=load_quantized
pub fn f376(path: &Path) -> Result<T215, String> {
    let data = std::fs::read(path).map_err(|e| format!("read: {}", e))?;
    serde_json::from_slice(&data).map_err(|e| format!("parse: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fwht_roundtrip() {
        let mut data = vec![1.0, 2.0, 3.0, 4.0];
        let original = data.clone();
        f366(&mut data);
        // After FWHT, data should be different
        assert_ne!(data, original);
        // Inverse should recover original
        f367(&mut data);
        for (a, b) in data.iter().zip(original.iter()) {
            assert!((a - b).abs() < 1e-5, "FWHT roundtrip failed: {} vs {}", a, b);
        }
    }

    #[test]
    fn test_fwht_signed_roundtrip() {
        let mut data = vec![1.0, -0.5, 3.0, -2.0, 0.1, 0.2, 0.3, 0.4];
        let original = data.clone();
        let seed = 42;
        f368(&mut data, seed);
        f369(&mut data, seed);
        for (a, b) in data.iter().zip(original.iter()) {
            assert!((a - b).abs() < 1e-5, "signed FWHT roundtrip failed: {} vs {}", a, b);
        }
    }

    #[test]
    fn test_pack_unpack_4bit() {
        let indices = vec![3, 15, 7, 0, 12];
        let packed = pack_4bit(&indices);
        let unpacked = unpack_4bit(&packed, indices.len());
        assert_eq!(unpacked, indices);
    }

    #[test]
    fn test_pack_unpack_2bit() {
        let indices = vec![0, 1, 2, 3, 1, 0];
        let packed = pack_2bit(&indices);
        let unpacked = unpack_2bit(&packed, indices.len());
        assert_eq!(unpacked, indices);
    }

    #[test]
    fn test_quantize_dequantize_layer() {
        // 4 rows x 8 cols (power of 2 for FWHT)
        let weights: Vec<f32> = (0..32).map(|i| (i as f32 - 16.0) * 0.1).collect();
        let layer = f371("test", &weights, 4, 8, 0.25, 4, 2, 42);

        assert_eq!(layer.shape, (4, 8));
        assert_eq!(layer.outlier_rows.len(), 1); // 25% of 4 = 1

        let recovered = f372(&layer);
        assert_eq!(recovered.len(), weights.len());

        // Check that dequantized weights are reasonably close
        let mse: f32 = weights.iter().zip(recovered.iter())
            .map(|(a, b)| (a - b) * (a - b))
            .sum::<f32>() / weights.len() as f32;
        eprintln!("quantize roundtrip MSE: {:.6}", mse);
        // At 2-4 bits, expect some error but not huge
        assert!(mse < 1.0, "MSE too high: {}", mse);
    }

    #[test]
    fn test_sign_pack_roundtrip() {
        let data = vec![1.0, -2.0, 0.5, -0.1, 3.0, -4.0, 0.0, -1.0, 2.0];
        let packed = pack_signs(&data);
        let unpacked = unpack_signs(&packed, data.len());
        for (orig, sign) in data.iter().zip(unpacked.iter()) {
            if *orig < 0.0 {
                assert_eq!(*sign, -1.0);
            } else {
                assert_eq!(*sign, 1.0);
            }
        }
    }
}
