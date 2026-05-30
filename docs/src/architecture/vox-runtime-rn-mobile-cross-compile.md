---
title: "vox-runtime-rn mobile cross-compile (Android + iOS)"
description: "How to build the vox-runtime-rn cdylib for the four Android architectures + four iOS architectures from a developer machine, and how the gate in crates/vox-cli-tests verifies it."
category: "Architecture SSOTs"
status: current
last_updated: 2026-05-30
---

# vox-runtime-rn mobile cross-compile

The Rust crate [`crates/vox-runtime-rn`](../../../crates/vox-runtime-rn/) ships
to mobile devices as a native shared library that the React Native + Expo
shell loads at runtime through a uniffi-bindgen-react-native–generated
TurboModule. This doc records the toolchain setup, the per-target build
commands, the workspace-hack opt-out that makes cross-compile possible, and
the gate in `crates/vox-cli-tests` that catches regressions.

## Platform status (2026-05-30)

| Target | Status | Verified |
|---|---|---|
| **Android** (aarch64 / armv7 / x86_64 / i686) | ✅ Cross-compiles; gated in CI | Yes — `crates/vox-cli-tests/tests/mobile_cross_compile.rs` |
| **iOS** (aarch64-apple-ios + sim) | ⏳ **PENDING — needs a macOS host** | **No — not yet run** |

> **TODO(ios-cross-compile): iOS cross-compile is unverified.** Apple's
> toolchain only runs on macOS, and this project currently has **no macOS host**.
> The build commands below (§iOS) are written but have never been executed here.
>
> A `#[ignore]`d gate — `vox_runtime_rn_cross_compiles_to_aarch64_ios` in
> `crates/vox-cli-tests/tests/mobile_cross_compile.rs` — surfaces this in every
> `cargo test` run as an *ignored* test with the reason.
>
> **When a macOS host or macOS CI runner becomes available:**
> 1. `rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios`
> 2. Run the §iOS build + `lipo` steps below and confirm the universal `.a`.
> 3. Remove the `#[ignore]` from that test and wire it into the EAS Build CI
>    matrix (see `docs/src/architecture/mobile-e2e-testing-strategy-2026.md`).
> 4. Update this table's iOS row to ✅ and delete this TODO box.

## Toolchain prerequisites

| Host | Need |
|---|---|
| Windows | Android Studio SDK + NDK r27+ (typically at `%LOCALAPPDATA%\Android\Sdk\ndk\<version>`); `cargo install cargo-ndk` (4.1+); `rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android` |
| macOS | Same as Windows for Android, plus Xcode + iOS SDK for iOS; `rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios` |
| Linux | Same as Windows for Android; iOS cross-compile from Linux is **not supported** (Apple's signing toolchain requires macOS) |

Export `ANDROID_NDK_HOME` to point at the NDK directory before invoking
`cargo ndk`.

## Per-target commands

### All four Android architectures in one invocation

```bash
ANDROID_NDK_HOME=/path/to/ndk \
cargo ndk \
  -t aarch64-linux-android \
  -t armv7-linux-androideabi \
  -t i686-linux-android \
  -t x86_64-linux-android \
  build -p vox-runtime-rn --release
```

Produces `libvox_runtime_rn.so` under each
`target/<triple>/release/` directory. Each `.so` is ~600-700 KB at
`--release`. Verified on Windows host with NDK r27c against
`cargo-ndk` 4.1.2.

### iOS (macOS-only)

```bash
for t in aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios; do
  cargo build --target "$t" -p vox-runtime-rn --release
done
lipo -create \
  target/aarch64-apple-ios/release/libvox_runtime_rn.a \
  target/aarch64-apple-ios-sim/release/libvox_runtime_rn.a \
  target/x86_64-apple-ios/release/libvox_runtime_rn.a \
  -output vox_runtime_rn.universal.a
```

(The universal `.a` is what the iOS Expo Module's `Podfile` links.)

## workspace-hack opt-out

The workspace's [`crates/workspace-hack`](../../../crates/workspace-hack/) crate
unifies dependency versions across the workspace to keep host builds fast.
But it transitively pulls in `gix` → `reqwest` → `native-tls` → `openssl-sys`,
and `openssl-sys` cannot cross-compile to `aarch64-linux-android` (or any
other Android architecture) without vendoring the OpenSSL source — a heavy
build-time dep that we'd rather avoid.

The mobile crates **`vox-runtime`** and **`vox-runtime-rn`** therefore
explicitly do not depend on `workspace-hack`. Each crate's `Cargo.toml`
has an inline note explaining why.

Trade-off: marginally slower host builds for these two crates (they
re-compile a few unification-shared deps independently). In exchange, the
crates ship to mobile.

## Gate in `crates/vox-cli-tests`

The test `mobile_cross_compile_aarch64_android_succeeds` (under
[`crates/vox-cli-tests/tests/mobile_cross_compile.rs`](../../../crates/vox-cli-tests/tests/mobile_cross_compile.rs))
invokes `cargo ndk -t aarch64-linux-android build -p vox-runtime-rn` and
asserts:

1. Exit code 0.
2. The output `.so` exists at the expected `target/<triple>/release/` path.

Skipped automatically when:
- `ANDROID_NDK_HOME` is not set.
- `cargo-ndk` is not on PATH.
- `VOX_CLI_TESTS_SKIP_NDK=1` is set (CI environments without NDK).

The gate runs against a single architecture (aarch64) because that's the
one users actually ship to; the other three architectures are exercised
during developer machine builds and EAS Build CI.

## CI integration

EAS Build runs the four-target Android cross-compile + the three-target iOS
cross-compile as part of its `eas-build-pre-install` hook (deferred per spec
§11.4 — to be wired when `vox build --target=mobile` starts producing the
Expo Module config that references the cdylib).
