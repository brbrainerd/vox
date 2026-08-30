---
title: "Nightly Builds"
description: "How the nightly channel is tagged, built, published, and consumed locally to cut down on per-worktree cargo build overhead."
category: "Architecture SSOTs"
status: "current"
---

# Nightly Builds

## What runs

[`nightly-tag.yml`](../../../.github/workflows/nightly-tag.yml) cuts a
prerelease tag from `main` daily (and on manual `workflow_dispatch`) when
there are new commits since the last one. It does not build anything itself
— it only creates and pushes a tag shaped
`v<workspace-version>-nightly.<build-number>` (e.g. `v0.6.0-nightly.4821`).
`release-binaries.yml`, `release-installers.yml`, and `release-gui.yml` all
already trigger on `push: tags: v*`, so pushing that tag is the entire
integration: no build logic is duplicated for the nightly channel.

`build-number` is `git rev-list --count HEAD` on `main` at tag time — the
same monotonic-counter convention used for `-rc.<N>` release-candidate tags
(see [distribution-ssot.md](distribution-ssot.md)). `version-tag-guard.yml`
already accepts any `-<suffix>` after the bare version (it strips everything
from the first `-` before comparing to `Cargo.toml`), so `-nightly.N` needed
no guard change.

Old nightlies are pruned automatically, keeping the newest 5 tags/releases,
so the release list does not grow unbounded.

## The GITHUB_TOKEN cascade problem

A tag pushed using the workflow-default `GITHUB_TOKEN` does **not** trigger
other workflows' `push` events — this is GitHub's built-in loop-prevention
rule, not a bug. Without a real PAT, `nightly-tag.yml` would successfully
create a tag that never triggers a build.

`nightly-tag.yml` therefore reads a `NIGHTLY_TAG_TOKEN` repo secret (a PAT
with `repo` scope) to push the tag. This mirrors the existing
`SSOT_AUTOREGEN_TOKEN` pattern (see `AGENTS.md` §"Do not hand-regenerate
SSOT after a merge") — same problem, same shape of fix. **Until a repo admin
adds that secret, the workflow runs on schedule, logs a warning, and skips
tagging** — it does not silently no-op without saying so.

## Using a nightly build locally

`voxup install --tag <tag>` fetches an exact release tag instead of
`/releases/latest` (which excludes prereleases by definition, so nightlies
are otherwise unreachable through the normal install path):

```bash
voxup install --tag v0.6.0-nightly.4821
```

This goes through the same download-and-verify path as a stable install
(fetch `checksums.txt`, verify SHA-256, extract) — see
[`crates/voxup/src/channel.rs`](../../../crates/voxup/src/channel.rs)'s
`fetch_by_tag` and [`install.rs`](../../../crates/voxup/src/install.rs).
Each tag gets its own cache dir under `~/.vox/toolchains/vox-<version>`
(nightly's dotted `version` includes the `-nightly.N` suffix), so a nightly
install never collides with or overwrites a stable one.

**Why this matters for local build overhead:** across multiple worktrees of
this repo, a full `cargo build -p vox-cli --profile dist` is expensive
(fat-LTO, single-digit minutes to tens of minutes per worktree's own
`target/`, which is per-worktree by design — see AGENTS.md's Perennial Bug
Patterns). For most local testing that only needs a working `vox` binary —
not a build of the exact working-tree commit — pulling the latest nightly
via `voxup install --tag <tag>` is faster than a from-source build and
avoids growing a `target/` in every worktree. Use a real local build when
you need to test the actual commit you're editing (the whole point of this
plan's other work is verifying that path works too); use a nightly install
when you just need *a* working `vox` to run something else against.

Find the latest nightly tag: `gh release list --repo vox-foundation/vox -L 20 | grep nightly` (or check the Releases page — nightly releases are marked prerelease).

## Non-goals

- This does not change what `voxup install` / `voxup update` do by default
  (still latest stable via `/releases/latest`); `--tag` is opt-in.
- No automatic local-machine nightly polling/auto-update — pulling a nightly
  is always an explicit, one-shot `voxup install --tag <tag>` command.
