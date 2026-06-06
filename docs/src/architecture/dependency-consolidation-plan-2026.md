---
title: "Dependency consolidation plan (2026)"
description: "Audit of every external tool, env var, and runtime dependency the repo expects; design for a unified Rust installer (vox doctor --install) extending the existing surface; phased migration that can merge cleanly back to main."
category: "Architecture SSOTs"
status: "roadmap"
last_updated: "2026-05-09"
training_eligible: true
training_rationale: "Explains the SSOT design for installer policy, cross-platform dependency management, and the Rust-native approach to replacing shell-scripted toolchain setup."
schema_type: "TechArticle"
---

# Dependency consolidation plan (2026)

**Status:** draft, awaiting approval before implementation.
**Goal:** minimize external install steps, consolidate setup under existing
Rust surfaces (`vox doctor`, `vox-bootstrap`, `vox-install-policy`),
maximize cross-platform parity, and exit clean enough to merge back to main.

## TL;DR

1. We already have **most of the installer infrastructure**:
   [`vox doctor`](../../../crates/vox-cli/src/commands/diagnostics/doctor/) (with
   per-check `auto_heal`),
   [`vox-install-policy`](../../../crates/vox-install-policy/) (SSOT for install
   surfaces), `vox-bootstrap` (offline-friendly companion binary), and
   `vox setup` (registration). The gap is **one unified entry point** —
   `vox doctor --install` (or `vox setup --full`) that walks every check and
   heals what it can.
2. The biggest payoff is **eliminating per-tool install instructions from
   docs**. Today every external (act, pnpm, mdbook, cargo-nextest, …) has a
   bespoke install paragraph. After consolidation, one command does it.
3. The biggest constraint is **`AGENTS.md §VoxScript-First Glue Code`** —
   automation MUST be `.vox`, not `.ps1`/`.sh`/`.py`. We have current
   violators (one-line `python3` blocks in `ci.yml`, two thin
   `vox-dev.{ps1,sh}` launchers). The launchers are policy-blessed
   exceptions; the python3 lines are not.
4. **Hard floor on what cannot be replaced:** Docker daemon, CUDA `nvcc`,
   real Chromium binary, Git, the OS itself. Everything else is in scope.

## Audit: every external the repo expects today

### A. CLI binaries shelled out from Rust crates

Sourced via `grep -rh 'Command::new(' crates/`. Categorized by
"can-we-replace-with-Rust" verdict:

| Binary | Used by | Replace with | Verdict |
|---|---|---|---|
| `cargo`, `rustc`, `rustup` | many | — | **keep** (Rust toolchain itself) |
| `git` | vox-git, vox-cli | already optional via `gix` (vox-git crate uses pure-Rust gix) | **keep CLI fallback**; primary path is gix |
| `docker`, `podman` | vox-container | `bollard` crate (Docker SDK in Rust) | **keep CLI** for daemon control; **add bollard** for in-process ops where useful |
| `node`, `npx`, `pnpm` | vox-cli (web bundles), playwright | — | **keep** — Vite/TanStack ecosystem requires Node |
| `nvcc` | CUDA features | — | **keep** (vendor toolchain) |
| `google-chrome` | vox-browser | `chromiumoxide` already used (CDP) | **keep CLI fallback** for cases without CDP; chromiumoxide is primary |
| `wasmtime` | vox-cli runtime | `wasmtime` crate (already a dep of `vox-wasm-engine`) | **drop CLI** — switch callers to in-process |
| `vox-lsp` | vox-vscode glue | — | **internal**, packaged with `vox-cli` |
| `cloudflared`, `ngrok`, `tailscale`, `wg`, `kubectl`, `runsc` | networking/orchestration paths | — | **keep CLI**, deps-only-when-feature-enabled, surface via doctor |
| `mold`, `sccache`, `zig` | build acceleration | — | **keep optional**, doctor reports + recommends |
| `jj` | optional VCS | — | **keep optional** |
| `xdg-open`, `open`, `cmd`, `pwsh`, `taskkill`, `kill`, `echo` | OS shell | — | **OS-native, keep** |
| `rg` (ripgrep) | misc search | `grep` crate or in-process walking | **dropdown candidate** (Rust-port available; small win) |

**Net new vox-side work:** drop `wasmtime` CLI dependency; consider migrating
`rg` callsites; keep everything else.

### B. CI workflow externals (`.github/workflows/`)

| Tool | Where | Notes |
|---|---|---|
| `actions/checkout@v4` / `@v6` | mixed | **inconsistency** — `bundle-release.yml` and `mobile-e2e-android.yml` lag at v4 |
| `actions/cache@v4` / `@v5` | mixed | same lag in `bundle-release.yml` |
| `actions/setup-node@v4` / `@v6` | mixed | `mobile-e2e-android.yml` on v4 |
| `pnpm/action-setup@v3` / `@v6` | mixed | `mobile-e2e-android.yml` on v3 |
| `actions/upload-artifact@v4` / `@v7` | mixed | `bundle-release.yml` on v4 |
| `actions/setup-java@v4` | `mobile-e2e-android.yml` | Android-only |
| `taiki-e/install-action@v2` | many | installs `cargo-nextest`, `cargo-llvm-cov`, `cargo-hakari`, `cargo-mutants` |
| `cargo install cargo-deb` | `release-installers.yml` | could use `taiki-e/install-action` for cache |
| `cargo install cargo-wix` | `release-installers.yml` | same |
| `node-version: 20` / `22` / `24` | mixed | **inconsistency** — three Node majors in flight |
| `python3 -c "…"` (one-liners) | `ci.yml` | **VoxScript-First violation** — replaceable with `vox ci` subcommands |
| `docker compose -f examples/mesh-compose.yml config` | `ci.yml`, `mesh-compose-config` | wraps Compose; could be a `vox ci compose-validate` |
| `docker build … -f Dockerfile` | `ci.yml`, `docker-vox-image-smoke` | wraps `docker build`; could be `vox ci image-smoke` |
| `pnpm exec playwright install chromium` | `ci.yml`, `mobile-e2e-android.yml` | required, kept |
| `act` | `vox ci pre-push --act` (just landed) | new external; in scope for this plan |

### C. Dockerfiles

| File | Bases / installers |
|---|---|
| `Dockerfile` | `rust:1.95.0-slim-bookworm`, `debian:bookworm-slim`, `apt-get install pkg-config libssl-dev build-essential ca-certificates curl` |
| `Dockerfile.ci-runner` (this branch) | `ubuntu:24.04`, Rust 1.95.0, Node 24, pnpm 9, cargo-nextest/llvm-cov/hakari, plus `apt-get install build-essential pkg-config libssl-dev libsqlite3-dev curl git ca-certificates jq ripgrep python3 python3-pip bash` |

`python3` and `python3-pip` are present in `Dockerfile.ci-runner` only because
`ci.yml` has those one-line `python3` blocks. **Remove the python3 callers
in `ci.yml` → drop python3 from the image.**

### D. Non-`.vox` automation files

| File | Lines | Status under VoxScript-First |
|---|---|---|
| `scripts/vox-dev.sh` | thin launcher | **policy-blessed exception** (chicken-and-egg) |
| `scripts/windows/vox-dev.ps1` | thin launcher | **policy-blessed exception** |
| `python3 -c "…"` blocks in `ci.yml` (≥2 sites) | inline | **violation** — migrate to `vox ci` |

No other `.ps1` / `.sh` / `.py` exist under `scripts/` (verified by `find`).

### E. Required env vars / PATH (developer + CI)

Sourced from `env-vars.md` SSOT and workflow `env:` blocks:

- **Always required:** `cargo` + `rustup` on PATH, `git` on PATH.
- **Required for full CI parity:** `node` + `pnpm` on PATH.
- **Optional, surface via doctor:** `nvcc` (CUDA), `mold` (faster link),
  `sccache` (cache), `docker`, `gh` (CLI for `gh act`), `jj`, browser binary.
- **Internal-only path additions:** `~/.cargo/bin` (rustup), `~/.vox/bin`
  (proposed — see Phase 2 below).
- **Secrets:** all flow through `vox-secrets`; no direct env reads in new
  code (`AGENTS.md §Secret Management`).

### F. Inconsistencies caught during audit

These are concrete, fix-once items independent of the larger plan:

1. **Action version drift.** `bundle-release.yml` and `mobile-e2e-android.yml`
   lag the rest of the repo by 1–3 majors on `actions/checkout`,
   `actions/cache`, `actions/setup-node`, `pnpm/action-setup`,
   `actions/upload-artifact`.
2. **Node version drift.** `mobile-e2e-android.yml` pins Node 20;
   `ci.yml` pins 22; `docs-quality.yml` and `docs-deploy.yml` pin 24. The
   self-hosted fleet image now ships Node 24 (this branch).
3. **`python3` glue lines** in `ci.yml` violate `AGENTS.md
   §VoxScript-First Glue Code`.
4. **`cargo install cargo-deb` / `cargo install cargo-wix`** in
   `release-installers.yml` should switch to `taiki-e/install-action` for
   binary cache (matches the rest of the repo).

## Existing installer surface (what we already have)

Before designing anything new, the relevant existing pieces:

| Surface | Path | What it does today |
|---|---|---|
| `vox doctor` | [`crates/vox-cli/src/commands/diagnostics/doctor/`](../../../crates/vox-cli/src/commands/diagnostics/doctor/) | Per-check audit (`toolchain.rs`, `gpu_hardware.rs`, `secrets.rs`, `model_catalog.rs`, `test_health.rs`, `vox_ignore.rs`, `web_frontend.rs`, `tail.rs`). Has `auto_heal` flag — already auto-installs `pnpm` via `npm install -g pnpm` when missing. |
| `vox-install-policy` | [`crates/vox-install-policy/`](../../../crates/vox-install-policy/) | SSOT constants for install/update surfaces (source path, release targets, GitHub coordinates). |
| `vox-bootstrap` | separate binary in workspace | Offline install / first-run companion. References [`vox-checksum-manifest`](../../../crates/vox-checksum-manifest/) for asset SHA verification. |
| `vox setup` | (planned/partial — referenced from `tail.rs:300`) | Currently just a registration step. Right place to grow into the unified installer. |
| `vox ci install-hooks` | [`crates/vox-cli/src/commands/ci/install_hooks.rs`](../../../crates/vox-cli/src/commands/ci/install_hooks.rs) | One-shot git hook installer (already pure-Rust). |
| `vox shell check` | [`crates/vox-cli/src/commands/runtime/shell/`](../../../crates/vox-cli/src/commands/runtime/shell/) | PowerShell AST + exec-policy check. |

**The gap is the orchestration layer** — there is no `vox doctor --install`
that walks every check and heals everything in one pass.

## Design

### One entry point: `vox doctor --install`

Extend the existing `vox doctor` command, do not introduce a parallel surface.

```text
vox doctor                       # audit only (today's behavior)
vox doctor --install             # audit + auto-heal everything possible
vox doctor --install --offline   # heal from cached artifacts only (uses vox-bootstrap)
vox doctor --install --dry-run   # show what would be installed; do not change state
vox doctor --install --scope=ci  # only the subset CI cares about (faster pre-push)
```

The `--install` flag is a strict superset of the existing `auto_heal` flag and
deprecates it (fix-forward, not back-compat — per `AGENTS.md`).

### Per-check responsibility

Each `Check` returns:

- `pass: bool` — current state (today's surface).
- `installer: Option<Installer>` — **new** — closure or enum variant that
  knows how to install the missing piece on each OS. `None` means "this
  cannot be auto-installed" (e.g. NVIDIA driver, Docker daemon).

`Installer` enum sketch (lives in `vox-install-policy`):

```rust
pub enum Installer {
    /// `cargo install <pkg> --locked` (Rust-side tools).
    Cargo { package: &'static str, locked: bool },
    /// Download + checksum-verify + place under ~/.vox/bin/.
    /// Cross-platform; no OS package manager required.
    SignedDownload {
        name: &'static str,
        urls: TargetMap<&'static str>,    // per-OS download URL
        sha256: TargetMap<&'static str>,  // per-OS expected SHA-256
    },
    /// Recoverable advice — print install instructions, do not execute.
    Manual { url: &'static str, hint: &'static str },
    /// Compose: try first; fall back to the next.
    Fallback(&'static [Installer]),
}
```

**No shelling out to `winget` / `scoop` / `choco` / `brew` / `apt` from
vox-side code.** Those introduce per-OS branching, varying privilege models
(`choco` needs Admin, `brew` doesn't, `apt` needs sudo), and silent updates
that bypass our SHA-pinned audit trail. The `SignedDownload` path is the
universal mechanism: download the official release, verify SHA-256 against a
pin in `vox-install-policy`, drop into `~/.vox/bin/`, prepend that to PATH
in our shell init.

### Tools to bring under `vox doctor --install`

| Tool | Installer variant | Reason it's in scope |
|---|---|---|
| `pnpm` | `Cargo { … }` is wrong — use `SignedDownload` (pnpm ships a self-contained binary) | already `auto_heal`-ed via npm; switch to direct download to drop the npm prereq |
| `cargo-nextest` | `Cargo { package: "cargo-nextest", locked: true }` | mirrors `taiki-e/install-action` outcome |
| `cargo-llvm-cov` | `Cargo` | same |
| `cargo-hakari` | `Cargo` | same |
| `cargo-mutants` | `Cargo` (optional, nightly lane only) | same |
| `cargo-deb` | `Cargo` | replaces `cargo install cargo-deb` in `release-installers.yml` |
| `cargo-wix` | `Cargo` | replaces `cargo install cargo-wix` |
| `mdbook` | `SignedDownload` (official release) | docs build |
| `act` | `SignedDownload` (binary releases) | new — per dependency-consolidation goal |
| `rustfmt`, `clippy`, `llvm-tools-preview` | rustup component install | `Installer::RustupComponent { … }` variant |
| `mold` | `Manual` | system-package preferable; doctor surfaces a recommendation |
| `sccache` | `SignedDownload` | optional speedup |
| `nvcc` (CUDA) | `Manual` | vendor-installed, never auto-install |
| `node` | `Manual` (with `SignedDownload` fallback for portable Node) | required; surface install URL |
| Docker daemon | `Manual` | system service, never auto-install |
| Browser (Chromium) | `Manual` | already gated behind `pnpm exec playwright install` |

### Cross-platform PATH setup

`~/.vox/bin/` is added to PATH by:

- **Bash/zsh:** entry in `~/.bashrc` / `~/.zshrc` (idempotent, marked with
  `# >>> vox >>>` / `# <<< vox <<<` block).
- **PowerShell:** entry in the user's `$PROFILE` (same idempotency markers).
- **Windows cmd:** persistent user PATH via `setx` once at install time
  (one of the two unavoidable Windows-specific code paths; the other is
  `pnpm.cmd` vs `pnpm` — already handled in `toolchain.rs`).

`vox doctor --install` writes these on first run. `--dry-run` prints them.

### What this replaces in docs

After landing, every "install X" instruction in `docs/src/contributors/`
collapses to:

> Run `vox doctor --install`. Re-run with `--scope=ci` if you only need the
> CI mirror.

The Windows install table I just added to
[`local-ci-pre-push.md`](../contributors/local-ci-pre-push.md#installing-act)
shrinks to a single sentence pointing at `vox doctor --install`. Same for
the dispersed pnpm/Node/cargo-nextest install hints.

## Phased migration plan

Each phase is mergeable on its own. Each ends with **CI green and no doc
drift**; that's the merge gate. Phases are ordered by safety, not by impact.

### Phase 0 — Inconsistency sweep (small, mechanical)

**Scope:** the items in §F above that have no design content.

- Bump `bundle-release.yml` action versions to match the rest of the repo
  (`@v6` / `@v5` / `@v6` / `@v7`).
- Bump `mobile-e2e-android.yml` action versions and `pnpm/action-setup` to
  `@v6` — node version stays at 20 only if dictated by Android tooling
  (otherwise bump to 22, matching `ci.yml`).
- Replace `python3 -c "…"` blocks in `ci.yml` with `vox ci toestub-budget`
  (new tiny subcommand, ≤30 LoC) and `vox ci json-validate <file>` (already
  pure-Rust elsewhere — check if subcommand exists, add if not).
- Drop `python3` and `python3-pip` from `Dockerfile.ci-runner` once the
  above lands.

**Exit criteria:** `cargo run -p vox-cli -- ci command-compliance` clean;
`docker build -f Dockerfile.ci-runner` succeeds; full CI green.

### Phase 1 — `Installer` enum + `vox-install-policy` extension

**Scope:** pure data + types. No behavior change yet.

- Add `Installer` enum to `vox-install-policy` per the design above.
- Add a registry: `Vec<(Tool, Installer)>` listing every entry from the
  table in §Tools-to-bring-under above.
- Pin SHA-256 hashes for the initial `SignedDownload` set (`pnpm`, `act`,
  `mdbook`, `sccache`).
- Unit tests: deserialize each entry, assert URL+SHA pinning shape.

**Exit criteria:** `cargo test -p vox-install-policy` green; no callers yet.

### Phase 2 — `vox doctor --install` orchestration

**Scope:** wire the registry into `vox doctor`.

- Extend `Check` struct in `crates/vox-cli/src/commands/diagnostics/doctor/common.rs`
  with `installer: Option<Installer>`.
- Each `checks_standard/*.rs` file populates `installer` for its check.
- New module `doctor/installer.rs` contains the executor:
  download → verify SHA → place under `~/.vox/bin/` → mark exec
  permissions → optionally update shell init.
- New CLI flags on `vox doctor`: `--install`, `--offline`, `--dry-run`,
  `--scope=<all|ci|dev>`. Deprecate the old per-check `auto_heal`.
- Implement PATH-init writers for bash/zsh/PowerShell with idempotency
  markers; cmd uses `setx` once.

**Exit criteria:** `vox doctor --install --dry-run` lists every action;
`vox doctor --install` on a clean Linux/macOS/Windows VM produces a working
CI environment without invoking any OS package manager.

### Phase 3 — Replace external installers in CI workflows

**Scope:** rip out `taiki-e/install-action` and `cargo install …` from
workflows; replace with one `vox doctor --install --scope=ci` step.

- New step at top of every workflow that needs Rust tooling:

  ```yaml
  - name: Vox doctor — install CI toolchain
    run: cargo run -p vox-cli --quiet -- doctor --install --scope=ci
  ```

- Remove `taiki-e/install-action@v2` blocks for cargo-nextest /
  cargo-llvm-cov / cargo-hakari / cargo-mutants.
- Remove `cargo install cargo-deb` / `cargo install cargo-wix` from
  `release-installers.yml`.

**Exit criteria:** workflows still pass; total YAML LoC drops; only Docker /
checkout / cache / setup-node / setup-java actions remain as third-party
calls.

### Phase 4 — Docs collapse

**Scope:** single source of install truth.

- Rewrite `docs/src/contributors/local-ci-pre-push.md` "Installing `act`"
  table → one sentence pointing at `vox doctor --install`.
- Same for any pnpm / Node / mdbook install hints elsewhere.
- Update `docs/src/contributors/contributor-hub.md` first-run flow to
  start with `vox doctor --install`.
- `vox doctor --install --help` becomes the canonical reference (and
  shows up in the auto-generated `cli-command-surface.generated.md`).

**Exit criteria:** `vox ci check-docs-ssot` clean; no doc references
duplicated install instructions for tools the registry handles.

### Phase 5 — Drop `wasmtime` and `rg` CLI dependencies (stretch)

**Scope:** in-process replacements.

- Switch all `Command::new("wasmtime")` callsites to use the `wasmtime`
  crate already in `vox-wasm-engine`.
- Audit `Command::new("rg")` callsites; for hot paths, switch to the
  `grep` crate; for cold/script-shaped paths, leave the CLI fallback.

**Exit criteria:** `Command::new` audit shows wasmtime gone; `rg` reduced
to ≤ N callsites with rationale.

### Phase 6 (optional) — `vox-bootstrap` self-update

**Scope:** make `vox-bootstrap` capable of updating itself + the registry
SHA pins from a signed manifest.

- Manifest hosted alongside release artifacts.
- `vox doctor --install --self-update` refreshes pins.
- Rollback on signature failure.

**Exit criteria:** offline install path verified end-to-end on a fresh VM.

## Merge-back-to-main exit criteria

A single PR (or stack) with this work is mergeable when:

1. **All phases through 4 are landed** — Phase 5/6 are stretch and can land
   later.
2. **`vox ci pre-push` clean** with both `--act` and the new doctor flow.
3. **CI green** on `main` and on at least one Windows + macOS smoke push.
4. **No unguarded `Command::new` for tools in the registry** — replaced or
   wrapped by the installer.
5. **`vox ci command-compliance` clean** — no doc drift.
6. **`docs/src/architecture/where-things-live.md` updated** with rows for
   `vox-install-policy` registry, `Installer` enum location, and any new
   `doctor/` modules.
7. **No new `.ps1` / `.sh` / `.py`** beyond the two policy-blessed
   launchers.
8. **`Dockerfile.ci-runner` no longer needs `python3`** (Phase 0 outcome).

## Out of scope

- Replacing the Vite / Node / pnpm chain. Web tooling stays as-is.
- Replacing CUDA / nvcc. Vendor toolchain.
- A new package manager. We are an installer of release artifacts, not a
  package manager.
- Container image rebuild policy. The GHCR `vox-ci-runner` work in this
  branch covers it.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| SHA pins go stale fast | `Phase 6` self-update path; in the interim, automate a `cargo run -p vox-install-policy --bin verify-pins` weekly via existing `mutation-nightly.yml`-style schedule. |
| `~/.vox/bin/` collides with user-installed binaries | PATH order documented; `vox doctor --install --dry-run` explicitly shows shadowing. |
| Windows PATH writes break user shells | `setx` is the documented Windows-blessed mechanism; `--dry-run` surfaces the change first; rollback marker `# >>> vox >>>` allows clean removal. |
| Auto-install of cargo plugins is slow | Cache `~/.cargo/bin` in CI (already done); first install on dev machines is one-time. |
| `act` binary auto-install gets blocked by corp proxy | `Manual` fallback already in the installer enum; doctor prints the URL. |

## Open questions for review

1. Do we want `vox setup` as the user-facing alias for `vox doctor --install`,
   or keep it as a separate registration concept? `tail.rs:300` already
   refers to `vox setup` — clarifying its scope before Phase 2 keeps us
   from shipping two doors to the same room.
2. Where do `Installer` SHA pins live — in `vox-install-policy`'s source
   (compile-time) or in a YAML manifest in `contracts/install/`? The
   contracts/ approach matches our other SSOT files and lets `vox-bootstrap`
   reload without rebuild.
3. Should Phase 0 (the inconsistency sweep) ship first as its own PR? It's
   self-contained and lowers Phase 3's diff size.
4. Is there appetite for replacing the `gh act` extension fallback in
   `pre_push.rs` with our own bundled `act` (downloaded by the registry)?
   That would make `--act` work the moment doctor finishes.

## See also

- [CI alternatives and local Docker mirroring](../ci/alternatives-and-local-mirroring.md)
- [Local CI parity (pre-push)](../contributors/local-ci-pre-push.md)
- [Where things live](where-things-live.md)
- [`AGENTS.md` §VoxScript-First Glue Code](../../../AGENTS.md)
- [`vox-install-policy` crate](../../../crates/vox-install-policy/)
- [`vox doctor` source](../../../crates/vox-cli/src/commands/diagnostics/doctor/)
