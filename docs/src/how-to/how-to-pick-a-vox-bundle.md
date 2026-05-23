---
title: "Picking a Vox bundle"
description: "Decision guide for choosing the right Vox distribution bundle for your use case."
category: "How-To Guides"
---

# Picking a Vox bundle

Vox ships as pre-assembled tarballs called *bundles*. Each bundle contains the
`vox` binary plus a curated set of plugins for a particular workload. Choose the
smallest bundle that covers your use case; you can always add more plugins later
with `vox plugin install <id>`.

## Quick-reference table

| Use case | Bundle |
|---|---|
| Local dev with full web-stack skills | `vox-fullstack` |
| Local ML training (NVIDIA GPU) | `vox-ml` |
| Headless backend / API server | `vox-server` |
| Mesh node (inter-process / distributed) | `vox-mesh` |
| Cloud-managed client (thin node) | `vox-cloud-only` |
| Edge / on-device inference | `vox-edge` |
| Power user (every plugin, every skill) | `vox-dev` |
| Build your own from scratch | `vox-base` + `vox plugin install ...` |

## Bundle contents

Run `vox bundle list` to see all bundles with their resolved plugin counts, or
`vox bundle build --help` for assembly options.

Each bundle extends one or more parent bundles through an `extends` chain:

```text
vox-base                   (no plugins)
  └── vox-server           (core skill plugins)
        ├── vox-fullstack  (+ web / frontend skills)
        ├── vox-mesh       (+ distributed-mesh skill)
        └── vox-cloud-only (subset — no local-inference plugins)
vox-ml                     (base + GPU/ML plugins; standalone)
vox-edge                   (base + lightweight inference; standalone)
vox-dev                    (extends vox-fullstack + vox-ml + vox-mesh + extras)
```

## Installation

Download the tarball for your platform from the GitHub Releases page, extract
it, and add the `bin/` directory to your PATH.

```bash
# Example: vox-fullstack on Linux x86_64
curl -L -o vox-fullstack.tar.gz \
  https://github.com/vox-foundation/vox/releases/latest/download/vox-fullstack-linux-x86_64.tar.gz

tar -xzf vox-fullstack.tar.gz -C ~/vox

export PATH="$HOME/vox/bin:$PATH"
```

On Windows (PowerShell):

```powershell
Invoke-WebRequest `
  -Uri "https://github.com/vox-foundation/vox/releases/latest/download/vox-fullstack-windows-x86_64.tar.gz" `
  -OutFile vox-fullstack.tar.gz

tar -xzf vox-fullstack.tar.gz -C "$env:USERPROFILE\vox"

$env:PATH = "$env:USERPROFILE\vox\bin;$env:PATH"
```

## Verifying a bundle

After downloading, confirm all bundled plugins load correctly:

```bash
vox bundle verify vox-fullstack.tar.gz
```

This extracts the tarball to a temporary directory and runs `vox plugin doctor`
against the extracted plugins root. Exit code 0 means all plugins pass ABI and
manifest checks.

## Adding plugins after installation

Start from `vox-base` and add exactly what you need:

```bash
# Install just the git skill plugin
vox plugin install vox-plugin-skill-git

# Or apply a whole bundle's plugin set without unpacking a tarball
vox bundle apply vox-server
```

## Building a custom bundle

```bash
# Assemble a tarball from whatever is currently installed
vox bundle build vox-base --out my-custom-bundle.tar.gz

# Cross-compile for a different platform
vox bundle build vox-server --target x86_64-unknown-linux-gnu --out vox-server-linux.tar.gz
```

## Building distribution binaries from source

For CI ship artifacts, use the `dist` profile. It enables fat LTO,
single codegen unit, and full symbol stripping — producing the smallest
and fastest `vox-cli` binary:

```bash
cargo build --profile dist -p vox-cli
```

The output lands in `target/dist/vox-cli[.exe]`. Use `cargo build --release`
for fast local dev builds (thin LTO, no symbol stripping).
