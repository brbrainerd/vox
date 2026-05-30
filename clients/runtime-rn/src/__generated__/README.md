# Generated uniffi bindings — do not edit by hand

These files are produced by `uniffi-bindgen-react-native` from the Rust crate
[`crates/vox-runtime-rn/`](../../../../crates/vox-runtime-rn/). They expose
every `#[uniffi::export]`-annotated item from that crate as a TypeScript
binding callable from a React Native TurboModule.

## Regenerating

After any change to `crates/vox-runtime-rn/src/lib.rs`:

```bash
# from the repo root (or the worktree root)
cargo build -p vox-runtime-rn
cd clients/runtime-rn
npm run generate-bindings
```

The `generate-bindings` script in `package.json` shells out to:

```
npx -y uniffi-bindgen-react-native@latest \
  generate jsi bindings \
  --library --no-format \
  --ts-dir src/__generated__ \
  --cpp-dir ../../target/uniffi-bindgen-cpp-tmp \
  ../../target/debug/vox_runtime_rn.dll
```

(The library extension differs per host — `.dll` on Windows, `.so` on Linux,
`.dylib` on macOS. The script adapts via a small wrapper.)

## tsc

The generated files carry `// @ts-nocheck` at the top so they don't have to
participate in our `strict: true` regime. They're excluded from
`tsconfig.test.json`'s `include` glob so the contract test runs against
the hand-written `runtime.ts` only.

## What's in here

- `vox_runtime_rn.ts`     — the high-level TypeScript surface mirroring the
                            Rust public API (`VoxRuntimeHandle`, `VoxConfig`,
                            `RuntimeProfile`, `VoxRnError`,
                            `defaultDesktopConfig`, `defaultMobileConfig`)
- `vox_runtime_rn-ffi.ts` — the lower-level TurboModule loader + FFI shims
                            that `vox_runtime_rn.ts` calls into. Uses
                            `@ubjs/core` types provided by the
                            uniffi-bindgen-react-native runtime peer dep.
- `vox_runtime_rn.cpp` /
  `vox_runtime_rn.hpp`    — generated alongside under
                            `target/uniffi-bindgen-cpp-tmp/`; copied into
                            the Expo Module's iOS / Android native projects
                            by the EAS Build hook (deferred — see
                            `docs/src/architecture/mobile-rn-expo-implementation-spec-2026.md`
                            §11.4).
