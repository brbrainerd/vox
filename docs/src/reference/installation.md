---
title: "Installing Vox"
description: "The single canonical guide to installing the Vox CLI and toolchain on macOS, Linux, or Windows, and what each install path actually does today."
category: "Getting Started"
status: "current"
training_eligible: true
schema_type: "TechArticle"
---

# Installing Vox

This is the **canonical** installation page. Every other install snippet in the
repository is one command plus a link back here.

Vox is pre-1.0. The supported paths are **`voxup`** (a `rustup`-style toolchain
installer that downloads a prebuilt release) and **`cargo install` from a
checkout**. Nothing else is published yet — see
[Packaging status](#packaging-status).

## Quick install (voxup)

### macOS and Linux

```sh
curl --proto '=https' --tlsv1.2 -sSf https://voxlang.org/voxup | sh
```

### Windows (PowerShell)

```powershell
Invoke-WebRequest -Uri https://voxlang.org/voxup.ps1 -OutFile voxup.ps1
.\voxup.ps1
```

Restart your terminal (or `source ~/.bashrc`) and verify:

```bash
vox --version
```

### What the script does

The script takes **no arguments and no flags**. It runs one fixed sequence:

1. Detects your platform and maps it to a published release target
   (`x86_64-unknown-linux-gnu`, `x86_64-apple-darwin`, `aarch64-apple-darwin`,
   `x86_64-pc-windows-msvc`). aarch64 Linux has no release asset and the script
   stops with a message telling you to build from source.
2. Queries the GitHub Releases API for the newest release of
   `vox-foundation/vox` (all releases are currently pre-releases, so
   `/releases/latest` is deliberately not used).
3. Downloads `voxup-<tag>-<target>.tar.gz` and `checksums.txt`, and verifies
   SHA-256.
4. Extracts and runs `voxup install default`.

Targets and artifact layout: [binary release contract](../ci/binary-release-contract.md).

### Running it from a checkout

The published URLs serve byte-identical copies of the in-repo scripts, so a
clone can run them directly:

```bash
sh scripts/install.sh        # macOS / Linux
.\scripts\install.ps1        # Windows (PowerShell)
```

`docs-astro/public/voxup` and `docs-astro/public/voxup.ps1` are kept identical to
`scripts/install.sh` and `scripts/install.ps1` by the
`documented_install_urls_are_served` test. Edit one, copy to the other in the
same commit.

## What gets installed

| Path | Contents |
|---|---|
| `~/.vox/bin/vox` | The Vox CLI (real binary from the release archive) |
| `~/.vox/bin/voxup` | The installer binary (used by `voxup update`) |
| `~/.vox/toolchains/vox-<version>/` | Versioned toolchain directory |
| `~/.vox/toolchains/active` | Active version number (plain text) |

`~/.vox/bin` is appended to your shell `PATH`.

## Updating

```bash
voxup update
```

Checks GitHub for a newer release and installs it if one exists.

## Building from source

**Prerequisites:** Rust **1.96** — the workspace `rust-version` is `1.96` and
`rust-toolchain.toml` pins the toolchain to `1.96.0`, which is what CI and every
shipped artifact are built with. Also Node.js >= 18 (runtime dependency of
`vox bundle` and `vox run` for frontend scaffolding) and a C compiler
(gcc / clang / MSVC).

```bash
git clone https://github.com/vox-foundation/vox.git
cd vox
cargo install --locked --path crates/vox-cli
```

Always pass `--locked`: it installs against the workspace `Cargo.lock`, which is
what the release binaries are built from. This exact argument vector is the
`CARGO_INSTALL_CLI_FROM_SOURCE` constant in
`crates/vox-cli/src/utils/install_policy/mod.rs`.

For a development build without installing:

```bash
cargo build -p vox-cli
```

### Installing voxup itself from source

```bash
cargo install --locked --path crates/voxup
voxup install default
```

## Beyond the CLI

`cargo install --locked --path crates/vox-cli` and `voxup` give you the **CLI
only**. `vox doctor` will keep reporting these as missing until you add them
separately — see [`CONTRIBUTING.md`](https://github.com/vox-foundation/vox/blob/main/CONTRIBUTING.md)
for the full table.

The one that is easy to get wrong:

```bash
cargo install --locked --path crates/vox-ml-cli --features populi
```

`populi` is a **non-default** feature of `vox-ml-cli`. A bare
`cargo install --path crates/vox-ml-cli` compiles the default `mens-base`
feature set and yields a binary with no mesh transport, so `vox populi` cannot
serve. The path and required feature are recorded as
`SOURCE_INSTALL_ML_CLI_REL_PATH` and `CARGO_INSTALL_ML_CLI_FROM_SOURCE` in
`crates/vox-cli/src/utils/install_policy/mod.rs`.

## Packaging status

| Channel | Status today |
|---|---|
| `voxup` one-liner (`https://voxlang.org/voxup`, `/voxup.ps1`) | **Working.** Downloads a checksum-verified release binary. |
| `cargo install --locked --path crates/vox-cli` | **Working.** Requires a checkout and a Rust toolchain. |
| GitHub Release archives | **Published** for the four targets above (pre-releases). |
| Homebrew tap | **Not published.** The release job builds and hashes a macOS tarball but the tap-update step is a placeholder; no `vox-foundation/homebrew-vox` formula is dispatched. |
| Windows `.msi` | **Not published.** The `cargo wix --no-build` job has no preceding `cargo build --profile dist`, so it has no binary to package, and nothing is uploaded. |
| Debian `.deb` | **Not published.** `cargo deb` does build the package, but no step uploads it to the release. |

Do not tell users to `brew install`, download an `.msi`, or `apt install` vox
until those jobs actually publish artifacts.

## Verify your environment

```bash
vox doctor
```

| Check | Required? | How to fix |
|---|---|---|
| Rust 1.96 (workspace `rust-version`; `rust-toolchain.toml` pins `1.96.0`) | Yes | [rustup.rs](https://rustup.rs) |
| Node.js >= 18 | Optional | [nodejs.org](https://nodejs.org) |
| Git | Yes | [git-scm.com](https://git-scm.com) |
| C compiler (MSVC / gcc / clang) | Yes | Platform-specific, see below |
| clang / LLVM | Optional | The workspace patches **`aegis`** with **`pure-rust`** defaults, so a typical Windows + MSVC build does **not** need `clang-cl` for Turso. Install LLVM only if you hit a toolchain that still expects native crypto builds. |
| Google AI Studio key | Recommended | Free at [aistudio.google.com/apikey](https://aistudio.google.com/apikey) |
| OpenRouter key | Optional | [openrouter.ai/keys](https://openrouter.ai/keys) |
| Ollama | Optional | [ollama.com](https://ollama.com) |
| VoxDB directory writable | Yes | `~/.vox/` must exist and be writable |

Example output:

```text
  ✓  Rust / Cargo              cargo 1.96.0
  ✓  Node.js                   v20.11.0 (>= v18)
  ✓  Git                       git version 2.44.0
  ✓  C Compiler                MSVC Build Tools found
  ✓  Google AI Studio Key      configured (free Gemini models available)
  ○  OpenRouter Key (optional) not configured
  ○  Ollama Local (optional)   not running
  ✓  VoxDB directory           C:\Users\you\.vox (writable)

  ✓ All checks passed — you're ready to build with Vox!
```

## AI provider keys

Vox uses a three-layer model cascade — you get free AI with just a Google
account.

### Layer 1: Google AI Studio (free, primary)

No credit card required. Provides Gemini 2.5 Flash, Flash-Lite, and Pro.

```bash
export GEMINI_API_KEY=YOUR_KEY
```

### Layer 2: OpenRouter (optional)

A free key unlocks dozens of `:free` models; a paid key unlocks frontier models.

```bash
export OPENROUTER_API_KEY=YOUR_KEY
```

### Layer 3: Ollama (optional, local)

Zero-auth local inference. Install Ollama, pull a model, and Vox auto-detects it
on `localhost:11434`.

```bash
ollama pull llama3.2
```

## Docker

```bash
# Build from source
docker build -t vox .

# Optional: image with `vox populi` (HTTP control plane)
docker build -t vox:mens --build-arg VOX_CLI_FEATURES=mens .

# Run MCP server
docker run -e GEMINI_API_KEY=... -p 3000:3000 vox

# MCP + in-container mens sidecar (background `vox populi serve` on 9847)
docker run -e VOX_MESH_MESH_SIDECAR=1 -e GEMINI_API_KEY=... -p 3000:3000 -p 9847:9847 vox:mens

# Full stack with docker compose
cp .env.example .env  # fill in GEMINI_API_KEY
docker compose up
```

An example multi-service mens compose file lives at `examples/mens-compose.yml`.

## Platform notes

### Windows

- **MSVC (C++):** `winget install -e --id Microsoft.VisualStudio.2022.BuildTools`
  (include the **Desktop development with C++** workload when prompted).
- **clang-cl (Turso / aegis):** `winget install -e --id LLVM.LLVM` so
  `clang-cl.exe` is on `PATH` (usually `C:\Program Files\LLVM\bin`). Only needed
  if a native crypto build is required.
- **WSL:** running `sh scripts/install.sh` inside WSL avoids MSVC / clang-cl
  friction for some workflows.

### macOS

- **C compiler:** `xcode-select --install` (ships `clang`).
- **Turso:** usually satisfied by the Xcode Command Line Tools; if `aegis` still
  fails, `brew install llvm` and follow Homebrew's `PATH` notes.

### Linux

- **C compiler:** `sudo apt-get install build-essential` (Debian / Ubuntu).
- **clang (recommended for Turso):** `sudo apt-get install clang`.
