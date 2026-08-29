---
title: "Distribution Completeness — Design"
description: "Close the four remaining distribution gaps: the Tauri desktop GUI bundle, GPU plugin release assets, a proven .deb install, and a Homebrew formula audit harness."
category: "Architecture SSOTs"
status: "draft"
---

# Distribution Completeness — Design

## Context

`2026-08-23-release-pipeline-verified-design.md` got the CLI distribution
working: the first GitHub Release in this repo's history now carries real,
downloadable `vox`, `vox-ml-cli`, and `voxup` assets for Linux, Windows,
and macOS (x86_64 + aarch64), plus a Windows MSI. That spec's audit closed
by naming four gaps it did not fix. This design closes them.

All four are folded into one spec by explicit user decision, over a
recommendation to split them. They are kept as independently
implementable, independently verifiable phases so that a failure in one
does not block the others.

## Phase 0 — Versioning and tag strategy

Every phase above needs a tag push to verify, and the current tagging
rules make that needlessly expensive. `version-tag-guard.yml` strips only
a leading `v` and then string-compares against `[workspace.package]
version`, so **`v0.6.0` is the only tag that can ever pass**. Every
prerelease form fails identically — `v0.6.0-rc.1` is rejected exactly as
hard as `v0.0.0-test`. Two consequences, one of them previously
unrecognised:

1. Verification runs had to use a tag that fails the guard, producing a
   permanently red check that had to be explained away on every run.
2. **Nightly releases are structurally impossible, not merely
   unimplemented.** No `v*` tag carrying a date or build suffix can pass
   the guard, so no nightly tag can exist. `distribution-ssot.md`'s
   "release + nightly" claim could not have been true.

**Change:** compare only the **core** version — strip any `-prerelease`
and `+build` metadata before matching. This preserves the guard's real
purpose (a `v0.7.0` tag against a `0.6.0` Cargo.toml still fails) while
admitting prereleases.

**Tag scheme.** `VOX_BUILD_NUMBER` is already `git rev-list --count HEAD`
(`crates/vox-build-meta`, currently ~4735) — monotonic, unique per commit,
auto-incrementing on merge. It gives collision-free tags for free:

| Tag | Guard | Publish behavior |
|---|---|---|
| `v0.6.0` | passes | real release, becomes `latest` |
| `v0.6.0-rc.<buildnum>` | passes after the fix | prerelease, never `latest` |
| `v0.6.0-nightly.<buildnum>` | passes after the fix | prerelease, never `latest` |

This composes exactly with the `prerelease`/`make_latest` rule already
shipped in both publish jobs (hyphen ⇒ prerelease; no hyphen ⇒ latest) —
no new publish logic is required.

**Decision:** verify Phases 1–3 on `v0.6.0-rc.<buildnum>` tags, and cut
the real `v0.6.0` only once the GUI bundle, the `.deb`, and the GPU assets
are all proven. The GUI bundler has never once produced an artifact, so
the probability of a clean first run is low; burning the real `0.6.0` tag
on it would force either a force-moved public tag or a version bump.

Wiring an actual scheduled nightly trigger is **not** in scope here — but
after this change it becomes possible, which it was not before.

## Phase 1 — Desktop GUI (`axis`) bundling

`release-gui.yml` has failed **6 of 6 runs**, always with
`##[error]No artifacts were found.` on every platform. Two independent
root causes, both confirmed by reading the config, not inferred from logs:

1. **The bundler is switched off.** `crates/vox-gui/tauri.conf.json`'s
   `bundle` block contains only `icon` and `externalBin` — no
   `"active": true` and no `"targets"`. In Tauri v2 `bundle.active`
   **defaults to `false`**, so `tauri build` produces the bare executable
   and zero installers. `tauri-action` then finds nothing to upload and
   emits exactly the observed error. This alone accounts for all six
   failures on all three platforms.
2. **The Windows signing step points at a path that does not exist.**
   `release-gui.yml`'s `Sign Windows Installer` step sets
   `files-folder: ./crates/vox-gui/src-tauri/target/release/bundle/msi`.
   There is no `src-tauri/` directory in this repo (the Tauri config lives
   at `crates/vox-gui/tauri.conf.json`), and `.cargo/config.toml` pins
   `CARGO_TARGET_DIR` to the workspace root — so that folder can never
   exist. This is latent: it cannot fire today because the build dies
   before signing, and would fail immediately once Fix 1 lands.

**Changes.** Add `"active": true` plus an explicit per-platform `targets`
list to the `bundle` block. Correct the signing `files-folder` to the real
bundle path under the workspace target dir, and guard the signing step so
absent Azure secrets skip signing instead of failing the release — an
unsigned installer is a worse-but-shippable outcome; a hard failure is not.

## Phase 2 — Prove the `.deb` on Ubuntu/WSL2

The previous pass found `build-linux-deb` reporting success while the
`.deb` never reached the release: the publish step's `files` glob
(`installer-artifacts/*/*`) reached only two directory levels, but
`cargo-deb`'s output preserves a deeper nested path inside the uploaded
artifact, and `fail_on_unmatched_files: false` swallowed the miss. Fixed
in `f52f3acef` with a recursive glob — **unverified**, because the
existing `v0.0.0-test` tag points at `e1b51cbac`, which predates the fix.
The release today still carries no `.deb`.

**Verification.** Re-tag to pick up `f52f3acef`, then in WSL2
(Ubuntu 24.04.1 LTS, dpkg 1.22.6 — both confirmed present): download the
`.deb` from the release, `sudo dpkg -i`, and run the installed `vox
--version`. Success is a semver string from a binary installed by dpkg —
not a green checkmark, and not the mere presence of the asset.

## Phase 3 — GPU plugin distribution

Both GPU plugins are undeliverable today, for different reasons:

- `mens-candle-cuda`'s `default-source` is
  `github:vox-foundation/vox-plugin-mens-candle-cuda`. **That repository
  returns HTTP 404.** The install can never resolve.
- `mens-candle-metal`'s source is `local:crates/...`, which the
  distribution-security-floor work deliberately gates behind
  `VOX_LOCAL_PLUGIN_FALLBACK` because a CWD-relative source lets any
  directory supply native code that gets `dlopen`'d. It is *correctly*
  refused for real users.
- Neither plugin appears anywhere in `contracts/distribution/profiles.v1.yaml`.

**Approach: ship them as checksummed assets of this repo's own release.**
No new source kind is needed — `install_from_catalog` already resolves
`github:OWNER/REPO` to
`https://github.com/{gh}/releases/download/v{version}/{id}-v{version}-{triple}.zip`,
which is a release-asset URL. Repointing the catalog at
`github:vox-foundation/vox` and naming the assets to match is sufficient.
This also collapses the plugin trust root into the one users already trust
to ship `vox` itself — the stated rationale in the security-floor plan.

### No hardcoded versions or hashes anywhere in the chain

A version or hash pinned by hand in committed source has to be re-edited
on every bump, and silently rots when someone forgets. The whole chain is
therefore derived, not pinned. Current state and the fix, layer by layer:

| Layer | Today | Dynamic source |
|---|---|---|
| Plugin crate `Cargo.toml` | `version.workspace = true` — **already SSOT-correct** | no change needed |
| `Plugin.toml` `version` | hardcoded `0.1.0` — stale duplicate | stamped at package time from `CARGO_PKG_VERSION`, plus a drift gate |
| `catalog.toml` `version` | absent | derived at install from the running `vox` binary's own version |
| `catalog.toml` `sha256` | absent | read from the release's own `checksums.txt` |
| Release asset name | n/a | built from the same workspace version in CI |

The `Plugin.toml` fix is the smallest: the crates already inherit the
workspace version, so `version = "0.1.0"` is a hand-maintained copy of a
number Cargo already knows. It gets stamped during packaging and guarded
by a drift check, in the same SSOT-plus-drift idiom the repo already uses
everywhere else. Nothing about a future `0.6.0 → 0.7.0` bump then requires
touching either plugin.

**Why first-party plugins may read `checksums.txt`, when third-party ones
may not.** The security-floor plan rejected fetch-at-install hashing
because the catalog spans many independent `github:owner/repo` trust
roots, and pinning collapses them to the one root users already trust.
That argument does not apply to a first-party plugin shipped *as an asset
of vox's own release*: it has the same repo, the same release, and the
same trust root as the `vox` binary the user is already running — which is
exactly the situation in which `voxup` already fetches `checksums.txt` to
verify the `vox` binary itself. So this reuses a proven mechanism within
its existing trust boundary rather than widening it. Third-party
`github:OWNER/REPO` plugins keep the pinned-hash model unchanged, still
fail-closed.

This removes the "two-release bootstrap" wart entirely: assets and their
verification arrive in the same release, so `vox plugin install
mens-candle-metal` works on the first one.

**Cost, stated plainly:** this requires a change to `install_from_catalog`
in `install.rs` — a fail-closed security path. The change does not weaken
verification (an archive is still refused unless its hash matches a hash
obtained from the release); it only changes *where* the expected hash
comes from, and only for first-party sources. It must be reviewed as a
security change, not a convenience one.

### Structural problem A: version/URL mismatch

The resolved URL embeds the **catalog entry's `version`** in both the
release tag and the filename. Both plugins' `Plugin.toml` declare
`version = "0.1.0"`, but they ship as assets of the main release — so a
catalog `version` of `0.1.0` would target a `v0.1.0` release that does not
exist.

**This largely dissolves under the versioning decision below.** Once the
release tag is `v0.6.0` — the same string as the workspace version — a
catalog `version` of `0.6.0` resolves correctly by construction, with no
`release_tag` field and no change to `install.rs`. What remains is a
tidy-up, not a rule exception: bump the two plugins' `Plugin.toml` from
`0.1.0` to the workspace version so the *installed* directory
(`plugins/<id>/<version>`, taken from `Plugin.toml`, not from the asset
name) agrees with the asset it came from. Leaving them at `0.1.0` would
still install and run; it would just be confusing.

### Structural problem B: CUDA cannot build where we need it

`--features cuda` compiles CUDA kernels transitively (`candle-core/cuda` →
`cudarc`/`candle-kernels`) and requires `nvcc` at build time. The only CI
that compiles with that feature is `vox ci cuda-features`, which runs on
`[self-hosted, linux, x64]` — **the fleet whose unavailability this whole
program exists to route around** — and which no-ops entirely when `nvcc`
is absent. No workflow in this repo installs a CUDA toolkit on a
GitHub-hosted runner; there is no precedent to copy. Metal, by contrast,
needs no extra toolkit (Metal ships with Xcode CLT) and builds cleanly on
`macos-latest`.

**Decision:** Metal ships on the proven path. CUDA is attempted on
`ubuntu-latest` via a CUDA-toolkit install action in a **separate,
non-blocking job** (`continue-on-error`, `fail-fast: false`) so an
unproven CUDA build cannot block Metal or the release. If CUDA fails, we
learn that from a real run without having gambled the release on it.

### Packaging contract

`install_from_path` requires `Plugin.toml` at the extraction root and
copies **only top-level files** — it does not recurse into
subdirectories. Therefore each plugin zip must be **flat**: `Plugin.toml`
and the cdylib side by side at the zip root, with the cdylib named exactly
as that platform's entry in `[plugin.payload.artifacts]` (e.g.
`libvox_plugin_mens_candle_metal.dylib`). A nested layout extracts
successfully and is then silently ignored — a failure mode worth stating
explicitly, because it produces an install that appears to succeed.

### Checksums

`verify_plugin_archive` is genuinely fail-closed: with no recorded hash
and without `--allow-unverified`, it refuses and prints the actual sha
before writing anything. The catalog schema already has `version`,
`sha256`, and `artifacts_sha256` (a per-triple map) — **all unpopulated
for every plugin today**.

This follows the intent already recorded in
`docs/superpowers/plans/2026-08-20-distribution-security-floor.md:1263`
("ship a `vox plugin publish` that writes `sha256` + `version` into
`catalog.toml`"), rather than voxup's fetch-`checksums.txt`-at-install
model, which was deliberately not chosen for plugins because they span
many independent trust roots.

**Superseded by the derived-hash design above.** Pinning the hash in
committed source would mean the hash of a built zip is unknowable until
after the build, forcing a two-release bootstrap (assets first, catalog
pin second) with install correctly refusing in between. Reading the hash
from the release's own `checksums.txt` for first-party plugins removes
that: assets and their verification ship together, and install works on
the first release. Third-party plugins keep the pinned model and its
bootstrap requirement.

## Phase 4 — Homebrew formula audit harness

There is no `brew` on Windows or inside WSL, and the tap integration was
never implemented — the original step was an `echo` stub, since replaced
with a real artifact upload but not a formula publish.

**Scope:** install Homebrew-on-Linux inside WSL2, generate a real `vox.rb`
formula from the release's darwin tarball URL + sha256, and validate it
with `brew audit --strict` and `brew style`.

**Explicit limit, stated up front:** this proves the formula is
well-formed, that its URL resolves, and that its checksum matches. It
**cannot** prove a macOS install, because there is no macOS in this
environment. Anyone reading a green result from this harness must not
conclude `brew install vox` works on a Mac. Publishing to a real tap
(`vox-foundation/homebrew-vox`) remains unimplemented and out of scope: that
repository does not exist and would need a token this agent cannot create.

## Verification

Each phase is verified by its own real signal, not by workflow status:

| Phase | Proof |
|---|---|
| 1 — GUI | Installer assets (`.msi`/`.dmg`/`.AppImage`/`.deb`) present on the release after a tag push |
| 2 — `.deb` | `dpkg -i` succeeds in WSL2 and the installed `vox --version` prints semver |
| 3 — GPU | Plugin zip present on the release, flat-structured, and (post-pin) `vox plugin install` succeeds without `--allow-unverified` |
| 4 — Homebrew | `brew audit --strict` passes against the generated formula in WSL2 |

Phases 1–3 share a verification tag push (`v0.6.0-rc.<buildnum>`, per
Phase 0); iterating costs a new rc tag, not a burned release version.
Phase 4 is local-only and needs no tag. The real `v0.6.0` is cut only
after all three are proven.

## Non-Goals

- Publishing to a real Homebrew tap (repo and token do not exist).
- Building CUDA on the self-hosted GPU pool (that fleet is unavailable;
  routing around it is the point).
- Fixing the `test-*` job race in `release-installers.yml` — those jobs
  call `voxup install`, which hits `/releases/latest`, but run in parallel
  with the jobs that create the release, so they fail on every release.
  Real, permanent, and out of scope here: fixing it needs cross-workflow
  ordering (`workflow_run`/`repository_dispatch`), which is its own design.
- Wiring a nightly/scheduled release trigger. Phase 0 removes the guard
  that made nightly tags *impossible*, but no scheduled workflow is added
  here. `distribution-ssot.md`'s "release + nightly" claim remains
  aspirational prose; this design unblocks it without fulfilling it.

## Risks

- **CUDA is the least certain item.** Installing a CUDA toolkit on a
  hosted runner is unproven here and slow. Structured as non-blocking
  precisely because the probability of first-try success is low.
- **Tauri `targets` may need iteration.** Turning the bundler on is
  necessary but may surface per-platform bundling errors (icon formats,
  Linux `.deb`/AppImage deps) that have never run once. First real signal
  comes from the tag push.
- **The `install.rs` change is on a fail-closed security path.** Deriving
  the expected hash from the release's `checksums.txt` for first-party
  sources must be reviewed as a security change. The invariant to hold: an
  archive is still refused unless its computed hash matches one obtained
  from the release, and third-party `github:OWNER/REPO` sources keep the
  pinned-hash path untouched. A bug here converts a fail-closed check into
  a fail-open one, which is strictly worse than the gap it replaces.
- **`checksums.txt` must actually contain the plugin zips.** It is
  generated over files staged into `release-*` artifact directories, so a
  plugin build job that stages its zip elsewhere would produce a release
  whose plugins can never be verified. The packaging step and the checksum
  step have to agree on the staging path.
