//! Link-only stubs for candle-kernels MoE entry points.
//!
//! The patched `candle-kernels-0.9.2` is intentionally a pure-Rust crate that
//! bundles pre-compiled PTX and skips nvcc — but it leaves `extern "C"`
//! declarations for `moe_gemm_wmma`, `moe_gemm_gguf`, and `moe_gemm_gguf_prefill`
//! in `src/ffi.rs`. `candle-nn`'s `cuda`-feature codepath references those
//! symbols unconditionally even on dense models, so linking the `cuda`-feature
//! cdylib fails with three LNK2019 errors despite the runtime never calling
//! them for a dense Qwen-class model.
//!
//! Path chosen: provide `#[unsafe(no_mangle)] pub extern "C" fn` stubs here
//! that satisfy the linker and panic loudly on the impossible case where MoE
//! IS reached at runtime. This keeps `patches/candle-kernels-0.9.2/` pristine
//! (it stays a vendored upstream copy) and localizes the workaround to the
//! one plugin crate that actually links candle-nn with cuda. When candle
//! ships proper feature gates for `moe`, delete this file + remove the
//! `mod moe_stubs` reference from `lib.rs`.
//!
//! Signatures mirror `patches/candle-kernels-0.9.2/src/ffi.rs` verbatim.

use core::ffi::c_void;

const PANIC_MSG: &str = "moe_gemm_* called: this plugin links candle-nn against pure-Rust \
                         candle-kernels-0.9.2 (no MoE kernels). Qwen-class dense models \
                         must not reach this path. If you hit this, you are training a \
                         Mixture-of-Experts model and need the upstream nvcc-compiled \
                         MoE CUDA kernels — re-enable them in patches/candle-kernels-0.9.2 \
                         or use a non-MoE base model.";

#[unsafe(no_mangle)]
pub extern "C" fn moe_gemm_wmma(
    _input: *const c_void,
    _weights: *const c_void,
    _sorted_token_ids: *const i32,
    _expert_ids: *const i32,
    _topk_weights: *const f32,
    _output: *mut c_void,
    _expert_counts: *mut i32,
    _expert_offsets: *mut i32,
    _num_experts: i32,
    _topk: i32,
    _size_m: i32,
    _size_n: i32,
    _size_k: i32,
    _dtype: i32,
    _is_prefill: bool,
    _stream: i64,
) {
    panic!("{PANIC_MSG}");
}

#[unsafe(no_mangle)]
pub extern "C" fn moe_gemm_gguf(
    _input: *const f32,
    _weights: *const c_void,
    _sorted_token_ids: *const i32,
    _expert_ids: *const i32,
    _topk_weights: *const f32,
    _output: *mut c_void,
    _num_experts: i32,
    _topk: i32,
    _size_m: i32,
    _size_n: i32,
    _size_k: i32,
    _gguf_dtype: i32,
    _stream: i64,
) {
    panic!("{PANIC_MSG}");
}

#[unsafe(no_mangle)]
pub extern "C" fn moe_gemm_gguf_prefill(
    _input: *const c_void,
    _weights: *const u8,
    _sorted_token_ids: *const i32,
    _expert_ids: *const i32,
    _topk_weights: *const f32,
    _output: *mut c_void,
    _num_experts: i32,
    _topk: i32,
    _size_m: i32,
    _size_n: i32,
    _size_k: i32,
    _input_dtype: i32,
    _gguf_dtype: i32,
    _stream: i64,
) {
    panic!("{PANIC_MSG}");
}
