---
title: "cryptography research findings 2026"
description: "Automatically added frontmatter for cryptography research findings 2026"
category: "Architecture SSOTs"
status: "research"
training_eligible: false
archived_date: 2026-04-18
---
# Cryptography Research Findings 2026

## Overview
This document summarizes our research into modern Rust cryptographic algorithms and their integration into Vox.

## Hash Selection
- **BLAKE3:** Proven to be the fastest general-purpose cryptographic hash, scaling efficiently across CPU cores and SIMD lanes. Chosen for `secure_hash`.
- **XXHash (XXH3):** Extremely fast non-cryptographic hash. Chosen for in-memory AST caching and bloom filters via `fast_hash`.
- **SHA-3:** Kept strictly for external interop and standardized compliance. Chosen for `compliance_hash`.

## AEAD Selection and the ZIG Ban
Initially, AEGIS was proposed due to hardware AES-NI acceleration. However, compiling its native C backends on Windows causes significant friction (requiring NASM, CMake). Patching it to `pure-rust` disables the hardware acceleration, leaving a pure-software fallback.

Benchmarks reveal that purely software-optimized primitives like `chacha20poly1305` significantly outperform the `pure-rust` version of AEGIS. To ensure maximum zero-friction compilation across platforms while maintaining top-tier software performance, we have banned AEGIS.

## Architecture
Cryptographic primitives are centralized into the `vox-crypto` crate. `vox-clavis` depends on this crate to prevent environment-parsing logic from bubbling into low-level compiler crates that only require hashing.

