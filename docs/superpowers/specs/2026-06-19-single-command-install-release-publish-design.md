---
title: "Single-Command Install, Release & crates.io Publishing — Program Design"
description: "Brainstorm-stage design: one-command tiered install+enable, gated end-to-end release/nightly automation, and a crates.io-ready publish path, all driven by one distribution SSOT."
category: "architecture"
status: "current"
---

# Single-Command Install, Release & Publishing — Program Design

> **Stage:** Brainstorm output (Session 1 of 4: brainstorm → plan → critique → Gemini-Flash handoff).
> **Date:** 2026-06-19 · **Repo:** `vox-foundation/vox` @ workspace `0.6.0`.
> **Author:** graphify-informed brainstorm (no code changed).
> **Successor:** implementation plan(s) under `docs/superpowers/plans/`, executed by Gemini 3.5 Flash in the Antigravity IDE.

## Purpose

Get Vox install + enable down to **one command on every platform**, make
release/nightly/publish **automated end-to-end**, and make the path to a public
`cargo install` distribution **provably ready and CI-gated** — without going
public until a switch is flipped. Centralize the interrelation of all
distribution machinery, surface missing dependencies, and make drift impossible.

## Locked decisions (from brainstorm)

| # | Decision | Choice |
|---|----------|--------|
| Q1 | Single command's job | **Install + enable** — clean machine → working, immediately-productive Vox. |
| Q2 | `voxup` shape | **Keep separate (rustup model)**, first-class surfaced command. `vox` never self-bootstraps. |
| Q3 | crates.io goal | **Real `cargo install` distribution path** as the target; actual public flip gated behind a CI readiness check. |
| Q4 | Dependency handling | **Tiered profiles** `minimal`/`default`/`full`; each declares its dep closure; `vox doctor` is the per-tier dependency surfacer/enabler. |
| Q5 | Enforcement vs automation | **Both, full send** — every capability lands *with* its enforcing CI gate. |

## Architecture — the spine

**One declarative SSOT** drives everything:
`contracts/distribution/profiles.v1.yaml`. It is the single source of truth for:

1. **Tiers** — `minimal` (compiler/lang CLI only), `default` (CLI + GUI),
   `full` (+ ML + `agy` + curated plugin bundle). Each tier declares:
   - **build deps** (toolchain, Node/pnpm, Tauri system libs) — only relevant on the from-source path,
   - **runtime-optional deps** (`agy` binary, model weights/keys, plugins) — detected/enabled, never required.
2. **Publishability** — reconciles `crates/_public.toml` + `layers.toml`
   `publishable = true` into one authoritative publish set with leaf-first order.
3. **Binary/artifact set** — which binaries each release + nightly produce
   (`vox`, `vox-ml-cli`, `voxup`; note `vox-bootstrap`/`vox-schola` are retired).

Everything reads from this file: `voxup`, `vox doctor`, all release/nightly
workflows, and every gate. This eliminates the split-brain/declared-but-unwired
bug class the prior audit (`docs/plans/INSTALL-RELEASE-AUDIT.md`) repeatedly found.

### `agy` / Antigravity containment (explicit)

`agy` is an **optional, shelled-out** runtime dependency (the Go `agy` binary),
concentrated in `vox-orchestrator-mcp` (`agy_exec/_doctor/_gates/_worktree/_pipeline`).
It is **not** a build dependency of anything on the install/publish path.

**Hard rules (each becomes a gate):**
- `agy` appears only in the `full` tier's runtime-optional set; surfaced/enabled via `agy_doctor`.
- No crate on the install/publish/build path may take a hard dependency on `agy`.
- `vox-orchestrator-mcp` is marked **non-publishable**; agy implications never reach crates.io or the install path.

## Current state (where we are — brainstorm audit)

**Already built (audit partially stale in our favor):**
- `voxup` *now downloads* from GitHub Releases, verifies SHA-256, extracts,
  links, self-installs (`crates/voxup/src/install.rs`) — the old "🔴 can't bootstrap" finding is **fixed**. Has `install`/`update`/`proxy`, plus `channel.rs`/`download.rs`/`proxy.rs`.
- Bootstrap one-liners: `scripts/install.{sh,ps1}` behind `https://voxlang.org/install`.
- Four release workflows (`release-binaries`, `release-gui` w/ macOS notarize + Windows Azure signing, `release-installers` w/ wix/deb/Homebrew, `bundle-release`) + `version-tag-guard.yml`.
- Publishability primitives: `layers.toml` `publishable=true`; `crates/_public.toml` publish set.

**Missing / drifting:**
- No nightly pipeline (design exists, unbuilt: `2026-06-17-nightly-release-pipeline-design.md`).
- GUI auto-update absent (no `tauri-plugin-updater`).
- Plugin install unverified (no checksum/signature — RCE surface).
- crates.io publish declared but unwired (no `cargo publish` automation, no leaf-first order, hakari workspace-hack likely blocks publish).
- No update notification (CLI footer / GUI toast).
- Minimal-vs-full archive seam (`vox-langtool`) prototyped but uncommitted in a stray branch.
- No per-profile dependency-closure declaration (the SSOT above does not yet exist).

## Track decomposition

Each track is an independent plan→critique→handoff cycle. **Every task lands
with its enforcing gate** (Q5).

- **Track 0 — Distribution SSOT.** Author `contracts/distribution/profiles.v1.yaml`
  + a parity gate asserting it matches reality (tiers, publish set, binary set).
  Prerequisite for all other tracks. Resolve Rust-version skew + wire version↔tag as gates here.
- **Track A — Tiered install + enable.** `voxup install <minimal|default|full>` reads the SSOT;
  PATH automation; `vox doctor` becomes the per-tier dependency surfacer with `--fix` opt-in
  (auto-provision is opt-in, never default). Commit the `vox-langtool` minimal-binary seam.
  Gate: per-tier dep-closure parity; clean-machine E2E smoke per OS.
- **Track B — Release + nightly automation.** Build the nightly pipeline (green-main gate before
  publish), unify the release matrix on the SSOT binary set, add update notification (CLI footer +
  GUI `tauri-plugin-updater`/toast). Gate: nightly-green-before-release; release artifact ↔ SSOT parity.
- **Track C — crates.io publish readiness.** Reconcile `_public.toml`/`layers.toml` into the SSOT;
  per-crate `cargo publish --dry-run` green with full metadata; leaf-first publish order;
  hakari/workspace-hack handled; `publish = true` flip gated behind a readiness check (stays off).
  Gate: publish-set dry-run parity; metadata completeness; no-cycles/leaf-order check.
- **Track D — Supply-chain trust.** Plugin checksum/signature required at install + load (close RCE);
  Linux signing + SBOM + provenance-on-release. Gate: unsigned-plugin-load fails closed; release SBOM present.

**Cross-platform parity** (required Win/macOS/Linux smoke lane decoupled from the self-hosted fleet)
is a constraint threaded through Tracks A and B, not a separate track.

## Out of scope (YAGNI for this program)

- Actually pushing to crates.io publicly (readiness only; flip deferred).
- Custom update server (GitHub Releases remains the SSOT).
- New ML/model distribution beyond the existing `vox-ml-cli` add-on.

## Constraints for the Gemini-Flash handoff

- VoxScript-only automation (no new `.ps1`/`.sh`/`.py`); honor `AGENTS.md` (`where-things-live.md`).
- `cargo fmt --all` is banned — use `vox run scripts/fmt.vox` / `cargo fmt -p <crate>`.
- Each plan carries a Flash execution preamble + per-task splits + `[PARALLEL-SAFE]`/`[SEQUENTIAL]` tags.
- Every capability ships with its gate, or it does not land.
