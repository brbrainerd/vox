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
2. **`ring`** — *no longer banned; see the correction below.* It is the workspace's
   deliberate TLS provider (`Cargo.toml` pins `rustls`/`tokio-rustls` to
   `features = ["ring"]`) and iroh 1.x's default. Banning it here while the root
   manifest selected it on purpose is the contradiction this revision resolves.
3. **`zig`-chains**: Prohibited for cross-compilation within the crypto stack.
4. **C-assembly build chains** — restated as a *build* invariant rather than a crate
   ban: no dependency may require **cmake, nasm, Go, perl, or libclang**. Verified by a
   CI image lacking those tools, not by blacklisting names. (`aws-lc-sys` >= 0.41 builds
   on `x86_64-pc-windows-msvc` with neither cmake nor nasm present — confirmed
   empirically on 2026-09-04 — so the old clause described a hazard that had ceased to
   exist, while missing feature-mediated provider selection, which is the real one.)

All cryptography must compile on `stable` Rust without a C toolchain requirement.

---

## Correction (2026-09-04) — this document was unenforced and self-contradictory

Three findings, each verified against the tree:

1. **The gate had never executed.** `vox-code-audit`'s `CryptoBanDetector` carries a
   Cargo-manifest branch and lists `Language::Unknown` in `supported_langs` precisely so
   manifests reach it. But `scanner.rs` dropped every `Unknown` file before any detector
   ran, and `Language::from_extension("toml")` is `Unknown`. The branch was unreachable in
   production; its unit tests passed only because they construct `SourceFile` directly and
   bypass the scanner. Fixed, with a regression test that goes *through* `scan()`.

2. **The workspace ships two TLS providers, and this document banned the one it chose on
   purpose.** `ring` is pinned in the root manifest. `aws-lc-rs` arrives with reqwest 0.13
   via `chromiumoxide` and `gix` -> `jj-lib` -> `vox-vcs`; `jj-lib` exposes no TLS feature,
   so it cannot be steered from our manifests. Full attribution, and the collapse path
   (hf-hub 1.0 moves to reqwest 0.13), is in
   [`contracts/crypto/transport-providers.v1.json`](../../../contracts/crypto/transport-providers.v1.json).

3. **A name ban is the wrong tool for provider selection.** Providers are chosen by a
   *feature* on a shared crate (`rustls/aws-lc-rs`), which no source-text regex can see, and
   cargo unifies features per-package so one crate turning it on affects everything. The
   sound input is the resolved lockfile. See `AGENTS.md` §Cryptography Policy for the
   two-concern split that replaces the single rule.
