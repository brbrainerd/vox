# Vox Distribution & Plugin Architecture — Design Spec

**Status:** Draft for review. No public release is authorized; every task below
is buildable and verifiable without publishing to any channel.

**Scope:** the single question of how Vox is versioned, built, split, and
delivered — across Homebrew, winget, apt, cargo, npm and MSI — and how the
plugin system must change to carry the part no package manager can.

---

## 1. The finding that determines the architecture

Four independent ecosystem audits (2026-09-04) produced one constraint table.
Every cell is measured or quoted from a primary source; see §11 for citations.

| Capability | brew | winget | apt/deb | cargo | npm | MSI/WiX |
|---|---|---|---|---|---|---|
| User-selectable optional components at install | no | **no** | no | yes (`--features`) | **no** | **yes** (`ADDLOCAL`) |
| …without recompiling | — | — | — | **no — recompiles** | — | yes |
| Ships prebuilt binaries | yes | yes | yes | **no — source only** | yes | yes |
| Per-CPU-arch variants | yes | yes (`Architecture`) | yes | via triple | yes (`os`/`cpu`/`libc`) | **one arch per MSI** |
| **Select on GPU / hardware capability** | **no** | **no** | **no** | **no** | **no** | conditional, but silently skipped whenever `ADDLOCAL` is set |
| Exact version pin between own packages | n/a | **no — `MinimumVersion` only** | yes (`= ${binary:Version}`) | yes (`=1.2.3`) | yes | yes |

**Not one package manager can express "install the GPU build if this machine
has a GPU."** MSI comes closest and disqualifies itself: its
`<Level Condition="HAS_CUDA">` auto-detection is skipped entirely the moment
anyone passes `ADDLOCAL`, which every silent and enterprise install does.

**Every one of them can express "a separate package name."**

That asymmetry is the whole design. The package manager's job is reduced to what
all six can do identically; everything capability-dependent moves behind a layer
that runs *on the target machine, after install*, because that is the only place
the hardware is visible. Vox already has that layer. It is the plugin system.

This is the same conclusion you reached independently — a lean core that installs
its own further dependencies. The research confirms it is not merely convenient;
it is the only design the intersection of these six ecosystems permits.

---

## 2. Architecture: three layers

```
  Layer 1  CORE          one lean binary per (product, OS, arch)
           voxlang       ← what package managers ship. No GPU, no models,
                           no GUI, no ML. Same bytes for every user on a triple.

  Layer 2  CAPABILITY    resolved at first use, on the target machine
           plugins       ← GPU backends, speech, mesh, ML. Probes hardware,
                           downloads a checksummed artifact, verifies, loads.

  Layer 3  DATA          never in a package, always explicit
           models        ← model weights (a Qwen3-8B is ~16 GB), caches,
                           corpora. Downloaded only on an explicit command.
```

The rule that makes this maintainable: **a package manager artifact must be
identical for every user with the same triple.** If two users on the same triple
would get different bytes, the difference belongs in Layer 2, not Layer 1.

### Why not "one fat package"

npm hosts 89 MB packages without complaint (`@next/swc-darwin-arm64`), so fat is
technically available there. It is rejected because (a) cargo cannot do it at
all — crates.io is source-only, so a fat cargo package means every user compiles
CUDA; (b) it forces a CUDA toolchain onto users who have no NVIDIA GPU; and (c)
the disk audit measured `~/.vox/bin/vox-orchestrator-d` alone at **136 MB**. Fat
does not scale past two backends.

### Why not "N packages per capability"

`vox`, `vox-cuda`, `vox-metal`, `vox-gui`, `vox-cpu` as six package identities
per ecosystem is 30+ manifests to keep version-locked, and winget cannot pin them
to each other (`MinimumVersion` only, no maximum). Debian *does* do this —
`pytorch-cuda` is a separate source package in contrib — but Debian has
`= ${binary:Version}` and a sponsor reviewing every upload. winget does not.

The chosen split is the minimum that the ecosystems force, not one package per
concept. See §4.

---

## 3. Layer 1 — what each ecosystem actually ships

**One core artifact per triple.** Name resolution is settled by collision, not
preference (§8):

| Ecosystem | Identity | Ships | Notes |
|---|---|---|---|
| Homebrew | `voxlang` (tap `vox-foundation/vox`) | `vox` binary | `vox` is taken by the VOX music-player cask. Install form is the fully-qualified `vox-foundation/vox/voxlang`, which needs no `brew trust`. |
| winget | `Vox.Voxlang` | `vox.exe` + `vox-compilerd.exe` | winget CANNOT pin dependencies exactly. Do not model add-ons as winget packages (§4). |
| apt | src `vox` → `vox`, `vox-common`, `vox-doc` | `/usr/bin/vox` | Self-hosted, GPG-signed aptly repo. Not Debian proper, not a PPA — PPAs are **source-only** and their builders have no CUDA. |
| cargo | `voxlang` crate | `vox` binary | **`vox` is taken on crates.io** by `bearcove/vox` (24 versions, active). Blocked — see §8. |
| npm | `@vox/cli` + per-triple `@vox/cli-<triple>` | `vox` binary | esbuild pattern: thin parent, `optionalDependencies` on platform packages pinned **exactly**, `os`/`cpu`/`libc` gating, no install script. |
| MSI | `voxlang-<arch>.msi` | `vox.exe` | One arch per MSI is enforced by the format. Burn bundle chains the three. |

**Correction to current state:** the MSI's WiX `Product Name` is `vox-cli` and
`Manufacturer` is `Bert Brainerd` (`crates/vox-cli/wix/main.wxs:63,65`). Both
must become the product identity, not the crate name and a personal name.

---

## 4. Layer 2 — evolving the plugin system into the capability layer

### 4.1 What is broken today

Measured against the tree at `main`:

| # | Defect | Evidence |
|---|---|---|
| P1 | **The entire plugin catalog is unreachable for an installed user** — broader than first stated. All **12** `github:vox-foundation/vox-plugin-*` sources 404, not just CUDA. Six sources are `local:<repo-relative path>`, which cannot resolve off a clone: `nvml-probe`, `mens-candle-metal`, `webhook`, `runtime-wasm`, `runtime-container`, `publication`. | `catalog.toml:20,30,40,154,172,181,190` |
| P1b | **The release-asset URL is malformed regardless of source.** `install.rs:232` hardcodes `let version = "latest"`, producing `…/releases/latest/download/{id}-latest-{triple}.zip`. **Corrects an earlier claim in this spec** that the resolver already produces `…/download/v{version}/{id}-v{version}-{triple}.zip` — it does not, so repointing a source at a valid repo still 404s. | `install.rs:226-241` |
| P1c | **`local:` has no env gate at all.** An earlier draft said it was "deliberately gated behind `VOX_LOCAL_PLUGIN_FALLBACK`". That variable **does not exist**. The real one is `VOX_NO_LOCAL_PLUGIN_FALLBACK` (opt-**out**, fallback on by default) and it guards only the `github:` branch (`install.rs:219`). The `local:` branch at `:226` is ungated. Metal is not "refused by design" — it is a plain unreachable-path bug. | `install.rs:213,219,226` |
| P2 | Nothing loads Metal. Every `MlBackend` call site names `"mens-candle-cuda"` literally. | `schola/merge_qlora.rs:105` |
| P3 | Metal training is a stub returning "requires host protocol (SP3-D)". | `vox-plugin-mens-candle-metal/src/training.rs:30,52` |
| P4 | `requires-tag = "nvidia-gpu"` / `"apple-silicon"` exists in the catalog and **nothing consumes it**. The capability concept is declared but unwired. | `catalog.toml:30,41` |
| P5 | The install tier system is a facade — `voxup/src/install.rs` never branches on tier; `full` never fetches `vox-ml-cli`; no tier can ship a GUI. | tier audit |
| P6 | Self-healing shells out to `cargo build` at runtime — requires a Rust toolchain and the repo on an end user's machine. | `plugin_heal.rs` |
| P7 | No plugin dependency or version resolution. `installed_version()` returns readdir order. | plugin host audit |
| P8 | No signature verification on plugin dylibs before `dlopen`. | deferred from the macOS audit |

P1 through P4 together mean: **the layer this architecture depends on does not
currently work for a single installed user on either GPU vendor.** That is the
critical path, ahead of any packaging work.

### 4.2 What the plugin system must become

Its purpose changes from *optional extras* to **the capability-resolution layer
that every package manager delegates to**. Concretely it must gain:

**(a) A capability probe with a declared contract.** `requires-tag` becomes
load-bearing. The host probes the machine once, emits a tag set
(`apple-silicon`, `nvidia-gpu`, `cuda-12`, `avx512`, `cpu-only`), and the
resolver selects plugins whose `requires-tag` is satisfied. This is the piece no
package manager can do, and the only reason Layer 2 exists.

**(b) Artifact sources that work off a clone.** Both GPU plugins must resolve to
checksummed release assets of *this* repo — the mechanism #472 already
identifies: `install_from_catalog` resolves `github:OWNER/REPO` to
`https://github.com/{gh}/releases/download/v{version}/{id}-v{version}-{triple}.zip`.
Repointing both to `github:vox-foundation/vox` and naming the assets to match is
sufficient. No new source kind. `local:` stays gated for development only.

**(c) Backend selection by capability, not by literal.** `cached_code_plugin("mens-candle-cuda")`
becomes a resolver call for the `MlBackend` extension point, which returns
whichever backend the tag set selected. This is the fix for P2 and it is what
makes the same core binary work on both an M-series Mac and a CUDA box.

**(d) Version lockstep with the core.** A plugin artifact is pinned to the core
version that produced it (`{id}-v{version}-{triple}.zip` already encodes this).
The host refuses to load a plugin whose declared version does not match the
running core — the ABI gate that `abi_stable` gives structurally, made explicit.

**(e) Signature verification before `dlopen`** (P8). Layer 2 is now the primary
delivery path for native code, so the deferred item becomes mandatory, not
optional.

**(f) No compiler at runtime** (P6). `plugin_heal` must fetch a verified
artifact, never invoke `cargo`. An installed user has no toolchain and no repo.

### 4.3 How this answers "CLI vs GUI vs cargo"

- **CLI only** — `voxlang` core. This is the default everywhere.
- **GUI (Axis)** — Layer 1, a *separate artifact*, not a plugin: a Tauri `.app`
  / `.msi` / `.deb` cannot be `dlopen`'d. Homebrew **cask** (not formula),
  winget `Vox.Axis`, apt `vox-axis`. It depends on the core at the same version.
  Note `vox-gui` is not currently a legal `binaries` value in any tier (P5).
- **cargo** — `cargo install voxlang` compiles the core from source with
  `default = []`. GPU features are *not* offered on the cargo path: a
  `--features cuda` install would require the CUDA SDK and a full recompile on
  the user's machine, and the Cargo Book is explicit that features must be
  additive, which mutually-exclusive backends are not. cargo users get the core
  and the same runtime plugin resolution as everyone else.
- **GPU/CPU** — never a package-manager decision. Layer 2, always.

---

## 5. Layer 3 — payload control

The disk audit measured what Vox actually writes. Users must be able to bound
each of these, and today mostly cannot.

| Path | Measured | Override | Gap |
|---|---|---|---|
| `~/.vox/bin` | **136 MB** (one staged daemon) | none | `VOX_HOME` is honored **nowhere** in the tree |
| `~/.vox/cache` (model catalog) | 492 KB, refreshed every 6 h | none | background network on by default |
| `<repo>/.vox/cache/graphify` | **138 MB** | none — path is only editable in `vox-graph-corpora.v1.yaml` | no disable, no relocate |
| `~/Library/Application Support/vox` | 6.6 MB | `VOX_DATA_DIR` | ok |
| HF model weights | 0 (never fetched here) | `HF_HOME` (hf-hub's own) | Qwen3-8B default is ~16 GB and that number appears nowhere in code or docs |
| Speech models | — | `VOX_ORATIO_SHERPA_MODEL_DIR` | **downloads automatically on first use**, no explicit command |

Required: honor `VOX_HOME` as the single root; add `VOX_GRAPHIFY_CACHE_DIR` and
a disable; make speech-model download explicit rather than a side effect of path
resolution; state the weight size before downloading it.

---

## 6. Versioning SSOT

Already built and verified this session (`vox-cli-ci/src/version_ssot.rs`,
`ssot_probe`): one workspace version, 9 path-dependency restatements, npm
packages, and the two-component hakari pin, bumped from one computed number with
a verified 127-line rewrite. Extended here to the package layer:

- `render_homebrew_formula` exists; add renderers for the winget manifest
  triple, `debian/control`, and the npm platform-package set.
- All read the same `checksums.txt` + workspace version. A release rewrites
  every manifest from one input, or fails.
- **MSI trap:** Windows Installer compares only the **first three** version
  fields. A `1.2.3.<build>` scheme makes every build "the same version" for
  upgrade detection. The build number must not live in the MSI version field.
- **The `rc.4748` problem:** every tag has been `-rc.<commit-count>`, so GitHub
  marks every release a prerelease and `/releases/latest` 404s — which is why
  both installers needed a workaround. `release-prepare.yml` computes a plain
  `vX.Y.Z` from conventional commits. Cutting one plain tag retires both
  workarounds.

---

## 7. CI/CD consolidation

Measured, and this is the load-bearing fact for the whole release story:

| Workflow | Runs | Successes |
|---|---|---|
| `release-installers.yml` | 5 | **0** |
| `release-gui.yml` | 10 | **0** (9 failed, 1 cancelled) |
| `bundle-release.yml` | 0 | — |

**Therefore the `.msi`, `.deb`, `vox-darwin.tar.gz` and `mens-candle-metal` zips
on the current release were hand-uploaded, not produced by any pipeline.** There
is no reproducible path from a tag to a release artifact today. Branch
protection requires exactly one check, and no release lane gates anything.

`vox-cli` is built six different ways across the workflows; only two pass
`heavy-retrieval`.

Target: one build matrix producing every Layer 1 artifact and every Layer 2
plugin zip, one checksum manifest, one renderer step emitting all package
manifests from it, and a release lane that is *required* by branch protection.
Trusted Publishing (OIDC, no long-lived tokens) is available on both crates.io
(GitHub Actions only; first publish must be manual) and npm (npm ≥ 11.5.1, with
automatic provenance).

---

## 8. Nomenclature — the collisions that force naming decisions

The sweep found 13 "two names for one thing" and 6 "one name for two things".
Most are cosmetic. These four are load-bearing because an external registry
already resolved them against us:

1. **`vox` on crates.io is taken** by `bearcove/vox` — active, 24 versions.
   The crate must be `voxlang`.
2. **`vox` in homebrew-cask is taken** by the VOX music player. Formula is
   `voxlang`; install fully-qualified. *(Fixed in the published tap README this
   session — it was still telling users `brew install vox`.)*
3. **`@vox/runtime-rn` is declared twice** — `clients/runtime-rn` and the CI stub
   `ci/expo-bundle-shim/vox-runtime-rn`. Publishing either breaks the other.
4. Bundle IDs split across `org.vox-foundation.*` (GUI) and `com.vox.*`
   (generated apps) for the same org.

Deliberate and fine as-is: `Vox` (product) / `Axis` (GUI) / `voxlang` (registry
token) / `vox` (command). Internal Latin names (Oratio, Clavis, Ludus, Codex,
Arca, Mens, Populi, Scientia, Graphify, Visus, Limes) stay internal — none should
appear in a package identity.

---

## 9. Audience demarcation — who is installing, and why it is a package identity

Added 2026-09-04 at the user's direction. This is the second axis of the design,
orthogonal to §2's three layers.

The same asymmetry from §1 settles it. **A separate package name is the one thing
all six ecosystems express identically.** Hardware capability is the thing none
of them can express. So:

- **Audience → a package identity** (Layer 1). Chosen at install time, by name.
- **Hardware → a plugin** (Layer 2). Chosen at first use, by probe.

Three audiences, and they want disjoint things:

| | A. Language user | C. Harness user | B. Contributor |
|---|---|---|---|
| Wants | writes `.vox`, compiles, runs | agentic CLI + Axis; does not care about the language | develops Vox itself |
| Identity | `voxlang` | `vox-cli` + `Vox.Axis` (GUI) | **none — not a package** |
| Gets | compiler, stdlib, LSP, tree-sitter, `.vox` association | orchestrator, models, plugin host, GUI | `git clone` + rustup + pnpm |
| Must NOT need | orchestrator, model catalog, GPU | the compiler surface | — |

**B is deliberately not a package.** A contributor needs the 126-crate workspace,
pinned Rust 1.96.0, pnpm, cargo-hakari, graphify and arch-check. No package
manager should ever be the contributor path, and trying to make one serve that
role is what produces the bleed described next.

### 9.1 Persona bleed — the defect class

Today, A- and C-shaped users are handed B's dependencies:

| Bleed | Evidence |
|---|---|
| A missing-feature hint tells the user to run `cargo install --path crates/vox-ml-cli --features populi` — a path that only exists in a clone | `crates/vox-cli/src/main.rs:115` |
| `plugin_heal` shells out to `cargo build` at runtime to repair a plugin | `plugin_heal.rs` (P6, §4.1) |
| The Metal plugin's catalog source is `local:crates/…` — a repo-relative path | `catalog.toml:42` |
| The error text for a missing GPU backend instructs `cargo build -p vox-ml-cli --features gpu,mens-candle-cuda` | `schola/train/run_train.rs:140` |

Every one of these assumes a Rust toolchain and a repo checkout. Neither A nor C
has either. This is the *same* root cause as the GPU problem in §4.1 — capability
work deferred to a compiler that is not present on the target machine — which is
why fixing Layer 2 properly fixes both.

**Rule:** no runtime code path may invoke `cargo`, and no user-facing remediation
may reference a repo-relative path, unless it is gated behind an explicit
"contributor mode" detected from the presence of the workspace itself.

---

## 10. Desktop integration and optionality

Requirement: an optional, configurable `/Applications` entry for Axis; optional
CLI-on-PATH; desktop shortcuts and proper app registration on all three OSes.

The capability exists in every target format. The gap is that **no CI lane has
ever produced a GUI bundle** (`release-gui.yml`: 0 successes, §11.1), so nothing
Tauri would generate has ever been generated.

### 10.1 Measured state

`crates/vox-gui/tauri.conf.json` is 36 lines, and its entire `bundle` block is
**two keys**: `icon` (five files, all present) and `externalBin`. Absent:
`targets`, `active`, `category`, `shortDescription`, `publisher`,
`fileAssociations`, and every `bundle.macOS.*` / `bundle.linux.*` /
`bundle.windows.*` key.

Consequences, in order of severity:

| # | Finding | Evidence |
|---|---|---|
| D1 | **No uninstall exists anywhere.** No `voxup uninstall` (only Install/Update/Proxy), no brew `uninstall` hook, no deb `postrm`. The only cleanup in the project is the MSI's PATH feature. | `crates/voxup/src/main.rs:24-37` |
| D2 | `~/.vox/toolchains/vox-<version>/` would accumulate **one full extraction per install, forever**, with nothing pruning. Note the directory **does not currently exist** — voxup has never completed an install on this machine; the measured 136 MB is `~/.vox/bin`. The defect is real in the code path, not yet observable on disk. | `install.rs:97` |
| D3 | PATH modification is **unconditional** — no `--no-modify-path`, which every packaging system and CI image expects. It also writes an undisclosed second copy at `~/.cargo/bin/vox`, not mentioned in the install output. | `shell.rs:8`, `install.rs:108,130` |
| D4 | `bundle.active` defaults to **`false`** (`#[serde(default)] pub active: bool`, tauri-utils 2.9.3) and is unset. Whether `tauri build` honours it is UNVERIFIED — tauri-cli was not available to inspect — but it is a latent reason for producing no installers. | `config.rs:1564` |
| D5 | No `bundle.category` / `shortDescription` → the generated Linux `.desktop` is **uncategorised and description-less**, and macOS gets no `LSApplicationCategoryType`. | derived from the empty bundle block |
| D6 | **Nothing registers `.vox`** with any OS. No `fileAssociations`, no UTI, no `MimeType=`, no WiX `Extension`. Double-clicking a `.vox` file does nothing anywhere. | repo-wide grep |
| D7 | The GUI bundle **embeds a full copy of the `vox` CLI** as a Tauri sidecar. Install only Axis and `vox` is not on PATH; install both and there are two large copies. The daemon lane resolves via PATH, so the two can be different versions — already anticipated by a `VersionMismatch { daemon_version, gui_version }` struct. | `tauri.conf.json:32`, `daemon.rs:83-96` |

Three documentation claims contradict the code: `how-to-code-signing.md:35`
instructs `xattr -cr /Applications/Vox.app`, a path **nothing in the repo ever
creates**; the same doc names notarization secrets (`APPLE_API_ISSUER`,
`APPLE_API_KEY`) that differ from the ones actually wired
(`APPLE_ID`/`APPLE_PASSWORD`/`APPLE_TEAM_ID`); and
`release-installers.yml:83` runs `cargo wix --no-build` with an inline comment
conceding the build step it depends on is missing.

Two of these were fixed while writing this spec: the Windows signing step
pointed at `crates/vox-gui/src-tauri/…`, a directory that does not exist, so
installers shipped unsigned; and `release-gui.yml` was the only workflow
installing pnpm unpinned.

### 10.2 Where each requirement is expressible

| Requirement | macOS | Linux | Windows |
|---|---|---|---|
| App in the launcher | **cask** → `/Applications` (a formula CANNOT do this; it installs to the Cellar) | `.desktop` in `.deb`/`.AppImage`, generated by Tauri | Start Menu shortcut from MSI |
| CLI on PATH, optional | formula (`voxlang`) — automatic, and separable from the cask | `.deb` → `/usr/bin/vox` | MSI feature `PATH` |
| Optional/configurable | two identities: cask + formula | `Suggests:` (never `Recommends:`) | **`ADDLOCAL=Core,GUI,PATH`** — a real feature tree with a built-in selection UI and post-install Add/Remove → Change |
| `.vox` file association | `CFBundleDocumentTypes` + UTI | `MimeType=` in the `.desktop` | MSI `Extension` table |

Two traps to design around:

1. **Homebrew:** `/Applications` requires a **cask**, not a formula. Axis and the
   CLI must therefore be two Homebrew identities regardless of anything else.
2. **MSI:** setting `ADDLOCAL` sets `Preselected=1`, which makes Windows Installer
   **skip the entire Condition table** — every `<Level Condition=…>`
   auto-detection is silently disabled. Since every silent and enterprise install
   passes `ADDLOCAL`, the explicit path must be fully self-sufficient and
   auto-detection can only ever be a convenience for the interactive installer.

---

## 11. CI lanes — what gates a merge, what runs locally, what runs nightly

Rewritten 2026-09-05 from measured fleet data, replacing the earlier
speculative version. Every number below is measured, not estimated.

### 11.1 The actual health of the fleet

47 workflows, 121 jobs. Classifying the last 30 runs of each, and separating
*cancelled* from *failed* — the distinction the earlier pass got wrong, because
raw success counts make concurrency supersessions look like failures:

| Class | Count | Meaning |
|---|---|---|
| **ALL CANCELLED** | 6 | Never completes. Superseded by the next push before finishing. |
| **BROKEN** | 9 | Completes, fails every time. |
| **ALL SKIPPED** | 1 | Runs, does nothing (`mobile-e2e-android`: 30/30 skipped). |
| **FLAKY** | 3 | 33–40% success. |
| **NEVER RUN** | 3 | Zero runs (incl. `release-prepare.yml`, not even registered with GitHub). |
| Healthy | ~21 | Real signal. |

**22 of 46 workflows produce no reliable signal.**

**Corrected 2026-09-05.** The first pass said 8 / 13 / 2 of 47 and *made the very
error §11.4 warns against*: four of the thirteen "BROKEN" were predominantly
**cancelled**, not failed — `mutation-nightly` (24c/5f), `qwen35-native-nightly`
(22c/7f), `harness-eval-nightly` (23c/6f), `vox-mental-tracker` (23c/3f). Two of
the eight "ALL CANCELLED" were wrong too: `mobile-e2e-android` is 30/30 *skipped*
and `mobile-eas-build` is mostly skipped. And the denominator is 46 files, not 47
— `gh workflow list` returns 47 because it counts two dynamic pseudo-workflows.
Stating the rule is not the same as applying it.

The ALL CANCELLED class is the important one, and it is the direct measurement
of the feedback-latency complaint: `ci.yml` (28 cancelled), `compile-matrix`
(28), `cr-l-gates` (28), `gui-cross-build` (28), `cr-l8-corpus-feedback` (27),
`docs-quality` (27), `mobile-eas-build`, `mobile-e2e-android`. These are not
broken. **They are slower than the rate at which commits arrive**, so every run
is superseded before it can finish. Adding more checks to this lane makes the
problem worse, not better.

BROKEN: `ci-fallback-hosted` (16 failures), `docs-deploy` (26), `link_checker`
(30), `mobile-e2e-ios` (30), `scorecard` (30), `setup-e2e` (30), `release-gui`
(9), `release-installers` (5), `mutation-nightly`, `qwen35-native-nightly`,
`harness-eval-nightly`, `deploy-telemetry`, `vox-mental-tracker`.
FLAKY: `release-binaries` 40%, `coolify-eval-sync` 40%, `version-tag-guard` 33%.
NEVER RUN: `bundle-release`, `pm-provenance-verify`.

### 11.2 Why latency, not cost, is the constraint

Hosted runners are **free** for public repositories — measured, not assumed: a
recent run reports `"billable": {"UBUNTU": {"total_ms": 0}}`. So self-hosting
saves nothing on money. What it buys is queue time, and that is the whole
problem:

| Job | Old fleet (wall) | Local runner (measured) |
|---|---|---|
| Cache warmup + path filter | 122 min median | **4 min** |
| Tests (nextest + llvm-cov) | 20 min exec / ~122 min wall | **4 min** |
| Fresh clippy + rustdoc | — | **0 min** |
| Audits | 6 min exec | **1 min** |

On the one historical run that was ever assigned a runner, **248 of 288 minutes
were queue and ~40 were work**. The local runner turns a 2-hour wall time into
~4 minutes: roughly 30x on the metric that actually matters. That is what moves
the ALL-CANCELLED workflows back under the push cadence.

### 11.3 The split

**Local (developer hardware) — everything latency-critical:**
the whole PR gating lane. setup, guards, lints, compiler gates, tests, audits.
These are all single-digit minutes locally and two-hour wall times hosted.

**Remote (GitHub-hosted) — breadth, trust, and anything local cannot do:**
- Windows and macOS builds. A Linux container cannot produce them, and `.actrc`
  deliberately refuses to fake it rather than emit a false green.
- x86_64 fidelity, when the local runner is arm64.
- The required status check itself. It must be fleet-independent, or a merge is
  blocked whenever the developer's machine is off.
- Fork PRs — no untrusted code on developer hardware, ever.
- Release and provenance builds, for reproducibility.

**Nightly — no latency pressure:**
mutation testing (95 min median, 407 max), benchmarks, the full cross-platform
matrix, GUI bundling, plugin artifacts, and a dated prerelease so the release
path is exercised daily instead of first-run-at-tag-time.

**Per-developer model:** contributors run
`REPO=<them>/vox scripts/ci-runner-local.sh` against **their own fork**. Their
PRs run on their hardware; upstream is never exposed to their code; compute
cost sits with whoever incurred it. Ephemeral by default (one job per
container) because the upstream repo is public.

### 11.4 A correction, and the rule it implies

An earlier draft called `cross-platform-check` "the fastest honest signal" and
recommended promoting it straight into branch protection. Checked properly it
is 6 success / 1 failure / **22 cancelled**. Excluding supersessions that is
6/7 ≈ 86%, so the recommendation survives — but only because the cancellations
were concurrency, not flakiness, and the first pass did not distinguish them.

**Rule: never promote a workflow to required on its raw success rate.** Split
cancelled from failed first. A lane that is 0/30 because it is always
superseded needs to be made faster; a lane that is 0/30 because it always fails
needs to be fixed or deleted. Those are opposite remedies and the raw number
cannot tell them apart.

### 11.5 Gaps — things nothing currently checks

- **Hermeticity.** Four `vox-orchestrator-mcp` `chat_tools::chat::*` tests mock
  the LLM transport (wiremock) but depend on an ambient model catalog, so they
  pass on a developer machine and fail on any clean runner. Found the moment CI
  first ran. Nothing gates tests against machine-state dependence.
- **Version SSOT drift** is checked only in `release-prepare.yml`
  (`workflow_dispatch`, never run), not in `ci.yml` — despite the module doc
  claiming drift is "a CI failure rather than a release surprise."
- **The Homebrew formula** is referenced by no workflow; its pinned version and
  three SHA-256s rot on every release.
- **No nightly artifact or prerelease** exists. Every release artifact is
  tag-triggered, so a tag is the first time those lanes ever execute — which is
  why `release-installers` is 0/5 and `bundle-release` has never run.
- **No release lane gates anything.** Branch protection requires exactly one
  check.
- **Toolchain pinning.** 45 sites use `dtolnay/rust-toolchain@stable`, which
  `rustup override`s and therefore ignores the pinned 1.96.0. CI compiles on
  floating stable while releases compile on 1.96.0, and no cache key includes
  the toolchain, so every stable bump silently poisons every cache.
- **9 duplicate `cargo build -p vox-cli`** invocations across workflows,
  byte-identical output, ~2.3 runner-hours per broad PR. `ci.yml` already solves
  this internally via artifact upload; the duplication is across files.

## 12. Toolchain and version SSOT, and the enforcement that makes it stick

### 12.1 What is actually true today (corrected)

An earlier draft of this spec, following the CI audit, claimed CI compiles on
floating stable while releases compile on the pin. **That is wrong**, and the
correction matters because it changes the severity. Measured from a real run:

```
info: note that the toolchain '1.96.0-aarch64-unknown-linux-gnu' is currently
      in use (overridden by ...)
[Build step] info: syncing channel updates for 1.96.0-aarch64-unknown-linux-gnu
```

`rust-toolchain.toml` wins for every cargo invocation inside the repo, so
`dtolnay/rust-toolchain@stable` never changed what compiled the code. Its real
cost is narrower: 45 sites install a stable toolchain that compiles nothing
(1.98.1 at time of writing), and they install `rustfmt`/`clippy`/`llvm-tools`
for *stable* rather than for the pinned version, so rustup re-downloads the
correct components at build time. Wasteful, not corrupting.

The genuine defects are elsewhere:

- **The pin is a `.0`.** `rust-toolchain.toml` says `1.96.0`, and **1.96.1
  exists** — i.e. upstream shipped a patch for it. Latest stable is 1.98.1.
- **Cache keys omit the toolchain.** Keys are
  `${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}`, so changing
  `rust-toolchain.toml` does not invalidate any cache.
- **The version is restated in 8 live places**, and the existing guard checks 3.

### 12.2 The restatements

| # | Location | Form |
|---|---|---|
| SSOT | `contracts/toolchain/workspace-toolchain.v1.yaml` | `versions.rust: "1.96.0"` |
| 1 | `rust-toolchain.toml:2` | `channel = "1.96.0"` |
| 2 | `Cargo.toml` | `rust-version = "1.96"` (major.minor) |
| 3 | `Dockerfile:4` | `FROM rust:1.96.0-slim-bookworm` |
| 4 | `Dockerfile.ci-runner:19` | `ARG RUST_VERSION=1.96.0` |
| 5 | `infra/ci-runner/Dockerfile:25` | `ARG RUST_VERSION=1.96.0` |
| 6 | `contracts/distribution/profiles.v1.yaml:8` | `rust_version: "1.96.0"` |
| 7 | `contracts/channels/stable.toml:9` | `min_rust = "1.96.0"` |
| 8 | `crates/voxup/src/profiles.rs:105` | embedded YAML fixture |
| — | 45 workflow sites | `dtolnay/rust-toolchain@stable` (contradicts all of the above) |

The existing "Toolchain SSoT Drift Guard" (`ci.yml:788`) covers only rows 1 and
2. It also has two structural problems: it uses `grep -oP`, which is GNU-only
and therefore fails on macOS and under `act` on a developer machine; and it
lives in `ci.yml`, which never completes (§11.1), so it effectively never runs.

### 12.3 Design

**One SSOT, already chosen.** `contracts/toolchain/workspace-toolchain.v1.yaml`
is the right file — it exists, it is already the guard's reference, and it
already carries `node`, `pnpm` and `cuda` alongside `rust`, so it generalises
past this one problem. Nothing else may state a toolchain version by hand.

**Rows 1-8 become generated, not authored.** Extend `ssot_probe` — which
already rewrites 13 restatements of the *Vox* version and verified a 127-line
bump — to cover the toolchain rows. One command bumps; a no-arg run is the
drift check. This reuses a mechanism that already works rather than inventing
a parallel one.

**The 45 workflow sites collapse to zero restatements** via a composite action,
`.github/actions/setup-rust/action.yml`, which reads the SSOT, installs exactly
that toolchain with the components asked for, and configures the cache with a
key that **includes the toolchain version**. Every site becomes
`uses: ./.github/actions/setup-rust`. This fixes the pin, the bump cost, and
the cache-key bug in a single move, and it is the same "one mutable core,
thin call sites" shape asked for on the package-manager side.

### 12.4 Enforcement rules

Rules are worth only as much as the thing that checks them, so each is paired
with its check and each check must be runnable locally.

1. **No `.0` toolchain pins.** A `x.y.0` Rust release is the first cut of a
   train; `1.96.1` existing is the proof for the current pin. The SSOT
   validator rejects `versions.rust` matching `^\d+\.\d+\.0$`.
2. **No hand-written toolchain versions.** A lint fails any workflow using
   `dtolnay/rust-toolchain@stable`, `@nightly`, or a literal version, instead
   of `./.github/actions/setup-rust`. Also fails any Dockerfile `ARG
   RUST_VERSION=` or `FROM rust:` that disagrees with the SSOT.
3. **Cache keys must include the toolchain.** A lint fails any `actions/cache`
   key that references `Cargo.lock` without also referencing the toolchain.
4. **Drift is a PR failure, not a release surprise.** Both the toolchain and
   Vox-version drift checks move out of `ci.yml` into a fast always-completing
   lane. Today `version_ssot` drift runs only in `release-prepare.yml`
   (`workflow_dispatch`, never executed) despite its module doc claiming
   otherwise.
5. **Portable checks only.** No `grep -oP`, no GNU-only flags, in any guard.
   If a developer cannot run the guard on macOS, it is not a guard.
6. **Release tags are plain semver.** Every tag so far has been
   `-rc.<commit-count>`, which marks every GitHub release a prerelease and is
   the root cause of the `/releases/latest` 404 both installers work around.

### 12.5 Upgrade target

`1.96.0` -> a non-`.0` release. Two candidates:

- **1.96.1** — the patch for the current pin. Minimal risk, satisfies the rule.
- **1.98.1** — current stable, also non-`.0`, two minors ahead.

`rust-version` (MSRV) is a *floor* and can stay at `1.96` while the toolchain
moves, so the choice is about build risk, not consumer compatibility. The
decision must be evidence-based: build the workspace on the candidate before
adopting it, not after. Do not infer from "CI already ran stable" — §12.1 shows
it did not.

### 12.6 Package-manager renderers

Already built and verified (§6): `ssot_probe` rewrites the workspace version, 9
path-dependency restatements, the npm package versions and the two-component
hakari pin from one computed number — 127 lines, `cargo check` exit 0.

The last mile is renderers, all reading the same two inputs (the workspace
version and the release `checksums.txt`):

| Endpoint | Renderer | Status |
|---|---|---|
| Homebrew formula (`voxlang`) | `render_homebrew_formula` | **exists** |
| Homebrew cask (Axis) | — | to build |
| winget manifest triple (version / installer / defaultLocale) | — | to build |
| `debian/control` + changelog | — | to build |
| npm parent + per-triple platform packages | partial (`npm_versions`) | to extend |
| WiX `main.wxs` product version | — | to build |

Traps already identified: the MSI three-field version comparison (§6); winget's
inability to pin a dependency to an exact version (`MinimumVersion` only); and
npm's requirement that platform packages be pinned **exactly** by the parent and
published *before* it, or every install breaks.

---

## 13. Sequencing

Ordered by dependency, not by size. Each stage is verifiable without a public
release.

1. **Merge sequencing.** #472 (31 commits, MERGEABLE, 0 of them in `main`)
   overlaps 12 files with #477. #473 stacks on #472's branch, which is why it
   reads CONFLICTING. Land #472 → rebase #473 → rebase #477, or the installer
   and plugin fixes get applied twice.
2. **Plugin critical path (P1–P4, §4.1).** Both GPU backends are currently
   unreachable for any installed user. Nothing else in this spec is deliverable
   until the plugin layer works, because §2 makes it load-bearing for every
   ecosystem.
3. **De-bleed the personas (§9.1).** Remove every runtime `cargo` invocation and
   every repo-relative remediation. Shares a root cause with stage 2 and should
   land with it.
4. **Nightly lane (§11).** Deliberately ahead of the release work: a nightly that
   builds the real artifacts is what turns the release path from never-succeeded
   into exercised-daily, and it is the cheapest way to make stages 5–7
   verifiable without publishing anything.
5. **Version SSOT → the remaining renderers (§12).**
6. **Layer 1 identities (§3) and the audience split (§9)** — `voxlang` formula,
   Axis cask, `vox-cli`, and the winget/apt equivalents.
7. **Desktop integration (§10)** — bundle targets, `.desktop`, `/Applications`,
   `.vox` association, optional PATH.
8. **Payload controls (§5)** — honor `VOX_HOME`, bound the 138 MB graphify
   cache, make model downloads explicit.
9. **Blocking-lane trim (§11)**, once the nightly has proven it catches what the
   moved jobs caught.

## 14. Open decisions requiring the user

- **Public release approval.** Everything above is verifiable without it, but
  `crates.io` Trusted Publishing requires one manual first publish, and winget
  requires a PR into `microsoft/winget-pkgs` with manual moderation.
- **`TAP_TOKEN`** secret, to automate the tap update from the release lane.
- **ACI shell-backend owner** (carried over, unrelated to distribution).
- Whether to claim the `@vox` npm scope now, before someone else does.

---

## 15. Sources

Ecosystem constraints measured 2026-09-04 against: `cargo 1.98.0`,
`npm 11.19.0`, `Homebrew 6.0.21`, winget manifest schema `1.30.0`
(`microsoft/winget-cli`, *not* winget-pkgs — that path 404s), Debian Policy
§2.2, §5.6, §7.1–7.5, §8.5, `apt-pkg/init.cc`, Microsoft Learn Feature/Condition
tables, WiX v4 schema. Repo facts measured against `main` at
`crates/vox-plugin-catalog/catalog.toml`, `crates/voxup/src/install.rs`,
`crates/vox-config/src/paths.rs`, and `gh run list`.

Explicitly UNVERIFIED and flagged as such: npm public-scoped pricing; npm and
crates.io hard size limits; Debian freeze durations; SmartScreen reputation
mechanics; current cuDNN package naming.

---

## 16. Build contention across agents, IDEs and sessions

Prompted by saturating the development machine while writing this spec: three
heavy workloads at once (a containerised CI job on 10 cores, plus two
concurrent host `cargo` builds) drove load average to 15.4 on 18 cores. Nothing
deadlocked; everything simply got slow, and the cargo package-cache lock
serialised work opaquely.

### 16.1 The tool already exists and has never been switched on

`crates/vox-cargo-shim` + `crates/vox-build-queue` are a complete, well-designed
build broker. Its own documentation states the exact problem: *"When many agents
/ IDE tabs / git hooks build across many worktrees on one machine…"*.

- A binary **literally named `cargo`**, placed on PATH ahead of the rustup proxy.
  Intercepts `build`/`test`/`check`/`clippy`/`run`/`bench`, acquires one of N
  slots, runs the real cargo (so `rust-toolchain.toml` and `+toolchain` still
  work), records a metric, releases.
- **Machine-wide N-slot cross-process file semaphore.** Default cap =
  logical cores / 3 clamped to [2, 8]; `VOX_BROKER_MAX_CONCURRENT` overrides.
- State at `~/.vox/build-broker/`, deliberately **outside any repo**, so
  concurrent agents' `git clean` / checkout cannot wipe it.
- **Already auditable and already global**: `metrics.jsonl` (one record per
  build) and `broker.log` (one line per build, every worktree, one file), with
  `wait=`, `ahead=`, `cap=`, `coalesce=`, `exit=`.
- Daemonless; falls through to real cargo on any error, so it is never a hard
  dependency. A `VOX_BROKER_DEPTH` guard aborts at depth >= 2 so a
  misconfiguration cannot fork-bomb.
- A deliberate **evidence gate**: the coalescing daemon is deferred until
  `would_coalesce` data justifies it.

**`~/.vox/build-broker` does not exist on this machine.** The broker has never
run. `which -a cargo` returns only the rustup proxy.

### 16.2 Why it never activated

| # | Gap | Evidence |
|---|---|---|
| B1 | Never installed. Install is a manual three-command ritual in a contributors doc. | `docs/src/contributors/build-broker-usage.md` "Install (per machine)" |
| B2 | **Activation guidance is Windows-only** — `terminal.integrated.env.windows.PATH`, `…\.vox\build-broker\bin`, `cargo[.exe]`. Zero mentions of `.osx`/`.linux`, `.zshrc`, `.bashrc`, or `export PATH`. macOS and Linux developers have no documented activation path. | same doc, measured: 0 matches |
| B3 | No verification — no `vox doctor` check for whether the shim precedes `~/.cargo/bin` on PATH. | no `broker` reference under `diagnostics/` |
| B4 | No enforcement — agents and CI call `cargo` directly and bypass it silently. | this session did exactly that |
| B5 | Workspace-excluded (its bin must be named `cargo`), so it is not built by default and not covered by workspace CI. It rots. | `Cargo.toml:6` |

B2 is the same Windows-era pattern as the 16-bit GUI icons and the
`.task.xml` / `.cmd` runner scheduling glue.

### 16.3 The limitation that config alone will not fix

The semaphore is a **filesystem** lock under `~/.vox/build-broker/slots/`. A
containerised CI runner has a separate mount namespace and never sees it, so the
broker governs host builds only. Today's contention was ~10 container cores plus
two host builds; the broker would have capped only the latter.

Two options, both real work rather than configuration:
- bind-mount `~/.vox/build-broker` into the runner container so container and
  host share one semaphore; or
- give the container a fixed budget subtracted from the host cap, and have the
  runner supervisor pass `VOX_BROKER_MAX_CONCURRENT` accordingly.

The first is more correct (one machine, one budget); the second is simpler and
does not couple the container to host state.

### 16.4 Enhancements

| Goal | Change |
|---|---|
| Works on any machine | Install the shim from the standard dev bootstrap, not a manual ritual |
| Multiple IDEs and sessions | Cross-platform activation: shell profiles plus `terminal.integrated.env.osx` / `.linux` |
| Auditable, available to all | Surface the existing log as a command (`vox ci build-queue`) rather than `tail -f`; the data is already correct and already global |
| Enforced, not optional | A `vox doctor` check for shim-precedes-cargo, and a lint so agent-authored commands cannot bypass it |
| Covers CI | §16.3 |
| Does not rot | A CI lane that builds and tests the workspace-excluded shim |

The important point: **none of this is new infrastructure.** The queue, the
semaphore, the audit log and the fork-bomb guard all exist and are sound. What
is missing is installation, cross-platform activation, a visibility check, and
enforcement — which is the same failure mode as the plugin capability tags
(§4.1 P4) and the install tiers (§4.1 P5): a correct mechanism that nothing
actually invokes.
