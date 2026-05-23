---
title: "Installation Reference"
description: "Authoritative guide for installing the Vox CLI, toolchain, and AI provider backends across Windows, macOS, and Linux."
category: "Language Reference"
last_updated: "2026-04-05"
training_eligible: true

schema_type: "TechArticle"
---

# Installation Reference

This guide covers everything you need to get Vox running on any platform.

## Quick Install (30 seconds)

### Cargo-free quick install (recommended for end users)

```bash
# Linux / macOS / WSL
curl -fsSL https://raw.githubusercontent.com/vox-foundation/vox/main/scripts/install.sh | bash -s -- --install

# Windows (PowerShell)
$tmp = Join-Path $env:TEMP "vox-install.ps1"
Invoke-WebRequest -Uri "https://raw.githubusercontent.com/vox-foundation/vox/main/scripts/install.ps1" -OutFile $tmp
powershell -NoProfile -ExecutionPolicy Bypass -File $tmp -Install
```

The scripts download a standalone `vox-bootstrap` release binary, verify it against release `checksums.txt`, and run it.

### Repository install (contributors / local development)

```bash
git clone https://github.com/vox-foundation/vox && cd vox

# Linux / macOS / WSL
./scripts/install.sh

# Windows (PowerShell)
.\scripts\install.ps1
```

Scripts prefer local `cargo run --locked -p vox-bootstrap` when run inside a repo checkout with Cargo available (best for debugging and contribution flows). Outside that path, scripts fetch and run a standalone `vox-bootstrap` release binary. When `--install` is used, bootstrap attempts a **binary-first** install from GitHub Releases (SHA-256 via `checksums.txt`; latest tag from the GitHub API so asset names match `vox-<tag>-<triple>.*`), then falls back to **`cargo install --locked --path crates/vox-cli`** from the resolved repo root (`VOX_REPO_ROOT` or upward search for `crates/vox-cli/Cargo.toml`). Source fallback therefore requires a repo checkout plus Cargo. Artifact layout and targets { [binary release contract](../ci/binary-release-contract.md). See `crates/vox-bootstrap/README.md`.

| Flag / args | Effect |
|-------------|--------|
| `--dev` / `-Dev` (PS1) | Request rustfmt + clippy (with `--apply`) |
| `--install-clang` / `-InstallClang` | Install clang where supported (e.g. winget `LLVM.LLVM` on Windows) |
| `--apply` / `-Apply` | Actually run installs; without it, the tool **plans** only |
| `--install` / `-Install` | Install `vox` after checks (binary-first; source fallback) |
| `--source-only` / `-SourceOnly` | Skip release binary path and force source install |
| `--version <tag>` / `-Version <tag>` | Pin release install to a specific tag (for example `v1.2.3`) |
| `plan` | Machine plan as **JSON** on stdout (exit 1 if requirements missing); `plan --human` for debug text |

Examples: `./scripts/install.sh --install --version v1.2.3`, `.\scripts\install.ps1 -Install`, `./scripts/install.sh --install --source-only`, `./scripts/install.sh plan`.

Then build the CLI with `cargo build -p vox-cli` and run **`vox doctor`** to verify your local environment.

## Cross-Platform Verification Checklist

After installing `vox`, run:

```bash
vox doctor
```

This check focuses on:

| Check | Required? | How to Fix |
|---|---|---|
| Rust ≥ 1.90 (workspace `rust-version`) | ✅ | [rustup.rs](https://rustup.rs) |
| Node.js ≥ 18 | Optional | [nodejs.org](https://nodejs.org) |
| Git | ✅ | [git-scm.com](https://git-scm.com) |
| C compiler (MSVC/gcc/clang) | ✅ | Platform-specific (see below) |
| **clang** / **LLVM** (optional) | Optional | The workspace patches **`aegis`** with **`pure-rust`** defaults so typical **Windows + MSVC** builds do **not** require `clang-cl` for Turso. Use `scripts/install.* --install-clang` only if you hit a toolchain that still expects native crypto builds. |
| Google AI Studio Key | Recommended | Free at [aistudio.google.com/apikey](https://aistudio.google.com/apikey) |
| OpenRouter Key | Optional | [openrouter.ai/keys](https://openrouter.ai/keys) |
| Ollama | Optional | [ollama.com](https://ollama.com) |
| VoxDB directory writable | ✅ | `~/.vox/` must exist and be writable |

## AI Provider Keys

Vox uses a **three-layer model cascade** — you get free AI with just a Google account:

### Layer 1: Google AI Studio (Free, Primary)

No credit card required. Provides Gemini 2.5 Flash, Flash-Lite, and Pro.

```bash
# Get your key (takes 10 seconds):
# https://aistudio.google.com/apikey

export GEMINI_API_KEY=YOUR_KEY
```

### Layer 2: OpenRouter (Optional)

Free API key unlocks dozens of `:free` models (Devstral 2, Qwen3 Coder, Llama 4 Scout, Kimi K2). Paid key unlocks SOTA models (DeepSeek v3.2, Claude Sonnet 4.5, GPT-5, O3).

```bash
export OPENROUTER_API_KEY=YOUR_KEY
```

### Layer 3: Ollama (Optional, Local)

Zero-auth local inference. Install Ollama, pull a model, and Vox auto-detects it.

```bash
ollama pull llama3.2
# Vox detects Ollama on localhost:11434 automatically
```

## Verify Your Environment

```bash
vox doctor
```

Example output:
```text
  ✓  Rust / Cargo              cargo 1.82.0
  ✓  Node.js                   v20.11.0 (>= v18)
  ✓  Git                       git version 2.44.0
  ✓  C Compiler                MSVC Build Tools found
  ✓  Google AI Studio Key      configured (free Gemini models available)
  ○  OpenRouter Key (optional) not configured
  ○  Ollama Local (optional)   not running
  ✓  VoxDB directory           C:\Users\you\.vox (writable)

  ✓ All checks passed — you're ready to build with Vox!
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

# Example multi-service mens compose (see `examples/mens-compose.yml`)
# docker compose -f examples/mens-compose.yml up

# Full stack with docker compose
cp .env.example .env  # fill in GEMINI_API_KEY
docker compose up
```

## Platform-Specific Notes

### Windows
- **MSVC (C++):** `winget install -e --id Microsoft.VisualStudio.2022.BuildTools` (include **Desktop development with C++** workload in the installer UI when prompted).
- **clang-cl (Turso / aegis):** `winget install -e --id LLVM.LLVM` so `clang-cl.exe` is on `PATH` (often under `C:\Program Files\LLVM\bin`). Or run `.\scripts\install.ps1 -InstallClang`.
- **One-liner bootstrap:** `.\scripts\install.ps1 -Dev -InstallClang` then `cargo build -p vox-cli`.
- **WSL:** `wsl ./scripts/install.sh --dev --install-clang` avoids MSVC/clang-cl friction for some workflows.

### macOS
- **C Compiler:** `xcode-select --install` (ships `clang` for most crates).
- **Turso:** Usually satisfied by Xcode CLT; if `aegis` still fails, `brew install llvm` and follow Homebrew’s `PATH` notes.

### Linux
- **C Compiler:** `sudo apt-get install build-essential` (Debian/Ubuntu).
- **clang (recommended for Turso):** `sudo apt-get install clang` or `./scripts/install.sh --install-clang`.




