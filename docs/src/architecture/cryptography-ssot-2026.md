---
title: "Cryptography Policy SSoT"
description: "Canonical cryptographic invariants and banned primitives for Vox."
category: "Architecture SSOTs"
sort_order: 20
status: "current"
---

# Cryptography Policy (SSoT)

This document enforces the cryptography invariants referenced in `AGENTS.md`.

## Allowed Primitives

All cryptographic logic MUST use the `vox-crypto` crate.

- **AEAD**: Pure-Rust `chacha20poly1305` is the standard.
- **Hashing**: Use `sha2` (SHA-256 or SHA-512) or `blake3` via pure-Rust crates.

## Banned Primitives & Dependencies

The following are **explicitly banned** in this repository:

1. **AEGIS**: Prohibited due to state-management complexity and cross-platform inconsistencies.
2. **`ring`**: Prohibited due to its reliance on C/assembly and complex build system requirements.
3. **`zig`-chains**: Prohibited for cross-compilation within the crypto stack.
4. **C-assembly optimizations**: Any wrapper dragging in `cmake` or `nasm` for C-assembly optimization on Windows is strictly banned.

All cryptography must compile on `stable` Rust without a C toolchain requirement.
