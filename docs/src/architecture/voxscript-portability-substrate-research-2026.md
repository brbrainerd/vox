---
title: "VoxScript portability substrate: research and findings (2026-09)"
description: "Measured answer to 'make VoxScripts run the same on every system': why one substrate cannot deliver it, what Vox already has, and the cheap fixes that are still available."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
---

# VoxScript portability substrate — research and findings

**Date:** 2026-09-05 · **Method:** five parallel web-research waves plus a direct audit of
this repository, with claims verified on this machine where verification was possible.

## Why this document exists

The stated goal was: *make VoxScripts run the same on every system, on every CPU and GPU
architecture, or as many as one maintainer can support.* The question asked was whether
compiling to machine code (or something similar) would make containers and sandboxes
unnecessary for that purpose.

The short answer is that **"runs the same everywhere" is two goals wearing one name**, and
they have opposite solutions:

| Goal | What it means | What delivers it | What does not |
|---|---|---|---|
| **Deterministic compute** | same input → same bytes out, on any machine | a defined abstract machine (WASM's deterministic profile), plus language-level rules | native code — the divergence is above codegen |
| **Universal availability** | the script runs *at all*, everywhere | native compilation per target | WASM — no subprocess, no threads, no GPU |

No substrate delivers both. That is not a gap waiting to be closed; it is the shape of the
problem, and every system that achieved cross-architecture bit-identity did so by giving up
availability on purpose.

---

## Part 1 — Audit of what Vox has today

Verified against the working tree on 2026-09-05.

### 1.1 There are three execution tiers, not two

| Tier | Path | Requires |
|---|---|---|
| **Interpreter** | `vox_compiler::eval::Interpreter`, tree-walking HIR, 10,000,000-step budget | nothing beyond the `vox` binary |
| **Native** | codegen → Rust → `cargo` → host binary | cargo + host toolchain |
| **WASI** | codegen → Rust → `cargo --target wasm32-wasip1` → Wasmtime | cargo + the wasm target |

The interpreter is the fallback when the `script-execution` feature is absent
(`crates/vox-cli/src/commands/run.rs`), and it is the only tier that needs no toolchain.
It is therefore the *de facto* portability substrate already, whether or not it was
designed as one.

### 1.2 Nothing verifies the tiers agree

`crates/vox-integration-tests/tests/golden_behavioral_gate.rs` runs goldens under
`vox run --mode interp` **only**. No test runs one script through two tiers and compares
output. The goal "VoxScripts run the same everywhere" currently has **zero enforcement** —
not across tiers on one machine, let alone across architectures.

This is the cheapest high-value gap in the entire investigation.

### 1.3 The capability gate is fail-open (verified by execution)

`// vox:caps` is Vox's existing capability declaration. Two scripts were run on this
machine against `target/debug/vox`:

```text
A) no `// vox:caps` line, calls fs.read   → "NO-DIRECTIVE fs.read reached the filesystem"
B) declares `env` only, calls fs.read     → "Capability denied: ... 'fs' namespace"
                                             "DECLARED-caps-env, then did fs.read anyway"
```

Three defects, all confirmed:

1. **The gate engages only when the script declares it.** `interp.caps` is `None` without
   the directive, and the check at `crates/vox-compiler/src/eval/builtins.rs` is guarded by
   `if let Some(c) = caps`. The subject declares its own restrictions.
2. **Denial is not fatal.** It prints, returns `VoxValue::Null`, and execution continues —
   B's second line ran. It also writes to stdout, polluting program output.
3. **Coverage is partial.** Only `fs`, `io`, `process`, `env`, `secrets` are gated. **`net`
   and `http` are not.** `eval/repo.rs` bypasses the gate entirely, per its own source
   comment at `crates/vox-compiler/src/eval/mod.rs:569`.

`// vox:caps` is advisory. It cannot be used as a security boundary in its present form.

### 1.4 The interpreter bounds CPU but not memory (verified by execution)

```text
infinite loop        → Error: StepLimitExceeded   (0.75 s)      ✓ bounded
unbounded allocation → still running at 60 s, killed externally  ✗ unbounded
```

The step budget stops a spin loop. Nothing caps heap growth, so a script can exhaust host
memory well inside its step budget.

### 1.5 The wasm engine advertises limits it does not enforce

`crates/vox-wasm-engine/`:

- **No memory limit.** No `ResourceLimiter`, no `StoreLimits`.
- **No wall-clock limit.** No epoch interruption. `wall_ms` is measured, never bounded.
- **Fuel only**, and only when constructed via `WasmHost::with_fuel`.
- `WasiBackend::execute` (`backend/wasi.rs`) calls `WasmHost::new()` with
  `fuel_override: None`, so **`vox run --backend wasi` runs with no limits at all.**
- **Bug:** `engine.rs` computes `self.fuel.or(opts.fuel_override)`, so host fuel always
  wins. The documented per-run override (`exec.rs`) is dead, and
  `RunOpts::cpu_limit_fuel` is silently ignored on every call.

`vox-mesh-transport::JobLimits` already puts `wall_clock: 300s` on the wire. Wiring the
mesh to this engine today would ship a sandbox that cannot enforce the limits it advertises.

### 1.6 macOS has no process sandboxing; Windows has less than it appears

`crates/vox-cli/src/commands/runtime/run/sandbox.rs`:

- **Linux:** Landlock ABI V3, fails closed. Genuine.
- **Windows:** Job Objects only. The source states outright that **filesystem restrictions
  are not enforced**.
- **macOS:** nothing. Prints a warning and sets `VOX_SANDBOX=1` as, quoting the file, an
  "informational hint only."

### 1.7 Four overlapping isolation abstractions already exist

| Where | Type |
|---|---|
| `vox-skill-runtime::runtime` | `trait SkillRuntime` + `enum Tier { BareMetal, Wasm, Container, MicroVm }` |
| `vox-cli::isolation` | `enum IsolationPolicy { Permissive, Container, Gvisor, MicroVM, Wasm }` + `IsolationCapabilities::detect()` |
| `vox-mesh-transport::protocol` | `enum Isolation { Wasm, Container, Native }` |
| `vox-container-types` | `ContainerRuntime` trait, `RunOpts::sandboxed()`, `DEFAULT_SANDBOX_*` |

`vox-container::detect::detect_runtime` **already prefers Podman (rootless) over Docker**.
A fifth abstraction is not needed; consolidation is.

### 1.8 The transcendental bug has not landed yet

Vox's float method surface in `crates/vox-compiler/src/eval/builtins.rs` is exactly
`abs`, `floor`, `ceil`, `round`, `sqrt`. **All five are IEEE-754-exact and guaranteed by
Rust.** There is no `sin`, `cos`, `pow`, `exp`, or `log`.

This matters enormously — see §2.4. The single largest source of cross-platform divergence
has not yet been introduced into the language, and the window to prevent it is open.

---

## Part 2 — Research findings

### 2.1 WASM is the only substrate that delivers bit-identity — and it cannot run Vox's scripts

**It genuinely is deterministic.** The Wasm 3.0 core spec pins every non-NaN float result
to IEEE-754 round-to-nearest-ties-to-even. The exhaustive nondeterminism list is three
items: NaN payload/sign bits, relaxed-SIMD, and shared-memory/`grow`/host-call results.
Core WASM has **no scalar FMA instruction**, which is exactly why it avoids the `a*b+c`
contraction divergence that breaks native builds across architectures. Wasm 3.0 defines a
*deterministic profile*; Wasmtime exposes `cranelift_nan_canonicalization` and
`relaxed_simd_deterministic`. **This is a stronger guarantee than native Rust can give.**

**It cannot run the corpus.** There is **no WASI proposal for process spawn at any phase**,
and none planned. This repository's own house rules mandate `.vox` glue for CI prep,
install helpers, and data migrations — scripts whose entire purpose is invoking `cargo`,
`git`, and `pnpm`. Those can never run under WASM. Additionally:

- **No parallelism.** `wasi-threads` was withdrawn in 2023; `shared-everything-threads` is
  unimplemented in Wasmtime; WASIp3's answer is *cooperative* stack-switching, and
  Wasmtime's implementation is work-in-progress and x86_64-Linux-only.
- **No GPU compute.** `wasi-nn` is a host-side inference RPC (Phase 2, ONNX backend on an
  RC). `wasi:webgpu` is Phase 2, has one demo, is **not in upstream Wasmtime**, and its
  surrounding stack was formally ejected from WASI governance in June 2026 for instability.

**The tax is 1.46×–2.41× native**, measured across a real workload suite. AOT does not
help: Wasmtime's AOT output *is* Cranelift output, and Cranelift is architecturally a fast
compiler rather than a good one (~14% behind LLVM-based wasm compilation). Worse, the
features that recover 2.41× → 1.46× are SIMD-family, and **determinism requires disabling
relaxed-SIMD**. Identical-everywhere and 1.46× are mutually exclusive.

**Correction to current practice:** Vox targets `wasm32-wasip1`, which rustc documents as
legacy ("intended for historical compatibility", no new APIs). **`wasm32-wasip2` is Tier 2
with fully-supported `std`.** Do not plan around `wasm32-wasip3`: Tier 3, and per its own
docs it does not build.

### 2.2 Native compilation fixes almost none of the divergence

The things that make one script behave differently on two machines live **above** codegen:

| Source | Rust normalises? |
|---|---|
| Transcendentals (`sin`, `cos`, `pow`, `exp`, `ln`, …) | **No** — documented "precision is non-deterministic … varies by platform, Rust version, and can even differ within the same execution" |
| NaN payload / sign | **No** — RFC 3514 makes it formally non-deterministic |
| Integer overflow | **No** — panics in debug, wraps in release (RFC 560) |
| HashMap iteration order | **No** — deliberately randomised *per instance*, differs between runs on one machine |
| Filesystem case sensitivity | **No** — APFS insensitive, ext4 sensitive, NTFS insensitive |
| Unicode normalisation of filenames | **No** — HFS+ NFD, APFS normalisation-insensitive, ext4 opaque bytes |
| `SystemTime` resolution | **No** — Windows ~15.6 ms tick vs Linux ns |
| Thread scheduling | **No** |
| FMA contraction | **Yes** — Rust does *not* auto-contract; `mul_add` is opt-in. Rust is on the good side here. |
| x87 excess precision | Only an `i586` problem. Do not ship `i586`. |

**AOT changes which instruction encoding runs. It does not change which `sin` you called.**

### 2.3 The TypeScript backend is a second, independent divergence source

ECMA-262 makes `Math.sin`, `Math.cos`, `Math.pow`, `Math.exp`, `Math.log`
**implementation-approximated**: behaviour "is not precisely specified except to require
specific results for certain argument values … some latitude is allowed in the choice of
approximation algorithms."

Vox compiles to **both** Rust and TypeScript. If transcendentals are added, Vox acquires
two independent divergence sources that **will not agree with each other either** — the
same VoxScript giving three answers (native Rust, TS on V8, wasm) is the default outcome,
not the edge case.

### 2.4 What every system that achieved bit-identity actually did

This is the strongest evidence in the report, because consensus VMs need exactly this
property and will pay any price for it. **None chose native code.**

| System | Choice |
|---|---|
| **Ethereum EVM** | 256-bit integers only. **No floating point at any width.** |
| **CosmWasm** | Rust → Wasm, and the VM **refuses to implement float instructions at all** |
| **Solana SBF** | floats route through software libcalls; docs warn against using them, because "if the underlying LLVM compiler optimizes a float operation differently during a runtime upgrade, validators might disagree" |
| **WebAssembly** | enumerates its nondeterminism exhaustively and ships a deterministic profile |

Every one achieved cross-architecture bit-identity by **restricting the language and
running on a defined abstract machine**. Tellingly, relaxed-SIMD is the one place WASM
*chose* to abandon determinism, specifically because some SIMD operations "cannot be made
to execute both identically and performantly across different architectures." That is the
entire tension in one sentence.

### 2.5 Portable GPU compute does not exist at acceptable cost

- **MLX — Apple's own library, with an Apple-written CUDA backend — has divergent
  quantization formats and sampling defaults across its two backends.** Same script,
  different tokens. If Apple cannot make one library agree with itself across two GPUs, a
  portable abstraction will not.
- **The portable tax is 2×–10×**, measured across 16 devices and 8 vendors: WebGPU is up to
  10× slower at prefill than CUDA and >2× than Metal. Even the *good* portable path
  (Vulkan, not WebGPU) costs ~36% prefill on NVIDIA. Decode ports nearly free; prefill does
  not — and prefill is what long prompts and RAG feel.
- **Every portable stack's fast path is vendor-specific code in disguise.** CubeCL matches
  cuBLAS using *inlined PTX*; its Vulkan path is stuck at line size 4, its ROCm path is
  blocked on HIP compiler bugs, its Metal path has no published numbers. CubeCL is
  self-declared alpha. Portability relocated the per-vendor work into a repository you
  cannot schedule.
- **Nobody ships production inference through a single abstraction.** llama.cpp, vLLM, and
  stable-diffusion.cpp all maintain separate CUDA/Metal/Vulkan backends deliberately.
- **candle's Metal GEMM is 2–11× slower than MLX** (fixed tile config vs dynamic), which
  disqualifies the obvious Rust choice for anything GEMM-heavy.

### 2.6 GPU in containers: possible in both directions, but never by one mechanism

| Direction | Mechanism | Result |
|---|---|---|
| Windows/WSL2 → container | podman + NVIDIA CDI | real CUDA |
| macOS → container | podman machine + libkrun/krunkit → Venus → MoltenVK | **Vulkan compute only, 50–80% of native Metal** |
| macOS → Metal / MLX / CoreML in a container | — | **architecturally impossible** |

The last row is not a maturity gap. A Linux guest has no Metal driver, Apple Silicon
exposes no IOMMU for passthrough, and `Virtualization.framework` offers no GPU passthrough.
Docker ship `vllm-metal` running natively on the host for precisely this reason. **Put MLX
or PyTorch-MPS in a Linux container on a Mac and it silently falls back to CPU** — no
error, roughly 30× slower.

Apple has effectively declined: `apple/containerization#46` (GPU access) is closed
**`wontfix`**; `apple/container#1511` (`--gpus`) has sat since 2026-05-06 with no
maintainer response.

**Therefore "first-class Metal" and "containerised" are mutually exclusive on macOS.**

Also load-bearing for this hardware: the Quadro T1000 is Turing, 4 GB VRAM, no FP8 — about
a 7–8B model at Q4 with short context. **The VRAM ceiling binds long before virtualisation
overhead does.** On WSL2, `--gpus all` is the only supported form, NVML queries do not work,
`libcuda.so` is not in the ldcache, and the WSL2 VM itself costs 20–40% on inference while
the container on top costs approximately nothing.

### 2.7 macOS Seatbelt works, including with Metal (verified on this machine)

A spike ran MLX under `sandbox-exec` with a `(deny default)` profile:

| | Unsandboxed | Sandboxed |
|---|---|---|
| network → 1.1.1.1:53 | ALLOWED | **BLOCKED** |
| write `$HOME` | ALLOWED | **BLOCKED** |
| write `/private/tmp` (profile-allowed) | ALLOWED | ALLOWED |
| **MLX matmul** | OK, `Device(gpu, 0)` | **OK, `Device(gpu, 0)`** |

The unsandboxed column is the negative control; without it, "blocked" would be
indistinguishable from "never ran". **Native Metal survives a seatbelt sandbox that denies
network and filesystem.** Chromium's GPU process is the production existence proof.

**The spike's blanket allows were not incidental — they are the finding.** The profile
allowed `mach-lookup` and `iokit-open` **unrestricted**, and independent evidence says that
is not a shortcut but the *known workaround*: `openai/codex#17644` (2026-04-13, macOS 26.4,
MLX 0.31.0) reports that a profile allowing only
`(allow iokit-open (iokit-registry-entry-class "RootDomainUserClient"))` makes
`MTLCopyAllDevices()` return an empty array and **MLX abort with SIGABRT**. The confirmed
fix is unrestricted `(allow iokit-open)`.

So the honest statement is narrower than "Metal survives a sandbox": **on macOS, GPU access
and Seatbelt confinement are in direct tension, and the working fix widens exactly the
surface being sandboxed against.** A GPU user-client is a large kernel attack surface and
historically the most-exploited path out of every sandbox on every platform. Enumerating
the specific GPU IOKit classes (e.g. `AGXDeviceUserClient`) instead of blanket-allowing is
required work, not polish. Chromium's GPU process is the production existence proof that it
can be done — and notably Chromium runs its GPU process at *lower* confinement than its
renderers for exactly this reason.

`sandbox-exec` is also deprecated (bind `sandbox_init_with_parameters` instead). Apple has
published **no removal date and no replacement**; `apple/containerization#737` asks exactly
this and is unanswered. Chromium, Bazel, and both major agent CLIs depend on the same
primitive, which is the practical reason it has not been removed.

### 2.8 There is no maintained cross-platform Rust sandboxing crate

| Crate | State |
|---|---|
| `landlock` | **Healthy.** v0.4.7 (2026-07-27), ~1.4M downloads/mo, maintained by the Landlock author. Linux only. |
| `cap-std` | Healthy, but **cooperative and in-process** — removes ambient authority from *trusted* code. Enforces nothing against a hostile process. Not a boundary. |
| `birdcage` | **DEAD** — archived 2026-07-06. Never supported Windows. |
| `gaol` | **DEAD** — last release 2019. |
| `extrasafe` | **STALE** — April 2024, **x86_64 only**. |

Expect to write the macOS profile generation and Windows token/ACL code directly, or shell
out — which is what everyone else does.

Platform ceilings worth stating plainly — **all three platforms have a network-isolation
hole**:

- **Linux:** Landlock's network control is TCP-only (no UDP, therefore **no DNS**), and it
  arrived in ABI v4 / kernel 6.7.
- **WSL2 specifically:** the default kernel in 2026 is **6.6.x**, which is *below* ABI v4 —
  so on the Windows machine, Landlock gives filesystem restriction and **no network
  restriction at all**. (Inferred from the ABI/kernel table rather than measured; worth a
  direct ABI probe inside WSL2 before relying on it either way.)
- **Windows:** network isolation of a child process **requires administrator rights** to
  install firewall deny rules. Codex CLI's unelevated fallback ships with no network
  boundary, knowingly. Job Objects cannot help — they have no filesystem or network
  facility at all; AppContainer can, but OpenAI rejected it because it assumes the
  contained process needs no host filesystem access, which is false for a dev tool.

Anthropic's own runtime documents its Linux proxy enforcement as env-var-based and
therefore bypassable by code that ignores the variables. **Env-var proxying is a boundary
against careless code, not hostile code.**

### 2.9 Optional container runtime: detect, never install

- **Docker Desktop is not redistributable** (Docker Subscription Service Agreement,
  effective 2026-08-26). Free use is limited to non-commercial open source, or companies
  with **<250 employees and <$10M revenue** — either threshold trips it. **Government
  entities get no free tier at any size.** Docker Engine/CLI/Moby remain free with no
  thresholds.
- **Do not recommend OrbStack as "the free alternative"** — OrbStack Free is personal,
  non-commercial only, with no small-company carve-out. Its restriction is *tighter* than
  Docker's.
- **Every macOS/Windows option costs a multi-GB Linux VM image**, because they all run a
  Linux VM. Bundling a runtime means shipping and maintaining that VM forever; Rancher
  Desktop and Podman Desktop are staffed products largely *about* that VM.
- **No path exists where an installer silently gains container capability without a
  privilege prompt.** `wsl --install` requires admin; there is no supported non-admin route.
- **Probe by connecting, not by `stat`.** Fedora's `podman-docker` leaves
  `/var/run/docker.sock` as a dangling symlink to a socket that does not exist, because
  podman is daemonless — so `Lstat`-based detection *lies*.
- **Precedence order** (agreed by act, testcontainers, and trivy): explicit flag → explicit
  env (`DOCKER_HOST`, `CONTAINER_HOST`) → config file → well-known socket paths → fail with
  instructions. Never let auto-detection beat an explicit setting.
- **Be Trivy, not Dev Containers:** absence removes a capability and is reported by
  `vox doctor`; it never fails install or startup.
- **No privileged sidecars** — testcontainers' Ryuk reaper is the most-documented cause of
  "podman doesn't work".
- **`podman machine start` reliability is an open defect class**, still receiving fixes in
  the September 2026 release. It will dominate support load if depended upon.

Sobering signal: act, Dagger, and Testcontainers are all well-resourced and **none has
finished** runtime detection — each carries years-old open issues.

---

## Part 3 — What follows

Ordered by leverage per unit of work. Nothing here is scheduled; this document records
findings, not commitments.

### 3.1 Cheap, high leverage, available now

1. **Add a cross-tier differential gate.** Run each golden through interpreter and native
   (and wasm where applicable) and compare stdout. Without this, "runs the same" is an
   aspiration with no test behind it. This is the single highest-value item in the document
   (§1.2).
2. **Ship Vox's own libm and route `math.*` through it on both backends.**
   `rust-lang/libm` (now in `compiler-builtins`) is a Rust port of musl's and gives
   identical bits on every target. Doing this **before** transcendentals are added converts
   the largest divergence class into a non-problem, on both the Rust and TypeScript
   backends (§1.8, §2.2, §2.3). The window closes the day someone adds `math.sin`.
3. **Specify determinism in the language, as the consensus VMs did** (§2.4): force
   `overflow-checks` in every profile or emit explicit `checked_*`/`wrapping_*`; make map
   iteration order defined or inaccessible; NFC-normalise paths; forbid raw `SystemTime` in
   deterministic contexts. This is lint work on infrastructure that already exists in
   `vox-code-audit`.

### 3.2 Correctness debt that blocks anything built on top

4. **Give `vox-wasm-engine` the limits it claims** — `StoreLimits` for memory, epoch
   interruption for wall-clock, and fix the fuel-precedence bug (§1.5).
5. **Bound interpreter memory**, not just steps (§1.4).
6. **Make `// vox:caps` a boundary**: receiver-imposed rather than script-declared, fatal
   rather than advisory, covering `net`/`http`, with no bypass paths (§1.3).
7. **Consolidate the four isolation abstractions** into one, rather than adding a fifth
   (§1.7).

### 3.3 Platform work, if GPU sharing is pursued

8. **macOS: native + seatbelt**, binding `sandbox_init_with_parameters`, with enumerated
   GPU IOKit classes rather than the spike's blanket `(allow iokit-open)` (§2.7). This is
   the only route to first-class Metal/MLX and it fills a real hole (§1.6) — but the
   enumeration is the hard part, not a detail, and until it is done the sandbox is
   substantially weaker than it looks.
9. **Windows/Linux: podman + CDI**, detected never installed, degrading to a reported
   missing capability (§2.6, §2.9).
10. **Do not pursue a single portable GPU abstraction.** Define the workloads as a Vox-level
    interface and dispatch to per-vendor engines that already maintain the backend split
    (§2.5).

### 3.4 The reframing that makes the goal achievable

"Identical" cannot mean bit-identical GPU numerics — nobody has that, including Apple
across its own two backends (§2.5). It can mean an **identical observable contract**: same
artifact, same tokenizer, same seeded sampler, results asserted within a stated tolerance,
enforced by the differential gate in §3.1. That is a goal a solo maintainer can hold and
test. Bit-identity is achievable only on the CPU tier, only under a deterministic abstract
machine, and only by giving up subprocess, threads, and GPU.

---

## Sources

WASM and determinism: [Wasm 3.0 numerics](https://webassembly.github.io/spec/core/exec/numerics.html) ·
[Nondeterminism.md](https://github.com/WebAssembly/design/blob/main/Nondeterminism.md) ·
[Wasmtime deterministic execution](https://docs.wasmtime.dev/examples-deterministic-wasm-execution.html) ·
[WASI proposal phases](https://github.com/WebAssembly/WASI/blob/main/docs/Proposals.md) ·
[WASI 0.3](https://wasi.dev/releases/wasi-p3) ·
[rustc wasm32-wasip1](https://doc.rust-lang.org/rustc/platform-support/wasm32-wasip1.html) ·
[wasip2](https://doc.rust-lang.org/nightly/rustc/platform-support/wasm32-wasip2.html) ·
[wasip3](https://doc.rust-lang.org/rustc/platform-support/wasm32-wasip3.html) ·
[Cranelift vs LLVM](https://github.com/bytecodealliance/wasmtime/blob/main/cranelift/docs/compare-llvm.md) ·
[WebAssembly runtime performance 2026](https://00f.net/2026/06/23/webassembly-runtimes-2026/) ·
[wasm2c 2026](https://00f.net/2026/07/08/webassembly-compilation-to-c-2026/)

Float and platform divergence: [Rust f64 docs](https://doc.rust-lang.org/std/primitive.f64.html) ·
[RFC 3514 float semantics](https://rust-lang.github.io/rfcs/3514-float-semantics.html) ·
[RFC 560 integer overflow](https://rust-lang.github.io/rfcs/0560-integer-overflow.html) ·
[rust-lang/libm](https://github.com/rust-lang/libm) ·
[std RandomState](https://doc.rust-lang.org/src/std/hash/random.rs.html) ·
[ECMA-262 Numbers and Dates](https://tc39.es/ecma262/multipage/numbers-and-dates.html) ·
[APFS FAQ](https://developer.apple.com/library/archive/documentation/FileManagement/Conceptual/APFS_Guide/FAQ/FAQ.html)

Consensus VMs: [CosmWasm floating point](https://book.cosmwasm.com/basics/fp-types.html) ·
[Solana program limitations](https://solana.com/docs/programs/limitations)

Portable GPU: [Llamas on the Web (arXiv 2605.20706)](https://arxiv.org/html/2605.20706v1) ·
[Burn 0.20.0](https://burn.dev/blog/release-0.20.0/) ·
[SOTA multiplatform matmul](https://burn.dev/blog/sota-multiplatform-matmul/) ·
[CubeCL](https://github.com/tracel-ai/cubecl) ·
[wgpu Features](https://docs.rs/wgpu/latest/wgpu/struct.Features.html) ·
[candle Metal GEMM #3302](https://github.com/huggingface/candle/issues/3302) ·
[MLX on CUDA](https://github.com/ml-explore/mlx/discussions/2422) ·
[IREE deployment](https://iree.dev/guides/deployment-configurations/)

GPU in containers: [Red Hat: AI inference on macOS Podman](https://developers.redhat.com/articles/2025/06/05/how-we-improved-ai-inference-macos-podman-containers) ·
[Red Hat: native speed llama.cpp container inference](https://developers.redhat.com/articles/2025/09/18/reach-native-speed-macos-llamacpp-container-inference) ·
[llama.cpp M-series container benchmarks](https://github.com/ggml-org/llama.cpp/discussions/12985) ·
[Podman Desktop GPU](https://podman-desktop.io/docs/podman/gpu) ·
[Docker: vLLM Metal on macOS](https://www.docker.com/blog/docker-model-runner-vllm-metal-macos/) ·
[apple/containerization#46](https://github.com/apple/containerization/issues/46) ·
[apple/container#1511](https://github.com/apple/container/issues/1511) ·
[CUDA on WSL user guide](https://docs.nvidia.com/cuda/wsl-user-guide/index.html) ·
[NVIDIA Container Toolkit release notes](https://docs.nvidia.com/datacenter/cloud-native/container-toolkit/latest/release-notes.html)

OS-native sandboxing: [Chromium Mac Seatbelt design](https://github.com/chromium/chromium/blob/main/sandbox/mac/seatbelt_sandbox_design.md) ·
[apple/containerization#737](https://github.com/apple/containerization/issues/737) ·
[landlock(7)](https://man7.org/linux/man-pages/man7/landlock.7.html) ·
[rust-landlock](https://github.com/landlock-lsm/rust-landlock) ·
[birdcage (archived)](https://github.com/phylum-dev/birdcage) ·
[anthropic-experimental/sandbox-runtime](https://github.com/anthropic-experimental/sandbox-runtime/blob/main/README.md)

Distribution: [Docker Subscription Service Agreement](https://www.docker.com/legal/docker-subscription-service-agreement/) ·
[OrbStack pricing](https://orbstack.dev/pricing) ·
[podman-for-windows](https://github.com/containers/podman/blob/main/docs/tutorials/podman-for-windows.md) ·
[act docker_socket.go](https://github.com/nektos/act/blob/master/pkg/container/docker_socket.go) ·
[act #2393](https://github.com/nektos/act/issues/2393) ·
[Trivy container image targets](https://trivy.dev/docs/latest/guide/target/container_image/) ·
[WSL install](https://learn.microsoft.com/en-us/windows/wsl/install)
