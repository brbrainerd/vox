---
title: "Binary release artifact contract"
description: "SSOT for GitHub Release binary names, archive layout, checksum manifest, and supported host triples for `vox-bootstrap` and `vox ci release-build`."
category: "CI & Quality"
last_updated: "2026-03-25"
training_eligible: true

schema_type: "TechArticle"
---

# Binary release artifact contract

This document is the **authoritative contract** for **release binaries** (names, archives, **`checksums.txt`**) between:

- **`crates/vox-install-policy`** (Rust SSOT for supported triples, default GitHub org/repo, and `cargo install --locked --path …` argv shared by bootstrap / `vox upgrade` / compliance guards),
- [`vox ci release-build`](../reference/cli.md) (packaging in CI / locally),
- [`.github/workflows/release-binaries.yml`](../../../.github/workflows/release-binaries.yml) (tag-triggered publish),
- [`vox-bootstrap`](../reference/cli.md) (binary-first install),
- **`vox upgrade --source release`** (operator self-update; same manifest verification).

The **`vox upgrade --source repo`** lane rebuilds from a local checkout and does **not** consume this checksum manifest (trust model = your git ref + Cargo lock in-tree).

## Supported release targets

These triples are built and published for each release **tag** `v*`:

| Target | Notes |
|--------|--------|
| `x86_64-unknown-linux-gnu` | Linux x86_64, glibc |
| `x86_64-pc-windows-msvc` | Windows x86_64 |
| `x86_64-apple-darwin` | macOS Intel |
| `aarch64-apple-darwin` | macOS Apple Silicon |

`vox-bootstrap` maps the **compile-time host** to one of these triples. If no matching asset exists published for that tag, binary install **fails** and the installer falls back to **`cargo install --locked --path crates/vox-cli`** (requires repo root; uses the workspace lockfile).

## Asset file names

For a Git tag `<tag>` (for example `v1.2.3`), each artifact **basename** is:

- **CLI (Unix)**: `vox-<tag>-<target>.tar.gz`
- **CLI (Windows)**: `vox-<tag>-<target>.zip`
- **Bootstrap (Unix)**: `vox-bootstrap-<tag>-<target>.tar.gz`
- **Bootstrap (Windows)**: `vox-bootstrap-<tag>-<target>.zip`

Example: `vox-v1.2.3-x86_64-unknown-linux-gnu.tar.gz`

## Archive contents

| Platform | Single entry name |
|----------|-------------------|
| Unix archives | `vox` (executable) |
| Windows zip | `vox.exe` |
| Unix bootstrap archives | `vox-bootstrap` (executable) |
| Windows bootstrap zip | `vox-bootstrap.exe` |

No nested directory prefix inside the archive for the executable entry.

## Checksums

- **Authoritative** `checksums.txt` for end users is produced in the **publish** job by hashing each uploaded release asset and emitting **basename-only** lines:

  ```text
  <sha256_hex><two_spaces><basename>
  ```

- Per-job `dist/checksums.txt` from `release-build` is for **local debugging** only; release downloads should use the root `checksums.txt` attached to the GitHub Release.

## Download URLs (bootstrap)

- Tagged asset: `https://github.com/vox-foundation/vox/releases/download/<tag>/<basename>`
- Latest asset: `https://github.com/vox-foundation/vox/releases/latest/download/<basename>`

**`vox upgrade --provider http`:** when you mirror this layout on another host, set **`VOX_UPGRADE_BASE_URL`** to `https://<host>/<org>/<repo>/releases` (no trailing slash). **`vox upgrade`** still requires the same **`checksums.txt`** and archive layout as this contract; use an explicit **`--version`** / tag for static mirrors (no listing API).

The **basename** for `latest` must match the **actual** filename on the latest release (same tag in the name as `tag_name` on that release). Installers **must not** invent a fake `vox-latest-…` filename.

## Smoke checks

Before artifacts are uploaded from a matrix build, each platform job extracts the produced archives and runs {

- `vox --version` / `vox.exe --version`
- `vox-bootstrap --help` / `vox-bootstrap.exe --help`

If any job fails smoke, **do not** consider the release green.

## Source fallback contract

`vox-bootstrap --install` is binary-first. If binary download/verify/extract fails, source fallback uses:

- `cargo install --locked --path crates/vox-cli`
- repo root discovery (`VOX_REPO_ROOT` or upward search for `crates/vox-cli/Cargo.toml`)

Therefore source fallback requires a local repo checkout and Cargo. Users running only a downloaded standalone `vox-bootstrap` binary should treat fallback failure as expected unless they provide a repo + Cargo environment.

## PM provenance (registry packages)

Publishing **Vox PM** packages with **`vox pm publish`** writes `vox.pm.provenance/1` JSON under **`.vox_modules/provenance/`** (fields include **`schema`**, **`package`**, **`version`**, **`content_hash`**, **`built_at_epoch`**, **`tool`**, and **`registry`** URL used for the publish). Release or registry pipelines can enforce those sidecars with **`vox ci pm-provenance --strict`** (see [`reference/cli.md`](../reference/cli.md)). Optional GitHub workflow [`.github/workflows/pm-provenance-verify.yml`](../../../.github/workflows/pm-provenance-verify.yml): **`workflow_dispatch` by default**; add a **`schedule:`** in fork/deploy branches for periodic (e.g. monthly) verification on self-hosted runners if you want it. This is separate from the binary tarball contract above but shares the same “verify before promote” posture.

## Rollback

If a bad release is published: delete or edit the GitHub Release assets, or ship a **new patch tag** with corrected artifacts. Semver: prefer `vX.Y.(Z+1)` over reusing a tag.

## Release dry-run (operators)

Before shipping a real tag:

1. Locally: `cargo run -p vox-cli -- ci release-build --target <host-triple>` (optional `--version`), extract the archive, run `./vox --version`.
2. `cargo test -p vox-cli release_build`, `cargo test -p vox-bootstrap`, `cargo run -p vox-cli -- ci command-compliance`.
3. CI: push a disposable test tag `v0.0.0-test.<timestamp>`, confirm all matrix jobs + publish; then delete the test tag/release if it was only for verification.

