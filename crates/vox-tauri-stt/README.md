# vox-tauri-stt

Speech-to-text plugin surface for **desktop Tauri 2** apps:

- **`guest-js/index.ts`** — `transcribe()` via `@tauri-apps/api/core` `invoke`.
- **`src/plugin.rs`** — Tauri 2 plugin registration (feature `tauri-plugin`).

> **Scope note (ADR: scope-tauri-desktop-only).** Tauri is desktop-only in Vox.
> The Android (`SpeechRecognizerBridge.kt`) and iOS (`AppleSpeechBackend.swift`)
> sources that used to live in this crate were removed with that decision —
> mobile transcription belongs to the React Native target
> (`vox build --target=mobile`) via `@vox/runtime-rn`. A desktop STT backend is
> not wired yet; the `transcribe` command returns an explicit error until one is.

## Rust crate

- **Default:** [`TranscribeResult`](crate::TranscribeResult), [`PLUGIN_ID`](crate::PLUGIN_ID), [`TRANSCRIBE_COMMAND`](crate::TRANSCRIBE_COMMAND) — serde-only; no `tauri` dependency.
- **Feature `tauri-plugin`:** [`plugin::init`](crate::plugin::init) returns a registered Tauri 2 plugin (`invoke` id matches guest JS).

Embed in generated `src-tauri` / app crate:

```rust,ignore
tauri::Builder::default()
    .plugin(vox_tauri_stt::plugin::init())
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
```

`Cargo.toml`:

```toml
vox-tauri-stt = { path = "../crates/vox-tauri-stt", features = ["tauri-plugin"] }
```

`build.rs` should register an **inlined** ACL plugin named `vox-stt` with command `transcribe` (Vox codegen emits this).

## JS

```ts
import { transcribe } from "vox-tauri-stt/guest-js"; // path alias or copy
await transcribe();
```
