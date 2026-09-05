---
title: "Interpreter-first execution: one toolchain-free tier for scripts and the mesh"
description: "Design for making the HIR interpreter the default and sandboxing execution tier for VoxScripts, locally and over the mesh, retiring cargo as an end-user dependency for pure Vox."
category: "Architecture SSOTs"
status: "approved"
training_eligible: false
---

# Interpreter-first execution — design

**Date:** 2026-09-05 · **Status:** approved by the maintainer; implementation plan follows.
**Supersedes** the container-tier direction discussed 2026-09-05 and the "no sandbox exists"
gap in [`2026-09-04-populi-mesh-iroh-transport-design.md`](2026-09-04-populi-mesh-iroh-transport-design.md).
**Evidence base:** [`voxscript-portability-substrate-research-2026.md`](../../src/architecture/voxscript-portability-substrate-research-2026.md).

## 1. Decision in one paragraph

Pure VoxScripts run under the HIR interpreter by default, locally and when received over the
mesh. The interpreter *is* the sandbox: capabilities are imposed by the receiver, denial is
fatal, every side-effecting namespace is gated, and CPU, memory and output are bounded. Native
compilation (`cargo`) becomes an explicit opt-in used only when a script pulls in Rust crates
or needs peak throughput. The mesh ships VoxScript source and declarative ML jobs; it never
ships native code. One builtin registry feeds both tiers and a differential gate proves they
agree. `cargo`, `rustc`, and the Vox source tree stop being end-user requirements for running
Vox.

## 2. What is true today (measured 2026-09-05, this machine, debug build)

| Tier | `hello.vox` cold | warm | End-user needs |
|---|---|---|---|
| Interpreter | **10 ms** | 10 ms | the `vox` binary |
| Native | **275 s** (764 crates) | 40–70 ms | cargo + rustc + **a Vox checkout** (`vox-actor-runtime = { path = … }`) |
| WASI | ≥ native + `rustup target add` | — | cargo + rustc + wasm target |

- `vox run` in `auto` mode routes every non-`@page` file to the **native** lane
  (`run.rs`, `is_script_file_by_page_heuristic`). The interpreter is reached only via
  `--mode interp` or when the `script-execution` feature is compiled out.
- The two tiers already disagree, verified: `crypto.hash_fast` → interp
  `UndefinedVariable("crypto")`, native works; `time.now()` → interp works, native has only
  `now_ms`. Six eval-only and three codegen-only builtins in total. Roughly 25 % of the ~194
  host entry points are table-driven; the rest are hand-duplicated across
  `vox-compiler/src/eval/builtins.rs` (2,867 LoC) and `vox-actor-runtime/src/builtins/mod.rs`
  (2,003 LoC).
- **No test runs one program through two tiers and compares output.**
- `// vox:caps` is fail-open: engages only if the script declares it; denial prints, returns
  `Null`, continues; `net`/`http` ungated; `eval/repo.rs` bypasses it.
- Interpreter bounds steps (10 M, `StepLimitExceeded` in 0.75 s) but not heap (an allocating
  loop ran until killed).
- The mesh's `ProbeOnlyExecutor` refuses `Run`. Separately, the *HTTP* A2A path in
  `remote_worker.rs::run_dispatched_bundle` executes precompiled **native binaries directly**
  when `VoxMeshExecPolicy` is `permissive` — **which is the default**
  (`remote_worker.rs:671`). That is failure mode F2, live today.
- Repository automation runs `vox run scripts/*.vox` (lefthook, CI) with no `--mode`, i.e.
  through the 764-crate native lane; several scripts individually opt into `--mode interp`.

## 3. Design

### 3.1 Tiers and defaults

| Mode | Behaviour after this change |
|---|---|
| `vox run x.vox` (auto, non-`@page`) | **interpreter** |
| `vox run --mode script x.vox` | native lane, unchanged (cargo, cached) — the opt-in |
| `vox run --mode interp x.vox` | interpreter, explicit |
| `vox run x.vox` (`@page` app) | unchanged: app lane via `vox-compilerd` |
| `vox run --isolation wasm` / `vox wasm run` | **retired** (see §3.5; plugins keep `vox-wasm-engine`) |

`WebRunMode` gains no new variant. `Auto` for scripts means interpreter. `Script` keeps its
meaning (native). `Vox.toml [web] run_mode = "script"` remains the project-level way to keep
the old default.

**Escalation is explicit, never automatic.** No runtime auto-switches tiers; every shipped
language that offers both makes it a user command (`deno compile`, `go build`). The
interpreter emits one advisory when a script exceeds a step threshold, naming
`--mode script`.

**Why not a JIT / bytecode VM now.** The evidence in the research doc: CPython's JIT was
slower than its interpreter for two releases with Microsoft funding; `cranelift-jit` is
self-described "extremely experimental"; the largest measured interpreter lever is inline
caching, not dispatch. The interpreter's first optimisation (name→slot resolution — today
every `Ident` is a `String` scope lookup and `Object` is an O(n) association vector) is
worth more than a JIT would buy and cannot miscompile. A bytecode lowering is *permitted*
later behind the same differential gate; it is not part of this design.

### 3.2 The interpreter is the sandbox

**Does executing Vox over the mesh require a sandbox at all? Yes — and the interpreter is
it.** The interpreter executes HIR, not machine code. It has no FFI, no `unsafe` execution
path, and every side effect passes through one function,
`call_builtin_method(&VoxValue, &str, Vec<VoxValue>, caps)`. If that boundary is gated
correctly and resources are bounded, Vox code *cannot* reach ambient authority. That is the
WASM property without WASM: the abstract machine is the interpreter, it is the same Rust code
on every platform, and it needs no OS sandbox, no container, and no runtime download.

What is **not** covered by this, and must stay outside the mesh: native code (Rust crates
pulled into a script, precompiled binaries) and GPU engines. Those have full ambient
authority the instant they start, and no interpreter property applies. §3.4 handles them.

Changes to make the claim true:

1. **Receiver-imposed capabilities.** `vox run --mode interp` gains `--caps <spec>`. When
   present it **replaces** any `// vox:caps` directive in the script; the script's own
   declaration is advisory documentation and is never trusted for mesh work. Spec grammar:
   `fs:ro=<dir>[,<dir>]`, `fs:rw=<dir>`, `net:none|net:allow`, `process:none|allow`,
   `env:none|allow`, `secrets:none|allow`, `time:frozen|real`. Default when `--caps` is
   present and a namespace is unmentioned: **denied**.
2. **Denial is fatal.** A denied builtin raises `EvalError::CapabilityDenied { ns, method }`
   and terminates the run with a distinct exit code. It writes to stderr, never stdout.
3. **Full coverage.** The gate covers `fs`, `io`, `path` (resolve only), `process`, `env`,
   `secrets`, `http`, `time`, `log` (rate-limited, not denied), `db`, `repo`, `agentos`.
   `eval/repo.rs` and `eval/db.rs` route through the same check. A test asserts every
   namespace seeded in `Interpreter::new` appears in the gate table — new namespaces cannot
   be added ungated.
4. **Filesystem scoping.** `fs:ro=`/`fs:rw=` are enforced by canonicalising the requested
   path and checking prefix against allowed roots, after symlink resolution. This is the
   same shape as WASI preopens and `vox run --wasi-dir`.
5. **Memory bound.** A counting `#[global_allocator]` in the `vox` binary, armed by
   `--max-memory <bytes>`; exceeding it aborts the run with a distinct exit code. Simple,
   exact, cross-platform. OS limits (Job Object working set, `RLIMIT_AS`) remain
   defence-in-depth where the existing `sandbox.rs` already applies them.
6. **Output bound.** The parent captures stdout/stderr through a capped pipe reader
   (`--max-output`), matching `JobLimits::max_output_bytes` on the mesh.
7. **Step bound** stays; `--max-steps` exposes it. Default unchanged (10 M).
8. **Determinism knobs.** `time:frozen` makes `time.now()` return a fixed value supplied by
   the receiver; `random` does not exist yet and, when added, is seeded the same way. The
   float surface is already IEEE-exact (`abs floor ceil round sqrt`); transcendentals, when
   added, go through the Vox-owned libm per the research doc — not `f64::sin`.

Threat model, stated: this defends against **malicious or buggy Vox code**. It does not
defend against a bug in the interpreter or its pure-Rust dependencies (`regex` is
linear-time; `serde_json` has a depth limit; `rust_decimal`, `serde_yaml`, `toml`, `csv`).
That is the same class of residual risk Wasmtime carries, and the process boundary in §3.4
is the mitigation.

### 3.3 One builtin surface, and proof the tiers agree

- `vox-compiler::builtin_registry` becomes the **single source of truth** for every builtin:
  namespace, method, arity, arg kinds, return shape, capability class, *and* the interpreter
  implementation binding. The Rust-emit mapping (`std_namespace_runtime_call`) derives from
  the same table instead of a parallel hand-written match.
- Every entry the interpreter can dispatch has a codegen twin or is explicitly marked
  `interp_only` with a reason; the converse likewise. A unit test fails on any unmarked
  asymmetry. This turns today's silent drift (`crypto`, `time.now`, `secrets.resolve`,
  `json.encode/stringify`, `process.cwd/spawn`) into a compile-time list to close.
- **Differential gate.** A new integration test runs every golden with `// EXPECT:` under
  the interpreter *and* the native lane and diffs stdout (normalised line endings). It is
  the acceptance criterion for "runs the same". It is slow (native compile) and runs in the
  `--full` pre-push tier and CI, with a warm shared `CARGO_TARGET_DIR`; the interpreter half
  runs in the fast tier.
- `@test` blocks already execute under the interpreter (`golden_vox_test_runner.rs`);
  `vox test` (cargo) becomes the opt-in for native-lane test runs and says so.

### 3.4 Mesh execution

**The mesh ships two things, and only two: VoxScript source, and declarative ML jobs.** It
never ships native code.

| `TaskKind` | Executor on the peer | Sandbox |
|---|---|---|
| `VoxScript` | `InterpExecutor` → child `vox run --mode interp --caps … --max-memory … --max-steps … --max-output …` | the interpreter (§3.2) + process boundary + existing `sandbox.rs` |
| `TextInfer`, `Embed`, `ImageGen`, `SpeechTranscribe`, `TrainQLoRA` | declarative `{engine, model, input}` → a **locally installed engine** the peer already trusts (llama.cpp/GGUF, MLX, …) | none needed — no foreign code executes; admission refuses kinds the peer has no engine for |

- `InterpExecutor` lives in `vox-orchestrator` (L3), which already owns `remote_worker.rs`,
  `mesh_relay.rs` and the `→ vox-mesh-transport` edge. It spawns a child process rather than
  embedding `Interpreter` in-process: crash isolation, kill-on-timeout, OS memory limits, and
  **no new crate edge** (`vox-orchestrator` does not depend on `vox-compiler` today and does
  not need to). It reuses `quiet_command` and `harden_dispatch_env` (env cleared, secrets
  only via the existing gate).
- Capabilities for mesh work come from the **receiver's** trust row for that peer
  (`MeshTrust`), not from the script. Default for a paired peer: `fs:rw=<per-job tmpdir>`,
  everything else denied. `grant_native()` remains separate and is the *only* path to
  `process:allow` or `net:allow`.
- `JobLimits` maps 1:1 onto the child's flags. `Cancel { job_id }` kills the child.
- **Protocol.** `Isolation { Wasm, Container, Native }` is replaced by
  `Isolation { Interpreter, Native }`; `DEFAULT_FOR_MESH = Interpreter`. `PROTO` bumps to 2
  (only `Hello` is frozen; no peer speaks v1 outside this repo). `Probed` gains
  `engines: Vec<EngineFacts>` so the directory and Phase 4 placement see what a peer can
  actually run.
- The HTTP A2A `run_dispatched_bundle` lane is **deleted** (§3.5). `BundleKind::Native`
  under default-`permissive` is F2 and goes now, not in Phase 6.
- `secret_gate::ExecTier` becomes `{ Interpreter, Native }` to match.
- **`PopuliHttpOp::Dispatch`/`Wait` are unblocked**: they dispatch a `VoxScript` job through
  `InterpExecutor` instead of HTTP. Task 3.4's "cannot be honestly ported" note is closed by
  this design.

**Consequence for the outstanding mesh work.**

| Item | Effect of this design |
|---|---|
| "No sandbox exists" gap | closed by §3.2 + §3.4 |
| F2 (unsandboxed executor) | eliminated by construction: nothing native is ever received |
| Task 3.1 ceiling (agent-id → `EndpointId` mapping) | unchanged; still needed for multi-peer A2A |
| Task 3.1 inbox drain | `InterpExecutor` becomes the consumer of `Inbox::messages` for `VoxScript` payloads |
| `task_submit.rs` / lease-over-mesh | unchanged; still blocked on a `Lease` protocol, still out of scope |
| Q4 mDNS | unchanged |
| Windows at-rest key (DPAPI) | unchanged |
| Phase 4 placement | `PlacementRecord` gets real `EngineFacts`; the "GPU we lack" rule now has data |
| Phase 5 Axis | `vox_mesh_nodes` shows engines + capability grants |
| Phase 6 deletion | `vox-populi/src/transport/handlers/dispatch.rs` (499 LoC) and the bundle lane leave earlier, in this program |

### 3.5 Deletions and retirements (approved: "kill any now dead or unnecessary code")

Deleted outright:

- `vox-orchestrator/src/a2a/remote_worker.rs::run_dispatched_bundle`, `BundleKind`, the
  `VoxMeshExecPolicy` `permissive`/`source-only`/`no-exec` ladder and its secret. Replaced
  by `InterpExecutor` + trust-row capabilities.
- `vox-cli` feature `script-wasi`, `WasiBackend` (`run/backend/wasi.rs`), `vox wasm run`
  (`commands/wasm.rs`), `--isolation wasm|wasi` for `vox run`. Rationale: nothing in the
  repository enables the feature; it requires cargo + rustc + `rustup target add`; its only
  advantages over the interpreter (isolation, determinism) are now interpreter properties;
  its cold path is slower than the interpreter by four orders of magnitude.
  **`vox-wasm-engine` and `vox-plugin-runtime-wasm` stay** — plugins are a different surface.
- `vox-cli/src/isolation.rs`: `IsolationPolicy::{Gvisor, MicroVM, Container}` and the
  `runsc` probe. They are parsed and then rejected at `script.rs:102-118`; the error text
  points at `docs/src/reference/isolation.md`, **which does not exist**. What remains is
  `{ Interpreter, Native }`, or the enum is removed and `RunMode` carries the meaning.
- `vox-skill-runtime/src/microvm.rs` (`MicroVmRuntime`, every method `bail!`s "not yet
  implemented"). A documented seam with no consumer is a promise the code does not keep.
- `vox-mesh-transport::ProbeOnlyExecutor` — replaced by `InterpExecutor`; the security
  tests that pinned "refuses `Run`" are rewritten to pin "refuses `Run` without a
  capability row" and "runs `VoxScript` at `Interpreter`".
- `vox-mesh-transport::Isolation::{Wasm, Container}`.
- `sandbox.rs` "Other: warning + `VOX_SANDBOX=1` hint" branch — replaced by a macOS Seatbelt
  branch **only if** the enumerated-IOKit profile work is done; otherwise the branch states
  plainly that macOS has no OS-level sandbox and the interpreter boundary is the sandbox.
  No informational env-var pretending to be one.

Retained, but corrected:

- `vox_ir` (87 LoC) is a JSON export behind `vox check --emit-ir`, not an IR. Rename the
  module and its doc to `hir_export`; do not build on it.
- `commands/wasm.rs` doc comment claims `vox-wasm-engine` is "a hard dependency"; the
  manifest makes it optional. Moot after deletion, noted for the record.
- `eval/value.rs` control-flow sentinels (`_Return`, `_Break`, `_Continue`, `_Panic`) stay
  for now; a future bytecode lowering must replace them with real control flow.

### 3.6 Repository-wide consequences

- **Automation gets faster.** lefthook and CI invoke `vox run scripts/*.vox` with no mode;
  they move from the native lane (cold 275 s, cache-keyed on the `vox` binary's mtime, so
  every rebuild of `vox` invalidates every script) to the interpreter (10 ms). Scripts that
  already pass `--mode interp` keep working. Scripts that need a Rust crate must say
  `--mode script` and are enumerated in the plan.
- **`vox doctor`** stops reporting cargo/rustc/`wasm32-wasip1` as required for scripts; they
  become `(optional) — needed for --mode script`. `voxup` stops `mkdir`-ing an empty
  `wasm-sysroot` directory.
- **Docs.** `docs/src/reference/cli.md` (`vox run` modes), the missing
  `docs/src/reference/isolation.md` (write it — one page: interpreter boundary, `--caps`
  grammar, limits, what it does not defend against), `where-things-live.md` rows for
  `InterpExecutor`, `AGENTS.md` §VoxScript-First tier table (the "Untrusted / sandboxed →
  `--isolation wasm`" row becomes `--caps …`), the mesh plan Status + Phase 3/4/6 notes,
  ADR-047 addendum, and a new ADR for "interpreter is the execution and sandbox tier".
- **Contracts.** `contracts/ci/crate-edges.allow.v1.json`: no new edges. Removing
  `script-wasi` removes `vox-cli → vox-wasm-engine`; tighten with
  `vox ci crate-edges --tighten`. `Cargo.lock` shrinks (wasmtime leaves the `vox-cli`
  feature graph; `workspace-hack` still carries the shared cranelift sub-crates via
  `vox-actor-runtime` until hakari is regenerated).
- **MENS corpus.** Scripts become the positive corpus for "runs identically"; the
  differential gate's pass/fail is a clean reward signal.
- **Retirement audit.** Nothing here needs a `vox-deprecated-since` marker: the surfaces are
  deleted, not left vestigial.

### 3.7 Use without `.vox` scripts — dependency map

The interpreter tier changes nothing for apps (`@page`, `component`, `routes`, `table`):
they compile through `vox-codegen` to Rust/TypeScript and need cargo/pnpm as before, because
they *are* Rust/React-ecosystem programs. Workflows (`workflow`/`activity`) keep their
durable runtime. The map after this change:

| You run… | Needs |
|---|---|
| a pure VoxScript | `vox` |
| a VoxScript over the mesh | `vox` on both ends |
| a VoxScript that imports a Rust crate | `vox` + cargo + rustc (`--mode script`) |
| a Vox web/desktop app | `vox` + cargo + node/pnpm (as today) |
| an ML job over the mesh | `vox` + the named engine installed on the executing peer |
| a Vox plugin in wasm | `vox` (embedded `vox-wasm-engine`, unchanged) |

### 3.8 What this design does not claim

- **GPU numerics identical across vendors** — not achievable by anyone (MLX diverges across
  its own two backends). The mesh reports engine facts and lets Phase 4 decide; results are
  asserted within tolerance, not bit-for-bit.
- **Peak compute throughput under the interpreter** — expect 3–10× off native. Glue scripts
  are I/O- and subprocess-bound and will not notice; numeric kernels use `--mode script`.
- **Defence against interpreter bugs** — mitigated by the process boundary and OS limits, not
  eliminated.
- **A bytecode VM or JIT** — explicitly deferred; permitted later behind the differential
  gate.

## 4. Testing

- **Mutation-verified guards** (AGENTS.md rule): every capability denial, the memory bound,
  the output bound, and the trust-row → `--caps` mapping each get a test that is run once
  with the guard broken to prove it fails. The interpreter-as-sandbox claim is worthless
  without this.
- **Differential gate** (§3.3) is the acceptance test for tiers agreeing.
- **Registry symmetry test** (§3.3) is the acceptance test for builtin coverage.
- **Mesh:** two live iroh endpoints on loopback, `VoxScript` job runs and returns output; the
  same job with `fs:none` is refused fatally with the denial on stderr; a script exceeding
  `--max-memory` is killed and reported; `Cancel` kills a running child; a peer with no
  trust row cannot `Run` (existing test, retargeted).
- **Repository scripts:** every `scripts/**/*.vox` runs under the interpreter in CI; the
  ones that legitimately cannot are listed with a reason and run with `--mode script`.

## 5. Risks and open questions

- **Interpreter performance on real repo scripts.** Unmeasured. Mitigation: the plan's
  first task times every `scripts/**/*.vox` under both tiers before flipping the default.
- **`--caps` grammar stability.** It is a public CLI surface once shipped. Keep it small;
  version it in the reference doc.
- **Global allocator in the `vox` binary** affects every subcommand's allocation path.
  Overhead is one atomic add per allocation when disarmed; measure, and gate behind a
  runtime flag rather than a feature so one binary serves both roles.
- **Scripts that shell out to cargo themselves** (`native.rs:123-136` propagates `CARGO`
  today). Under the interpreter, `process:allow` is required; local `vox run` grants it by
  default (trusted developer), the mesh does not.
- **macOS Seatbelt** stays optional defence-in-depth pending the IOKit-enumeration work; this
  design does not depend on it.
