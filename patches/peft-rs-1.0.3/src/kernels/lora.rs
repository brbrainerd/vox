//! LoRA (Low-Rank Adaptation) GPU kernels using CubeCL.
//!
//! LoRA reduces trainable parameters by decomposing weight updates:
//! `ΔW = scale * (A @ B)` where `A ∈ R^{in×rank}` and `B ∈ R^{rank×out}`.
//!
//! The forward pass computes: `Y = X @ W + scale * (X @ A @ B)`
//!
//! These kernels fuse operations to minimize memory bandwidth:
//! - `fused_lora_forward_kernel`: Complete fused forward pass
//! - `lora_delta_kernel`: LoRA-only delta (assumes base already computed)
//! - `lora_merge_kernel`: Merge LoRA into base weights for deployment
//! - `lora_unmerge_kernel`: Unmerge LoRA from merged weights
//!
//! Reference: <https://arxiv.org/abs/2106.09685>

// CubeCL macro generates internal items that trigger missing_docs warnings
#![allow(missing_docs)]
#![allow(clippy::excessive_precision)]
#![allow(clippy::similar_names)]

use cubecl::prelude::*;

/// Tile size for matrix multiplication kernels.
/// 32x32 tiles provide good occupancy on most CUDA architectures.
const TILE_SIZE: u32 = 32;

/// Fused LoRA forward kernel.
///
/// Computes: `Y = X @ W + scale * (X @ A @ B)`
///
/// This kernel fuses three matrix multiplications into one pass,
/// reducing memory bandwidth requirements significantly by:
/// 1. Computing `X @ W` in tiles with shared memory
/// 2. Simultaneously computing `X @ A` in the same tiles
/// 3. Computing the final `(X @ A) @ B` and combining with base output
///
/// Memory layout (all row-major):
/// - X: `[M, K]` - Input activations (batch_size * seq_len, in_features)
/// - W: `[K, N]` - Base weight matrix (in_features, out_features)
/// - A: `[K, R]` - LoRA down-projection (in_features, rank)
/// - B: `[R, N]` - LoRA up-projection (rank, out_features)
/// - Y: `[M, N]` - Output (batch_size * seq_len, out_features)
///
/// # Arguments
/// * `x` - Input tensor `[M, K]`
/// * `w` - Base weight tensor `[K, N]`
/// * `a` - LoRA A matrix `[K, R]`
/// * `b` - LoRA B matrix `[R, N]`
/// * `y` - Output tensor `[M, N]`
/// * `scale` - LoRA scaling factor (alpha / rank)
/// * `m` - Batch dimension (batch_size * seq_len)
/// * `k` - Input features dimension
/// * `n` - Output features dimension
/// * `r` - LoRA rank
#[cube(launch)]
pub fn fused_lora_forward_kernel<F: Float + CubeElement>(
    x: &Array<F>,
    w: &Array<F>,
    a: &Array<F>,
    b: &Array<F>,
    y: &mut Array<F>,
    scale: F,
    #[comptime] m: u32,
    #[comptime] k: u32,
    #[comptime] n: u32,
    #[comptime] r: u32,
) {
    // Calculate output position for this thread
    let row = CUBE_POS_Y * TILE_SIZE + UNIT_POS_Y;
    let col = CUBE_POS_X * TILE_SIZE + UNIT_POS_X;

    // Early exit for out-of-bounds threads
    if row >= m || col >= n {
        terminate!();
    }

    // Shared memory for tiles
    let mut x_tile = SharedMemory::<F>::new((TILE_SIZE * TILE_SIZE) as usize);
    let mut w_tile = SharedMemory::<F>::new((TILE_SIZE * TILE_SIZE) as usize);

    // Accumulator for base matmul: X @ W
    let mut base_acc = F::new(0.0);

    // Accumulator for LoRA intermediate: X @ A
    // We compute this per-row during the K-tiled loop
    // Using local memory since r is typically small (8-64)
    let mut xa_local = Array::<F>::new(64usize); // Max supported rank
    for rank_idx in 0u32..r {
        xa_local[rank_idx as usize] = F::new(0.0);
    }

    // Number of tiles along K dimension
    let num_k_tiles = (k + TILE_SIZE - 1u32) / TILE_SIZE;

    // Iterate over tiles in K dimension
    for k_tile in 0u32..num_k_tiles {
        let k_base = k_tile * TILE_SIZE;

        // Collaborative load of X tile into shared memory
        let x_row = CUBE_POS_Y * TILE_SIZE + UNIT_POS_Y;
        let x_col = k_base + UNIT_POS_X;

        let x_val = if x_row < m && x_col < k {
            x[(x_row * k + x_col) as usize]
        } else {
            F::new(0.0)
        };
        x_tile[(UNIT_POS_Y * TILE_SIZE + UNIT_POS_X) as usize] = x_val;

        // Collaborative load of W tile into shared memory
        let w_row = k_base + UNIT_POS_Y;
        let w_col = CUBE_POS_X * TILE_SIZE + UNIT_POS_X;

        let w_val = if w_row < k && w_col < n {
            w[(w_row * n + w_col) as usize]
        } else {
            F::new(0.0)
        };
        w_tile[(UNIT_POS_Y * TILE_SIZE + UNIT_POS_X) as usize] = w_val;

        // Synchronize to ensure tiles are fully loaded
        sync_cube();

        // Compute partial dot product for base matmul
        #[unroll]
        for i in 0u32..TILE_SIZE {
            let x_elem = x_tile[(UNIT_POS_Y * TILE_SIZE + i) as usize];
            let w_elem = w_tile[(i * TILE_SIZE + UNIT_POS_X) as usize];
            base_acc = base_acc + x_elem * w_elem;
        }

        // Compute partial X @ A contribution for this K-tile
        // For each element of the tile, accumulate into xa_local
        for i in 0u32..TILE_SIZE {
            let ki = k_base + i;
            if ki < k {
                let x_elem = x_tile[(UNIT_POS_Y * TILE_SIZE + i) as usize];
                // Accumulate X[row, ki] * A[ki, r] for each rank
                for rank_idx in 0u32..r {
                    let a_val = a[(ki * r + rank_idx) as usize];
                    xa_local[rank_idx as usize] = xa_local[rank_idx as usize] + x_elem * a_val;
                }
            }
        }

        // Synchronize before loading next tile
        sync_cube();
    }

    // Now compute (X @ A) @ B for the LoRA contribution
    // xa_local contains X[row, :] @ A for this row
    // We need to compute xa_local @ B[:, col]
    let mut lora_acc = F::new(0.0);
    for rank_idx in 0u32..r {
        let b_val = b[(rank_idx * n + col) as usize];
        lora_acc = lora_acc + xa_local[rank_idx as usize] * b_val;
    }

    // Combine base output and scaled LoRA delta
    y[(row * n + col) as usize] = base_acc + scale * lora_acc;
}

/// Tiled fused LoRA forward kernel with optimized shared memory usage.
///
/// This variant uses a two-pass approach for better memory efficiency:
/// 1. First pass: Compute X @ A and store in shared memory
/// 2. Second pass: Compute (X @ A) @ B while computing X @ W
///
/// Best for larger ranks (r > 32) where local memory becomes a bottleneck.
#[cube(launch)]
pub fn fused_lora_forward_tiled_kernel<F: Float + CubeElement>(
    x: &Array<F>,
    w: &Array<F>,
    a: &Array<F>,
    b: &Array<F>,
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

    let mut x_tile = SharedMemory::<F>::new((TILE_SIZE * TILE_SIZE) as usize);
    let mut w_tile = SharedMemory::<F>::new((TILE_SIZE * TILE_SIZE) as usize);
    let mut a_tile = SharedMemory::<F>::new((TILE_SIZE * TILE_SIZE) as usize);

    // Shared memory for X @ A intermediate (one row per thread row in tile)
    let mut xa_shared = SharedMemory::<F>::new((TILE_SIZE * r) as usize);

    // Initialize XA shared memory
    if UNIT_POS_X < r {
        xa_shared[(UNIT_POS_Y * r + UNIT_POS_X) as usize] = F::new(0.0);
    }
    sync_cube();

    let mut base_acc = F::new(0.0);
    let num_k_tiles = (k + TILE_SIZE - 1u32) / TILE_SIZE;

    for k_tile in 0u32..num_k_tiles {
        let k_base = k_tile * TILE_SIZE;

        // Load X tile
        let x_row = CUBE_POS_Y * TILE_SIZE + UNIT_POS_Y;
        let x_col = k_base + UNIT_POS_X;
        x_tile[(UNIT_POS_Y * TILE_SIZE + UNIT_POS_X) as usize] = if x_row < m && x_col < k {
            x[(x_row * k + x_col) as usize]
        } else {
            F::new(0.0)
        };

        // Load W tile
        let w_row = k_base + UNIT_POS_Y;
        let w_col = CUBE_POS_X * TILE_SIZE + UNIT_POS_X;
        w_tile[(UNIT_POS_Y * TILE_SIZE + UNIT_POS_X) as usize] = if w_row < k && w_col < n {
            w[(w_row * n + w_col) as usize]
        } else {
            F::new(0.0)
        };

        // Load A tile (K_tile x min(R, TILE_SIZE))
        // We load A[k_base:k_base+TILE_SIZE, 0:min(r, TILE_SIZE)]
        if UNIT_POS_X < r {
            let a_row = k_base + UNIT_POS_Y;
            a_tile[(UNIT_POS_Y * r + UNIT_POS_X) as usize] = if a_row < k {
                a[(a_row * r + UNIT_POS_X) as usize]
            } else {
                F::new(0.0)
            };
        }

        sync_cube();

        // Base matmul: X_tile @ W_tile
        #[unroll]
        for i in 0u32..TILE_SIZE {
            base_acc = base_acc
                + x_tile[(UNIT_POS_Y * TILE_SIZE + i) as usize]
                    * w_tile[(i * TILE_SIZE + UNIT_POS_X) as usize];
        }

        // X @ A accumulation (use first r threads in X dimension)
        if UNIT_POS_X < r {
            let mut xa_contrib = F::new(0.0);
            for i in 0u32..TILE_SIZE {
                xa_contrib = xa_contrib
                    + x_tile[(UNIT_POS_Y * TILE_SIZE + i) as usize]
                        * a_tile[(i * r + UNIT_POS_X) as usize];
            }
            xa_shared[(UNIT_POS_Y * r + UNIT_POS_X) as usize] =
                xa_shared[(UNIT_POS_Y * r + UNIT_POS_X) as usize] + xa_contrib;
        }

        sync_cube();
    }

    // Compute (X @ A) @ B[:, col] for LoRA contribution
    let mut lora_acc = F::new(0.0);
    for rank_idx in 0u32..r {
        let xa_val = xa_shared[(UNIT_POS_Y * r + rank_idx) as usize];
        let b_val = b[(rank_idx * n + col) as usize];
        lora_acc = lora_acc + xa_val * b_val;
    }

    // Final output
    y[(row * n + col) as usize] = base_acc + scale * lora_acc;
}

/// Optimized LoRA delta kernel.
///
/// Only computes the LoRA contribution: `Y += scale * (X @ A @ B)`
///
/// Assumes Y already contains `X @ W` from a previous cuBLAS matmul operation.
/// This is useful when base weights are frozen and we can leverage optimized
/// cuBLAS for the base computation while only using this kernel for the delta.
///
/// # Arguments
/// * `x` - Input tensor `[M, K]`
/// * `a` - LoRA A matrix `[K, R]`
/// * `b` - LoRA B matrix `[R, N]`
/// * `y` - Output tensor `[M, N]` (modified in-place, must contain base output)
/// * `scale` - LoRA scaling factor (alpha / rank)
/// * `m` - Batch dimension
/// * `k` - Input features
/// * `n` - Output features
/// * `r` - LoRA rank
#[cube(launch)]
pub fn lora_delta_kernel<F: Float + CubeElement>(
    x: &Array<F>,
    a: &Array<F>,
    b: &Array<F>,
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

    // Shared memory for collaborative X @ A computation
    let mut x_tile = SharedMemory::<F>::new((TILE_SIZE * TILE_SIZE) as usize);
    let mut xa_shared = SharedMemory::<F>::new((TILE_SIZE * r) as usize);

    // Initialize XA shared
    if UNIT_POS_X < r {
        xa_shared[(UNIT_POS_Y * r + UNIT_POS_X) as usize] = F::new(0.0);
    }
    sync_cube();

    let num_k_tiles = (k + TILE_SIZE - 1u32) / TILE_SIZE;

    // First pass: Compute X @ A
    for k_tile in 0u32..num_k_tiles {
        let k_base = k_tile * TILE_SIZE;

        // Load X tile
        let x_row = CUBE_POS_Y * TILE_SIZE + UNIT_POS_Y;
        let x_col = k_base + UNIT_POS_X;
        x_tile[(UNIT_POS_Y * TILE_SIZE + UNIT_POS_X) as usize] = if x_row < m && x_col < k {
            x[(x_row * k + x_col) as usize]
        } else {
            F::new(0.0)
        };

        sync_cube();

        // Accumulate X @ A
        if UNIT_POS_X < r {
            let mut contrib = F::new(0.0);
            for i in 0u32..TILE_SIZE {
                let ki = k_base + i;
                if ki < k {
                    let x_elem = x_tile[(UNIT_POS_Y * TILE_SIZE + i) as usize];
                    let a_val = a[(ki * r + UNIT_POS_X) as usize];
                    contrib = contrib + x_elem * a_val;
                }
            }
            xa_shared[(UNIT_POS_Y * r + UNIT_POS_X) as usize] =
                xa_shared[(UNIT_POS_Y * r + UNIT_POS_X) as usize] + contrib;
        }

        sync_cube();
    }

    // Second pass: Compute (X @ A) @ B[:, col]
    let mut delta = F::new(0.0);
    for rank_idx in 0u32..r {
        let xa_val = xa_shared[(UNIT_POS_Y * r + rank_idx) as usize];
        let b_val = b[(rank_idx * n + col) as usize];
        delta = delta + xa_val * b_val;
    }

    // Add scaled delta to output (in-place)
    y[(row * n + col) as usize] = y[(row * n + col) as usize] + scale * delta;
}

/// LoRA merge kernel.
///
/// Computes: `W_merged = W + scale * (A @ B)`
///
/// Used when deploying a trained LoRA adapter for inference.
/// Merging the weights eliminates the runtime overhead of LoRA
/// at the cost of not being able to switch adapters.
///
/// # Arguments
/// * `w` - Base weight tensor `[K, N]`
/// * `a` - LoRA A matrix `[K, R]`
/// * `b` - LoRA B matrix `[R, N]`
/// * `merged` - Output merged weight tensor `[K, N]`
/// * `scale` - LoRA scaling factor (alpha / rank)
/// * `k` - Input features dimension
/// * `n` - Output features dimension
/// * `r` - LoRA rank
#[cube(launch)]
pub fn lora_merge_kernel<F: Float + CubeElement>(
    w: &Array<F>,
    a: &Array<F>,
    b: &Array<F>,
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

    // Compute A[row, :] @ B[:, col]
    let mut ab_val = F::new(0.0);
    for rank_idx in 0u32..r {
        let a_elem = a[(row * r + rank_idx) as usize];
        let b_elem = b[(rank_idx * n + col) as usize];
        ab_val = ab_val + a_elem * b_elem;
    }

    // Merge: W + scale * (A @ B)
    let w_val = w[(row * n + col) as usize];
    merged[(row * n + col) as usize] = w_val + scale * ab_val;
}

/// LoRA unmerge kernel.
///
/// Computes: `W_unmerged = W_merged - scale * (A @ B)`
///
/// Used to restore the original base weights after merging.
/// This allows switching between different LoRA adapters or
/// reverting to the base model.
///
/// # Arguments
/// * `merged` - Merged weight tensor `[K, N]`
/// * `a` - LoRA A matrix `[K, R]`
/// * `b` - LoRA B matrix `[R, N]`
/// * `unmerged` - Output unmerged weight tensor `[K, N]`
/// * `scale` - LoRA scaling factor (alpha / rank)
/// * `k` - Input features dimension
/// * `n` - Output features dimension
/// * `r` - LoRA rank
#[cube(launch)]
pub fn lora_unmerge_kernel<F: Float + CubeElement>(
    merged: &Array<F>,
    a: &Array<F>,
    b: &Array<F>,
    unmerged: &mut Array<F>,
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

    // Compute A[row, :] @ B[:, col]
    let mut ab_val = F::new(0.0);
    for rank_idx in 0u32..r {
        let a_elem = a[(row * r + rank_idx) as usize];
        let b_elem = b[(rank_idx * n + col) as usize];
        ab_val = ab_val + a_elem * b_elem;
    }

    // Unmerge: W_merged - scale * (A @ B)
    let merged_val = merged[(row * n + col) as usize];
    unmerged[(row * n + col) as usize] = merged_val - scale * ab_val;
}

/// Batched LoRA forward kernel.
///
/// Handles batched inputs where the weight matrices are shared:
/// `Y[b] = X[b] @ W + scale * (X[b] @ A @ B)`
///
/// This is the common case in inference where multiple sequences
/// share the same model weights.
///
/// # Arguments
/// * `x` - Batched input `[B, M, K]`
/// * `w` - Shared base weight `[K, N]`
/// * `a` - Shared LoRA A `[K, R]`
/// * `b` - Shared LoRA B `[R, N]`
/// * `y` - Batched output `[B, M, N]`
/// * `scale` - LoRA scaling factor
/// * `batch` - Batch size
/// * `m` - Sequence length
/// * `k` - Input features
/// * `n` - Output features
/// * `r` - LoRA rank
#[cube(launch)]
pub fn batched_lora_forward_kernel<F: Float + CubeElement>(
    x: &Array<F>,
    w: &Array<F>,
    a: &Array<F>,
    b: &Array<F>,
    y: &mut Array<F>,
    scale: F,
    #[comptime] batch: u32,
    #[comptime] m: u32,
    #[comptime] k: u32,
    #[comptime] n: u32,
    #[comptime] r: u32,
) {
    let batch_idx = CUBE_POS_Z;
    let row = CUBE_POS_Y * TILE_SIZE + UNIT_POS_Y;
    let col = CUBE_POS_X * TILE_SIZE + UNIT_POS_X;

    if batch_idx >= batch || row >= m || col >= n {
        terminate!();
    }

    let x_batch_offset = batch_idx * m * k;
    let y_batch_offset = batch_idx * m * n;

    let mut x_tile = SharedMemory::<F>::new((TILE_SIZE * TILE_SIZE) as usize);
    let mut w_tile = SharedMemory::<F>::new((TILE_SIZE * TILE_SIZE) as usize);

    let mut base_acc = F::new(0.0);
    let mut xa_local = Array::<F>::new(64usize);
    for rank_idx in 0u32..r {
        xa_local[rank_idx as usize] = F::new(0.0);
    }

    let num_k_tiles = (k + TILE_SIZE - 1u32) / TILE_SIZE;

    for k_tile in 0u32..num_k_tiles {
        let k_base = k_tile * TILE_SIZE;

        // Load X tile (with batch offset)
        let x_row = CUBE_POS_Y * TILE_SIZE + UNIT_POS_Y;
        let x_col = k_base + UNIT_POS_X;
        x_tile[(UNIT_POS_Y * TILE_SIZE + UNIT_POS_X) as usize] = if x_row < m && x_col < k {
            x[(x_batch_offset + x_row * k + x_col) as usize]
        } else {
            F::new(0.0)
        };

        // Load W tile (shared across batches)
        let w_row = k_base + UNIT_POS_Y;
        let w_col = CUBE_POS_X * TILE_SIZE + UNIT_POS_X;
        w_tile[(UNIT_POS_Y * TILE_SIZE + UNIT_POS_X) as usize] = if w_row < k && w_col < n {
            w[(w_row * n + w_col) as usize]
        } else {
            F::new(0.0)
        };

        sync_cube();

        // Base matmul
        #[unroll]
        for i in 0u32..TILE_SIZE {
            base_acc = base_acc
                + x_tile[(UNIT_POS_Y * TILE_SIZE + i) as usize]
                    * w_tile[(i * TILE_SIZE + UNIT_POS_X) as usize];
        }

        // X @ A accumulation
        for i in 0u32..TILE_SIZE {
            let ki = k_base + i;
            if ki < k {
                let x_elem = x_tile[(UNIT_POS_Y * TILE_SIZE + i) as usize];
                for rank_idx in 0u32..r {
                    let a_val = a[(ki * r + rank_idx) as usize];
                    xa_local[rank_idx as usize] = xa_local[rank_idx as usize] + x_elem * a_val;
                }
            }
        }

        sync_cube();
    }

    // (X @ A) @ B
    let mut lora_acc = F::new(0.0);
    for rank_idx in 0u32..r {
        let b_val = b[(rank_idx * n + col) as usize];
        lora_acc = lora_acc + xa_local[rank_idx as usize] * b_val;
    }

    y[(y_batch_offset + row * n + col) as usize] = base_acc + scale * lora_acc;
}

/// Multi-adapter LoRA forward kernel.
///
/// Supports multiple LoRA adapters with different scales:
/// `Y = X @ W + sum_i(scale_i * (X @ A_i @ B_i))`
///
/// Useful for techniques like adapter fusion or ensemble methods.
///
/// # Arguments
/// * `x` - Input `[M, K]`
/// * `w` - Base weight `[K, N]`
/// * `adapters_a` - Concatenated LoRA A matrices `[num_adapters * K * R]`
/// * `adapters_b` - Concatenated LoRA B matrices `[num_adapters * R * N]`
/// * `scales` - Per-adapter scales `[num_adapters]`
/// * `y` - Output `[M, N]`
/// * `m` - Batch dimension
/// * `k` - Input features
/// * `n` - Output features
/// * `r` - LoRA rank (same for all adapters)
/// * `num_adapters` - Number of adapters
#[cube(launch)]
pub fn multi_adapter_lora_forward_kernel<F: Float + CubeElement>(
    x: &Array<F>,
    w: &Array<F>,
    adapters_a: &Array<F>,
    adapters_b: &Array<F>,
    scales: &Array<F>,
    y: &mut Array<F>,
    #[comptime] m: u32,
    #[comptime] k: u32,
    #[comptime] n: u32,
    #[comptime] r: u32,
    #[comptime] num_adapters: u32,
) {
    let row = CUBE_POS_Y * TILE_SIZE + UNIT_POS_Y;
    let col = CUBE_POS_X * TILE_SIZE + UNIT_POS_X;

    if row >= m || col >= n {
        terminate!();
    }

    let mut x_tile = SharedMemory::<F>::new((TILE_SIZE * TILE_SIZE) as usize);
    let mut w_tile = SharedMemory::<F>::new((TILE_SIZE * TILE_SIZE) as usize);

    let mut base_acc = F::new(0.0);

    // Per-adapter XA accumulators
    let mut xa_all = Array::<F>::new(64usize * 8); // max 8 adapters, max rank 64
    for i in 0u32..(num_adapters * r) {
        xa_all[i as usize] = F::new(0.0);
    }

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

        let w_row = k_base + UNIT_POS_Y;
        let w_col = CUBE_POS_X * TILE_SIZE + UNIT_POS_X;
        w_tile[(UNIT_POS_Y * TILE_SIZE + UNIT_POS_X) as usize] = if w_row < k && w_col < n {
            w[(w_row * n + w_col) as usize]
        } else {
            F::new(0.0)
        };

        sync_cube();

        // Base matmul
        #[unroll]
        for i in 0u32..TILE_SIZE {
            base_acc = base_acc
                + x_tile[(UNIT_POS_Y * TILE_SIZE + i) as usize]
                    * w_tile[(i * TILE_SIZE + UNIT_POS_X) as usize];
        }

        // X @ A for each adapter
        for i in 0u32..TILE_SIZE {
            let ki = k_base + i;
            if ki < k {
                let x_elem = x_tile[(UNIT_POS_Y * TILE_SIZE + i) as usize];
                for adapter_idx in 0u32..num_adapters {
                    let a_offset = adapter_idx * k * r;
                    for rank_idx in 0u32..r {
                        let a_val = adapters_a[(a_offset + ki * r + rank_idx) as usize];
                        xa_all[(adapter_idx * r + rank_idx) as usize] =
                            xa_all[(adapter_idx * r + rank_idx) as usize] + x_elem * a_val;
                    }
                }
            }
        }

        sync_cube();
    }

    // Combine all adapter contributions
    let mut total_lora_acc = F::new(0.0);
    for adapter_idx in 0u32..num_adapters {
        let b_offset = adapter_idx * r * n;
        let adapter_scale = scales[adapter_idx as usize];

        let mut lora_acc = F::new(0.0);
        for rank_idx in 0u32..r {
            let xa_val = xa_all[(adapter_idx * r + rank_idx) as usize];
            let b_val = adapters_b[(b_offset + rank_idx * n + col) as usize];
            lora_acc = lora_acc + xa_val * b_val;
        }
        total_lora_acc = total_lora_acc + adapter_scale * lora_acc;
    }

    y[(row * n + col) as usize] = base_acc + total_lora_acc;
}

#[cfg(test)]
mod tests {
    // Tests would go here but require CUDA runtime
    // See peft-rs/tests/kernels_cuda.rs for integration tests
}
