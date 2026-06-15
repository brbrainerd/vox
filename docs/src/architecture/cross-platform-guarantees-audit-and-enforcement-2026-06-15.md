---
title: "Cross-Platform Guarantees: Audit + Dynamic Enforcement Plan (2026-06-15)"
description: "Codebase-wide audit of Windows/Linux/macOS compilation and build surfaces, the CI/CD + Docker coverage gap, the two confirmed build breaks, and a phased plan to forcibly prove and dynamically enforce 3-OS compatibility as the code changes."
category: "Architecture SSOTs"
---

# Cross-Platform Guarantees: Audit + Dynamic Enforcement Plan

> Status: **Plan / design — no code landed.** Produced by a full-repo graphify graph
> build (62,594 AST nodes / 143,930 edges over 4,252 code files + semantic doc layer)
> overlaid with an 8-avenue semantic audit. All file:line citations below were
> hand-verified or cross-corroborated by ≥2 independent audit passes.

## 1. Verdict

This workspace was built primarily on Windows and must compile + build + test on
**Windows (`x86_64-pc-windows-msvc`)**, **Ubuntu Linux (`x86_64-unknown-linux-gnu`)**,
and **macOS (`aarch64-apple-darwin`)**.

The **code** is, on most avenues, disciplined about portability — per-OS path
resolution, `[target.'cfg(...)']` dependency tables, and cfg-gated symbol pairs are
the norm. The **enforcement** is the hole:

1. **The blocking merge gate runs on Linux self-hosted only.** The required check
   `Check, Build, and Test (Rust)` (`ci.yml:935` `ci-summary`) `needs:
   [guards-fast, lints, compiler-gates, tests, audits]` — every one of those jobs is
   `runs-on: [self-hosted, linux, x64]`. **No Windows or macOS job is in the required
   dependency chain.** The only thing proven on Windows/macOS *per PR* is "`vox-cli`
   builds and `vox compile` has a help screen," and only when CLI-adjacent paths change.
2. **There is no cross-platform detector or gate today.** Portability is convention-only
   (`CLAUDE.md`/`MEMORY.md`: no `cargo fmt --all`, `CREATE_NO_WINDOW` on spawns). A
   purpose-built detector (`scripts/coverage-graph/os_compat.py`) and a script-policy
   gate (`vox ci script-hygiene`) **both exist but are unwired** into CI/pre-push.
3. **Two real build/run breaks already exist on Linux/macOS** (§3) — invisible because
   the gate never compiles those OSes with the relevant feature set.

The fix is two-pronged and matches existing machinery exactly: **(A)** promote a real
3-OS `clippy + check + test` matrix into the required gate; **(B)** add a *dynamic*
cross-platform enforcement layer built on the existing `vox-arch-check` forbidden-pattern
scanner and `vox-code-audit` detector trait — both glob-over-disk walkers driven by SSOT
config, so new files/crates are covered automatically as the codebase changes.

## 2. The complete cross-platform surface ("avenues")

The audit decomposed portability risk into 8 non-overlapping avenues. This is the
**surface map** — the enforcement layer in §5 must cover every one.

| # | Avenue | Health | Where it lives |
|---|--------|--------|----------------|
| 1 | Conditional compilation (`cfg`) OS-coverage | Strong (203 sites, ~all balanced) | `crates/**/*.rs`, `build.rs` |
| 2 | Path & filesystem portability | Strong in Rust; weak in `.vox` scripts | `vox-config/src/paths.rs`, `vox-identity/src/storage.rs`, `scripts/**` |
| 3 | Process spawning & shell-outs | Mixed (~7 risky + console-window gap) | `Command::new` sites (~498, 186 files) |
| 4 | Platform deps & build scripts | Strong; **1 hard break** | every `Cargo.toml`, `build.rs`, `.cargo/config.toml` |
| 5 | Line endings / encoding / goldens | Mature but coverage mismatch | `.gitattributes`, `*.snap`, `*.txt`, `vox-cli-ci::line_endings` |
| 6 | CI/CD + Docker coverage | **The core gap** (Linux-only gate) | `.github/workflows/*.yml`, `Dockerfile*` |
| 7 | Existing enforcement machinery | 5 layers exist; **0 portability rules** | `vox-arch-check`, `vox-code-audit`, `contracts/**`, `lefthook.yml` |
| 8 | Scripts / GUI / mobile / toolchain | Spine exists; gate unwired | `scripts/**`, `vox-gui` (Tauri), mobile workflows |

## 3. Confirmed defects (hand-verified — fix these first)

### D1 — `vox-ml-cli` fails to build on Linux + macOS (HIGH, verified)
`crates/vox-ml-cli/src/commands/schola/process_priority.rs:32` calls
`libc::setpriority(libc::PRIO_PROCESS, 0, 10)` under `#[cfg(unix)]`, but
`crates/vox-ml-cli/Cargo.toml` declares **`windows-sys` (line 94) and no `libc`** and
**no `[target.'cfg(unix)'.dependencies]` block**. Reached via the `schola` train path
gated behind `feature = "gpu"` (also implied by `mens-qlora` / `mens-candle-cuda` /
`cloud`) — the MENS training build.

- **Breaks:** `cargo build -p vox-ml-cli --features gpu` on Linux/macOS →
  `E0433: use of undeclared crate or module 'libc'`.
- **Why hidden:** on Windows the `#[cfg(unix)]` arm is stripped, so the workspace's
  primary dev OS never exercises it. *This is the canonical "built on Windows, broken
  elsewhere" defect.*
- **Fix:** add to `crates/vox-ml-cli/Cargo.toml`
  `[target.'cfg(unix)'.dependencies]\nlibc = { workspace = true }` (`libc` is already a
  root workspace dep). Mirror the canonical template `crates/vox-cli/Cargo.toml:280-290`.
  (Low, same crate: move its unconditional `windows-sys` into
  `[target.'cfg(windows)'.dependencies]`.)

### D2 — `flywheel.rs` autonomous-training spawn is dead + Windows-only (HIGH, verified)
`crates/vox-orchestrator/src/services/flywheel.rs:136`
(`trigger_autonomous_training`) spawns `pwsh -File scripts/mens-full-pipeline.ps1`.
The script **does not exist anywhere in the repo** (verified) and `pwsh` is hardcoded
with no Unix branch.

- **Breaks:** always errors (missing script) on Windows; additionally non-portable.
- **Fix:** remove the dead function, or drive the pipeline via `vox run scripts/*.vox`
  (project VoxScript-first policy) and resolve the shell via
  `which::which("pwsh").or_else(|_| which::which("powershell"))`.

### D3 — `cargo fmt --all` inside a `.vox` script (HIGH, policy self-violation, verified-by-agent)
`scripts/perf/test-baseline.vox:81` calls `cargo fmt --all --check`. The entire
`os error 206` Windows mitigation depends on **never** running `cargo fmt --all`
(`CLAUDE.md`, `AGENTS.md §Formatting`). Fix: use the sanctioned `VOX_FMT_CHECK=1 vox run
scripts/fmt.vox`.

## 4. CI/CD + Docker gap analysis (avenue 6 — the core ask)

### 4.1 What proves what, today

| Workflow : job | OS / host | Trigger | Depth | Gating |
|---|---|---|---|---|
| `ci.yml` : `ci-summary` = **"Check, Build, and Test (Rust)"** | self-hosted linux | PR / push / merge_group | aggregate of below | **REQUIRED** |
| `ci.yml` : guards-fast / lints / compiler-gates / tests / audits | self-hosted linux | PR / push / mq | fmt, **clippy** (excl. `vox-gui`), nextest `--workspace`, `--all-features` check | feed required |
| `cross-platform-check.yml` | **windows-latest + macos-latest** | **weekly cron Mon** + dispatch | `cargo check --workspace` (**excl. `vox-gui`**) + nextest of 3 crates | ❌ not required |
| `compile-matrix.yml` (win/mac legs) | windows-latest, macos-latest | **PR, path-filtered (~8 globs)** | `cargo build -p vox-cli` + **`vox compile --help` only** | ❌ |
| `setup-e2e.yml` | ubuntu + windows + macos | push:main / nightly / dispatch (**not PR**) | clean-room `cargo run` + `cargo check --workspace` | ❌ |
| `ci-fallback-hosted.yml` : `gui-windows-build-smoke` | windows-latest | **dispatch only** | full Tauri GUI build + headless smoke | ❌ |
| `release-*` / `bundle-release` / `release-gui` | win/mac/linux | **tag / release only** | build + package + installer | tag only |
| `docker-eval.yml` + root `Dockerfile` | ubuntu-latest | push:main (Dockerfile paths) | buildx `vox-cli` image + Trivy (report-only, exit 0) | ❌ |

### 4.2 What each OS does NOT prove per PR

- **Linux (the only gated OS):** full clippy + `nextest --workspace` + coverage — but
  **`vox-gui` is excluded** from gated clippy (`ci.yml:509`) and never compiled on a PR;
  Docker image build, GUI Playwright, browser/CDP, and the 24-crate `--all-features`
  matrix are `push:main`/`full-ci`-label only.
- **Windows:** on a routine PR, **nothing** unless a `compile-matrix.yml` path changed —
  and then only `vox compile --help`. **No `cargo check --workspace`, no clippy, no
  tests.** Windows `#[cfg(windows)]` paths (path separators, `CREATE_NO_WINDOW`, the
  isolated mens-gate runner, `os error 206` logic, Tauri sidecar staging) are unexercised
  per PR. Full `cargo check --workspace` is weekly-cron-only and excludes `vox-gui`.
- **macOS:** identical to Windows. Real `cargo check --workspace` is weekly-cron-only,
  `vox-gui`-excluded, targeting `aarch64-apple-darwin` (the stated goal triple).

**Quantified:** `cargo check --workspace`, `clippy -D warnings`, and `nextest
--workspace` are proven on **zero** non-Linux OSes per PR.

### 4.3 Docker gaps
Docker only ever simulates **Linux** (`Dockerfile.ci-runner` is the sanctioned
"simulate Linux from a Windows dev box" path). Gaps: Trivy is report-only (never blocks);
image is single-arch `linux/amd64` (no `linux/arm64`); the PR gate builds **no** Docker
image (smoke is push:main-only), so a Dockerfile-breaking PR merges green.

## 5. Enforcement design (phased — the deliverable)

Two prongs. Prong A forces proof; Prong B prevents the *sources* of breakage from being
introduced. Both reuse existing machinery so they cover all surfaces dynamically.

### Prong A — Force 3-OS proof in the required gate

| Phase | Change | Cost note |
|---|---|---|
| **A0** | Fix D1/D2/D3 so a 3-OS gate can go green at all. | trivial |
| **A1** | Promote `cross-platform-check.yml` from weekly cron → `pull_request` + `merge_group` + `push:main`; upgrade depth from `cargo check` to **`clippy -D warnings` + `nextest run --workspace`** (mirror `ci-fallback-hosted.yml:73-92`); **add an `ubuntu-latest` row** (the workflow's name claims cross-platform but omits Linux). sccache-GHA + `CARGO_INCREMENTAL=0` already wired. | GitHub-hosted win (2×) + mac (10×) minutes |
| **A2** | Make the new Win/mac/Ubuntu jobs **required**: either add them to `ci-summary.needs`, or register per-OS checks `Check, Build, and Test (Windows)/(macOS)` in branch protection. To bound cost, run cheap `check`+`clippy` on every PR but defer full `nextest --workspace` on Win/mac to `merge_group` only (run once per merge batch). | merge-queue already enabled |
| **A3** | Close the `vox-gui` hole: promote `ci-fallback-hosted.yml`'s `gui-windows-build-smoke` (full pnpm→sidecar→`cargo build -p vox-gui`) to PR-triggered + required, and add Linux WebKitGTK + macOS WKWebView legs. Add an explicit sidecar `vox-<triple>[.exe]` build+rename step before `tauri-action` (fixes the `release-gui.yml` externalBin + `src-tauri` signing-path issues). | Linux needs `libwebkit2gtk-4.1-dev` |
| **A4** | Replace `compile-matrix.yml`'s `vox compile --help` win/mac legs with `cargo check --workspace --exclude vox-gui`, or retire it (superseded by A1). Add a `linux/arm64` buildx leg + make Trivy gating for the Docker path. | — |

### Prong B — Dynamic cross-platform detectors (cover surfaces as code changes)

The right homes already exist and are **glob-over-disk SSOT-driven**, so they scan every
new file/crate automatically — no hand-maintained file lists:

1. **`vox-arch-check` Rule 11 `[[forbidden_pattern]]`** (`docs/src/architecture/layers.toml`,
   scanned by `crates/vox-arch-check/src/forbidden_patterns.rs:63`). Has per-line
   `allow_annotation` escape hatch + `exempt_files` + per-rule strictness promotion
   (`[guards]`, ship `warn` → promote to `error` once clean, exactly as `orphan` was).
   Add rules for:
   - `no-hardcoded-shell-spawn` — `Command::new("(cmd|powershell|pwsh|sh|bash)")` outside a
     cfg-gated/`which`-resolved wrapper (avenue 3).
   - `missing-create-no-window` — `Command::new(` in `crates/vox-gui/**` lacking
     `creation_flags`/`quiet_command` (only **2 of ~180** spawn sites set it today). Best as
     a small bespoke check (multi-line) scoped to GUI.
   - `ungated-platform-use` — `use (winapi|nix|core_foundation|objc)::` not under a matching
     `#[cfg(...)]`, cross-checked against the crate's `[target.*]` tables (catches the D1
     *class*).
2. **`contracts/code-audit/rules.v1.yaml`** (loaded by `rule_pack_detector.rs`, no recompile,
   fixture-scored) — regex hazards across Rust+TS+Vox, beside the existing
   `magic-value/path` rule (`rules.v1.yaml:530`): hardcoded path separators / `C:\` / `/tmp`
   string literals, `.ps1`/`.sh` literals, `/dev/null` vs `NUL`.
3. **`vox-code-audit` detector trait** (`rules.rs:214`, register in `detectors/mod.rs:148`,
   bump `rule_count`) — for anything needing AST context (the `llm_provider_call` detector is
   the template). Candidate: `\\?\` canonicalize-leak detector (avenue 2).
4. **Wire the two dark gates that already exist:**
   - `scripts/coverage-graph/os_compat.py` — a complete portability scanner that **no CI job
     references**, yet its committed output `graphify-out/OS_COMPATIBILITY.md` **already reports
     168 un-gated portability findings + 31 asymmetric-cfg files** across 3,271 Rust files. This
     is the proof the gate works and the tree is already dirty. Its current category counts
     (un-gated): `abs-unix-path` 64, `dynlib-ext` 33 (`.so`/`.dll`/`.dylib` suffix assumptions —
     an avenue the manual sweep under-weighted), `path-sep-env` 12, `crlf-literal` 17,
     `path-join-fmt` 11, `env-home-asym` 9, `shell-command` 5, `home-tilde` 3, `win-drive-path` 2,
     `os-unix-api` 11, `os-windows-api` 1. Wire it into `vox ci` at `warn`, drive the 168 down,
     then promote to `error`; extend its glob from `crates/**/*.rs` to also cover
     `scripts/**/*.vox` (where the real breakage lives).
   - `vox ci script-hygiene` (`run_script_hygiene`, `matrix.rs:97`) — enforces the
     "`.vox`-only automation" policy but is **never invoked**; 12 `scripts/coverage-graph/*.py`
     files slipped past it. Add to `guards-fast` + pre-push; add `.bat`/`.cmd` to the
     prohibited-extension set; prune the 2 dead allowlist entries.
5. **Line-endings backstop** (avenue 5): change `.gitattributes:2` to `* text=auto eol=lf`
   then `git add --renormalize .`; add a config-independent CI gate `git ls-files --eol |
   grep -E 'i/crlf|i/mixed'` (whitelisting `*.ps1 *.bat *.cmd`); sync `EXT_LF`
   (`vox-cli-ci/src/line_endings.rs:10`) with the `.gitattributes` `eol=lf` set + add a parity
   test; strip the 46 byte-0 BOM files + add a BOM gate; normalize newlines in
   `command_catalog_paths_baseline.rs:51`.

### Why this is "dynamic"
Both forbidden-pattern and the detector trait are **walkers over disk driven by SSOT
config**, not allowlists. A crate added next month is scanned on the next run with zero
edits; the only per-site maintenance is adding an `allow_annotation` when a platform call is
*intentionally* one-sided — which is the desired forcing function. A new
`contracts/portability/surface-registry.v1.yaml` was considered and **rejected**:
cross-platform hazards are anti-patterns to *forbid*, not surfaces to *enumerate*, so the
forbidden-pattern model fits better than the GUI-style surface registry.

## 6. Recommended execution order

1. **A0 / §3** — land D1, D2, D3 fixes (without them no 3-OS gate can pass).
2. **B.4** — wire `script-hygiene` + `os_compat.py` (cheap, immediate, no infra cost).
3. **B.5** — `.gitattributes eol=lf` + CRLF/BOM gates (eliminates a whole flake class).
4. **A1 → A2** — promote `cross-platform-check.yml` to a required 3-OS clippy+test matrix.
5. **B.1–B.3** — land forbidden-pattern + detector rules at `warn`, drive the tree clean,
   then promote to `error`.
6. **A3 / A4** — `vox-gui` 3-OS build proof + Docker arm64/Trivy hardening.

## 7. Done-right templates (copy these)

- `crates/vox-cli/Cargo.toml:280-290` — four `[target.'cfg(...)']` dep blocks (the D1 fix).
- `crates/vox-cli/src/commands/runtime/run/sandbox.rs:16` — linux/windows/`not(any())` code-gating.
- `crates/vox-config/src/paths.rs` + `crates/vox-identity/src/storage.rs:58-70` — per-OS dir + `#[cfg(unix)]` perms with a `not(unix)` fallback.
- `crates/vox-cli/src/fs_utils.rs:83-104` — `strip_windows_verbatim_path` (`\\?\` neutralizer).
- `crates/vox-cli-core/src/fs_utils.rs:23` — `open_browser_sync` cfg-gating `cmd /C start` / `open` / `xdg-open`.
- `crates/vox-cli/build.rs:9-17` — `CARGO_CFG_TARGET_OS`-based linker branch (cross-compile-safe).
- `scripts/render-durable-animation.vox:19-23` — multi-OS binary auto-detect candidate list.

## 8. Branch-protection wiring (manual, one-time)

After `cross-platform-check.yml` runs green on a PR, a repo admin must add the status check
**`Cross-Platform (Win/macOS/Ubuntu)`** to the `main` branch-protection required-checks set
(alongside the existing `Check, Build, and Test (Rust)`). This is the step that makes 3-OS proof
*forced* rather than advisory. Until then the matrix runs on every PR but does not block merge.
