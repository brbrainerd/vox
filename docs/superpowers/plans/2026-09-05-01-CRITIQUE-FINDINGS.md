# Critique Findings — six parallel audit tracks against the spec and the seven plans

**Date:** 2026-09-05
**Method:** six read-only agents, each auditing a different axis against the tree at `e88907bf8`.
**Status:** all findings triaged. Fixes applied to the INDEX and the plans are marked ✅; open items are marked ⬜ with an owner.

This file is the record of *why* the plans say what they say. When a plan looks
over-cautious, the reason is almost certainly here.

---

## The three findings that would have caused real damage

### 1. ✅ `main` goes red from P2 alone — the INDEX's core guarantee was false

`crates/voxup/tests/distribution_parity.rs:68` asserts
`contracts/toolchain/workspace-toolchain.v1.yaml`'s `versions.rust` **equals**
`contracts/distribution/profiles.v1.yaml`'s `rust_version`. Both are `"1.96.0"`.

P2 owns the toolchain contract and exists to change it. P7 owns
`profiles.v1.yaml`. P5 owns `crates/voxup/src/profiles.rs:105`, which carries the
same literal a third time. So the moment P2 merges alone, two workflows go red —
directly contradicting *"Every plan must leave `main` green on its own."*

**Fix:** those two version rows transfer to P2 at the point of the bump. P2 lands
the bump together with the one-line changes, or not at all.

### 2. ✅ P4's nightly would have published a genuinely public release

The standing constraint is *no public release on any channel*. P4 Task 5 said
"publish a dated prerelease… prereleases must stay drafts." That instruction is
unenforceable against the existing machinery:

`release-binaries.yml:296` uses `softprops/action-gh-release` with **no `draft:`
and no `prerelease:`**, triggered on `push: tags: v*`. It creates a public,
published, *latest* release. `bundle-release.yml` then fires on
`release: published` and attaches assets. `version-tag-guard.yml` would go red —
but it has no `needs:` edge, so it fails *beside* the live public release rather
than preventing it.

**Fix:** the nightly must not push a tag. `gh release create --draft --prerelease`
on a `nightly-YYYYMMDD` ref that does not match `v*`; add `draft: true` to
`release-binaries.yml`; add a PR lint asserting every `action-gh-release` step
sets `draft: true`. Verification is `gh release list` showing every row `Draft`.

Reassuring counter-finding: there is **no** `cargo publish`, `npm publish`,
`CARGO_REGISTRY_TOKEN` or `NPM_TOKEN` anywhere in CI. crates.io and npm are not
reachable from this repo today.

### 3. ✅ `voxup uninstall` could destroy a user's shell config and their vault key

Two unrecoverable losses, both silent:

- **`~/.zshrc`.** On a pristine macOS account voxup *creates* it (`shell.rs:26-57`)
  containing only its own PATH line. Weeks later it holds the user's aliases and
  shell init. "We created it, so delete it" destroys that. It is not in Trash and
  not in git.
- **`~/.vox/.vox-master-key`** — 32 bytes, the vault's fallback decryption key. It
  sits inside the directory an uninstaller is aimed at. Nothing in the plan
  prevented `remove_dir_all(home.join(".vox"))`; "reports what it will not remove"
  is a println, not a guard.

**Fix:** uninstall operates on an explicit allowlist (`~/.vox/bin`,
`~/.vox/toolchains`, `~/.vox/run`), never on `~/.vox` itself, never
`remove_dir_all` outside the list. Profile edits remove only the exact two-line
block, via write-to-temp + rename, after a timestamped backup. Refuse entirely if
`HOME` is unset — `paths.rs:141` falls back to `"."`, so an uninstall with no HOME
targets `./.vox` relative to the agent's CWD.

---

## Structural: the partition was unsound

### 4. ✅ A whole layer of generated SSOT artifacts had no owner

`ci.yml:250-278` regenerates, and `ci.yml:517` gates on, a set of committed files
that are *functions of the dependency graph*. Multiple plans are structurally
forced to regenerate them, and none owned them:

`contracts/ci/crate-graph.v1.json`, `crate-edges.allow.v1.json`,
`crate-layers.v1.json`, `crate-build-map.v1.json`, `fan-in-snapshot.v1.json`,
`crates/workspace-hack/Cargo.toml` (507 lines, depended on by ~116 crates),
`contracts/config/config-registry-baseline.txt`,
`crates/vox-config/src/config_registry.rs`.

`.config/hakari.toml` even documents a prior incident where an unrelated hakari
regeneration silently erased a deliberate exclusion.

**Fix:** new "serialized shared files" category — never hand-edit, resolve
conflicts by regeneration, listed explicitly in the INDEX.

### 5. ✅ P7 owned Cargo.tomls but not the source it must feature-gate

P7 owned `crates/vox-codegen/Cargo.toml` but the code needing `#[cfg]` is in
`crates/vox-codegen/src/…`. Same for `vox-config` (the edit is in
`resolve_egress.rs` and `lib.rs`) and `vox-cli` (Task 5 names
`src/main.rs:115`, which P7 did not own). **As written, P7 was unexecutable.**

**Fix:** P7 owns whole crates, not lone manifests.

### 6. ✅ P2 was told to edit a file P4 owns

P2 Task 4 said "delete the `grep -oP` guard". That guard is `ci.yml:792-794` —
P4's file. **Fix:** P2 supplies the replacement, P4 lands it.

### 7. ✅ The doctor-file protocol was wrong

The INDEX claimed "append-only, one `mod x;` line and one registration line".
Registration is actually **four regions**: the alphabetised `mod` block, the
ordered call sequence inside `run_checks` (where both plans append at the same
point — a genuine add/add conflict, and the order is semantically meaningful
because `tail::run` is deliberately last), `known_diag_ids()`, and the
`remediation_tests` module whose `known` list gates every remediation string.

**Fix:** P3 and P6 run in the **same tab**, which removes the shared file entirely.

---

## Correctness: claims that were wrong

| Claim | Reality |
|---|---|
| "45 sites use `@stable`" | **55** refs across 28 files: 45 `@stable` + **8 `@master` with a literal `1.96.0`** in release workflows. The 8 were assigned to no plan, and P4's own new lint would red them. |
| "47 workflows" | 46 files. |
| `requires-tag` — "nothing consumes it" | `plugin/info.rs:22` reads it (display only). The absolute phrasing would stall an executor who greps. |
| P1 Task 6 "create `resolve_bundle`" | `bundle_resolved()` already exists in `vox-plugin-catalog/src/lib.rs:72`, plus a private `resolve_bundle` in `build.rs`. Three spellings; the requested one keys on *tier*, the catalog knows *bundles*. |
| P1 Task 2 test: "no `[[plugin]]` has a `local:` source" | `nvml-probe` is also `local:` — **five of eleven** code plugins are, not two. The test as written fails after the task is complete. |
| P1 Task 2: install.rs "already resolves to `…/download/v{version}/…`" | **False.** `install.rs:229-237` interpolates the literal string `latest`, producing a URL that 404s regardless of repointing. |
| P1: `local:` is "gated behind `VOX_LOCAL_PLUGIN_FALLBACK`" | No such var exists. Only `VOX_NO_LOCAL_PLUGIN_FALLBACK`, an opt-*out* on a different path. The gate lives in an unlanded plan. |
| Spec §16.2 / earlier claims re 16-bit icons | Icons are **8-bit** — because they were fixed earlier in this effort. Not a live defect; do not re-fix. |
| P2 Task 1's rustup command | `--component rustfmt,clippy` is wrong; rustup takes `--component` once per component. |

---

## Verification theater — steps that would pass on a wrong change

- **P1 Task 2's reachability test** asserts the *repo* exists. `github:vox-foundation/vox` obviously exists, so it passes on a URL that 404s at download — the exact defect being fixed. Replace with an offline assertion on the resolved URL string, plus a separate `#[ignore]`d networked HEAD of the actual asset.
- **P1 Task 4's `Command::new("cargo")` grep** is defeated by `Command::new(cargo_bin())` or `env cargo`, and only covers `vox-ml-cli` while `vox run` shells to cargo from `vox-cli`.
- **P2 Task 1's `cargo check`** runs neither clippy nor rustdoc — and both are `-D warnings` here, with `unexpected_cfgs` at **deny**. `AGENTS.md:447` names toolchain-bump lint waves as the repo's *#1 perennial* and warns "cached clippy hides them". A two-train bump gated by `cargo check` proves almost nothing.
- **P3's `which -a cargo`** reads the *current* process's PATH; the installer edits a *profile*. It proves the export, not the edit.
- **P3's "two concurrent builds show `ahead>0`"** is a race in both directions. Pin `VOX_BROKER_MAX_CONCURRENT=1` and hold a slot deliberately.
- **P5's "assert the filesystem is clean"** is finding 3 written as a test — it would *drive* an agent toward `remove_dir_all`.
- **`cargo test -p X` exit 0** is not evidence: a crate whose tests live in a bin target prints `running 0 tests` and exits 0. Every Verification block now greps for `test result:` and fails on `0 passed`.

---

## Concurrency: what actually limits parallelism

- **Disk is the binding constraint.** `target/` is 139 GB and **per-worktree by design**. 7 worktrees ≈ 973 GB against 557 GiB free. The machine fills around agent 4, and `ENOSPC` leaves partially-written rlibs and a corrupt fingerprint DB — failures that look like source bugs.
- **`.cargo/config.toml` sets `jobs = 24`** on an 18-core box, already oversubscribed before colima's 12. Four agents = 96 rustc processes; the largest rlib here was 1.7 GB.
- **`~/.cargo/.package-cache` is one global mutex** across every cargo process regardless of worktree.
- **P3 installing a machine-wide `cargo` shim while other agents build** is the sharpest hazard: a never-before-executed binary named `cargo`, prepended to PATH, affecting every other tab. There is in-tree precedent for the confusion (`build_health.rs:3` documents a shim that printed a version while every real build aborted).

**Consequence:** 4 tabs, 2 build slots, separate `CARGO_TARGET_DIR` per tab,
`CARGO_INCREMENTAL=0`, and the broker installed in Tab 0 *before* anything else.

---

## Gaps: discussed but in no plan

| Item | Assigned to |
|---|---|
| `crates/vox-cli-ci/src/package_manifests.rs` and the four unbuilt renderers (cask, winget, `debian/control`, WiX) | ⬜ **P8** (new) |
| winget / apt / npm / crates.io Layer-1 identities | ⬜ **P8** |
| `@vox/runtime-rn` declared twice; bundle-id split `org.vox-foundation` vs `com.vox` | ⬜ P8 / P5 |
| The `-rc.<commitcount>` tag causing `/releases/latest` 404 — root cause of *both* installer workarounds | ✅ P4 |
| The 20 pre-existing test failures (11343 passed / 20 failed / 157 ignored across 912 binaries) | ✅ P4 — a required gate cannot be meaningful while 20 fail |
| **GPU end-to-end: nothing runs a backend on real hardware** | ✅ P1 |
| `crates/vox-plugin-cloud` also workspace-excluded — same rot as the shim | ✅ P1 |
| Windows-era `.task.xml` / `.cmd` scheduling glue | ✅ P4 |
| Conventional-commit enforcement — load-bearing, since `git cliff --bumped-version` derives the release version from commit subjects | ✅ P4 |
| `vox run --sandbox` claims isolation it does not provide | ⬜ P7 |
| GUI auto-update; Linux release signing; `aarch64-unknown-linux-gnu` target; `vox upgrade --rollback` | ⬜ P5 / P4 |
| ACI shell-backend owner — deferred a **third** time | ⬜ user decision |

---

## GPU delivery: P1 was necessary but not sufficient

Track 6 traced the full path and found nine failure points, four of which fire
before any GPU code is reached. The three most consequential:

- **Nothing builds or publishes a GPU cdylib anywhere in the repo.** `release_build.rs`
  packages exactly two executables. So P1's repointing targets an asset that does
  not exist — and P1 cannot be verified on its own, contradicting the INDEX rule.
- **Both GPU crates have `default = []`**, so a default-features build produces a
  **CPU cdylib wearing a GPU name**, and nothing records which features an artifact
  was built with. This can ship "GPU support" that silently delivers no acceleration.
- **CUDA is gated at compile time in the *host*** (`run_train.rs:136`), so a released
  `vox-ml-cli` can never take the CUDA branch even with the plugin correctly
  installed. Replacing the remediation string does not fix the gate.

Plus: `Plugin.toml` already declares `native-libs = [cudart >= 12.0, cublas]` and
`vox plugin doctor` prints `"(presence not verified)"` verbatim. And Metal
training is an unconditional bail pending an unimplemented host protocol — so
Metal *delivery* is testable on this machine but Metal *training* is not, for
reasons unrelated to delivery.

---

## Standards compliance

The plans were written against `superpowers` 6.3.0, but **the repo vendors a stale
copy** at `assets/skills/writing-plans/SKILL.md` missing Task Right-Sizing, the
`Spec:`/`Global Constraints` header rows, and the mandatory `Interfaces:` block.
An executor reading the vendored copy applies a weaker standard than the plans
were written to. ⬜ One-file sync, before any tab starts.

Measured against 6.3.0: only **P1** had any `Interfaces:` block, and only **one
step in seven plans** (P1 Task 1 Step 2) had a command *and* an expected output.
Six of seven plans contained **zero code blocks**. The research quality was high;
the executability was not.
