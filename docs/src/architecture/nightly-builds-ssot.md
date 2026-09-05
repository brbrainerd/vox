---
title: "Nightly Builds"
description: "How the nightly release-pipeline exercise runs, and why the repo has no automatic public nightly release."
category: "Architecture SSOTs"
status: "current"
---

# Nightly Builds

## What runs

[`nightly-artifacts.yml`](../../../.github/workflows/nightly-artifacts.yml)
runs on `schedule:` + `workflow_dispatch:` only — **never** on a tag push —
and exercises the same build/package/smoke-test steps as
`release-binaries.yml` / `release-gui.yml` / `release-installers.yml` every
day, using a ref shaped `nightly-YYYYMMDD` (e.g. `nightly-20260904`) that
does not match the `v*` pattern those tag-triggered workflows fan out on.
Its single release-writing step is `gh release create ... --draft
--prerelease` (idempotent: a second run the same day updates the existing
draft's assets instead of creating a duplicate). See the workflow's own
safety header for the full invariant list.

**This is deliberately not a public release.** A draft release is visible
only to repo collaborators with write access; it never fires `release:
published`, so `bundle-release.yml` cannot fan out from it, and — because a
draft release does not create the underlying git tag ref until published —
no tag-push event fires either. Nothing in the workflow calls `gh release
edit`/`publish`, deletes a release, or pushes a tag.

## History: the removed `nightly-tag.yml`

An earlier version of this pipeline (`nightly-tag.yml`, removed) computed a
tag shaped `v<workspace-version>-nightly.<build-number>` and pushed it with a
`NIGHTLY_TAG_TOKEN` PAT specifically so the push would cross GitHub's
loop-prevention rule (a `GITHUB_TOKEN`-authored tag push does not trigger
other workflows' `on: push` triggers, but a PAT-authored one does). Because
that tag matched `v*`, pushing it fanned out to `release-binaries.yml`,
`release-installers.yml`, and `release-gui.yml` exactly like a real version
tag — and those workflows' publish steps create a genuinely public release
unless every one of them sets `draft: true`. `release-installers.yml`'s
publish step did not, so once `NIGHTLY_TAG_TOKEN` was configured this
pipeline would have auto-published a public release every night — the
outcome this repo has repeatedly been told never to automate. `nightly-tag.yml`
was deleted for this reason, and `crates/vox-cli-ci/src/release_draft_guard.rs`
was extended to also scan `run:` script bodies (not just `uses:` steps) for
an unguarded `gh release create` so an equivalent script-based path can't
reopen the same hazard silently.

## Using a nightly build locally

There is no public nightly release to fetch by tag. To try a nightly
exercise build, pull the artifact directly from the workflow run (requires
repo read access):

```bash
gh run download --repo vox-foundation/vox \
  $(gh run list --repo vox-foundation/vox --workflow nightly-artifacts.yml -L 1 --json databaseId -q '.[0].databaseId') \
  --name nightly-x86_64-unknown-linux-gnu
```

or browse the draft release's assets directly (visible to collaborators
only) via `gh release view nightly-YYYYMMDD --repo vox-foundation/vox`.

`voxup install --tag <tag>` still works for any *real, published* release
tag (e.g. a release-candidate `v0.6.0-rc.4`) — `/releases/latest` excludes
prereleases by definition, so `--tag` is the way to fetch one. It has no
nightly tag to target today.

## Non-goals

- No automatic, unattended creation of a public GitHub release from any
  schedule or cron trigger — see `crates/vox-cli-ci/src/release_draft_guard.rs`
  and `docs/src/adr/` for the standing policy this protects.
- This does not change what `voxup install` / `voxup update` do by default
  (still latest stable via `/releases/latest`); `--tag` is opt-in.
- No automatic local-machine nightly polling/auto-update.
