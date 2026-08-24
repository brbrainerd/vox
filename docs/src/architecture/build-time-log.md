---
title: "Build-Time Log"
description: "Dated build-time measurements: the 2026-05-08 workspace-reorg phases, the 2026-06-15 reduction program, and the 2026-08-23 dependency-weight pass."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
---

# Build-Time Log

Per-phase measurements for the workspace reorg. See [build-time-baseline.md](./build-time-baseline.md).

## Phase 0 — Baseline established (2026-05-08)

| Scenario | Time |
|---|---|
| Orchestrator incremental (lib.rs) | 5.59s |
| Orchestrator incremental (mcp_tools/) | 5.06s |
| CLI incremental | 26.76s |
| L0 leaf (vox-orchestrator-types) | 0.36s |

## Phase 1 — L0 type cleanup + plugin-host inversion fix (2026-05-08)

`vox-plugin-types` extracted (manifest + skill_manifest + state-backend trait).
`vox-plugin-host` no longer depends on `vox-db`. Daemon binary gated via
`required-features=mcp-native`.

| Scenario | Time | vs baseline |
|---|---|---|
| Orchestrator (touch lib.rs) | 6.24s | +0.65s (added plugin-types edge) |
| CLI incremental | 7.60s | **−72%** (−19.16s) |

## Phase 2 — workspace-hack leaf exclusion (2026-05-08)

Configured hakari's `[traversal-excludes]` and `[final-excludes]` so L0 leaves
don't pull in workspace-hack.

| Scenario | Time |
|---|---|
| L0 leaf (vox-plugin-types, true leaf) | 0.53s |

## Phase 3 — vox-db split (2026-05-08)

Audited; deferred. Orphan rule forces extension-trait migration for 67 impl
blocks (~50 callers). vox-compiler dep is structural (used for `@table`
parsing). Cost > benefit at current crate size.

## Phase 4 — Extract vox-orchestrator-mcp + vox-orchestrator-d (2026-05-08)

The 88K-LoC vox-orchestrator splits along its biggest internal seam:
- `mcp_tools/*` (33,885 LoC) → new crate `vox-orchestrator-mcp`
- `services/routes/` (axum HTTP routes) → moved with mcp
- `bin/vox_orchestrator_d.rs` → new crate `vox-orchestrator-d`

vox-orchestrator drops mcp-native feature and 14 deps that mcp owns
(schemars, axum, rmcp, tower-http, vox-compiler, vox-grammar-export,
vox-mcp-registry, vox-capability-registry, vox-openai-wire,
vox-project-scaffold, vox-skills, vox-openclaw-runtime, vox-plugin-host).

| Scenario | Time | vs baseline |
|---|---|---|
| Orchestrator (touch lib.rs) | **4.06s** | **−27%** |
| MCP isolated (touch mcp lib.rs) | 5.15s | new |
| CLI incremental | 7.60s | (unchanged from Phase 1) |

## Phase 5 — Extract vox-orchestrator-queue (2026-05-08)

Move locks/, oplog/, affinity.rs, sync_lock.rs (~3K LoC) to a new crate.
Also moved 4 pure-data types (SnapshotId, ChangeId, FileAffinity, AccessKind)
to `vox-orchestrator-types` so the queue crate has only L0 deps.

| Scenario | Time | vs baseline |
|---|---|---|
| Orchestrator (touch lib.rs) | **3.58s** | **−36%** |
| Queue isolated (touch lib.rs) | 6.84s* | new (*first build) |
| CLI incremental | 6.99s | **−74%** (cumulative from Phase 1) |

## Phase 6 — Orchestrator runtime split (deferred → C5 / Tier D) (2026-05-08)

`runtime.rs`, `orch_daemon/`, `dei_shim/` form the orchestrator's CORE — they
reference the `Orchestrator` struct directly along with `events`, `models`,
`services`, `types`. Extracting requires either a trait abstraction over the
Orchestrator type itself (huge — covers ~40 method surfaces) or moving the
Orchestrator struct out (which empties the parent crate). Either approach
exceeds reasonable scope.

The previously-completed Phase 4 + Phase 5 already cut 36K LoC out of the
orchestrator. The remaining core is dominated by the runtime/daemon layer,
which is the part that genuinely benefits from co-location.

> **Superseded (2026-05-15):** The correct extraction wedge is `src/orchestrator/`
> (12,825 LoC, the densest subdir) — not the full runtime split. The Rust coherence
> constraint (all `impl Orchestrator { }` blocks must live in the defining crate)
> requires co-moving the struct. Full analysis and 7-task plan at
> [`2026-05-15-orchestrator-tier-d-plan.md`](2026-05-15-orchestrator-tier-d-plan.md).
> Start when Rule 13 fires (>15% LoC growth since last release tag).

## Phase 7 — vox-cli decoupling (partial) (2026-05-08)

Most orchestrator-using commands in vox-cli (`attention`, `dei`, `safety`,
`visus`, `live`, `mcp_server/*`, `extras/ludus/hud`) are **already** gated
behind features (`dei`, `live`, `mcp-server`, `ludus-hud`). Only `generate`,
`model/*`, `ci/*` are unconditional, and the cumulative win from gating them
is small (~0.5s at most) given Phase 4+5 already trimmed 36K LoC from the
orchestrator transitive compile.

The originally-planned `OrchestratorClient` trait facade (covering ~40
methods) is large refactor work for limited additional payoff and is not
pursued.

| Scenario | Time | vs baseline |
|---|---|---|
| CLI incremental | 6.99s | **−74%** (cumulative — bulk came from Phase 1 + Phase 4) |

## Phase 8 — Plugin family flattening (no action) (2026-05-08)

Audited; structurally clean. vox-cli/vox-orchestrator don't compile-time
depend on any plugin (cdylib delivery). L4 → L3 plugin → vox-db deps are
allowed by the layer model.

## Phase 9 — Strict CI guard + final docs (2026-05-08)

Layer-check flipped from `--warn-only` to strict in CI. Three known
inversions documented in `layers.toml`:
- `vox-cli → vox-orchestrator` (deliberate; runtime/observability surfaces)
- `vox-pm → vox-compiler`, `vox-pm → vox-db` (transitional; future re-tier)

> **Update (2026-05-15):** The `vox-pm` inversions were removed when C2 from the
> followup design split `vox-package` → `vox-package-types` (L1 pure-data leaf)
> + `vox-package` (L3 build/registry). Current known inversions: `vox-cli →
> vox-orchestrator`, `vox-arch-check → vox-compiler` (dev-dep only),
> `vox-ml-cli → vox-cli` (optional mens-dei workflow). See `layers.toml`
> `[[known_inversions]]` for the authoritative list.

## Headline outcome

| Scenario | Baseline | Final | Win |
|---|---|---|---|
| Orchestrator incremental | 5.59s | 3.58s | **−36%** |
| CLI incremental | 26.76s | 6.99s | **−74%** |
| L0 leaf (true leaf) | n/a | 0.53s | new floor |
| MCP isolated | (in 5s orch) | 5.15s | newly parallel |
| Queue isolated | (in 5s orch) | <2s warm | newly parallel |

vox-orchestrator went from 88K LoC to 52K LoC. The 36K-LoC reduction
came from extracting `vox-orchestrator-mcp` (33K) and `vox-orchestrator-queue`
(3K). Editing files in those subsystems no longer triggers a full
orchestrator recompile.

## 2026-06-15 — Build-Time Reduction Program

Instruments: `vox ci build-bench` (Task 0.2–0.4) + `vox ci dep-cycles` (Task 1.1–1.3) + `scripts/crate-build-audit.vox` (Task 0.1).

| Phase | Change | Scenario | Δ% (measured) |
|---|---|---|---|
| 2.1 | retrieval → vox-db-types leaf | blastradius_vox_db_to_cli | PENDING-CI |
| 2.2 | vox-sql backend gating | check_vox_sql | PENDING-CI |
| 2.3 | mcp drop news-publish default | check_vox_mcp | PENDING-CI |
| 2.4 | vox-audit ci-gates | check_vox_audit | PENDING-CI |
| 3.1 | mcp `heavy-browser` gates browser_tools (~1.1k LoC) | check_vox_mcp | implemented, PENDING-CI measurement |
| 5.1 | affected-crate selective CI (PR #348) | PR-time wall-clock | PENDING-CI |

Note: PENDING-CI cells are filled by the non-blocking CI build-bench artifact on the first run after merge.
Phase dep-cycles inventory: see graphify-out/DEP_CYCLES.md (generated by CI).

## 2026-08-23 — Dependency-weight pass (workspace-hack, Windows debuginfo, feature trims)

Host: Windows 11, 12 logical cores. Workspace at the time: 136 crates, 1656
locked packages. Unlike the 2026-05-08 phases (which measured *incremental*
rebuilds), this pass targets **cold scoped builds** — the dominant cost when an
agent session or CI lane checks one crate in a fresh `target/`.

Headline, stated carefully because the obvious reading of it is wrong: cold
`cargo check -p vox-telemetry` went **590 → 409 compile units (−31 %)**, and on
this host that run went 43 m 57 s → 11 m 22 s. `vox-telemetry` is the
**maximum-benefit crate in the workspace** — 0 divergent units, 569 → 399 deps
(−30 %). Nothing else moves that far: `vox-cli` went 988 → 917, **−7 %**.

The −74 % wall-clock is *not* a throughput gain and must not be quoted as one.
31 % fewer units cannot yield 74 % less wall-clock on a 12-core host unless the
removed units sat on the **serial critical path**, and they did: `windows 0.62`
(843 s) plus `windows 0.61` (798 s) are two single-threaded units worth 27
minutes between them, and neither has anything to overlap with at the tail of a
scoped build. Remove them and the critical path collapses; the other ~179
removed units were nearly free.

**Unit count is the durable metric. Wall-clock here is not reproducible** — it
was measured on a host running concurrent agent builds out of four worktrees.
Trust the unit deltas and the `--timings` per-unit seconds; treat every
minutes-and-seconds figure in this section as an order-of-magnitude indication
from one contended host.

Several of the largest compile units (turso, chromiumoxide, both `windows`
trees) are still there — by necessity or by deliberate choice. See *Rejected*
below and *Top remaining wins* at the end.

### 0. Precondition: ENOSPC corruption (diagnostic signature)

`C:` was down to **916 MB free of 476 GB** and `cargo check --workspace` failed
with:

```text
error: couldn't create a temp dir: There is not enough space on the disk. (os error 112)
```

The truncated `.rmeta` files that leaves behind then produce errors that look
like *code* problems but are not:

```text
only metadata stub found for rlib dependency 'std'
found invalid metadata files for crate 'tokio'
error[E0460]: found possibly newer version of crate 'windows'
```

If you see any of those three, check free disk before debugging the code. This
repo is routinely checked out as several git worktrees, each with its own
`target/` (per `.cargo/config.toml`) — ~89 GB across 4 worktrees on this host.
Deleting `target/*/incremental` across all worktrees reclaimed **44 GB**.

Because both pre-change full-workspace runs (31 m 08 s and 11 m 45 s) ended in
this corruption, **neither is a clean "before" number** for §1. The clean
comparator is the controlled scoped A/B.

### 1. workspace-hack over-inclusion (the main change)

77 of 127 workspace crates depend on `workspace-hack`, so every package inside
it is paid by all of them. `.config/hakari.toml` `[final-excludes] third-party`
gained 14 entries: `tauri`, `windows`, `windows-core`, `gix`, `gix-diff`,
`gix-features`, `gix-pack`, `gix-packetline`, `gix-protocol`, `gix-revision`,
`gix-tempfile`, `gix-transport`, `wasmtime-environ`, `wasmtime-internal-core`.

`windows-sys` and `reqwest` were deliberately left **in**: they are genuinely
shared (windows-sys via tokio/rustix; reqwest via `vox-http-client` at fan-in
24), so hakari's unification pays there.

Worst per-unit compile times from `cargo check --workspace --all-targets
--timings` (seconds wall, 12-core host):

```text
843.5  windows 0.62.2        600.0  turso_core 0.6.1     326.3  gix 0.84.0
797.6  windows 0.61.3        546.2  chromiumoxide_cdp    265.2  jj-lib 0.42.0
381.3  reqwest 0.13.4        338.1  candle-transformers  262.3  wit-parser
242.3  wasmtime-environ      233.3  windows 0.54.0       219.5  cranelift-assembler-x64
```

Dependency counts before → after `cargo hakari generate`:

| crate | before | after |
|---|---|---|
| workspace-hack | 568 | 398 |
| vox-telemetry | 569 | 399 |
| vox-git | 577 | 475 |
| vox-config | 648 | 551 |
| vox-repository | 649 | 553 |
| vox-code-audit | 653 | 557 |
| vox-compiler | 676 | 581 |
| vox-cli | 988 | 917 |

Controlled cold A/B — `cargo check -p vox-telemetry`, fresh `CARGO_TARGET_DIR`,
same machine, same `.cargo/config.toml`, only the hakari change differing; both
runs exited 0:

| | units | wall |
|---|---|---|
| before | 590 | 43 m 57 s |
| after | 409 | 11 m 22 s |

**This change buys no full-workspace win at all.** That is not a hedge, it is
the sign of the effect: the workspace-wide feature union is unchanged (hakari
unification never removed a package from the workspace, it only stopped 77
crates from paying it *individually*), and dropping unification means
per-consumer feature divergence now compiles some packages **more than once**.
Current `target/debug/deps` holds **5 distinct `libgix-*.rmeta` variants and 8
distinct `libwindows-*.rmeta` variants**, each with its own fingerprint
directory. The full-workspace gains in this pass come only from §4 (`sysinfo`)
and §5 (`cpal`), which delete work outright.

The cost of that divergence is **disk, not time**. Each variant is a separate
artifact set, multiplied across 4 worktrees, on a host whose §0 above is an
ENOSPC post-mortem — that is the real bill. It does **not** cause repeated
rebuilds: every variant stays warm in its own fingerprint dir, so nothing
thrashes; it just occupies space.

On the full-workspace number: the only *instrumented* post-change
`cargo check --workspace --all-targets` run is **87 m 32 s / 2821 units**, of
which **4577.9 s (76 min) is the `vox-gui` build-script run** — a nested full
release build of `vox-cli` to produce the missing Tauri sidecar (see the
sidecar note in AGENTS.md §Perennial Bug Patterns). The **19 m 08 s** figure is
real but conditional: it is only reachable in a target dir where that sidecar
already exists. Quote it only with that precondition attached.

### 2. `jobs = 24` removed from `.cargo/config.toml`

12-core host, and the repo is routinely built from several worktrees
concurrently by agent sessions — 4 × 24 is up to 96 rustc processes. Cargo's
default is the logical CPU count. **Not separately benchmarked**: it was held
constant across both halves of the §1 A/B.

### 3. Windows debuginfo was emitted and then discarded

`.cargo/config.toml` passes `-C link-arg=/DEBUG:NONE`, so lld-link emits no
PDB — and MSVC symbolication reads the PDB. `[profile.dev] debug = 1` was
therefore paying codegen for line tables that never reached a linker output.

Measured: `llvm-objdump -h` on `libvox_compiler.rlib` shows **8.4 MB of 23.6 MB
of section bytes (35.6 %) in `.debug$*`**. First-party rlibs total 0.86 GB
across 92 rlibs in this checkout. Zero PDBs exist anywhere in `target/`.

Backtrace evidence, same source, `debuginfo=1` both times:

- with `/DEBUG:NONE`: every frame `<unknown>`, no `file:line`
- without: `std::backtrace_rs::backtrace::win64::trace at ...win64.rs:85`

So the old profile comment "Backtraces and breakpoints still work" was **false
on Windows MSVC**. Note that `crates/vox-cli/src/diagnostics.rs:115` tells users
to set `RUST_BACKTRACE=1`, which on Windows currently yields a wall of
`<unknown>`.

Fix: `-C debuginfo=0` prepended to the Windows `rustflags`, `/DEBUG:NONE`
**kept**. Both are required — verified:

```text
rustflags = ["-C","debuginfo=0"]                              -> dbgtest.pdb, 1,282,048 bytes
rustflags = ["-C","debuginfo=0","-C","link-arg=/DEBUG:NONE"]  -> no PDB
```

(Config `rustflags` append after cargo's profile flag, and rustc takes the last
`-C debuginfo`.) Linux keeps `debug = 1`.

Two costs the first write-up of this section left out.

**(a) It invalidated every existing target dir.** Changing `[target.…]`
rustflags changes the fingerprint of every unit, so this was a one-time full
rebuild in each of **4 worktrees (~89 GB of artifacts)** plus a full sccache
miss wave. Given that §0 of this same section is a disk-exhaustion incident,
that is not a footnote — a rustflags edit on this host is a capacity event, and
should be sequenced after a `target/*/incremental` sweep, not before.

**(b) It silently overrides `[profile.release] debug = true`.** By the same
append-and-last-wins rule quoted above, a `debug = true` added to `Cargo.toml`
for a profiling run is emitted *first* and then overridden by the config's
`-C debuginfo=0`. No warning, no error — the profile key simply stops working.
Every Windows sampling profiler (Superluminal, WPA, VTune, cargo-flamegraph)
needs a PDB, so **profiling can no longer be enabled from `Cargo.toml` on this
host**. The only escape is `$env:RUSTFLAGS`, which *replaces* the whole array
rather than appending — and so silently drops `lld-link`'s `/STACK:8388608`,
which this workspace needs. `.cargo/config.toml` documents this tradeoff at the
flag; this log did not, which is how it got lost.

Unaffected: `#[track_caller]` panic locations. Those are compile-time
`Location` values baked into the binary, not debuginfo, so `panic!`/`unwrap`
messages still name the right file and line with `debuginfo=0`. Only *symbolicated
backtraces* and breakpoints are lost.

Also found: `split-debuginfo = "unpacked"` is a **silent no-op on MSVC** — the
value is unstable on that target and cargo drops the flag entirely (verified via
`cargo build -v`: no `-Csplit-debuginfo` in the emitted rustc line). It is
effective on Linux only.

### 4. `sysinfo` default features trimmed

```toml
sysinfo = { version = "0.39", default-features = false, features = ["system"] }
```

`sysinfo`'s `default = ["component","disk","network","system","user"]` fans out
to ~35 `Win32_*` modules on the `windows` crate, including `Win32_System_Wmi`.
`process-wrap`, the only other `windows` 0.62 consumer, declares no features at
all — so sysinfo's defaults *were* the entire 0.62 feature union behind the
843 s unit. All 8 workspace call sites use only `System` / `Pid` /
`ProcessesToUpdate` (verified). Zero source changes.

**Precision on the `windows` crate, because the loose phrasing elsewhere in this
pass implied more than happened.** `windows 0.62.2` is **not gone**. It is in
`Cargo.lock`, it is in the graph, and it is in the latest `--timings` at
**154 s** — down from 843 s, **−82 %**, which is the actual result of the
feature trim. `windows 0.61.3` likewise stayed, dropping **798 s → 222 s**, and
is now reachable only through `tauri → vox-gui`. What is genuinely *gone* is
**`windows 0.54.0`** (the 233 s unit), eliminated by the `cpal` bump in §5.
`Cargo.lock` now contains exactly two `windows` versions, 0.61.3 and 0.62.2.

### 5. `cpal` 0.15 → 0.17

`cpal` 0.15/0.16 pin `windows ^0.54`, the sole reason a third `windows` tree
(233 s) builds at all. 0.17 accepts `windows >=0.59,<=0.62`, and also drops
`windows-core` 0.54, `windows-result` 0.1, `windows-targets` 0.42, and eight
`windows_*_0.42` arch crates. 0.17 rather than 0.18 because 0.18 adds
unconditional `pipewire` + `pulseaudio` deps on the Linux CI fleet.
`SampleFormat::U16` (used at `crates/vox-gui/src/commands/mic.rs:217`) still
exists in 0.17.3 — verified against the registry source.

### 6. `turso` feature-gated out of `vox-secrets` — REVERTED, inert

**This section describes a change that is no longer in the tree. The numbers in
it do not hold.** It is kept as a record of a bad trade, not as a description of
the build.

What was done: `vox-secrets` (first-party fan-in 34) carried an unconditional
`turso` dep used only in `backend/vox_vault.rs`; `turso` was made `optional`
behind a default-ON `secrets-vox-vault` feature, and `crates/vox-config` took
`vox-secrets` with `default-features = false`, reported as `cargo tree -p
vox-config` **551 → 510**.

What is true now: `crates/vox-config/Cargo.toml:14` reads
`vox-secrets = { workspace = true }`. The opt-out was **reverted after code
review** found it silently disabled vault-backed API-key resolution for every
consumer of `vox-config` — which is 63 of 127 crates. `cargo tree -p vox-config`
still resolves **9 `turso*` crates**, so the 551 → 510 delta is inert.

The lesson is the trade, not the number: this was a **scoped-build-only** win
(cargo unifies features per package, so a whole-workspace build compiled `turso`
regardless) bought with a **silent-secret-loss hazard** in the default
configuration of a crate two thirds of the workspace depends on. The
`SecretError::BackendUnavailable` + `tracing::error!` path made the failure loud
at the backend, but not at the call sites that had quietly stopped resolving.
Do not re-apply this shape without inverting the default.

### 7. Agent build parallelism capped at `CARGO_BUILD_JOBS=4`

`.claude/settings.json` sets `CARGO_BUILD_JOBS=4` for agent shells. A human shell
outside the harness is unaffected, which is the intent.

**The basis is memory, not cores.** Measured on this host:

```text
physical RAM ........................ 15.7 GB
free physical RAM, zero rustc ....... 1.6 GB
commit charge / limit ............... 31.1 GB / 58.7 GB
peak working set, one rustc ......... 1,193 MB
```

4 × ~1.2 GB ≈ 4.8 GB, inside the reclaimable headroom. A core-derived value of 12
would imply roughly 14 GB of resident compilers on a 15.7 GB machine — a paging
plan, not a cap. Re-derive as `available_bytes / peak_rustc_working_set`, never
from `nproc`.

This replaced a host-wide jobserver broker that was designed, audited, and
rejected: CPU oversubscription has no causal path to a nonzero rustc exit, and a
Windows named semaphore has no owner-death recovery, so every hard-killed cargo
would have leaked tokens permanently — fed by `vox ci kill-stuck-tests`, the
design's own recovery tool. Full reasoning:
[`docs/superpowers/specs/2026-08-23-build-concurrency-governor-design.md`](../../superpowers/specs/2026-08-23-build-concurrency-governor-design.md).

### Rejected, and why

- **Excluding `vox-plugin-browser` (`chromiumoxide_cdp`, 546 s) from the
  workspace.** Only 6 crates are unique to it — 572 of its 578 deps are already
  paid by `vox-cli` — and `default-members = ["crates/vox-cli"]` means a bare
  `cargo check` skips it already. Excluding it would break the dedicated
  `vox-browser-cdp-smoke` job at `.github/workflows/ci.yml:1619` (an excluded
  package is not addressable by `-p`), lose 12 unit tests and clippy over 1,257
  lines, and drift four gated SSOT contracts. The repo's own idiom for this is
  `--exclude` at the invocation on check-shaped lanes — as `ci.yml` already does
  for `vox-gui` in six places — not a manifest exclude.
  **Related misconception, corrected:** the other `vox-plugin-*` entries in the
  root `Cargo.toml` `exclude` list are *not* excluded for dependency weight.
  They contain only `Plugin.toml` + `.skill.md` and have no `Cargo.toml` at all;
  they are excluded because `members = ["crates/*"]` globs them and cargo errors
  on non-package directories. `server/telemetry` is the only genuine heavy-deps
  exclusion, and it has its own workspace.
- **Collapsing `windows` 0.61 + 0.62 to one version.** Impossible today. Every
  published `tauri` 2.x (2.5.0 → 2.11.5) pins `windows ^0.61`; there is no 2.x
  on 0.62. The reverse — everything onto 0.61 — needs a 2-major `sysinfo`
  downgrade to ≤ 0.37.2 plus a transitive `process-wrap` 9.0.0 pin that breaks
  on the next `rmcp` bump.
- ~~**`reqwest` 0.12 → 0.13**, blocked by the cryptography policy.~~
  **Wrong, and inverted — reclassified as an open opportunity below.** The
  premise was that adopting 0.13 would *pull in* `aws-lc-rs`. It is already
  here: `reqwest 0.13.4` is in `Cargo.lock` (via `chromiumoxide`,
  `gix-transport`, `rmcp`, `self_update`, `tauri`), `workspace-hack` enables its
  `rustls` feature which expands to `__rustls-aws-lc-rs`, and **`aws-lc-rs
  1.17.0` is compiled in this tree today**. Moving first-party code to 0.13
  therefore *removes* a ~206 s duplicate unit rather than adding a banned
  dependency. See *Top remaining wins*.
- **`syn` 1.0.109, `schemars` 0.9, `safetensors` 0.4 / `cudarc` 0.17 /
  `gemm*` 0.18, `bindgen` 0.69, `prost` 0.13.** All transitive, no first-party
  lever. `schemars` 0.9 in particular is lock-resolution noise from an optional
  dep of `serde_with` that is never activated — which is why it never appeared
  in `--timings` — and the deliberate `schemars` / `schemars08` dual pin in the
  root manifest is correct. The `safetensors` / `cudarc` / `gemm` trio all trace
  to one stale crate, `ug 0.5.0`, via candle.

### Top remaining wins (ranked, as of 2026-08-23)

Ordered by measured seconds, not by ease.

1. **`crates/vox-gui/build.rs`'s nested release build of `vox-cli` — 4577.9 s.**
   By an order of magnitude the #1 cost in the workspace, and it dwarfs
   everything this pass touched. It exists to produce the Tauri `externalBin`
   sidecar when it is missing; any target dir that already has the sidecar skips
   it entirely, which is exactly why it stayed invisible in earlier numbers.
   Making it a checked precondition rather than an implicit nested build is the
   single highest-value change available.
2. **`wasm-encoder` + `pulley-interpreter` still in `workspace-hack` — ~856 s.**
   A straight omission from the §1 pass: the `wasmtime-*` entries were excluded
   but these two were not, and they drag `wasmparser` (558 s) plus
   `pulley-interpreter` (166 s) into all 77 hack-dependent crates. Same
   one-line `[final-excludes]` fix as §1, same divergence caveat.
3. **`vox-orchestrator-mcp` + `vox-orchestrator` ≈ 1311 s across 4 units.**
   First-party, so this is the one item here we fully control. A Tier-D
   decomposition plan already exists at
   [`2026-05-15-orchestrator-tier-d-plan.md`](./2026-05-15-orchestrator-tier-d-plan.md).
4. **`reqwest` 0.12 → 0.13 — ~206 s duplicate unit.** Not policy-blocked (see
   *Rejected*). Needs feature and/or `[patch]` surgery across five transitive
   consumers — `chromiumoxide`, `gix-transport`, `rmcp`, `self_update`, `tauri`
   — plus `reqwest-middleware` / `reqwest-retry`, which still pin 0.12. Fiddly,
   not forbidden.
5. **`sherpa-onnx-sys` build script — 170 s.** Feature-gateable out of every
   non-audio lane.
