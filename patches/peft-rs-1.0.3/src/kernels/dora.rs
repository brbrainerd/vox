//! DoRA (Weight-Decomposed Low-Rank Adaptation) GPU kernels using CubeCL.
//!
//! DoRA improves upon LoRA by decomposing weight updates into magnitude and direction:
//! `W' = m * (W + ΔW) / ||W + ΔW||_col`
//!
//! where:
//! - `ΔW = scale * (A @ B)` is the standard LoRA update
//! - `m` is a learned per-column magnitude vector
//! - `||...||_col` denotes column-wise L2 norm
//!
//! This decomposition allows the model to learn both the direction (via LoRA)
//! and magnitude (via m) of weight changes, leading to better fine-tuning quality.
//!
//! Reference: <https://arxiv.org/abs/2402.09353>

// CubeCL macro generates internal items that trigger missing_docs warnings
#![allow(missing_docs)]
#![allow(clippy::excessive_precision)]
#![allow(clippy::similar_names)]

use cubecl::prelude::*;

/// Tile size for matrix multiplication kernels.
const TILE_SIZE: u32 = 32;

/// DoRA forward kernel with precomputed column norms.
///
/// Computes: `Y = X @ (m * (W + scale * A @ B) / norm)`
///
/// This is the primary DoRA kernel - it requires precomputed column norms
/// which can be cached when weights don't change between forward passes.
///
/// # Arguments
/// * `x` - Input tensor `[M, K]`
/// * `w` - Base weight tensor `[K, N]`
/// * `a` - LoRA A matrix `[K, R]`
/// * `b` - LoRA B matrix `[R, N]`
/// * `magnitude` - Learned magnitude vector `[N]`
/// * `col_norms` - Precomputed column norms `[N]`
/// * `y` - Output tensor `[M, N]`
/// * `scale` - LoRA scaling factor (alpha / rank)
/// * `m` - Batch dimension (batch_size * seq_len)
/// * `k` - Input features dimension
/// * `n` - Output features dimension
/// * `r` - LoRA rank
#[cube(launch)]
pub fn dora_forward_kernel<F: Float + CubeElement>(
    x: &Array<F>,
    w: &Array<F>,
    a: &Array<F>,
    b: &Array<F>,
    mag_norms: &Array<F>, // Interleaved: [mag0, norm0, mag1, norm1, ...]
    y: &mut Array<F>,
    scale: F,
    #[comptime] m: u32,
    #[comptime] k: u32,
    #[comptime] n: u32,
    #[comptime] r: u32,
) {
    let row = CUBE_POS_Y * TILE_SIZE + UNIT_POS_Y;
    let col = CUBE_POS_X * TILE_SIZE + UNIT_POS_X;

    if row >= m || col >= n {
        terminate!();
    }

    // Get magnitude and norm for this column (interleaved storage)
    let mag = mag_norms[(col * 2u32) as usize];
    let col_norm = mag_norms[(col * 2u32 + 1u32) as usize];
    let norm_scale = mag / (col_norm + F::new(1e-8));

    let mut x_tile = SharedMemory::<F>::new((TILE_SIZE * TILE_SIZE) as usize);
    let mut acc = F::new(0.0);

    let num_k_tiles = (k + TILE_SIZE - 1u32) / TILE_SIZE;

    for k_tile in 0u32..num_k_tiles {
        let k_base = k_tile * TILE_SIZE;

        let x_row = CUBE_POS_Y * TILE_SIZE + UNIT_POS_Y;
        let x_col = k_base + UNIT_POS_X;
        x_tile[(UNIT_POS_Y * TILE_SIZE + UNIT_POS_X) as usize] = if x_row < m && x_col < k {
            x[(x_row * k + x_col) as usize]
        } else {
            F::new(0.0)
        };

        sync_cube();

        for i in 0u32..TILE_SIZE {
            let ki = k_base + i;
            if ki < k {
                let x_elem = x_tile[(UNIT_POS_Y * TILE_SIZE + i) as usize];

                let w_val = w[(ki * n + col) as usize];
                let mut ab_val = F::new(0.0);
                for rank_idx in 0u32..r {
                    ab_val =
                        ab_val + a[(ki * r + rank_idx) as usize] * b[(rank_idx * n + col) as usize];
                }
                let w_plus_lora = w_val + scale * ab_val;
                let w_dora = norm_scale * w_plus_lora;

                acc = acc + x_elem * w_dora;
            }
        }

        sync_cube();
    }

    y[(row * n + col) as usize] = acc;
}

/// Compute column norms for DoRA.
///
/// Computes: `norms[j] = ||W[:, j] + scale * (A @ B)[:, j]||`
///
/// This is a helper kernel to precompute column norms when they
/// can be cached across multiple forward passes.
///
/// # Arguments
/// * `w` - Base weight tensor `[K, N]`
/// * `a` - LoRA A matrix `[K, R]`
/// * `b` - LoRA B matrix `[R, N]`
/// * `norms` - Output column norms `[N]`
/// * `scale` - LoRA scaling factor
/// * `k` - Input features dimension
/// * `n` - Output features dimension
/// * `r` - LoRA rank
#[cube(launch)]
pub fn dora_compute_column_norms_kernel<F: Float + CubeElement>(
    w: &Array<F>,
    a: &Array<F>,
    b: &Array<F>,
    norms: &mut Array<F>,
    scale: F,
    #[comptime] k: u32,
    #[comptime] n: u32,
    #[comptime] r: u32,
) {
    let col = CUBE_POS_X * TILE_SIZE + UNIT_POS_X;

    if col >= n {
        terminate!();
    }

    // Compute ||W[:, col] + scale * (A @ B)[:, col]||^2
    let mut norm_sq = F::new(0.0);

    for ki in 0u32..k {
        let w_val = w[(ki * n + col) as usize];

        // A[ki, :] @ B[:, col]
        let mut ab_val = F::new(0.0);
        for rank_idx in 0u32..r {
            ab_val = ab_val + a[(ki * r + rank_idx) as usize] * b[(rank_idx * n + col) as usize];
        }

        let combined = w_val + scale * ab_val;
        norm_sq = norm_sq + combined * combined;
    }

    norms[col as usize] = F::sqrt(norm_sq);
}

/// DoRA merge kernel.
///
/// Merges DoRA weights for deployment:
/// `W_merged = m * (W + scale * A @ B) / ||W + scale * A @ B||_col`
///
/// # Arguments
/// * `w` - Base weight tensor `[K, N]`
/// * `a` - LoRA A matrix `[K, R]`
/// * `b` - LoRA B matrix `[R, N]`
/// * `mag_norms` - Interleaved magnitude and norms `[2*N]`
/// * `merged` - Output merged weight tensor `[K, N]`
/// * `scale` - LoRA scaling factor
/// * `k` - Input features
/// * `n` - Output features
/// * `r` - LoRA rank
#[cube(launch)]
pub fn dora_merge_kernel<F: Float + CubeElement>(
    w: &Array<F>,
    a: &Array<F>,
    b: &Array<F>,
    mag_norms: &Array<F>, // Interleaved: [mag0, norm0, mag1, norm1, ...]
    merged: &mut Array<F>,
    scale: F,
    #[comptime] k: u32,
    #[comptime] n: u32,
    #[comptime] r: u32,
) {
    let row = CUBE_POS_Y * TILE_SIZE + UNIT_POS_Y;
    let col = CUBE_POS_X * TILE_SIZE + UNIT_POS_X;

    if row >= k || col >= n {
        terminate!();
    }

    // Get magnitude and norm for this column (interleaved storage)
    let mag = mag_norms[(col * 2u32) as usize];
    let col_norm = mag_norms[(col * 2u32 + 1u32) as usize];
    let norm_scale = mag / (col_norm + F::new(1e-8));

    // W[row, col]
    let w_val = w[(row * n + col) as usize];

    // A[row, :] @ B[:, col]
    let mut ab_val = F::new(0.0);
    for rank_idx in 0u32..r {
        ab_val = ab_val + a[(row * r + rank_idx) as usize] * b[(rank_idx * n + col) as usize];
    }

    // Apply DoRA transformation
    let w_plus_lora = w_val + scale * ab_val;
    merged[(row * n + col) as usize] = norm_scale * w_plus_lora;
}

/// DoRA delta kernel (in-place update).
///
/// Computes the DoRA contribution and adds to existing output:
/// `Y += X @ (m * (W + scale * A @ B) / norm)`
///
/// Assumes Y already contains some base computation.
#[cube(launch)]
pub fn dora_delta_kernel<F: Float + CubeElement>(
    x: &Array<F>,
    w: &Array<F>,
    a: &Array<F>,
    b: &Array<F>,
    mag_norms: &Array<F>,
    y: &mut Array<F>,
    scale: F,
    #[comptime] m: u32,
    #[comptime] k: u32,
    #[comptime] n: u32,
    #[comptime] r: u32,
) {
    let row = CUBE_POS_Y * TILE_SIZE + UNIT_POS_Y;
    let col = CUBE_POS_X * TILE_SIZE + UNIT_POS_X;

    if row >= m || col >= n {
        terminate!();
    }

    let mag = mag_norms[(col * 2u32) as usize];
    let col_norm = mag_norms[(col * 2u32 + 1u32) as usize];
    let norm_scale = mag / (col_norm + F::new(1e-8));

    // Compute X[row, :] @ normalized(W[:, col] + scale * A @ B[:, col])
    let mut acc = F::new(0.0);

    for ki in 0u32..k {
        let x_elem = x[(row * k + ki) as usize];
        let w_val = w[(ki * n + col) as usize];

        let mut ab_val = F::new(0.0);
        for rank_idx in 0u32..r {
            ab_val = ab_val + a[(ki * r + rank_idx) as usize] * b[(rank_idx * n + col) as usize];
        }

        let w_plus_lora = w_val + scale * ab_val;
        let w_dora = norm_scale * w_plus_lora;
        acc = acc + x_elem * w_dora;
    }

    // Add to existing output
    y[(row * n + col) as usize] = y[(row * n + col) as usize] + acc;
}

/// Batched DoRA forward kernel.
///
/// Handles batched inputs with shared weights:
/// `Y[b] = X[b] @ DoRA(W, A, B, m)`
///
/// Uses packed dimension parameter to stay under CubeCL parameter limits.
#[cube(launch)]
pub fn batched_dora_forward_kernel<F: Float + CubeElement>(
    x: &Array<F>,
    w: &Array<F>,
    a: &Array<F>,
    b: &Array<F>,
    mag_norms: &Array<F>,
    y: &mut Array<F>,
    scale: F,
    #[comptime] batch_seq: u32, // batch * seq_len packed
    #[comptime] k: u32,
    #[comptime] n: u32,
    #[comptime] r: u32,
) {
    let row = CUBE_POS_Y * TILE_SIZE + UNIT_POS_Y;
    let col = CUBE_POS_X * TILE_SIZE + UNIT_POS_X;

    if row >= batch_seq || col >= n {
        terminate!();
    }

    let mag = mag_norms[(col * 2u32) as usize];
    let col_norm = mag_norms[(col * 2u32 + 1u32) as usize];
    let norm_scale = mag / (col_norm + F::new(1e-8));

    let mut x_tile = SharedMemory::<F>::new((TILE_SIZE * TILE_SIZE) as usize);
    let mut acc = F::new(0.0);

    let num_k_tiles = (k + TILE_SIZE - 1u32) / TILE_SIZE;

    for k_tile in 0u32..num_k_tiles {
        let k_base = k_tile * TILE_SIZE;

        let x_row = CUBE_POS_Y * TILE_SIZE + UNIT_POS_Y;
        let x_col = k_base + UNIT_POS_X;
        x_tile[(UNIT_POS_Y * TILE_SIZE + UNIT_POS_X) as usize] = if x_row < batch_seq && x_col < k {
            x[(x_row * k + x_col) as usize]
        } else {
            F::new(0.0)
        };

        sync_cube();

        for i in 0u32..TILE_SIZE {
            let ki = k_base + i;
            if ki < k {
                let x_elem = x_tile[(UNIT_POS_Y * TILE_SIZE + i) as usize];
                let w_val = w[(ki * n + col) as usize];

                let mut ab_val = F::new(0.0);
                for rank_idx in 0u32..r {
                    ab_val =
                        ab_val + a[(ki * r + rank_idx) as usize] * b[(rank_idx * n + col) as usize];
                }

                let w_plus_lora = w_val + scale * ab_val;
                let w_dora = norm_scale * w_plus_lora;
                acc = acc + x_elem * w_dora;
            }
        }

        sync_cube();
    }

    y[(row * n + col) as usize] = acc;
}

#[cfg(test)]
mod tests {
    // Tests require CUDA runtime
    // See peft-rs/tests/kernels_cuda.rs for integration tests
}
