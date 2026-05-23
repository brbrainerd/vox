---
title: Pure-Rust Build — Eliminating C/MSVC Dependencies (2026-05-08)
description: "Pure-Rust workspace build posture and elimination of C/MSVC from default paths (2026 snapshot)."
category: "Architecture SSOTs"
date: 2026-05-08
status: current
training_eligible: true
training_rationale: "Snapshot of pure-Rust build invariant for the workspace; useful when ML or build deps are added."
---

# Pure-Rust Build: Eliminating C and MSVC Dependencies

Goal: build the full Vox workspace — including CUDA-backed ML inference and
QLoRA training — without requiring nvcc, cl.exe, Visual Studio, or any C/C++
compiler in the default build path.

## Before / After

| C dependency | Before | After | Method |
|---|---|---|---|
| `candle-kernels` build.rs (nvcc + cl.exe) | **REQUIRED** — panicked if VS not found | **ELIMINATED** | build.rs rewritten to bundle pre-compiled PTX; removed `bindgen_cuda` + `cc` build-deps |
| `onig_sys` (Oniguruma regex, C) | via `tokenizers` default features | **ELIMINATED** | `tokenizers = { default-features = false, features = ["fancy-regex"] }` |
| `libz-sys` (zlib, C) | via `libgit2-sys` → via `tokenizers`'s onig path | **ELIMINATED** (from tokenizers path) | same tokenizers fix above |
| `flate2` with C zlib backend | default feature pulled `libz-sys` | **ELIMINATED** | `flate2 = { features = ["rust_backend"] }` (miniz_oxide) |
| `blake3` with asm | build-time `cc` for SIMD assembly | **ELIMINATED** | `blake3 = { features = ["pure"] }` |
| `zstd-sys` (C zstd) | via `tantivy-sstable` → `zstd` | **RESIDUAL** (uncontrolled transitive) | See residual section |
| `clang-sys` (libclang, C) | via `turso_core` → `bindgen` | **RESIDUAL** (uncontrolled transitive) | See residual section |
| `libgit2-sys` (libgit2, C) | via `turso_core` → `built` (build-dep) | **RESIDUAL** (uncontrolled transitive) | See residual section |
| `simsimd` (C SIMD) | via `turso_core` | **RESIDUAL** (uncontrolled transitive) | See residual section |
| `ring` (Rust + asm/C) | TLS stack | **RESIDUAL** (by design) | See residual section |

## Pure-Rust CUDA Path

CUDA support works at **runtime** through the driver library (`nvcuda.dll` on
Windows, `libcuda.so` on Linux). The CUDA toolkit (nvcc, cudart, etc.) is
NOT required at build time.

### How it works

1. `candle-kernels` ships pre-compiled PTX files in `src/ptx/`.
   `build.rs` bundles them via `include_str!` — no C compiler needed.
2. `cudarc` (a pure-Rust CUDA runtime wrapper) loads the driver at runtime
   via `dlopen`/`LoadLibrary`. It compiles PTX to native GPU machine code
   using the driver's JIT compiler — no nvcc required.
3. The `MlBackend` trait (`vox-populi`) is unchanged. The CUDA plugin
   (`vox-plugin-mens-candle-cuda`) links to `cudarc` and loads CUDA
   functionality only when `--features cuda` is enabled.

### Regenerating PTX bundles

When `candle` bumps and kernel source changes, PTX must be regenerated once
by a machine with the CUDA toolkit:

```bash
# From a Linux CI runner with nvcc (CUDA 12+):
cd patches/candle-kernels-0.9.2
nvcc --ptx --expt-relaxed-constexpr -std=c++17 -O3 \
     -I src src/affine.cu -o src/ptx/affine.ptx
# ... repeat for each kernel: binary cast conv fill indexing quantized reduce sort ternary unary
```

Commit the updated `.ptx` files. The next build picks them up automatically.
A CI job (CUDA runner) should do this on each candle version bump.

### MOE kernels

The MOE (Mixture-of-Experts) kernels (`src/moe/*.cu`) require WMMA
intrinsics that vary per GPU architecture and cannot be pre-compiled to
portable PTX. They are excluded from the bundled build. If MOE inference
is needed in future, gating it behind a `moe` feature flag and accepting
the nvcc build-dep for that feature only is the right approach.

## Residual C Dependencies

These remain and cannot be eliminated without upstream changes:

### `zstd-sys` (via tantivy → zstd)
- **Path**: `vox-search` → `tantivy` → `tantivy-sstable` → `zstd`
- **Why residual**: `tantivy-sstable` uses `zstd` for SSTable compression.
  The `zstd` crate has no pure-Rust encoder (only `ruzstd` for decode).
  Swapping tantivy is out of scope.
- **Impact**: build-time C compilation of libzstd. Does NOT affect runtime
  GPU/inference path. Only affects search index compression.

### `clang-sys` + `libgit2-sys` + `simsimd` (via turso_core)
- **Path**: `vox-secrets` → `turso` → `turso_core`
- **Why residual**: `turso_core` uses `bindgen` (needs clang-sys) and `built`
  with `git2` feature (needs libgit2-sys). `simsimd` is turso's SIMD-accelerated
  vector distance library. These are internal turso build-time deps we cannot
  configure from our workspace.
- **Impact**: build requires libclang and libgit2 C libraries on the build
  machine. On Windows, these are typically satisfied by LLVM/clang distributed
  with Rust toolchain or MSYS2, not by Visual Studio. They do NOT require nvcc
  or VS Build Tools.
- **Mitigation path**: If turso publishes a `pure-rust` or `no-bindgen` feature
  flag, adopt it. Otherwise, consider vendoring turso with a patched Cargo.toml
  that disables git2/bindgen features in turso_core's build-dep.

### `ring` (crypto)
- **Path**: `rustls` → `ring`
- **Why residual**: `ring` uses a small amount of hand-written assembly (asm)
  for cryptographic primitives (AES-GCM, ChaCha20). This is not C compilation
  — no `cc` invocation — but it is non-Rust source. There is no maintained
  pure-Rust crate with equivalent security guarantees that rustls supports.
- **Impact**: negligible. The asm is in the crate source, compiled by the Rust
  toolchain's bundled LLVM assembler. No external C toolchain required.

### `libbz2-rs-sys`
- **Path**: `zip` → `libbz2-rs-sys`
- **Why not a concern**: Despite the `-sys` suffix, `libbz2-rs-sys` is a
  pure-Rust reimplementation of bzip2. No C compiler needed. Name is misleading.

### `nvml-wrapper-sys`
- **Path**: `vox-populi` → `nvml-wrapper-sys`
- **Why not a concern**: This is a thin Rust FFI wrapper over the NVIDIA
  Management Library. It does NOT invoke a C compiler at build time — it
  uses pre-generated bindings. NVML itself is a runtime library shipped with
  NVIDIA drivers, not a build-time dep.

## CI Implications

After this change, CI can build the full workspace (including `--features cuda`)
on any machine with:
- Rust toolchain (stable)
- libclang (from LLVM, e.g. `choco install llvm` on Windows, `apt install clang`)
- **No** Visual Studio
- **No** CUDA toolkit / nvcc

Runtime CUDA tests still require a CUDA-capable GPU + NVIDIA driver, but those
are isolated to dedicated GPU runners.

### Recommended CI matrix

| Job | Required tools | Tests |
|---|---|---|
| `check-default` | Rust stable | `cargo check --workspace` |
| `check-cuda` | Rust stable | `cargo check -p vox-plugin-mens-candle-cuda --features cuda` |
| `test-unit` | Rust stable + libclang | `cargo test --workspace` |
| `test-gpu` | Rust stable + NVIDIA driver (GPU runner) | plugin integration tests |
| `regen-ptx` | Rust stable + nvcc (triggered manually or on candle bump) | Updates `src/ptx/*.ptx` |
