# voxup

Hermetic toolchain installer and multiplexer for Vox — modelled after `rustup`.

## Install

```sh
curl --proto '=https' --tlsv1.2 -sSf https://voxlang.org/voxup | sh
```

Windows, source builds, and everything else:
[Installing Vox](../../docs/src/reference/installation.md) — the canonical page.

## What `voxup install` does

1. Queries the GitHub Releases API for the latest `v*` tag on `vox-foundation/vox`.
2. Downloads the correct archive for your platform (e.g. `vox-0.7.0-aarch64-apple-darwin.tar.gz`).
3. Verifies SHA-256 against `checksums.txt` from the same release.
4. Extracts to `~/.vox/toolchains/vox-<version>/`.
5. Hard-links the binary to `~/.vox/bin/vox` (and `~/.cargo/bin/vox`).
6. Appends `~/.vox/bin` to `$PATH` in your shell profile.

## Commands

| Command | Purpose |
|---|---|
| `voxup install [profile]` | Download and install the latest stable Vox CLI |
| `voxup update` | Upgrade to the latest release if a newer one exists |
| `voxup proxy -- <args…>` | Run `vox` with hermetic `PATH` prepended |

## Related

- `vox doctor` — post-install environment check
- `contracts/channels/stable.toml` — stable channel manifest SSOT
- `docs/src/architecture/voxup-omnibus-installer-spec-2026.md` — architecture spec
