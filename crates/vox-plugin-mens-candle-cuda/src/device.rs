//! Device selection for Candle training.
//!
//! Ported from `vox-populi/src/mens/tensor/device.rs` (SP3 sub-batch C).
//! `probe_gpu` is stubbed to avoid pulling in vox-populi hardware registry.

/// CLI / env device intent for the Candle backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceKind {
    /// Prefer host CPU / software paths.
    Cpu,
    /// Let the stack pick (Candle CUDA → Metal → CPU when available).
    #[default]
    Best,
    /// Prefer NVIDIA CUDA.
    Cuda,
    /// Prefer Apple Metal (macOS).
    Metal,
}

/// Best-effort local GPU description.
#[derive(Debug, Clone)]
pub struct GpuInfo {
    pub model_name: String,
    pub vram_mb: u64,
    pub vendor: String,
}

/// Minimal GPU probe — returns unknown vendor when no probe is possible.
/// SP3-C stub: hardware registry is a vox-populi concern; reconnect via host capability in sub-batch D.
#[must_use]
pub fn probe_gpu() -> GpuInfo {
    GpuInfo {
        model_name: "unknown".to_string(),
        vram_mb: 0,
        vendor: "unknown".to_string(),
    }
}

/// CUDA driver-API wrapper for releasing unused VRAM from the device's async memory pool.
///
/// Long-running QLoRA jobs accumulate freed-but-not-returned allocations in the CUDA
/// stream-ordered memory pool. Periodically calling `cuMemPoolTrimTo(pool, 0)` returns
/// that memory to the OS, lowering RSS on the host and keeping the GPU available for
/// co-tenants. Gated on the `cuda` feature so non-CUDA builds compile without a CUDA
/// toolchain.
#[cfg(feature = "cuda")]
pub mod mem_pool {
    //! Bindings against the three CUDA driver entry points needed for pool trimming.
    //!
    //! We link against the CUDA driver library (`libcuda.so` / `nvcuda.dll`); the
    //! `candle-core/cuda` feature already requires this library at build/run time, so
    //! no additional toolchain step is introduced.

    use std::ffi::c_void;

    /// Opaque CUDA device handle (`CUdevice` per the CUDA Driver API).
    type CUdevice = i32;
    /// Opaque CUDA memory-pool handle (`CUmemoryPool`).
    type CUmemoryPool = *mut c_void;
    /// CUDA driver result code (`CUresult`).
    type CUresult = i32;

    /// `CUDA_SUCCESS == 0` per `cuda.h`.
    const CUDA_SUCCESS: CUresult = 0;

    #[link(name = "cuda")]
    unsafe extern "C" {
        fn cuCtxGetDevice(device: *mut CUdevice) -> CUresult;
        fn cuDeviceGetMemPool(pool: *mut CUmemoryPool, dev: CUdevice) -> CUresult;
        fn cuMemPoolTrimTo(pool: CUmemoryPool, min_bytes_to_keep: usize) -> CUresult;
    }

    /// Trim the current context's default memory pool, retaining at most `retain_bytes`.
    ///
    /// Caller must already hold a live CUDA context (any candle CUDA tensor op establishes one).
    /// Returns a `String` error rather than a typed error: this is best-effort housekeeping
    /// and the training loop logs + continues on failure.
    pub fn trim_default_pool(retain_bytes: u64) -> Result<(), String> {
        // SAFETY: each FFI call has its single out-parameter checked for CUDA_SUCCESS
        // before subsequent calls dereference the returned handles. cuMemPoolTrimTo
        // accepts a `usize` byte count; `as usize` is the standard pattern on the
        // platforms candle supports (Linux x86_64, Windows x86_64, both 64-bit).
        unsafe {
            let mut device: CUdevice = 0;
            let r = cuCtxGetDevice(&mut device);
            if r != CUDA_SUCCESS {
                return Err(format!("cuCtxGetDevice failed: code={r}"));
            }
            let mut pool: CUmemoryPool = std::ptr::null_mut();
            let r = cuDeviceGetMemPool(&mut pool, device);
            if r != CUDA_SUCCESS {
                return Err(format!("cuDeviceGetMemPool failed: code={r}"));
            }
            let r = cuMemPoolTrimTo(pool, retain_bytes as usize);
            if r != CUDA_SUCCESS {
                return Err(format!("cuMemPoolTrimTo failed: code={r}"));
            }
        }
        Ok(())
    }
}
