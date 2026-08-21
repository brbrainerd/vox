# Vox Distribution System — Design

> **Status:** Approved for planning · **Date:** 2026-08-20
> **Supersedes the open items of:** [`docs/plans/INSTALL-RELEASE-AUDIT.md`](../../plans/INSTALL-RELEASE-AUDIT.md) (2026-06-07)

## Goal

Ship Vox as a **double-clickable, offline, dependency-selectable installer** for Windows, macOS, and
Linux, built at true release optimization, verified end-to-end against the artifact users actually
download, versioned and changelogged from a single source, distributed through GitHub Releases on
both a stable and a nightly channel, and self-updating thereafter.

## Decisions (locked during brainstorming)

| # | Decision | Rationale |
|---|----------|-----------|
| D1 | **Native declarative installer UI**, not a custom installer app | WiX `WixUI_FeatureTree` (Windows) and `productbuild --distribution` choices (macOS) provide a checkbox tree with select-all / clear-all for free, as declarative XML. Zero new application code, fully native look. Linux has no native picker and gets metapackages plus a first-run TUI. |
| D2 | **Bundle dependencies offline** | Install must succeed with no network. |
| D3 | **Offline payload excludes model weights** | GitHub Releases caps a single asset at 2 GiB. The default local model (`qwen/qwen3-coder-next-32b`, `contracts/orchestration/model-catalog.bootstrap.v1.json`) is ~18–20 GB quantized. Code dependencies alone total ~0.7–1.2 GB and fit. Weights are pulled post-install via Ollama; `vox doctor` reports status. Docker is likewise excluded — Docker Desktop is not freely redistributable. |

## Audited baseline — what already exists

Do not rebuild these.

- **Six release workflows**, all triggered on `v*` tags: `release-binaries.yml` (4-target matrix,
  smoke tests, consolidated `checksums.txt`, SPDX SBOM), `release-installers.yml` (MSI via
  `cargo-wix`, `.deb` via `cargo-deb`, Homebrew tap push, `install.sh`/`install.ps1` E2E),
  `release-gui.yml` (Tauri bundles, macOS notarization via `APPLE_ID`, Windows Azure Trusted
  Signing), `bundle-release.yml` (plugin bundles on `release: published`),
  `distribution-parity.yml` (PR gate), `version-tag-guard.yml` (tag/version gate).
- **`voxup`** (~1.4 kLoC): `install` / `update` / `proxy` / `channel` / `download`, with real
  SHA-256 verification at `crates/voxup/src/download.rs:43`.
- **A dependency SSOT**: `contracts/distribution/profiles.v1.yaml` declares tiers → binaries,
  `build_deps`, `runtime_optional`. Typed reader in `crates/voxup/src/profiles.rs`; parity-gated by
  `crates/voxup/tests/distribution_parity.rs`; consumed by `vox doctor` via
  `crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/tier_deps.rs`.
- **A plugin/component catalog**: `crates/vox-plugin-catalog/catalog.toml` declares `[[plugin]]`,
  `[[component]]` (on-demand first-party executables, e.g. `gui`), and `[[bundle]]` (nine plugin sets
  with `extends` inheritance).
- **`cliff.toml`** at the repo root, currently unused by any workflow.
- **`[profile.dist]`** in `Cargo.toml`: `lto = "fat"`, `codegen-units = 1`, `strip = "symbols"`,
  `panic = "abort"`.

## Findings

### F1 — `[profile.dist]` is dead code (severity: critical)

`Cargo.toml` defines `[profile.dist]`, but
`crates/vox-cli/src/commands/ci/release_build.rs` passes `--release` and reads the built binary from
`target/<triple>/release/`. **Every binary ever shipped is thin-LTO, unstripped of symbols, and
unwinding.** This is the single highest-value fix in this document.

### F2 — Nothing is tested at ship optimization (severity: critical)

Release smoke tests are `--help` / `--version` only. Worse, `panic = "abort"` means the normal
`cargo test` harness **cannot run under `[profile.dist]` at all** — the harness relies on unwinding
to catch panics, so `#[should_panic]` tests and panic-recovery paths break.

Resolution, two complementary lanes:

1. **`[profile.dist-test]`** — `inherits = "dist"` with `panic = "unwind"`. Runs the full suite
   under fat LTO and `codegen-units = 1`, catching miscompilation and cross-crate inlining bugs
   that `--release` does not.
2. **Black-box E2E against the real `dist` binary** — subprocess-driven, needs no test harness in
   the binary, and therefore does exercise `panic = "abort"` exactly as users get it.

Neither lane alone is sufficient; both are required.

### F3 — Bundle matrix drift ships a phantom and omits two real bundles (severity: high)

`.github/workflows/bundle-release.yml:36` lists `vox-cloud-only`, which does not exist in
`catalog.toml` — it survives only in superseded plan documents. The real bundles `vox-ml-metal` and
`vox-mobile` are never built. Every release burns two matrix jobs on a bundle that cannot resolve,
and no Apple-Silicon ML bundle is published. No gate catches this because `distribution_parity.rs`
validates `profiles.v1.yaml` only and never reads `catalog.toml` or the workflow matrix.

### F4 — macOS has no double-click installer (severity: high)

Windows gets an MSI and Linux a `.deb`; macOS ships a bare `tar.gz`. The CLI tarballs are also
unnotarized, so Gatekeeper quarantines them. Only the Tauri GUI bundle is notarized.

### F5 — No nightly channel (severity: high)

Twenty workflows carry a `schedule:` trigger; none produces an installable artifact.
`crates/voxup/src/channel.rs` hardcodes the GitHub `/releases/latest` endpoint and has no channel
concept despite its name.

### F6 — Three changelog sources (severity: medium)

`cliff.toml` is orphaned; release bodies come from `generate_release_notes: true`; `CHANGELOG.md` is
hand-maintained. All three drift independently.

### F7 — GUI has no auto-update (severity: high)

`crates/vox-gui/tauri.conf.json` has no `updater` block, no plugin, and no public key. Only the CLI
self-updates, via `voxup`.

### F8 — Target matrix gaps (severity: medium)

`aarch64-unknown-linux-gnu`, `x86_64-unknown-linux-musl`, and `aarch64-pc-windows-msvc` are absent.

### F9 — No build provenance (severity: medium)

Checksums and an SPDX SBOM exist; nothing is signed or attested. `pm-provenance-verify.yml` covers
the package registry, not release binaries.

### F10 — Managed installs and `voxup` will fight (severity: high, latent)

`voxup update` swaps the `vox` binary in place. Once Vox is installed from an MSI or `.pkg`, an
in-place swap desynchronizes the OS package database from disk: the installer's repair, modify, and
uninstall paths then operate on stale file records.

## Architecture

### A1 — Three orthogonal axes, one generator

The three SSOT files are **not** a split-brain; they describe different things. Keep all three,
change none of their formats, and add one generator that projects them into installer manifests.

| File | Axis | Change |
|---|---|---|
| `contracts/distribution/profiles.v1.yaml` | Tiers → binaries + toolchain deps | **Extend**: add an `offline_payload` block per dep (upstream URL, sha256, licence SPDX id, compressed size, per-OS applicability). |
| `crates/vox-plugin-catalog/catalog.toml` | Plugin sets (`[[bundle]]`) and on-demand executables (`[[component]]`) | **Unchanged.** |
| `contracts/toolchain/workspace-toolchain.v1.yaml` | Pinned toolchain versions | **Unchanged.** |

New command **`vox ci gen-installer-manifests`** reads the first two and emits, deterministically:

- `packaging/windows/features.generated.wxs` — a WiX `<Feature>` tree, one node per tier, bundle,
  and component, with `AllowAdvertise="no"` and a `WixUI_FeatureTree` reference.
- `packaging/macos/distribution.generated.xml` — a `productbuild` `<choices-outline>` mirroring the
  same tree.
- `packaging/linux/control.generated` — `Depends` / `Recommends` / `Suggests` fields for the `.deb`
  metapackages.

These are generated artifacts: marked `linguist-generated` in `.gitattributes`, regenerated by the
`ssot-autoregen` job, and drift-checked by `vox ci ssot-drift`. **The checkbox tree is never
hand-maintained.**

### A2 — Parity gate closes F3 permanently

`crates/voxup/tests/distribution_parity.rs` gains assertions that the `bundle-release.yml` matrix is
exactly the set of `[[bundle]]` ids in `catalog.toml`, and that every `offline_payload` entry carries
a non-empty SPDX licence id and sha256.

### A3 — Installers

**Windows.** A WiX v4 **Burn** bootstrapper produces a single `VoxSetup-<version>-x86_64-windows.exe`
chaining the Vox MSI plus embedded payloads (Rust toolchain, Node/pnpm, WebView2 fixed-version
runtime). Burn is required rather than a plain MSI because a plain MSI can only toggle features
inside itself; installing external prerequisites needs a chain. `WixUI_FeatureTree` supplies the
checkbox tree, including select-all and clear-all, natively.

**macOS.** `productbuild --distribution` builds a `.pkg` whose `<choices-outline>` renders as a
native checkbox list, wrapped in a `.dmg`, signed and notarized through the existing `APPLE_ID`
secret path already proven in `release-gui.yml`.

**Linux.** `.deb` metapackages (`vox`, `vox-ml`, `vox-full`) express selection through
`Depends` / `Recommends`, plus an AppImage for distro-agnostic double-click. No native picker exists;
selection after install is handled by the first-run TUI and `vox doctor`.

**Defaults.** The `default` tier is preselected on every platform. Select-all and clear-all are
provided by the native dialogs on Windows and macOS.

### A4 — Versioning and channels

Filenames are stamped by the build, not by hand:

- Stable: `VoxSetup-0.7.0-x86_64-windows.exe`, `VoxSetup-0.7.0-aarch64-macos.dmg`, `vox_0.7.0_amd64.deb`
- Nightly: `VoxSetup-0.7.0-nightly.20260820.a1b2c3d-x86_64-windows.exe`

The nightly workflow runs on cron, creates no git tag, and force-updates a single rolling `nightly`
pre-release. `voxup` gains a `--channel {stable,nightly}` flag; `channel.rs` resolves `stable` to
`/releases/latest` (today's behaviour) and `nightly` to `/releases/tags/nightly`.

### A5 — Changelog SSOT

`git-cliff` becomes the only generator. It produces `CHANGELOG.md`, auto-committed by the existing
`ssot-autoregen` job, **and** the GitHub release body via `--body-path`, replacing
`generate_release_notes: true`. Three sources collapse to one.

### A6 — Auto-update

The GUI gains `tauri-plugin-updater` with a `latest.json` endpoint published by `tauri-action`.

For F10, `voxup` learns to detect a **managed install** — a marker file `~/.vox/.managed-by` written
by each installer, naming the manager (`msi`, `pkg`, `deb`, `rpm`) — and, when present, refuses the
in-place swap and instead downloads the new installer and hands off to it. Unmanaged installs keep
today's in-place behaviour unchanged.

### A7 — End-to-end verification

A clean-room matrix (container for Linux, fresh runner for Windows/macOS) performs, per platform and
per channel: install from the **real published artifact**, run `vox doctor --strict`, compile and run
a golden `.vox` program, run `voxup update` and assert a no-op, uninstall, and assert clean removal
with no orphaned files. Extends the existing `setup-e2e.yml` clean-room pattern.

### A8 — Supply chain

Add `actions/attest-build-provenance` for every released artifact, and extend the offline payload
manifest with per-dependency SPDX licence ids so redistribution is auditable.

## Out of scope

- Bundling model weights or Docker (see D3).
- Replacing the release system with `cargo-dist`. It solves much of this, but this repo has already
  hand-rolled `voxup` plus six workflows plus a signing story; a migration would discard working,
  signed, notarized infrastructure to gain features this design adds incrementally.
- Mobile (`vox-mobile` bundle is `status = "alpha"`, planned v0.8).

## External blockers

Cannot be designed around; the work stops at these lines until resolved by the maintainer:

1. **A Windows code-signing certificate** for the Burn bootstrapper. Azure Trusted Signing is already
   wired for the GUI in `release-gui.yml:147` and should be reused.
2. **A Tauri updater keypair** (`TAURI_SIGNING_PRIVATE_KEY` + public key in `tauri.conf.json`).
3. **A Linux signing key** for `.deb` / AppImage, or an explicit decision to ship Linux unsigned.

## Phasing

| Phase | Content | Blocked? |
|---|---|---|
| **1** | F1, F2, F3 — real optimization, `dist-test` profile, black-box E2E, parity gate, SSOT extension + manifest generator | No |
| **2** | A3 installers (Windows Burn, macOS pkg/dmg, Linux deb/AppImage) | Windows + Linux signing |
| **3** | A4 nightly channel, A5 changelog SSOT, F8 matrix expansion, A8 provenance | No |
| **4** | A6 auto-update (GUI updater, managed-install handoff), A7 full E2E matrix | Tauri keypair |

Phase 1 is self-contained, unblocked, and delivers the highest-severity fixes. It is planned first.
