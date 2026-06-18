---
title: "Installing Vox"
description: "Install the Vox programming language and toolchain on macOS, Linux, or Windows using the official voxup installer."
category: "Getting Started"
status: "current"
---

# Installing Vox

Vox installs via **`voxup`**, a toolchain installer modelled after `rustup`.
A single command downloads the Vox CLI and configures your shell `PATH`.

## Quick Install

### macOS and Linux

```sh
curl --proto '=https' --tlsv1.2 -sSf https://voxlang.org/voxup | sh
```

### Windows (PowerShell)

```powershell
Invoke-WebRequest -Uri https://voxlang.org/voxup.ps1 -OutFile voxup.ps1
.\voxup.ps1
```

After installation, restart your terminal (or `source ~/.bashrc`) then verify:

```bash
vox --version
```

## What Gets Installed

| Path | Contents |
|---|---|
| `~/.vox/bin/vox` | The Vox CLI (real binary from the release archive) |
| `~/.vox/bin/voxup` | The installer binary (used by `voxup update`) |
| `~/.vox/toolchains/vox-<version>/` | Versioned toolchain directory |
| `~/.vox/toolchains/active` | Active version number (plain text) |

`~/.vox/bin` is added to your shell `PATH` automatically.

## Updating

```bash
voxup update
```

This checks GitHub for a newer release and installs it if available.

## Manual Install (developers only)

If you have the Rust toolchain and want to build from source:

```bash
cargo install --locked --path crates/voxup
voxup install default
```
