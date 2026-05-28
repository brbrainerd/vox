# Compile suite fixture (workspace smoke)

Minimal **root `Vox.toml`** with **`[workspace.members]`** plus optional **`[bundle]`**, mirroring the multi-package compile journey in [`vox-application-packaging-ssot-2026.md`](../../docs/src/architecture/vox-application-packaging-ssot-2026.md).

This tree documents manifest shape; running **`vox compile --workspace`** from here still requires a full Vox toolchain, frontend deps, and Tauri/Android/Xcode prerequisites for non-native targets.

```bash
cd examples/compile-suite
vox compile --workspace --target native-binary
```

Members are compiled with their default `src/main.vox` entry (`vox compile` workspace resolver).
