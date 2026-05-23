---
title: "vox-container vs WASM Sandbox (2026-05-08)"
description: "Audit of vox-container's two distinct roles (deployment-codegen vs runtime sandbox) and an evaluation of WASM as a replacement for the runtime path."
category: "Architecture SSOTs"
status: "research"
training_eligible: true
training_rationale: "Architectural reasoning for sandbox-runtime selection; clarifies the dual role of vox-container and the bounds of WASM as a sandbox for arbitrary Vox skill code."
---

# vox-container vs WASM Sandbox

> User question: *"Is Podman and Docker really useful for anything if we can just use WASM as a sandbox for execution? Is that more desirable and smaller, or will there be things that are already part of the Vox language that won't run inside the WASM container?"*

Short answer: **vox-container does two unrelated jobs, and WASM only addresses one of them — partially.** The right move is to split vox-container, keep deployment-codegen as-is, and add WASM as a *first-tier* sandbox for the runtime path while keeping Docker/Podman available as a *second-tier* fallback for skills WASM can't host.

---

## 1. What vox-container actually does (2,270 LOC)

It conflates **two architecturally distinct concerns** that share a name and a crate but not a runtime:

### Concern A — Deployment artifact codegen (~1,119 LOC, ~49%)

| File | LOC | Job |
|---|---:|---|
| `deploy_target.rs` | 620 | Emit Fly.io / Coolify / Kubernetes / Compose / bare-metal targets |
| `generate.rs` | 407 | Translate Vox `environment` declarations → Dockerfile |
| `bare_metal.rs` | 92 | Generate systemd unit files |

This is **pure code generation**. It produces text files (Dockerfile, compose.yaml, k8s manifests, systemd unit files) that users feed to *their* deployment platform. There is no runtime here. The user runs `vox deploy --target fly` and gets a Fly app spec. Replacing this with WASM is a category error — nobody deploys to "WASM cloud" the same way they deploy to AWS/GCP/Fly.

**Verdict for Concern A**: keep as-is. Maybe rename to `vox-deploy-codegen` for clarity. Not in scope for the WASM question.

### Concern B — Runtime sandbox for skill execution (~541 LOC, ~24%)

| File | LOC | Job |
|---|---:|---|
| `docker.rs` | 166 | `DockerRuntime` impl |
| `podman.rs` | 169 | `PodmanRuntime` impl |
| `runtime.rs` | 79 | `ContainerRuntime` trait + `BuildOpts` / `RunOpts` |
| `detect.rs` | 127 | Auto-detect Docker vs rootless Podman |

Used by `vox-skills/src/sandbox/runner.rs` (`SandboxedSkillRunner`) — when a skill is invoked, vox-skills builds the `vox-skill-sandbox` OCI image, runs the skill command inside it under resource/network limits, captures stdout/stderr/exit code.

**This is the only place a real container daemon is required at runtime.** Everything else in vox-container is text generation.

### Concern C — Python env management (~542 LOC, ~24%)

| File | LOC | Job |
|---|---:|---|
| `env.rs` | 315 | `PythonEnv` (uv/venv detection) |
| `pyproject.rs` | 154 | `pyproject.toml` generation (currently mostly retired) |
| `python_dockerfile.rs` | 35 | Python-flavored Dockerfile snippet |
| `setup.rs` | 38 | `run_py_setup` (currently hard-errors per lib.rs note) |

Half-retired Python-environment helpers. Mostly emit Dockerfile fragments. Touch the WASM question only tangentially (Python doesn't run in WASM).

---

## 2. Can WASM replace Concern B?

Concern B is the only part where a sandbox runtime actually executes. Question: can wasmtime/wasmer host arbitrary skill code instead of a Docker container?

### What WASM can do well

- **Pure-compute skills**: text transforms, JSON munging, regex, parsing, classification, formatting → trivial in WASM.
- **HTTP via `wasi-http`**: outbound HTTP works in wasmtime today; inbound HTTP works via Spin/Lunatic-style hosting.
- **Filesystem via WASI preopens**: explicit, capability-bound directory access. Better security model than `--mount` flags.
- **Cold start**: microseconds (vs. seconds for Docker). Memory footprint: small KBs (vs. tens of MB).
- **Distribution size**: a wasmtime runtime is ~5–8 MB statically linked. The Docker daemon alone is hundreds of MB before any image.
- **Cross-platform**: works on machines without Docker/Podman installed. macOS/Windows/Linux/embedded.
- **Pure-Rust**: wasmtime is pure-Rust (Bytecode Alliance). Aligns with the recent VS/C-elimination work.

### What WASM **cannot** do (at least not yet)

These are the items that determine whether WASM is sufficient for **arbitrary** skill code:

| Capability | WASI / WASM status | Impact for Vox skills |
|---|---|---|
| **Subprocess exec** (`fork`/`exec`) | Not in WASI preview-1 or preview-2. No standard API. | A skill that shells out to `git`, `python`, `node`, `npm`, or any CLI tool **cannot run in WASM**. |
| **Threads** | `wasi-threads` exists but immature; not portable across runtimes. | Multi-threaded compute paths (parallel data-loading, async work-stealing) limited or unavailable. |
| **Sockets / arbitrary networking** | `wasi-sockets` (preview-2) is recent and not yet universally implemented. | Custom protocols (gRPC, WebSocket, raw TCP) work only in newer runtimes; HTTP is fine. |
| **GPU access** (CUDA, ROCm) | Not addressable. WebGPU exists in browsers, no WASI mapping. | ML training/inference plugins (`vox-plugin-mens-candle-cuda`, `vox-plugin-tensor-burn-wgpu`) cannot run inside WASM at all. |
| **Dynamic loading** (`dlopen`) | Not in WASM. | Plugin-loading itself can't run nested inside WASM. (Doesn't matter — the plugin host is OUTSIDE the WASM sandbox.) |
| **Raw signals, ptrace, fanotify** | Not in WASI. | Unusual; generally fine to drop. |
| **Native crates with `build.rs` C compilation** | Have to be compiled as WASI-Rust upfront. | Skills written in Rust and compiled to `wasm32-wasi` are fine; Python/Node skills are not. |

### What does the **current** vox-skills sandbox actually run?

`vox-skills/src/sandbox/runner.rs` calls `ContainerRuntime::run()` with arbitrary commands inside the `vox-skill-sandbox` OCI image. The image is described as having Python and other tooling pre-installed (per `image.rs`). This means today's skills routinely:

- Run Python scripts (`python script.py`)
- Invoke CLI tools (git, jq, etc.)
- Read/write files in a working directory
- Make HTTP calls

**Of those four, only file IO and HTTP run in WASM as-is.** Python scripts and CLI invocations require subprocess execution — flatly unsupported in WASI today.

---

## 3. Is WASM more desirable and smaller?

**Smaller**: yes, dramatically. wasmtime ~5MB statically vs. Docker daemon (200MB+) + an image. WASM also needs no daemon — it's an in-process embedding.

**More desirable for the slim-core ethos**: yes, especially given the recent pure-Rust / no-MSVC / no-C work. Docker brings external-toolchain ambient state; wasmtime brings none.

**Strictly better as a replacement**: **no**, because subprocess execution and GPU access are both common needs that WASM cannot fulfill.

---

## 4. The tiered architecture that actually fits

Treat the sandbox runtime as an **abstraction with multiple impls**, picked per-skill based on what the skill needs.

```
┌────────────────────────────────────────┐
│  vox-skills::SandboxedSkillRunner      │
│  (calls trait SkillRuntime)            │
└──────────────┬─────────────────────────┘
               │
               ▼
       ┌───────────────┐
       │ SkillRuntime  │  trait
       └───┬───┬───┬───┘
           │   │   │
   ┌───────┘   │   └────────┐
   ▼           ▼            ▼
WASM        Container    Bare-metal
(default)   (fallback)   (trusted)
wasmtime    Docker/Podman  process
~5MB        ~200MB+        host fork
µs cold     s cold         ms cold
no subproc  full subproc   full subproc
no GPU      GPU passthru   native GPU
no Python   Python OK      Python OK
```

Skill manifests already have a `permissions` / `requirements` block. Extend it:

```yaml
runtime:
  required: [filesystem-rw, http-out]
  forbidden: [subprocess, gpu]
```

With those, the runtime selector picks WASM by default and falls back to a container *only* when a skill declares it needs `subprocess` or `gpu`. Skills that don't declare needs get the cheaper, faster, safer WASM path.

---

## 5. What's "already part of the Vox language" that won't run in WASM?

The user asked specifically. Going through the surfaces:

| Vox capability | Runs in WASM? | Notes |
|---|---|---|
| Pure expression / data ops | ✅ | Anything compiled from `.vox` to native or to TS-then-bundled is candidate for `wasm32-wasi` if the target supports it. |
| Compiler invocation (`vox build`) | ❌ | The compiler is the host, not the sandbox. Out of scope. |
| File operations (read/write within preopens) | ✅ | WASI preopens. |
| HTTP calls | ✅ | wasi-http (with a recent wasmtime). |
| `vox mens` training (Burn / candle) | ❌ | GPU + native ML stacks. Stays in plugin-host process. |
| `vox-plugin-*` loading | ❌ (n/a) | The plugin host is the WASM host; plugins themselves are native cdylibs. WASM-as-sandbox runs *user skill code*, not plugins. |
| Skills that shell out (git, python, etc.) | ❌ | Need container fallback. |
| Skills written purely in Rust → wasm32-wasi | ✅ | Compile-time choice; works great. |
| Skills written in Python / Node / shell | ❌ today | Containers required (or compile to WASM via Pyodide/QuickJS — possible but heavyweight). |
| Network listeners (the new `vox-plugin-webhook`) | Partial | wasi-http components can host listeners; but the webhook plugin is a native plugin, not a WASM-sandbox skill. |
| MCP / orchestrator / dashboard | ❌ (n/a) | Host-side; not sandboxed. |
| GPU paths (CUDA, wgpu) | ❌ | No path. |

---

## 6. Recommendation

Three actions, in order of leverage:

### (1) Split vox-container along its actual concerns

- `vox-deploy-codegen` ← `bare_metal.rs`, `deploy_target.rs`, `generate.rs`, `python_dockerfile.rs`, `pyproject.rs` (~1,500 LOC)
- `vox-skill-runtime` (or fold into `vox-skills`) ← the `ContainerRuntime` trait + `detect.rs` only (~200 LOC)
- `vox-plugin-runtime-container` (new plugin) ← `docker.rs`, `podman.rs` impls (~340 LOC). Opt-in.
- `vox-plugin-runtime-wasm` (new plugin) ← wasmtime-based impl (new, ~500 LOC estimate)

After this split, the slim CLI ships with NO container runtime by default. Users who want container-based skill execution `vox plugin install runtime-container`. Users who want WASM `vox plugin install runtime-wasm`. The `vox-skill-runtime` trait is the SSOT.

### (2) Add WASM as the default sandbox

`vox-plugin-runtime-wasm` using `wasmtime` (pure-Rust). It becomes the **default** sandbox for skills that don't declare `runtime.required: [subprocess, gpu]`. This makes the common case faster, smaller, and more secure. Skills written in Rust and compiled to `wasm32-wasip2` are first-class.

### (3) Keep Docker/Podman for skills that need them

`vox-plugin-runtime-container` is the fallback. Skills that shell out to Python, git, etc. declare `runtime.required: [subprocess]` and get routed to the container runtime. Same for `gpu`.

### What about deployment?

`vox deploy` keeps generating Dockerfile / Compose / Fly / Coolify / k8s artifacts as it does today, because deploying a Vox app to AWS/GCP/Fly is a container-or-VM problem regardless of how skills are sandboxed. **vox-deploy-codegen has nothing to do with WASM** and shouldn't be conflated.

---

## 7. Effort and risk

| Action | Effort | Risk |
|---|---|---|
| Split `vox-container` | M | Low. Mostly file moves + import updates. |
| Add `vox-plugin-runtime-wasm` (wasmtime) | L | Medium. Needs WASI preopen plumbing, fuel/timeout limits, optional wasi-http wiring. |
| Add `vox-plugin-runtime-container` (extract from current docker.rs/podman.rs) | S | Low. Mostly a re-home. |
| Skill manifest schema bump (add `runtime` block) | S | Low if backward-compatible (default = WASM, present `subprocess`/`gpu` keys upgrade to container). |
| Migrate existing skills | varies | Some skills may declare `subprocess` and stay on container. Most can WASM-ify. |

---

## 8. Open questions for product direction

- Should `runtime.required: [subprocess]` skills be **deprecated over time** (push everything to WASM via Pyodide/QuickJS), or stay first-class indefinitely?
- Do we ship a default Rust→WASM build helper for skill authors, or expect them to bring their own?
- For skills that today rely on a pre-built `vox-skill-sandbox` OCI image full of tools, do we offer a "WASM blessed component bundle" with common utilities (jq-style JSON, regex, etc.) compiled to WASM components?

---

## Bottom line

**Docker/Podman still useful?** Yes — for **two** reasons:
1. **Deployment artifact generation** (~half of vox-container) is a totally different concern that has nothing to do with sandboxing. WASM doesn't replace it.
2. **The 24% that IS the runtime sandbox** has to keep a container option as the fallback for skills that shell out to Python/git/etc. or need GPU.

**WASM more desirable + smaller?** Yes, **for the default sandbox path** — pure-compute, HTTP, FS-bound skills. Cold-starts in microseconds, ships in ~5MB, no external daemon, pure-Rust (aligns with the no-VS/no-C work).

**Things in Vox that WON'T run in WASM**: anything subprocess-shaped (Python scripts, git, npm), anything GPU-shaped (mens training, candle CUDA), anything threads-heavy. Most of these are already in plugins running outside the skill sandbox anyway.

**Action**: split vox-container, make WASM the default skill sandbox via a new plugin, demote Docker/Podman to an opt-in plugin for skills that need them.
