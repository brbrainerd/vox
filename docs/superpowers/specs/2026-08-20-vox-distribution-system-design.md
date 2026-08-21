# Vox Distribution System — Design

> **Status:** Revision 3 · **Date:** 2026-08-20
> Rewritten after an eight-track adversarial audit (finding verification, plan executability, gap
> hunting, CI correctness, cargo semantics, repo policy, packaging feasibility, supply chain).
> **Supersedes the open items of:** [`docs/plans/INSTALL-RELEASE-AUDIT.md`](../../plans/INSTALL-RELEASE-AUDIT.md) (2026-06-07)

## The headline

**Vox has no working release pipeline.** Not "a pipeline with gaps" — a pipeline that cannot produce
a single artifact.

`.github/workflows/release-binaries.yml:43` runs `ci release-build … --package all`.
`ReleasePackage::All` sets `want_bootstrap` (`release_build.rs:43`) and shells
`cargo build -p vox-bootstrap` (`:62-73`). **`vox-bootstrap` was deleted** — it is absent from
`crates/`, absent from `Cargo.lock`, and `contracts/distribution/profiles.v1.yaml:10` says so
outright: *"vox-bootstrap and vox-schola are RETIRED."* Every matrix leg fails,
`if-no-files-found: error` (`:168`) fires, and no release, no `checksums.txt`, no `voxup install`,
and no `install.sh` ever completes.

Revision 1 of this document treated six release workflows as working infrastructure and said "do not
rebuild these." That was wrong. The corrected order of work is: **make a release possible at all,
then make it optimized, then make it trustworthy, then make it an installer.**

## Goal

Ship Vox as a **double-clickable, self-contained, component-selectable installer** for Windows,
macOS, and Linux, built at true release optimization, verified end-to-end against the artifact users
actually download, cryptographically authenticated, versioned and changelogged from a single source,
distributed through GitHub Releases on stable and nightly channels, and self-updating thereafter.

## Decisions

| # | Decision | Rationale |
|---|----------|-----------|
| D1 | **Native declarative installer UI** | WiX `WixUI_FeatureTree` and `productbuild --distribution` render checkbox trees from declarative XML. No new application code. |
| D2 | **A single feature-tree MSI. No Burn bootstrapper.** | Burn chains external prerequisites, and after D3 there is at most one. Decisive on its own: **WixStdBA has no feature tree.** The WiX maintainer, verbatim: *"There is no stock BA that offers feature trees. That does require a custom BA."* A Burn bundle would give either two sequential wizards or a custom bootstrapper application — both contradict D1. |
| D3 | **Do not bundle Rust, Node, or pnpm.** | `profiles.v1.yaml` scopes `build_deps` to *"only relevant on the from-source path"*; we ship prebuilt binaries. Node is a build dep of the **CI** Tauri build, not of an installed Vox. Decisive: **Rust cannot legally ship a working offline Windows build environment** — rustup deliberately omits `link.exe` "to avoid redistributing Microsoft's proprietary tools." |
| D4 | **WebView2 via Evergreen Standalone Installer**, not fixed-version | Microsoft explicitly blesses including the Evergreen Standalone Installer in an app installer; Tauri exposes it as `webviewInstallMode: offlineInstaller`. Fixed-version is **>250 MB per Microsoft** and carries obligations (obtain directly from Microsoft, impose protective end-user terms, indemnify Microsoft) plus frozen-engine CVE exposure. |
| D5 | **One installer per OS/arch. No GPU / non-GPU split.** | GPU backends are already `cdylib` plugins (`vox-plugin-mens-candle-cuda`, `-metal`, `-nvml-probe`), not compile-time features. The `vox` binary is GPU-agnostic. Hardware selection is a *feature-tree default*, not a build variant. |
| D6 | **Weights, CUDA, and Docker are never bundled** | The default local model is ~18–20 GB quantized; the CUDA runtime is multi-GB. GitHub caps a single asset at **2 GiB**. Git LFS does not change this. |

**Resulting payload:** ~0.3–0.5 GB, versus ~1.6–2.5 GB under revision 1's bundle-everything plan.
Revision 1's size table was wrong in every checkable figure (Node is 110 MB installed, not 50;
WebView2 fixed is >250 MB, not 180; a real rustup install exceeds 1 GB, not 300 MB).

## Findings

Severity-ordered. F1–F3 block every other finding.

### F1 — The release build invokes a deleted crate (critical)

See The headline. Nothing downstream of this has ever run in production.

Related: `release-binaries.yml:41`'s comment also promises a `vox-schola` artifact; that crate is
likewise gone, and the smoke loops at `:96-103` are silent `nullglob` no-ops.

**Gate owed:** `distribution_parity.rs:122` validates `profiles.v1.yaml` `binaries:` against crate
directories on disk, but nothing validates it against `release_build.rs`'s package set. That is the
missing assertion.

### F2 — All 16 bundle artifacts publish under one colliding filename (critical)

`bundle-release.yml:80-110` builds `--out bundle.tar.gz` for every one of 8 bundles × 2 targets and
attaches literally `bundle.tar.gz` to the release. They overwrite each other; the survivor carries no
bundle id, no target, and no version. None appear in `checksums.txt`, which is assembled in
`release-binaries.yml`'s publish job before bundles exist.

### F3 — Bundle matrix drift fails two jobs on every release (high)

`bundle-release.yml:36` lists `vox-cloud-only`, absent from `catalog.toml`. It does **not** fail
silently: `bundle_resolved` returns `ResolveError::UnknownBundle`
(`crates/vox-plugin-catalog/src/lib.rs:77-80`) and the build step has no `continue-on-error`.
`fail-fast: false` let the other jobs proceed, so nobody noticed.

`vox-ml-metal` is never built — but must **not** simply be added: the matrix is x86-64 Linux and
Windows, and that bundle carries an Apple-Metal plugin. `vox-mobile` is deferred (`status = "alpha"`).
The gate must be **platform-aware**, not exact-equality.

### F4 — No installer reaches users on any platform (critical)

- MSI: `release-installers.yml:65` runs `cargo wix --no-build` with no preceding build — the inline
  comment at `:66-67` admits it. Never uploaded.
- `.deb`: built at `:83`, then discarded. No upload step.
- Homebrew: `:98` is `echo "Simulating Homebrew Tap update..."`.
- macOS: unnotarized `tar.gz` only; Gatekeeper quarantines it.

Windows and Linux packaging is **greenfield**, not an upgrade of working infrastructure.

### F5 — Windows GUI signing has never signed anything (critical)

`release-gui.yml:140-152` runs `azure/trusted-signing-action` **after** `tauri-action` has already
uploaded the bundle (`:120`), so even on success the *published* MSI is the unsigned one — nothing
re-uploads. And `files-folder` (`:151`) points at `crates/vox-gui/src-tauri/target/release/bundle/msi`,
a directory that does not exist: the Tauri project root **is** `crates/vox-gui`.

Revision 1 called this "already wired, should be reused." It is wired and inert.

### F6 — `[profile.dist]` is dead code at nine build sites (critical)

Every shipped binary is thin-LTO. The fix is not one line — nine sites pass `--release`:
`release_build.rs:150`; `release-binaries.yml:46`; `release-installers.yml:23, 91, 119`;
`release-gui.yml:90, 91, 104`; `bundle-release.yml:67, 76, 84, 92`. The artifact read path
(`release_build.rs:171-174`) must move to `target/<triple>/dist/` in the same change, preserving
`--features heavy-retrieval` (`:159-161`).

The root `Dockerfile:21` also builds `cargo build --release` for a **daily public image** (F16).

### F7 — `panic = "abort"` would break three production paths (critical)

**The F6 fix is what introduces this.** Three non-test sites in the shipped binary need unwinding:

1. `crates/vox-actor-runtime/src/supervisor.rs:30,52` — `spawn_supervised`, the workspace's
   panic-containment primitive, matches on `JoinError::is_panic()`. Under abort a panicking task
   kills the process and **every caller silently loses supervision.**
2. `crates/vox-vcs/src/jj_actor.rs:196,282` — the `guarded!` macro wraps `block_on` in
   `catch_unwind` so a panicking `jj-lib` call returns `Err(Unavailable)` instead of killing the
   actor loop. This ships: `jj` is a default feature of `vox-orchestrator`, which `vox-cli` takes
   with defaults.
3. `crates/vox-search/src/memory_cache.rs:87-88` — `resume_unwind` on a `spawn_blocking` panic.

**Decision: remove `panic = "abort"` from `[profile.dist]`.** Keep `lto = "fat"`,
`codegen-units = 1`, `strip = "symbols"`. Must land before or with F6.

### F8 — Nothing is tested at ship optimization (high)

Smoke tests are `--version` / `--help` only.

**Corrected premise.** Revision 1 claimed `cargo test` cannot run under `[profile.dist]` because of
`panic = "abort"`. **False.** The Cargo Book: *"Tests, benchmarks, build scripts, and proc macros
ignore the `panic` setting."* `cargo test --profile dist` works on stable today. The
`[profile.dist-test]` proposed in r1 was a workaround for a nonexistent bug and is deleted.

A full `cargo test --profile dist` lane is **rejected on capacity grounds**:
`crates/vox-cli/tests/` holds 82 integration targets over a 1656-package graph, each triggering its
own fat-LTO link, against `MEM_PER_RUNNER = 14000m` with a measured ~12 GB peak at *thin* LTO
(`runner_scale.rs:41-42`). That lane OOM-kills, and an OOM-killed job cannot report itself.

Only a subprocess black-box suite against the real artifact can exercise ship behavior, because every
test target is force-built with unwind.

### F9 — Native plugins execute with zero integrity verification (critical, security)

`plugin/install.rs:110-138` fetches a zip over HTTPS and calls `archive.extract()` with **no
checksum, no signature, no pinned version**, then `vox-plugin-host/src/loader.rs:39` `dlopen`s the
cdylib. The only gate is an ABI integer (`loader.rs:49`). `install_from_catalog:178-186` builds a
mutable `…/releases/latest/download/<id>-latest-<triple>.zip` URL — anyone able to publish a release
asset in that repo owns the user's process.

Adjacent: `vox-plugin-host/src/user_install.rs:145-170` clones arbitrary skill URLs with no SHA pin.
(That path is otherwise well-hardened — it rejects `ext::`/`file::`/`fd::`, sets
`GIT_PROTOCOL_FROM_USER=0`, and never executes `scripts/`. The native-plugin lane has none of that.)

This is the prior audit's finding #4, still open, and revision 1 dropped it entirely.
**D5's on-demand GPU plugin fetch depends on this being fixed first.**

### F10 — `checksums.txt` is same-origin and unsigned (high, security)

The artifact and its checksum come from the same host, same TLS session, same job, with no key
material anywhere. Precisely: this detects **corruption** and **partial tampering**, and gives a
stable identifier for out-of-band comparison. It does **not** detect a compromised release — a stolen
token, a compromised publish runner, or a malicious workflow edit regenerates `checksums.txt` to
match a trojaned artifact and the check passes.

**Fix:** sign `checksums.txt` with a key held **outside GitHub** (minisign/cosign), public key
compiled into `voxup`. This single change converts every downstream check from integrity to
authenticity. Attestation (F17) is complementary — it still roots in GitHub's OIDC identity.

### F11 — `install.sh` fails open on integrity (high, security)

`scripts/install.sh:54-61`: with no `sha256sum` and no `shasum`, it prints a warning and
`return 0`s — installing an unverified binary. `need_cmd` guards `curl` and `tar` but never a hash
tool. In a `curl | sh` pipeline the warning scrolls past.

### F12 — Tar extraction is unguarded on the platforms that use it (high, security)

`voxup/src/download.rs:64-74` is `Archive::new(gz).unpack(dest)` with no path validation, no
symlink/hardlink rejection, and no size cap. The zip path at `:83-92` **does** check
`enclosed_name()` and `starts_with(dest)` and has a regression test — but both are `#[cfg(windows)]`,
and the archive extension is `.zip` only on Windows. **The tar path is what every Linux and macOS
user takes, and it is the unguarded one.**

Impact is concrete: extraction lands in `~/.vox/toolchains/vox-<ver>/`, one `..` from
`~/.vox/toolchains/bin`, which `proxy.rs:209-216` **prepends to `PATH`** for every proxied `vox`
invocation. Mitigation today rests entirely on the `tar` crate skipping escaping entries — silently,
so a tampered archive surfaces as "Extraction succeeded but 'vox' not found."

### F13 — Release artifacts are built on an unpinned toolchain (medium)

Every release workflow uses `dtolnay/rust-toolchain@stable` while the repo pins 1.96.0. Shipped
artifacts are built on a different compiler than CI gates, and each new stable silently imports its
lint wave. The fleet image already ships the pinned toolchain, so the step is also waste.

### F14 — Over-scoped release credentials (medium, security)

`release-binaries.yml:8-9` and `release-gui.yml:10-11` declare `contents: write` **top-level**, so
build-matrix jobs that compile third-party crates hold a write token they never use.
`release-installers.yml` declares **no `permissions:` block at all**, inheriting the repo default.

Good news to preserve: all three release workflows trigger on `push: tags` only. **No
`pull_request_target` and no `workflow_run` exists anywhere in the repo**, so signing secrets are not
reachable from untrusted code. Phase 2 and 3 must not introduce either.

### F15 — Two updaters, one of which bricks itself (high)

Revision 1 claimed "only the CLI self-updates, via `voxup`." There are **two**.
`toolchain_upgrade.rs:669` calls `maybe_install_openclaw_sidecar` **after** the binary swap at `:665`,
and that function hard-errors when no `openclaw-gateway-*` asset is in `checksums.txt` (`:703-705`) —
which is never, since nothing builds or uploads it. The binary is replaced, then the command exits
non-zero. It then probes the retired `vox-bootstrap` at `:678`.

### F16 — Unaudited container distribution (medium)

`ghcr.io/brbrainerd/vox-eval` is built **daily** (`docker-eval.yml:12`) from the root `Dockerfile`,
pushed public to a **personal** namespace, entrypointing `vox mcp`. `Dockerfile:21` uses
`cargo build --release`, so F6's blast radius is wider than the release workflows.
`ghcr.io/<owner>/vox-ci-runner:latest` is mutable and pulled by the fleet. Neither is signed,
attested, or version-pinned to a release.

### F17 — No SBOM, no provenance, and no client-side verification (medium)

The SBOM step is **commented out** (`release-binaries.yml:69-78`) and is CycloneDX, not SPDX —
revision 1 credited it as existing. Nothing is signed or attested.

And an attestation nobody verifies is a compliance artifact, not a control. Revision 1 proposed
`actions/attest-build-provenance` and stopped there; `voxup` has no hook where a check could go.

### F18 — The documented install URL has no route (high)

`docs/src/reference/installation.md:18` and both script headers advertise `https://voxlang.org/voxup`.
`docs-astro/public/` contains no such file and `_redirects` has no rule. **`curl … | sh` pipes a 404
page into a shell.**

### F19 — No uninstall path exists (high)

`voxup` exposes `Install` / `Update` / `Proxy` only. Nothing removes `~/.vox`, the
`~/.cargo/bin/vox` ↔ `~/.vox/bin/vox` hard link, or PATH entries. The MSI and deb that would own
uninstall are the F4 stubs. Any E2E asserting clean removal has nothing to assert against.

### F20 — Install scripts synthesize unpublished triples, and the guard test only greps comments (high)

`install.sh:37-42` produces `aarch64-unknown-linux-gnu`; `install.ps1:26-28` produces
`aarch64-pc-windows-msvc`. Neither is in `SUPPORTED_RELEASE_TARGETS`
(`install_policy/mod.rs:21-26`), so ARM users get an opaque 404 rather than a supported-platform
message. The guard `install_scripts_cover_release_targets` (`release_build.rs:287-307`) asserts only
that the string *appears somewhere in the file* — satisfied by the header comments, and it passes
while `detect_target` misbehaves.

### F21 — Proposed installer filenames conflict with the naming SSOT (high)

`release_artifacts/mod.rs:29-31` is the SSOT: `<name>-<version>-<rust-triple>.<ext>`, version
retaining the `v` prefix. Revision 1's `VoxSetup-0.7.0-x86_64-windows.exe` drops the `v` and replaces
the triple with an ad-hoc token. Five consumers would silently fail to find it: `voxup`
(`install.rs:62-93`), `install.sh:87-97`, `install.ps1:61-83`, and `toolchain_upgrade.rs:851` and
`:745`, both of which filter on `file.contains(target_triple)` — `x86_64-windows` never matches
`x86_64-pc-windows-msvc`.

### F22 — GUI release orchestration: draft/publish race, no checksums, stale embedded CLI (medium-high)

`release-gui.yml:135` sets `releaseDraft: true` while `release-binaries.yml:206` creates a
*published* release for the same tag, with no ordering. `bundle-release.yml` triggers on
`release: published`, so its firing depends on which wins. GUI bundles never enter `checksums.txt` —
**the GUI download path has zero integrity verification.** And `tauri.conf.json:32-34` ships a full
`vox` CLI as an `externalBin` that neither updater can reach, so a GUI install carries a frozen CLI
forever. `tauri.conf.json:4` also hardcodes `"version": "0.6.0"`.

### F23 — Changelog unification breaks a gate if done naively (medium)

`cliff.toml` is unused; release bodies use `generate_release_notes: true`; `CHANGELOG.md` is
hand-maintained.

`vox-arch-check/src/main.rs:1877` `parse_release_date` skips lines containing `"Unreleased"` (capital
U), but cliff emits `## [unreleased]` lowercase — so it does not skip, `strip_prefix("] - ")` fails,
and `?` returns `None` **from the whole function**, silently disabling arch-check Rule 6 and the
staleness check. Separately, `CHANGELOG.md` lines 1–9 carry YAML frontmatter a cliff header would
delete.

### F24 — No nightly channel (high)

Nineteen workflows carry `schedule:`; none publishes a release artifact. `voxup/src/channel.rs:7` has
a single `API_LATEST` const and no channel concept.

A rolling force-updated `nightly` tag has a trap: `install.rs:97` keys the cache directory on
`release.version`, and `update.rs:117` compares `latest <= installed` by semver. Two nightlies
sharing `0.7.0` mean the cache collides and `voxup update` is a **permanent no-op** — and an E2E
asserting "update is a no-op" would pass for entirely the wrong reason.

### F25 — GUI has no auto-update (high)

`tauri.conf.json` has no `plugins` block, no `updater`, no pubkey.

### F26 — Managed installs and `voxup` will fight (high, latent)

`voxup update` swaps the binary in place, desynchronizing the OS package database after an MSI or
`.pkg` install.

### F27 — Target matrix gaps (medium)

Four targets. Expanding requires a five-way SSOT update: `install_policy/mod.rs:21-26`,
`release-binaries.yml`'s matrix (gated by `release_build.rs:311`), both install scripts (gated by the
broken F20 test), and `docs/src/ci/binary-release-contract.md`.

### F28 — D6's post-install weights path does not exist (medium)

No code pulls a model: no `ollama pull`, no `/api/pull`, no `vox model pull`. `vox doctor` only
TCP-probes `127.0.0.1:11434` and prints an advisory (`tail.rs:217-225`). Ollama is never installed by
anything. A fresh install has no local model and no automated route to one.

### F29 — A crates.io publish set with no publish automation (medium)

`crates/_public.toml` declares `vox-crypto`, `voxup`, `vox-plugin-types`, `-api`, `-sdk`, gated by
parity tests. No workflow runs `cargo publish`. `cargo install voxup` is an intended surface no
release cuts.

## Architecture

### A1 — Component model and installer configurability

The feature tree is **generated from `catalog.toml`**, never hand-written.

| Tree level | Source | Default |
|---|---|---|
| `vox` CLI | tier `minimal` | Always installed, not deselectable |
| GUI | `[[component]] id = "gui"` | Selected |
| Plugin group per bundle | `[[bundle]]` + `extends` closure | `vox-fullstack` selected; others cleared |
| Individual plugin leaf | `[[plugin]]` | Selected iff in `vox-fullstack` **and** `status = "stable"` |
| GPU plugin leaf | `[[plugin]]` with `requires-tag` | Selected **iff the tag matches detected hardware**; otherwise shown, unchecked, disabled |

**Hardware gating is declarative.** Windows: a WiX `<RegistrySearch>` sets a property from the NVIDIA
driver key; GPU features carry `<Condition Level="0">`. No custom action. macOS: distribution XML
permits JavaScript in `<choice selected=…>`, so `system.sysctl('hw.optional.arm64')` gates Metal.
Linux: `Recommends:` plus `vox doctor`.

**Select-all / clear-all: not available as stock controls.** `WixUI_FeatureTree`'s `CustomizeDlg`
provides `Tree`, `Browse`, `Reset`, `DiskCost`, `Back`, `Next`, `Cancel` — no Select All or Clear
All; `Reset` restores the *initial* selection. macOS `<choices-outline>` is likewise checkbox-only.
**Resolution:** author a single root feature so the MSI SelectionTree's per-node dropdown ("Entire
feature will be installed / unavailable") cascades to everything, giving select-all and clear-all as
one gesture on the root. A custom dialog would contradict D1 and is rejected.

**macOS constraint on the generator:** `pkg-ref` may attach only to *childless* choices. Since the
WiX tree has parent features carrying components, the generator must emit a synthetic leaf per
non-leaf node or it will produce invalid distributions.

**SSOT change this forces.** `vox-plugin-catalog/src/schema.rs:45` documents `requires_tag` as
*"informational only."* Acting on it promotes it to load-bearing: update that comment and validate
the tag vocabulary (`nvidia-gpu`, `apple-silicon`) as a closed set at catalog build time, or a typo'd
tag is silently ignored by installers.

**Deselected ≠ unavailable.** Anything unchecked stays installable via `vox plugin install <id>`,
resolving `default-source` from the same catalog. The installer sets initial state; it is not a
separate channel. This is why D5 needs no GPU split — **and why F9 blocks Phase 2.**

### A2 — Generator and its registration points

`vox ci gen-installer-manifests [--write]` projects `profiles.v1.yaml` + `catalog.toml` into
`packaging/windows/features.generated.wxs`, `packaging/macos/distribution.generated.xml`, and
`packaging/linux/control.generated`. `packaging/` does not exist yet.

All five registration points are hand-maintained lists (revision 1 named one):

1. `.gitattributes` — its only generated pattern is `docs/src/**/*.generated.md`. Three explicit
   lines needed; note `control.generated` has **no extension**, so `* text=auto` otherwise applies.
2. `run_ssot_drift` (`ci/run_body_helpers/docs.rs:551`) — add a `ds!()` entry.
3. The `ssot-autoregen` job (`ci.yml:238-257`) — a hardcoded list of `<generator> --write` strings.
4. `lefthook.yml` — a `stage_fixed: true` command keyed on the two inputs.
5. AGENTS.md §"Committed and kept in sync by pre-commit hooks".

### A3 — Installers

**Windows.** A single MSI with `WixUI_FeatureTree`, built by `cargo-wix` **after a real
`cargo build --profile dist`**. Note `cargo-wix` emits MSI only (no Burn), defaults to WiX v3.14.1,
and **WiX v7 is current** (2026-04-06) — revision 1's "WiX v4" was two majors stale. Today's
invocation has no `.wxs`, no `[package.metadata.wix]`, and builds nothing.

**macOS.** `productbuild --distribution` `.pkg` with a `<choices-outline>`, wrapped in `.dmg`, signed
and notarized — after F5's wiring is repaired.

**Linux.** `.deb` metapackages plus an AppImage. **Correction:** `.deb` is *not* a double-click
install in 2026 — Ubuntu 26.04 stock reports "No app installed for Debian package files" since GNOME
Software dropped `.deb` handling. **Only the AppImage delivers the double-click promise on Linux**,
and it still needs `chmod +x`. `.rpm` via `cargo-generate-rpm` is cheap to add later.
Flatpak/Snap are rejected: sandboxing breaks arbitrary compiler and subprocess access, which is
Vox's entire job.

**Glue language.** Per AGENTS.md §VoxScript-First, packaging glue is `.vox` run via `vox run`. Where
a format hard-requires a native maintainer script (deb `postinst`, AppImage `AppRun`), it is a
≤10-line thin launcher forwarding to `vox run`, matching the `scripts/windows/vox-dev.ps1` bootstrap
exception. XML manifests are data, not glue, and are exempt.

**Naming.** Installer filenames extend `release_artifacts::artifact_filename` as the SSOT (F21), not
an ad-hoc scheme, and all five consumers are updated together.

### A4 — Versioning and channels

Nightly identity derives from `vox-build-meta`, which already emits `VOX_BUILD_NUMBER` and
`VOX_GIT_HASH` (`lib.rs:24-30`) — revision 1 discarded these for a re-derived date and hash.

- Stable: `vox-v0.7.0-x86_64-pc-windows-msvc.msi`
- Nightly: `vox-v0.7.0-nightly.{VOX_BUILD_NUMBER}+{VOX_GIT_HASH}-x86_64-pc-windows-msvc.msi`

The same pre-release string is the **cache-directory key, the `active` value, and the binary's
self-reported version** — closing F24's collision and no-op trap. Semver orders
`0.7.0-nightly.N` before `0.7.0`, so a stable-channel user is never downgraded onto a nightly. The
last N nightlies are kept as immutable individually-tagged pre-releases with the rolling `nightly`
tag as a pointer, so rollback is `voxup install --version <exact>`.

### A5 — Changelog SSOT

`git-cliff` becomes the only generator, under three constraints from F23: the frontmatter moves into
`cliff.toml`'s `header` (or cliff runs `--prepend`); the body emits `## [Unreleased]` capitalized;
and `parse_release_date` is hardened to `continue` rather than `?`-returning — a latent footgun
independent of cliff. Hand-written `[Unreleased]` narrative is not reconstructable from commit
subjects and is deliberately discarded.

### A6 — Trust chain

Ordered by cost-to-benefit; the first item is what converts the chain from integrity to authenticity.

1. **Sign `checksums.txt`** with a key held outside GitHub; embed the public key in `voxup`; verify
   signature before hash in `install.rs` between `:68` and `:92`.
2. **Verify plugins at load, not just install** (F9): add required `sha256` to catalog `[[plugin]]`
   and `Plugin.toml`; record it in an installed-manifest lockfile; make `Loader::load` refuse a
   dylib whose on-disk hash differs — closing the swap-after-install path. Reuse
   `voxup::download::verify_sha256` by defactoring (~10 lines, no new crate edge).
3. **Delete the `install.sh` fail-open branch** (F11) — one line.
4. **Explicit tar entry validation** (F12): reject symlink/hardlink/device entries, validate paths
   are relative and `..`-free, re-check `starts_with(dest)`, cap uncompressed bytes and entry count.
   Port the existing zip-slip test to a `#[cfg(unix)]` tar equivalent.
5. **Job-scoped `permissions:`** (F14); add a lint asserting every workflow declares one, shaped like
   `vox ci workflow-concurrency-guard`.
6. **Client-verifiable provenance** (F17): publish the attestation bundle as a release asset so
   airgapped installs can carry it; add `voxup verify` and call it during install; make
   `gh attestation verify` a **required** step in the clean-room matrix. State the residual plainly:
   attestation roots in GitHub's OIDC, so only item 1 defends against a compromised workflow.
7. **Add SBOM generation** and make it required — `continue-on-error: true` on a supply-chain
   artifact is worse than none, because consumers assume it exists.

**Offline payload provenance.** A `sha256` minted by fetching a URL and recording what comes back is
downstream of the trust decision — it launders an unverified download into a permanent
gate-enforced-looking constant. Every bundled third-party payload must have its **upstream signature
verified at the moment the hash is minted** (Node's GPG-signed `SHASUMS256.txt`; Rust's `.asc`;
WebView2's Authenticode publisher subject), pinned immutable URLs, the upstream signing key id
recorded alongside, and a CI step that re-fetches and re-verifies. D3 and D4 remove most of this
surface; what remains still needs it.

**Managed-install handoff (F26).** Revision 1 proposed `voxup` read a user-writable
`~/.vox/.managed-by` and download-and-execute an installer. That is forgeable, TOCTOU-prone, an
elevation-prompt laundering path, and — with signing unresolved — strictly worse than today's
verified in-place swap. **Revised: the marker is a one-bit refusal signal only.** If present and
parseable, `voxup update` prints the correct OS-native command and exits non-zero. It does not
download and does not execute. The OS package manager is already the trusted update path on a managed
install; re-implementing it inside `voxup` buys nothing. The marker lives in an installer-owned,
non-user-writable directory (`%ProgramData%\Vox\`, `/var/lib/vox/`) and is removed on uninstall.

### A7 — End-to-end verification (TDD)

Every layer gets a failing test first.

| Layer | Test | Gate |
|---|---|---|
| Release possible at all | `release_build.rs` package set == `profiles.v1.yaml` `binaries:` | `cargo test -p vox-cli` |
| Artifact naming | Every bundle artifact name unique, versioned, and in `checksums.txt` | `cargo test -p vox-cli` |
| Matrix parity | `bundle-release.yml` vs catalog, platform-aware | `cargo test -p vox-cli` |
| Profile correctness | Source guard: no `--release` at any of the nine sites | `cargo test -p vox-cli` |
| Ship optimization | Black-box subprocess suite keyed on `VOX_DIST_BIN` so a wrong path **fails rather than skips** | `dist-verify` job |
| Plugin integrity | Hash mismatch refuses to load | `cargo test -p vox-plugin-host` |
| Archive safety | Tar-slip and symlink-escape cases, `#[cfg(unix)]` | `cargo test -p voxup` |
| Install-script targets | Behavioural `detect_target` test, not a comment grep | `cargo test -p vox-cli` |
| Manifest generation | Golden-file: fixture catalog → expected WiX / plist / control | `cargo test -p vox-cli` |
| SSOT drift | Generated manifests match catalog | `vox ci ssot-drift` |
| Installer behavior | Silent install with an explicit feature selection; assert exactly the selected plugins on disk | Clean-room matrix |
| Full lifecycle | Install real artifact → `vox doctor --strict` → run a golden `.vox` → `voxup update` no-op → **uninstall → assert clean removal** | Clean-room matrix |

**Ordering is structural, not advisory.** Tag pushes **cannot** be gated by GitHub required checks —
those apply to branch and PR refs only. The sole reliable mechanism is `needs:` within one workflow.
Therefore `dist-verify` is a **job inside `release-binaries.yml`** and `publish` declares
`needs: [build, dist-verify]`. A standalone workflow on `push: tags` would publish while verification
ran, and publish unaffected if it failed.

**A7's clean room is new, not reuse.** `setup-e2e.yml` is a *source-checkout* clean room — it strips
rustup, reinstalls, and runs `scripts/setup.vox`. It never touches a published artifact.

## Out of scope

- Bundling weights, CUDA, or Docker (D6); replacing the pipeline with `cargo-dist`; mobile
  (`vox-mobile` is alpha, v0.8).

## External blockers

1. **A Windows code-signing certificate.** Azure Trusted Signing exists but has never signed a
   published artifact (F5); it must be repaired, not reused.
2. **A Tauri updater keypair** (`TAURI_SIGNING_PRIVATE_KEY`).
3. **A release-signing key held outside GitHub** for A6 item 1 — the highest-leverage blocker.
4. **A Linux signing key**, or an explicit decision to ship Linux unsigned.

## Phasing

| Phase | Content | Blocked? |
|---|---|---|
| **0** | F1 (deleted crate), F2 (artifact collision), F3 (platform-aware parity), F18 (install URL route). **Nothing else matters until a tag produces artifacts.** | No |
| **1a** | F7 (drop `panic = "abort"`), F6 (nine sites + Dockerfile), F8 (black-box lane ordered before publish), F13 (pin toolchain) | No |
| **1b** | Security floor: F11, F12, F14 (each under ten lines), then F9 plugin integrity. **F9 gates Phase 2.** | Partly (F10 needs a key) |
| **1c** | `gen-installer-manifests` + five registration points; F21 naming SSOT; F20 behavioural target test | No |
| **2** | F4 installers (greenfield), F5 signing repair, A1 feature tree and hardware gating, F19 uninstall | Signing certs |
| **3** | F24 nightly, F23 changelog, F27 matrix, F17 SBOM + provenance, F16 containers, F29 crates.io | No |
| **4** | F25 GUI updater, F22 GUI orchestration, F26 managed handoff, F15 second updater, F28 model pull, full A7 matrix | Tauri keypair |

Phase 0 is planned first. It is unblocked, small, and every other phase is dead code without it.
