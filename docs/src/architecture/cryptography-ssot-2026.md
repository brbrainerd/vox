---
title: "Cryptography Policy SSoT"
description: "Canonical cryptographic invariants, the scope of the banned-primitive list, and the accepted transitive crypto dependencies."
category: "Architecture SSOTs"
sort_order: 20
status: "current"
---

# Cryptography Policy (SSoT)

This document enforces the cryptography invariants referenced in `AGENTS.md`.

## Scope of the ban: first-party code and direct dependencies only

**The banned list below applies to first-party code and to crates declared
directly by a workspace `Cargo.toml`. It is not a whole-graph ban** — a
transitively-pulled crate is governed by the [build-toolchain
criterion](#the-real-invariant-no-c-toolchain-on-windows), not by its name.

What settles this: the root `Cargo.toml` `[workspace.dependencies]` **mandates**
`ring` as the rustls backend —

```toml
rustls      = { version = "0.23", default-features = false, features = ["ring"] }
tokio-rustls = { version = "0.26", default-features = false, features = ["ring"] }
```

— and `ring` has five reverse-dependencies in `Cargo.lock` (`quinn-proto`,
`rcgen`, `rustls`, `rustls-webpki`, `x509-parser`). A whole-graph reading would
make the workspace permanently in violation of its own SSoT while the same
SSoT's centralized dependency config requires the violation. That is not a
policy, it is a contradiction. The first-party reading is the one that has
actually been enforced (see [Enforcement](#enforcement)) and the one that
matches how the pin was chosen.

## The real invariant: no C toolchain on Windows

Every rule below is a consequence of one invariant:

> **All cryptography in the dependency graph must build on `stable` Rust on
> Windows with only the MSVC toolchain — no `cmake`, no `nasm`, no `perl`, no
> `zig` chain.**

Judge a *transitive* crypto dependency against that invariant, not against the
name list.

## Allowed primitives (first-party)

All first-party cryptographic logic MUST go through the `vox-crypto` crate
(`crates/vox-crypto/`), which is the sole SSoT surface:

- **AEAD**: `chacha20poly1305` (pure Rust) — `vox_crypto::{encrypt, decrypt}`.
- **Hashing**: `blake3` (`secure_hash`, `keyed_hash`), `sha3` (`compliance_hash`),
  `xxhash-rust` (`fast_hash`, non-cryptographic caches only).
- **Signing / key agreement**: `ed25519-dalek`, `x25519-dalek`.

`vox-crypto` is `#![forbid(unsafe_code)]` and pure Rust end to end.

## Banned in first-party code

Do not `use`, `import`, or add as a direct dependency:

1. **AEGIS** — state-management complexity and cross-platform inconsistency.
   Use `chacha20poly1305`. (See the accepted transitive exception below; the
   ban is on *writing* AEGIS code, not on turso's internal page encryption.)
2. **`ring`** — as a *direct* first-party dependency and as a call target
   (`use ring::…`). No workspace crate does this today. Its presence as the
   rustls TLS backend is deliberate and permitted — see below.
3. **`aws-lc-rs`** — genuinely trips the invariant: C + assembly with a
   `cmake`/`nasm` build chain. This is why the workspace **cannot** adopt
   `reqwest` 0.13, whose `rustls` feature expands to `__rustls-aws-lc-rs`. The
   `reqwest` 0.12 pin and the explicit `features = ["ring"]` on `rustls` /
   `tokio-rustls` exist to keep `aws-lc-rs` out of the graph. Do not "simplify"
   either by dropping the explicit feature list.
4. **`openssl`, `md5`, `sha1`** — native-library dependency / broken primitives.
5. **`zig`-chains** — banned for cross-compilation within the crypto stack.

## Why `ring` is permitted as the rustls backend

The SSoT previously said "prohibited due to its reliance on C/assembly and
complex build system requirements." That rationale does not survive contact
with `ring` 0.17:

- `ring` 0.17.14 ships **pre-generated object files** for Windows
  (`pregenerated/*-nasm.o`); `nasm` is only invoked when *packaging* the crate,
  not when consuming it. No `nasm` or `cmake` is required by consumers.
- Its C is compiled by `cc` with the MSVC toolchain that a Windows Rust install
  already provides.

`ring` therefore does not trip the invariant, and the alternative
(`aws-lc-rs`) demonstrably does. Rustls needs *a* crypto provider; `ring` is the
one that builds clean. **This is a deliberate, permitted exception, not an
oversight.**

## Accepted transitive crypto dependencies

| Crate | Version | Path in | Why accepted |
|---|---|---|---|
| `ring` | 0.17 | `rustls`, `tokio-rustls`, `quinn-proto`, `rcgen`, `rustls-webpki`, `x509-parser` | Mandated by the workspace as the rustls backend; keeps `aws-lc-rs` out. Pure pre-generated asm + `cc`, no `cmake`/`nasm` for consumers. |
| `aegis` | 0.9.8 | `turso_core` only | **Not removable.** `aegis` is a non-optional target-dependency of `turso_core` 0.6.1 on every platform (`cfg(any(android, macos))` and `cfg(not(any(android, macos)))` branches both declare it unconditionally); it is not behind a Cargo feature, so `turso = { default-features = false, features = ["sync"] }` cannot avoid it. Its only dependency is `softaes`, pure Rust. Satisfies the build-toolchain invariant. |

Note on `aegis`: turso exposes a `pure-rust-crypto` feature
(`turso/pure-rust-crypto` → `turso_core/pure-rust-crypto` → `aegis/pure-rust`)
which forces the software-AES path instead of CPU intrinsics. It does **not**
remove `aegis`. Enable it only if an intrinsics-related build or portability
problem actually appears; it costs performance otherwise.

**Adding a row to this table requires the same scrutiny as adding the
dependency.** Check the crate's `build.rs` for `cc`/`cmake`/`nasm`/`perl` and
whether pre-generated artifacts ship in the package.

## Enforcement

| Mechanism | Covers | Status |
|---|---|---|
| `vox-code-audit` detector `vox/crypto/banned-crate-import` (`crates/vox-code-audit/src/detectors/crypto_ban.rs`), severity `Error` | Rust `use`/`extern crate` and Vox `import` of `aegis`, `ring`, `md5`, `sha1`, `openssl` | **Working** — this is what enforces the first-party ban. |
| The same detector's Cargo-manifest arm (`ring = "0.17"`, a renamed `crypto = { package = "ring" }`, `openssl.workspace = true`, `[dependencies.md5]`) | Direct dependency declarations | **Working as of 2026-08-23.** `Scanner::walk_root` previously skipped every file whose extension maps to `Language::Unknown`, and `Language::from_extension("toml") == Unknown`, so no `Cargo.toml` ever reached a detector — the arm was dead code. The scanner now admits files named `Cargo.toml`, and the arm parses the manifest as TOML rather than matching lines, so renamed and workspace-inherited forms are visible. `[patch.*]`/`[replace]` tables and the hakari-generated `crates/workspace-hack/Cargo.toml` are deliberately skipped. |
| `deny.toml` `[bans]` | no crypto `deny` list | **Deliberately absent.** A `[[bans.deny]] crate = "aws-lc-rs"` entry was added and reverted on 2026-08-23: `aws-lc-rs` 1.17.0 is already in `Cargo.lock` and already compiles here, arriving through `reqwest` 0.13's `rustls` feature via chromiumoxide / gix-transport / rmcp / self_update / tauri. The ban would have hard-failed `cargo deny check` on the current lock without removing anything. See the comment at `deny.toml`. |

Consequence: the direct-dependency half of the ban was unenforced until
2026-08-23, which is how `aegis` entered the lockfile unnoticed. In `aegis`'s
case the outcome was benign — it is transitive, unavoidable, and patched to its
pure-Rust backend. A first-party `aws-lc-rs = "1"` in a workspace manifest is
now caught by the detector. The *transitive* graph remains unguarded by design:
there is no `[[bans.deny]]`, because the one crate that would justify one is
already present and building. Transitive crypto arrivals are reviewed here, in
this document, not by a gate.
