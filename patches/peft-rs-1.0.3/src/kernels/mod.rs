//! GPU kernels for PEFT adapter operations using CubeCL.
//!
//! This module provides CUDA-accelerated implementations of:
//! - Fused LoRA forward pass (avoids materializing intermediate results)
//! - LoRA merge/unmerge operations
//! - DoRA (Weight-Decomposed LoRA) forward pass
//!
//! These kernels are critical for achieving high performance in LoRA
//! inference and training workloads, reducing memory bandwidth requirements
//! by fusing multiple operations.
//!
//! # Architecture
//!
//! The kernels use tiled matrix multiplication with shared memory to maximize
//! arithmetic intensity and minimize global memory accesses. The LoRA forward
//! kernel fuses the computation `Y = X @ W + scale * (X @ A @ B)` into a single
//! pass, avoiding the need to materialize the intermediate `X @ A` result.
//!
//! # Example
//!
//! ```ignore
//! use peft_rs::kernels::lora::fused_lora_forward_kernel;
//! use cubecl::prelude::*;
//!
//! // Launch the fused LoRA forward kernel
//! unsafe {
//!     fused_lora_forward_kernel::launch_unchecked::<f32, CudaRuntime>(
//!         &client,
//!         cube_count,
//!         cube_dim,
//!         x_arg,
//!         w_arg,
//!         a_arg,
//!         b_arg,
//!         y_arg,
//!         scale,
//!         m, k, n, r,
//!     );
//! }
//! ```

#[cfg(feature = "cuda")]
pub mod lora;

#[cfg(feature = "cuda")]
pub mod dora;

// Re-export key items for convenience
#[cfg(feature = "cuda")]
pub use lora::{
    batched_lora_forward_kernel, fused_lora_forward_kernel, fused_lora_forward_tiled_kernel,
    lora_delta_kernel, lora_merge_kernel, lora_unmerge_kernel, multi_adapter_lora_forward_kernel,
};

#[cfg(feature = "cuda")]
pub use dora::{
    batched_dora_forward_kernel, dora_compute_column_norms_kernel, dora_delta_kernel,
    dora_forward_kernel, dora_merge_kernel,
};
