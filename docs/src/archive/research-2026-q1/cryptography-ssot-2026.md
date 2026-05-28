---
title: "cryptography ssot 2026"
description: "Automatically added frontmatter for cryptography ssot 2026"
category: "Architecture SSOTs"
status: "research"
training_eligible: false
archived_date: 2026-04-18
---
# Cryptography SSoT (2026)

This document defines the structural rules for cryptography across the Vox project.

## 1. The Vox-Crypto Rule
No crate may directly import cryptographic dependencies (e.g., `blake3`, `sha3`, `aegis`, `ring`, `aws-lc-rs`). All cryptographic operations MUST bridge through `vox-crypto::facades`.
This eliminates dependency sprawl and isolates compilation overhead into a single lightweight crate.

## 2. Algorithm Mapping
- **General Cryptographic Hash:** `blake3` via `vox_crypto::secure_hash`
- **Fast/Cache Hash (Non-Cryptographic):** `xxhash-rust` (XXH3) via `vox_crypto::fast_hash`
- **Compliance Hash:** `sha3` via `vox_crypto::compliance_hash`
- **Authenticated Encryption (AEAD):** `chacha20poly1305` via `vox_crypto::encrypt` and `vox_crypto::decrypt`

## 3. ZIG and AEGIS Ban
AEGIS and wrapper libraries containing native C/assembly (like `aws-lc-rs` or `ring`) are explicitly banned. They severely impact Windows MSVC cross-platform compatibility. The pure-rust version of AEGIS significantly degrades performance compared to `chacha20poly1305`, which is optimized for software.

## 4. Zeroing Memory
Use `zeroize` for clearing sensitive variables from memory immediately when they are dropped.

