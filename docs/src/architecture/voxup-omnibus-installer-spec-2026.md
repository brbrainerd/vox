---
title: "Voxup Omnibus Installer Spec"
description: "Architecture and implementation plan for the voxup unified installer, designed to provide a hermetic, zero-dependency environment for Vox development."
category: "Architecture SSOTs"
status: "current"
---

# `voxup` Omnibus Installer Specification

*May 2026*

## Motivation

While Vox ships as a single CLI binary (`vox`), building and executing full-stack applications locally still silently delegates to external system toolchains: Node.js/`pnpm` for frontend bundling, `rustup` for WASM standard libraries, and Cargo for ML execution. 

Relying on the user's host OS environment causes significant friction, as users often lack these dependencies or have incompatible versions. `vox doctor --auto-heal` attempts to fix this, but fails if the foundational tools (like `npm` or `curl`) are entirely missing or permission-gated.

The solution is `voxup`: an omnibus installer modeled after `rustup`. It bootstraps the Vox environment by securely fetching hermetic, pre-compiled, and portable versions of all required toolchains into an isolated `~/.vox/toolchains/` directory, completely bypassing the host OS package managers.

## Architecture

### 1. The Bootstrap Script

Users will install Vox via a single command that does not require `brew`, `dpkg`, or `.msi`:

```bash
# macOS/Linux
curl --proto '=https' --tlsv1.2 -sSf https://voxlang.org/voxup | sh

# Windows (PowerShell)
Invoke-WebRequest -Uri https://voxlang.org/voxup.ps1 -OutFile voxup.ps1; .\voxup.ps1
```

The bootstrap script is intentionally minimal. Its only job is to detect the host architecture (e.g., `x86_64-apple-darwin`, `aarch64-unknown-linux-gnu`), download the `voxup` Rust binary for that target, and execute it.

### 2. The `voxup` Binary

The `voxup` binary acts as the local toolchain multiplexer and installer. It manages:
- `~/.vox/bin/` (where the `vox` proxy executable lives, added to `$PATH`)
- `~/.vox/toolchains/` (where isolated toolchains live)

#### Core Commands:
- `voxup install default` (Installs the latest stable `vox-cli` and mandatory toolchains)
- `voxup update` (Updates the CLI and toolchains)
- `voxup toolchain add <name>` (e.g., `node-v22`, `wasm-sysroot`)

### 3. Hermetic Toolchain Management

When `voxup install` runs, it resolves a manifest (`channels/stable.toml`) that defines the exact matrix of required dependencies for Vox to operate on the current platform.

It then downloads these as isolated bundles into `~/.vox/toolchains/`:
1. **Vox CLI:** The actual `vox` executable.
2. **Hermetic Node.js:** A minimal, portable Node.js binary + `pnpm` specifically for Vox's internal usage. The user's system `node` is ignored.
3. **WASM Sysroots:** Pre-compiled `wasm32-wasip1` standard libraries. No local Cargo/Rustup is needed for `vox run --isolation wasm`.
4. **LLVM/Mold (Linux):** If required for fast ML inference compilation.

### 4. CLI Path Execution

When the user types `vox build`, the shell executes the `~/.vox/bin/vox` proxy.
This proxy sets up the isolated environment:
```bash
export PATH=~/.vox/toolchains/node-v22/bin:~/.vox/toolchains/wasm-sysroot/bin:$PATH
```
It then forwards the command to the actual `vox` binary. The `vox` CLI now reliably finds `pnpm` and `node` in its PATH without polluting the user's global system PATH.

## Implementation Plan

### Phase 1: `voxup` Scaffold
1. Create `crates/voxup` in the repository.
2. Implement downloading and extracting of `.tar.gz` and `.zip` bundles using `reqwest` and `tar`/`zip`.
3. Implement `PATH` modification logic for bash (`.bashrc`), zsh (`.zshrc`), and PowerShell profile.

### Phase 2: Manifest Resolution
1. Define the `channels/stable.toml` schema for tracking toolchain versions.
2. Setup GitHub Actions CI to automatically build and upload hermetic Node and WASM bundles to the release page.
3. Implement signature verification (Ed25519) in `voxup` to verify downloaded bundles.

### Phase 3: Proxy Execution
1. Implement the `vox` proxy wrapper in `voxup` that manipulates `std::env::set_var("PATH", ...)` before calling `execv` to the real CLI.

### Phase 4: Migration
1. Deprecate the direct `.msi`, `brew`, and `dpkg` instructions in the `README.md`.
2. Advise existing users to run `voxup install` to migrate to the hermetic environment.
3. Phase out Node/Rust installation logic from `vox doctor --auto-heal`, as `voxup` guarantees their presence.
