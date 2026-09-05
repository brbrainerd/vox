---
title: "Interpreter-first execution: one toolchain-free tier for scripts and the mesh"
description: "Design for making the HIR interpreter the default and sandboxing execution tier for VoxScripts, locally and over the mesh, retiring cargo as an end-user dependency for pure Vox."
category: "Architecture SSOTs"
status: "approved"
training_eligible: false
---

# Interpreter-first execution — design

**Date:** 2026-09-05 · **Revision 2**, after a seven-track critique against the source
(escape paths, filesystem scoping, resource limits, mesh protocol, deletion consumers, plan
executability, tier parity). Revision 1's central claim was false as stated; §3.2 records
what changed.
**Status:** approved by the maintainer; implementation plan follows.
**Supersedes** the container-tier direction discussed 2026-09-05 and the "no sandbox exists"
gap in [`2026-09-04-populi-mesh-iroh-transport-design.md`](2026-09-04-populi-mesh-iroh-transport-design.md).
**Evidence base:** [`voxscript-portability-substrate-research-2026.md`](../../src/architecture/voxscript-portability-substrate-research-2026.md).

## 1. Decision in one paragraph

Pure VoxScripts run under the HIR interpreter by default, locally and when received over the
mesh. The interpreter *is* the sandbox for Vox code: capabilities are imposed by the receiver,
denial is fatal, every side-effecting path — including `import` — is gated, and CPU, memory,
recursion depth and output are bounded. Native compilation (`cargo`) becomes an explicit
opt-in used only when a script pulls in Rust crates or needs peak throughput. The mesh ships
VoxScript source and declarative ML jobs; it never ships native code. One builtin registry
feeds both tiers, and a differential gate proves they agree on every golden that declares an
expected output — a corpus this design also grows to cover arithmetic, display, ordering and
argv, which it does not cover today. `cargo`, `rustc`, and the Vox source tree stop being
end-user requirements for running Vox.

## 2. What is true today (measured 2026-09-05, this machine, debug build)

| Tier | `hello.vox` cold | warm | End-user needs |
|---|---|---|---|
| Interpreter | **10 ms** | 10 ms | the `vox` binary |
| Native | **275 s** (764 crates) | 40–70 ms | cargo + rustc + **a Vox checkout** (`vox-actor-runtime = { path = … }`) |
| WASI | ≥ native + `rustup target add` | — | cargo + rustc + wasm target |

- `vox run` in `auto` mode routes every non-`@page` file to the **native** lane
  (`run.rs`, `is_script_file_by_page_heuristic` is literally `!head.contains("@page")`).
- The two tiers already disagree, verified by execution: `crypto.hash_fast` → interp
  `UndefinedVariable("crypto")`, native works; `time.now()` → interp works, native has only
  `now_ms`. The critique found more by reading source: native **integers wrap silently**
  (`script-dev` sets `overflow-checks = false`) while the interpreter halts; `str([1, 2])` is
  `[1, 2]` under interp and `[1,2]` natively; `print(Some(3))` prints Rust `Debug` under
  interp and does not compile natively; `env.args()` returns the `vox` command line under
  interp and the script's argv natively; `fs.glob` is unsorted and error-swallowing under
  interp, sorted and propagating natively; `.sorted()` has no codegen arm; object iteration is
  insertion-ordered under interp and key-sorted natively; script faults exit 1 vs 101.
- **No test runs one program through two tiers and compares output.** Of 79 goldens, 11
  declare `// EXPECT:` and nine of those expect a single `ok` sentinel. None exercises any
  divergence class above. **No golden calls `crypto.*`.**
- `// vox:caps` is fail-open: engages only if the script declares it; denial prints, returns
  `Null`, continues; `net`/`http` ungated; `eval/repo.rs` bypasses it.
- Interpreter bounds steps (10 M, `StepLimitExceeded` in 0.75 s) but not heap (an allocating
  loop ran until killed), not recursion depth (Vox recursion is Rust recursion; a
  200 000-deep nesting SIGSEGVs at *parse* time), and not wall time.
- **`import "…"` reads and executes arbitrary host `.vox` files with no capability check**
  (`eval/mod.rs`, before `main` runs). `process.exec` replaces the interpreter's process image.
  `process.register_exit_command` pushes into a process-global static whose signal handler
  runs those commands and hard-exits the host — surviving the interpreter that armed it.
- Five in-process embedders construct `Interpreter` with `caps: None`, including the
  workspace MCP dispatch that runs `.vox` tools on behalf of an LLM.
- The mesh's `ProbeOnlyExecutor` refuses `Run`. Separately, the *HTTP* A2A path in
  `remote_worker.rs::run_dispatched_bundle` executes precompiled **native binaries directly**
  when `VoxMeshExecPolicy` is `permissive` — **which is the default**. That is failure mode
  F2, live today.
- `db.rs` and `repo.rs` are **pure in-memory stores** with no I/O; the `@versioned fn`
  auto-snapshot calls `repo.snapshot` directly, not through the method dispatch.

## 3. Design

### 3.1 Tiers and defaults

| Mode | Behaviour after this change |
|---|---|
| `vox run x.vox`, script-shaped (see below) | **interpreter** |
| `vox run --mode script x.vox` | native lane, unchanged (cargo, cached) — the opt-in |
| `vox run --mode interp x.vox` | interpreter, explicit |
| `vox run x.vox`, app- or service-shaped | unchanged: native/app lane, with a one-line message naming `--mode script` |
| `vox run --isolation wasm` / `vox wasm run` | **retired** (see §3.5; plugins keep `vox-wasm-engine`) |

**Script-shaped** means: the file declares `fn main()` and none of `@page`, `routes`,
`server`/`query`/`mutation`, `table`, `actor`, `workflow`/`activity`. Those surfaces are booted
by the native lane (DB init, HTTP listener) and cannot run under the interpreter; the old
`!contains("@page")` predicate would have routed them there. `WebRunMode` gains no new variant;
`Vox.toml [web] run_mode = "script"` remains the project-level opt-out.

**Escalation is explicit, never automatic.** No shipped language auto-switches tiers; every one
that offers both makes it a user command (`deno compile`, `go build`). The interpreter's exit-78
message names `--mode script`.

**Why not a JIT / bytecode VM now.** CPython's JIT was slower than its interpreter for two
releases with Microsoft funding; `cranelift-jit` self-describes as "extremely experimental";
the largest measured interpreter lever is inline caching, not dispatch. The interpreter's first
optimisation (name→slot resolution — every `Ident` is a `String` scope lookup, every `Object`
field access an O(n) scan) is worth more than a JIT and cannot miscompile. A bytecode lowering
is *permitted* later behind the differential gate; it is not part of this design.

### 3.2 The interpreter is the sandbox — what that claim requires

**Does executing Vox over the mesh require a sandbox? Yes, and the interpreter is it — once
the following is true.** Revision 1 said "every side effect passes through
`call_builtin_method`". The critique showed four paths that do not: `import` resolution, the
`@versioned` snapshot, `process.exec`, and the exit-command queue. The claim is therefore a
*target*, and the list below is what makes it true. It is an **isolation** property: the
interpreter has no FFI and no `unsafe` execution path, so gating every side-effecting entry
point is sufficient against Vox code. It is **not** a reproducibility property (§3.8).

1. **Receiver-imposed capabilities.** `vox run --mode interp --caps <spec>` **replaces** any
   `// vox:caps` directive; the script's own declaration is documentation, never trusted.
   Grammar, comma-separated, order-free, **one value per token** (repeat the token for more
   directories; `|` is a shell pipe and was removed):

   | token | grants |
   |---|---|
   | `fs:ro=<dir>` | reads under `<dir>` (canonicalised at parse; must exist) |
   | `fs:rw=<dir>` | reads and writes under `<dir>` |
   | `net:none` / `net:allow` (`http:` accepted as alias) | `std.http.*` |
   | `process:none` / `process:allow` | `process.*` — see the warning below |
   | `env:none` / `env:ro` / `env:rw` | `env.get`/`env.args` need `ro`; `env.set` needs `rw` |
   | `secrets:none` / `secrets:allow` | `secrets.*` |
   | `time:real` / `time:frozen=<ms>` (non-negative) | `time.now_ms()` wall-clock or fixed |
   | `random:seed=<u64>` / `random:deny` | every randomness-consuming builtin (`crypto.uuid` when it lands) |
   | `agentos:none` / `agentos:allow` | `agentos.*` |
   | `deterministic` | shorthand: `time:frozen=0,random:seed=0,process:none,net:none,env:none` |

   Unmentioned namespaces are **denied**. `io:` is rejected as a token — `io.open`/`io.save`
   are fs reads/writes and take their authority from `fs:`. Directory names containing `,` or
   `=` cannot be expressed; the mesh executor builds its `CapabilitySet` through a typed
   constructor and refuses such a job directory rather than encoding it.
   **Pure namespaces**, never gated: `path` (except `path.resolve`, which touches disk and is
   an fs read), `json`, `csv`, `toml`, `yaml`, `regex`, `log`, **`db`, `repo`** (in-memory
   stores; gate them if and when they acquire a backing store).
2. **`caps` is not optional.** `Interpreter.caps: CapabilitySet`, defaulting to
   `developer_default()` (everything allowed) in `Interpreter::new`. A `None` that meant
   "no gate" becomes unrepresentable. The five in-process embedders each receive an explicit
   set; the MCP workspace dispatch and `vox-terminal-core::eval_line` get a restrictive one.
3. **Denial is fatal.** `EvalError::CapabilityDenied { ns, method }` terminates the run with
   exit 77, one line on stderr, nothing on stdout. Deferred exit commands do not run — a
   script denied a capability must not get cleanup it queued.
4. **Every side-effecting entry point is gated**, not only `call_builtin_method`:
   - the `fs` arm, with the guard at the **head of the arm keyed by a method table**
     (`read`/`read_file`/`read_to_string`, `read_bytes`, `canonicalize`, `write`/`write_file`/
     `write_to_file`, `cwd`, `copy`, `remove`, `walk`/`list_recursive`, `exists`, `is_file`,
     `is_dir`, `remove_dir_all`, `list_dir`, `glob`, `list_dir_detailed`, `stat`, `mkdir`, and
     `io.open`/`io.save`), each marked read or write, with unknown methods **denied by
     default** so a new method cannot ship ungated;
   - `path.resolve`, as an fs read;
   - **`import` resolution** in `eval/mod.rs`, as an fs read of the canonicalised target;
   - `process.register_exit_command`, at queue time, under `process`;
   - a test derives the seeded namespace list at runtime from the interpreter's scope and
     asserts every one is classified pure or gated.
5. **Filesystem scoping.** Roots are canonicalised **at parse time** (on macOS `/tmp` is
   `/private/tmp`; on Windows `canonicalize` yields `\\?\C:\…`; comparing a canonical path
   against a raw root denies everything). The check returns the resolved path and the
   syscall uses *that* path, so check and operation name the same file. Non-existent targets
   resolve through the parent and require a real file name (`""`, `.`, `..` are denied, not
   degraded). Windows compares components case-insensitively. **Residual, stated:** this is
   not `openat`-based like WASI preopens; directory components are resolved twice, and a
   script that also holds `process:allow` can race the final component. Relative paths
   resolve against the process CWD; over the mesh that is the job directory, always an rw root.
6. **Memory bound.** A counting `#[global_allocator]` in the `vox-cli` **library** (not the
   binary — `run()` is library code, and two module copies would give the allocator and
   `arm()` different statics). Exceeding the ceiling writes a stack-formatted line and calls
   `libc::_exit(79)` / `TerminateProcess` — never `std::process::exit`, which runs atexit
   handlers that allocate and re-enter the allocator. `realloc` and `alloc_zeroed` are
   overridden so the compiler's own `Vec` growth keeps `realloc(3)`'s in-place path. **What
   it counts:** Rust heap allocations in this process, armed after the CLI parse. Not child
   processes, not `mmap` by C dependencies, not thread stacks, not bytes written to disk.
7. **Step bound** (`--max-steps`, exit 78) **bounds evaluated HIR nodes, not work.** A single
   builtin call is one step whatever it does (`str.repeat(1e9)`, `fs.glob("/**")`, a 10 MB
   regex compile, a 30 s HTTP timeout). Work is bounded by the memory ceiling (those blow up
   through the heap) and, over the mesh, by the receiver's wall-clock kill, which the job
   cannot influence. **A locally-run script has no wall-clock bound; that is a deliberate
   difference between the local and mesh tiers.**
8. **Depth bound.** Vox recursion maps onto Rust stack frames; without a limit
   `fn f() { f() }` SIGSEGVs the process, defeating every other bound and surfacing as
   `exit None`. An `eval_depth` counter (default 1 024) and a parser nesting limit both raise
   a limit error (exit 78). The executor maps a signal exit to a named failure.
9. **Output bound** is enforced by the parent (`InterpExecutor`) reading a capped pipe; a
   truncated result carries a marker line so the peer can tell.
10. **Determinism knobs.** `time:frozen`, `random:seed`, and sorted directory enumeration
    (`fs.list_dir`, `fs.glob`) on **both** tiers. The float surface is IEEE-exact
    (`abs floor ceil round sqrt`); transcendentals, when added, go through the Vox-owned libm.

**Threat model, stated.** This defends against **malicious or buggy Vox code**. It does not
defend against a bug in the interpreter or its pure-Rust dependencies (`regex` is
linear-time; `serde_json` has a depth limit); the process boundary in §3.4 is the mitigation.
**`process:allow` is shell access:** `process.exec` replaces the interpreter's process image
and `process.spawn_background` outlives it; no interpreter property survives either. Over the
mesh, `process` is denied for `Sandboxed` trust and `grant_native` — never pairing — is the
only way to grant it, and the documentation says "runs any binary on this host as the daemon
user" in those words. A script can call `process.exit(77)` to *forge* a denial; the executor
therefore requires the stderr marker as well as the exit code.

### 3.3 One builtin surface, and proof the tiers agree

- `vox-compiler::builtin_registry` is the intended SSOT for every builtin. This program does
  not yet make `eval/builtins.rs` table-driven (2,867 LoC); it closes the measured drift, adds
  a **must-be-empty `KNOWN_TIER_ASYMMETRIES` list** (every entry needs a reason), and makes
  new drift a differential-gate failure.
- **Parity fixes in scope:** `overflow-checks = true` in the native script profiles (one line;
  makes both tiers halt, by construction); a shared `vox_display` so `str()`/`print()` of
  lists, objects, tuples, `Option`, `Result`, and tagged values print the same text on both
  tiers (today neither is acceptable); `print` of n arguments; `env.args()` returns
  `[<script path>] ++ args` on both tiers and `run_interp` actually threads the args; sorted
  `fs.glob`/`fs.list_dir` with propagated errors on both; a `.sorted()` codegen arm; the
  native `main` catches panics and exits 1 so script faults agree (0 and 77–79 are contractual,
  1 is the fault code, 101 means "the interpreter/binary has a bug").
- **Maintainer decisions recorded, not taken:** (a) `crypto.*` parity needs a
  `vox-compiler → vox-crypto` edge (L2→L0, downward, layer-legal) — until authorised, `crypto`
  is a `KNOWN_TIER_ASYMMETRIES` entry and the native `vox_hash_fast` (XXH3-128, 32 hex) and
  `vox_uuid` (`vox-<16hex>-<16hex>`, 37 chars) are what parity would have to match, not
  BLAKE3/UUIDv4; (b) **object iteration order** — insertion (interp) or key-sorted (native,
  `serde_json` without `preserve_order`); a golden forces the decision.
- **Differential gate.** Every golden with `// EXPECT:` runs under both tiers and stdout is
  diffed; a new `// EXPECT-EXIT: nonzero-both` directive covers faults whose stdout is a
  prefix. **Eight new goldens** cover overflow, division by zero, composite display, float
  formatting, object order, directory order, argv shape, and crypto parity — the classes the
  current corpus does not touch. A green run before those land is evidence the corpus is
  inadequate, not that the tiers agree. Slow; registered in the `--full` tier's nextest
  filter (a Rust constant, not only the doc list).
- `@test` blocks already execute under the interpreter; `vox test` (cargo) is the opt-in for
  native-lane test runs and says so.

### 3.4 Mesh execution

**The mesh ships two things, and only two: VoxScript source, and declarative ML jobs.** It
never ships native code.

| `TaskKind` | Executor on the peer | Sandbox |
|---|---|---|
| `VoxScript` | `InterpExecutor` → child `vox run --mode interp --caps … --max-steps … --max-memory …` | the interpreter (§3.2) + process boundary + existing `sandbox.rs` |
| `TextInfer`, `Embed`, `ImageGen`, `SpeechTranscribe`, `TrainQLoRA` | declarative `{engine, model, input}` → a **locally installed engine** the peer already trusts | none needed — no foreign code executes; admission refuses kinds with no engine |

- `InterpExecutor` lives in `vox-mesh-transport` (L2): it spawns a child and needs no
  compiler dependency, so **no new crate edge**. It reuses the `env_clear` + curated
  passthrough shape from `remote_worker.rs`.
- **Child environment.** Unix: `PATH`, `HOME`, `TMPDIR`. Windows: `Path`, `USERPROFILE`,
  `TEMP`, `TMP`, `SystemRoot` (without it Winsock and DLL loading break). `HOME`/`USERPROFILE`
  point at a **second, read-only tempdir**, not the writable job directory, so a script cannot
  author a `~/.vox/config.toml` that a later config load reads. CWD is the job directory.
  Spawned in its own **process group** so the wall-clock kill and `Cancel` reach grandchildren;
  pipe drains are bounded so a daemonised grandchild holding stdout cannot hang the job.
- **Capabilities** come from the **receiver's** trust row, through a typed constructor, never
  a formatted string: `Sandboxed` → `fs:rw=<job dir>,time:real`; `Native` →
  `+ net:allow,process:allow,env:ro`. `secrets` is never granted by trust level.
- **`JobLimits` is the one place node policy lives**: `wall_clock`, `max_output_bytes`,
  `max_payload_bytes`, `max_memory_bytes`, `max_steps`, and a per-kind payload cap —
  **4 MiB for `VoxScript`** (source is text; 1 GiB was sized for the bundle lane this design
  deletes). `max_payload_bytes` default drops to 16 MiB. The receiver checks the claim, reads
  the frame with the varint allowance, and refuses a payload whose length differs from the
  claim.
- **Concurrency is bounded per node** by a semaphore sized to available parallelism; past it
  the executor refuses with a retryable message rather than queueing behind a wall clock. The
  memory bound is per child and does not compose without this.
- **Job identity.** `JobId` becomes a newtype. `JobRequest::Run` carries a **sender-assigned
  `job_id`**, the running map is keyed by `(EndpointId, JobId)`, and `Cancel` looks up only
  the caller's own entries — one message for "not yours" and "not there". A payload-hash id
  was derivable by any peer holding the same script and collided on identical concurrent jobs.
- **Protocol.** `Isolation { Interpreter, Native }` replaces `{ Wasm, Container, Native }`;
  `DEFAULT_FOR_MESH = Interpreter`; `PROTO` bumps to 2. **Every frame is version-locked by
  `PROTO`.** postcard is positional: `#[serde(default)]` on a trailing field does *not* let an
  old sender's frame decode (the reader's field count bounds the sequence, so a short buffer is
  `DeserializeUnexpectedEnd`); the existing `task_kinds` default is ornamental for the same
  reason, and a test pins that fact. `Hello` is frozen so a mismatch is *diagnosable*: the
  receiver answers a `Failed` frame naming both versions and closes with a `REFUSED_PROTO` code
  instead of dropping the connection into its own debug log. `Probed` gains `engines`.
- **Exit-code mapping in the executor:** 0 → output; 77 → denied **only with** the stderr
  marker; 78 → limit; 79 → memory; 101 → "interpreter bug", stderr **not** forwarded (backtraces
  leak host paths); `None` → "killed by signal (stack overflow or OOM)".
- The HTTP A2A bundle lane (`run_dispatched_bundle`, `BundleKind`, the `VoxMeshExecPolicy`
  SecretId and its `no-exec`/`source-only`/`permissive` ladder) is **deleted now**, not in
  Phase 6. `secret_gate.rs` goes with it — after the rename its only surviving variant would
  have had zero call sites and a doc comment describing wasmtime. The HTTP source lane gains
  `--caps` scoped to a **per-dispatch** tempdir (not the shared `/tmp`).
- **`PopuliHttpOp::Dispatch`** runs on a mesh peer — *once it has a payload that can execute*.
  The current synthesised source, `workflow_durable_shim::execute_activity(…)`, names a
  symbol that exists nowhere in the repository; under HTTP a control plane could interpret it,
  under the interpreter it is `UndefinedVariable`. `Dispatch` is supported for activities that
  carry dispatchable source and errors by name otherwise; the shim synthesis is deleted as
  dead. `Wait` becomes inline and keeps `success`/`result_output`/`exit_code` keys so existing
  readers degrade instead of failing.
- **Inbox drain is not in this program.** `Inbox::messages` has zero production consumers
  today; draining it needs the agent-id→`EndpointId` mapping that also caps `mesh_relay` at one
  peer. Revision 1 promised the drain in this table; that row is withdrawn and the dependency
  stated.

**Consequence for the outstanding mesh work.**

| Item | Effect of this design |
|---|---|
| "No sandbox exists" gap | closed by §3.2 + §3.4 |
| F2 (unsandboxed executor) | eliminated by construction: nothing native is ever received |
| Task 3.1 ceiling (agent-id → `EndpointId`) | unchanged; blocks both `mesh_relay` and the inbox drain |
| Task 3.1 inbox drain | **not funded here**; dependency stated above |
| `task_submit.rs` / lease-over-mesh | unchanged; PROTO 2 was the cheap moment to add `Lease` and does not — that is a deliberate scope cut, and PROTO 3 will be needed |
| Q4 mDNS, Windows DPAPI | unchanged |
| Phase 4 placement | `engines` on the wire (empty until an engine registry exists); first-fit peer choice records the chosen peer and candidate count as the seam |
| Phase 5 Axis | data exists; no GUI task here |
| Phase 6 deletion | bundle lane and `VoxMeshExecPolicy` leave now; `vox-populi/src/transport/handlers/dispatch.rs` survives with a deprecation marker |

### 3.5 Deletions and retirements

Deleted outright:

- `remote_worker.rs::run_dispatched_bundle`, `BundleKind`, `classify_bundle`, the
  `VoxMeshExecPolicy` SecretId and its ladder, `secret_gate.rs` (+ `secret_bag.rs` if it has no
  other consumer), `envelope.rs` `exec_bundle_*` fields.
- `vox-cli` feature `script-wasi`, `WasiBackend`, `vox wasm run`, `--isolation` for `vox run`
  / `vox script`, `isolation.rs` (`IsolationPolicy`, `IsolationCapabilities`). Nothing in the
  repository enables the feature; it needs cargo + rustc + a rustup target; its only advantages
  over the interpreter are now interpreter properties. **`vox-wasm-engine` and
  `vox-plugin-runtime-wasm` stay** — plugins are a different surface. The orphaned
  `codegen_rust/pipeline.rs` `WasiBinary` target and `vox-script-wasi` path dependency go too.
- `vox-skill-runtime::microvm` (`MicroVmRuntime`, every method `bail!`s), `Tier::MicroVm`, and
  its test file — with the file's two real assertions (tier ordering, `plan_for_min_tier`
  error path) moved, not lost.
- `vox-mesh-transport::ProbeOnlyExecutor`, `Isolation::{Wasm, Container}`.
- `sandbox.rs`'s "Other: warning + `VOX_SANDBOX=1` hint" branch **and** the second
  `VOX_SANDBOX=1` at `backend/native.rs`. macOS has no OS-level sandbox in Vox; the file says
  so instead of setting an informational variable.
- `voxup::provision_wasm_sysroots` (creates an empty directory nothing reads).

**Retirement, not just deletion.** Revision 1 claimed nothing needed a retirement marker
because the surfaces are deleted. That conflates deletion with retirement:
`contracts/retirement/retired-surfaces.v1.yaml` and
`contracts/documentation/retired-symbols.v1.yaml` exist precisely so an agent cannot
re-introduce `--isolation wasm`, `vox wasm run`, `ProbeOnlyExecutor`, `MicroVmRuntime`, or
`VoxMeshExecPolicy` from stale training. Rows are added for each, and to AGENTS.md §Retired
Surfaces.

Retained, but corrected:

- `vox_ir` (87 LoC) is a JSON export behind `vox check --emit-ir`, not an IR. Renamed
  `hir_export` with a **scoped** symbol rename (not a blind `sed` over `crates/`), and the
  public `vox-ir-specification.md` page plus both schema mirrors updated together.
- **`VOX_MESH_EXEC_POLICY` exists twice.** The SecretId (deleted) and a **config key** in
  `vox-config`/`vox-gui` meaning task *placement* (`local_only`/`prefer_remote`/`remote_only`).
  The config key survives. Removing the SecretId drops the name from the managed-secret regex,
  so `secret-env-guard` stops policing direct `env::var` reads of the survivor — noted in the
  registry so the collision is not rediscovered.

### 3.6 Repository-wide consequences

- **Automation gets faster** — lefthook and CI invoke `vox run scripts/*.vox` with no mode
  and move from the native lane to the interpreter. The critique verified no script under
  `scripts/**` or `infra/**` uses `@place`, a `rust` import, or an interpreter hard-refusal
  namespace. The pre-commit hook (`scripts/fmt.vox`) is verified under the interpreter
  **before** the default flips, not after, because a failure there breaks every commit.
- **Contract chain.** Deleting `vox wasm` reaches `contracts/operations/catalog.v1.yaml`,
  `contracts/cli/command-registry.yaml`, the capability registry and model manifest, both
  `gui-surface-*.v1.json` reports, the command-catalog test baseline, and `cli.md`'s command
  table. `vox ci command-sync` alone regenerates only the markdown from an input it does not
  update; the chain is regenerated in order or `ssot-drift` fails on four sub-gates.
  Removing the SecretId requires `vox ci secrets-contracts` **before** `secrets-parity`.
- **The two HTTP dispatch handlers** (`vox-populi`, `vox-plugin-populi-mesh`) spawn `vox wasm
  run` and `vox run --isolation wasm` — commands this design deletes. They fail at runtime, not
  compile time, and are fixed in the same change.
- **`vox doctor`** reports cargo/rustc/`wasm32-wasip1` as optional for scripts.
- **Docs.** `docs/src/reference/isolation.md` (the page `script.rs` already tells users to
  read, which does not exist); ADR-048; `AGENTS.md` tier table; `GEMINI.md` and
  `.cursor/rules/voxscript-first-automation.mdc` (both name `--isolation wasm`);
  `where-things-live.md` rows; the mesh plan's Status; two markdown links to deleted files
  that `vox ci check-links` would fail on.
- **Contracts.** No new crate edges (the `vox-compiler → vox-crypto` edge is a recorded
  maintainer decision, not taken). Removing `script-wasi` removes `vox-cli → vox-wasm-engine`;
  tighten with `vox ci crate-edges --tighten`.
- **MENS corpus.** The differential gate's pass/fail is a clean reward signal.

### 3.7 Use without `.vox` scripts — dependency map

| You run… | Needs |
|---|---|
| a pure VoxScript | `vox` |
| a VoxScript over the mesh | `vox` on both ends |
| a VoxScript that imports a Rust crate | `vox` + cargo + rustc (`--mode script`) |
| a Vox web/desktop app, or a `table`/`routes`/`server`/`workflow` program | `vox` + cargo (+ node/pnpm for web), as today |
| an ML job over the mesh | `vox` + the named engine installed on the executing peer |
| a Vox plugin in wasm | `vox` (embedded `vox-wasm-engine`, unchanged) |

### 3.8 What this design does not claim

- **Bit-identical output across operating systems for jobs granted `fs` or `process`** —
  path separators, directory order (sorted by the runtime, but the *set* is the host's), and
  subprocess output are the host's. Deterministic execution is the `deterministic` capability
  profile, not a property of the tier.
- **GPU numerics identical across vendors** — not achievable by anyone (MLX diverges across
  its own two backends). Results are asserted within tolerance.
- **Peak compute throughput under the interpreter** — expect 3–10× off native.
- **Defence against interpreter bugs** — mitigated by the process boundary and OS limits.
- **Identical exit codes across tiers for script faults** until the native `main` catches
  panics; after that, 1 on both. Only 0 and 77–79 are contractual.
- **A bytecode VM or JIT** — explicitly deferred.
- **JSON object key order preserved from source** — `json.parse` is key-sorted on both
  tiers today; that is deterministic, not insertion-ordered.

## 4. Testing

- **Mutation-verified guards** (AGENTS.md rule): capability denial, the fs symlink check, the
  memory ceiling, the trust-row→caps mapping, the `Cancel` ownership check, and the import gate
  each get a test that is run once with the guard broken to prove it fails.
- **Differential gate** (§3.3) with the eight new goldens is the acceptance test for tiers
  agreeing; `KNOWN_TIER_ASYMMETRIES` must be empty or every entry must carry a reason.
- **Registry symmetry test** for builtin coverage.
- **Mesh, on loopback:** a `VoxScript` job runs and returns output; `fs:none` is refused
  fatally with the denial on stderr; a memory blow-up is killed and reported as 79; output is
  truncated at the cap with the marker; `Cancel` kills the caller's own child and not another
  peer's; a payload of exactly the declared size is accepted and a short one refused; a v1
  `Hello` gets a `Failed` naming both versions; ML kinds are refused with "no engine"; a peer
  with no trust row cannot `Run`.
- **Repository scripts:** every `scripts/**/*.vox` is *executed* (not only `vox check`) under
  the interpreter before the default flips; the ones that legitimately cannot are listed with
  a reason and run with `--mode script`.

## 5. Risks and decisions recorded

- **Two maintainer decisions are recorded, not taken** (§3.3): the `vox-compiler → vox-crypto`
  crate edge, and object iteration order. Both have a golden that forces them.
- **PROTO 2 is incompatible with a PROTO 1 peer by design.** BLAPTOP04 must be rebuilt before
  the cross-machine smoke.
- **`--caps` grammar stability.** Public CLI surface once shipped; keep it small, version it
  in `isolation.md`.
- **Global allocator in the `vox-cli` library** applies to every binary linking it
  (`vox-gui`, `vox-ml-cli`, test crates): two relaxed atomics per allocation when disarmed, and
  no dependent crate may declare its own `#[global_allocator]` (none does today).
- **Interpreter performance on real repo scripts** is measured (Task 0) before the flip.
- **macOS Seatbelt** stays optional defence-in-depth; this design does not depend on it.
