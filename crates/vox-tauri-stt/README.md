# vox-tauri-stt

On-device speech-to-text for **Tauri 2** apps:

- **`guest-js/index.ts`** — `transcribe()` via `@tauri-apps/api/core` `invoke`.
- **`android/.../SpeechRecognizerBridge.kt`** — `SpeechRecognizer` + `EXTRA_PREFER_OFFLINE` (ported from the mental-tracker Capacitor plugin; **no Capacitor**).
- **`ios/AppleSpeechBackend.swift`** — `SFSpeechRecognizer` + `AVAudioEngine`, `requiresOnDeviceRecognition = true`.

## Rust crate

- **Default:** [`TranscribeResult`](crate::TranscribeResult), [`PLUGIN_ID`](crate::PLUGIN_ID), [`TRANSCRIBE_COMMAND`](crate::TRANSCRIBE_COMMAND) — serde-only; no `tauri` dependency.
- **Feature `tauri-plugin`:** [`plugin::init`](crate::plugin::init) returns a registered Tauri 2 plugin (`invoke` id matches guest JS). On-device STT still requires wiring Kotlin/Swift into this command on mobile targets.

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

`build.rs` should register an **inlined** ACL plugin named `vox-stt` with command `transcribe` (Vox codegen emits this). Wire JNI (Android) and Swift glue per [Tauri mobile plugins](https://v2.tauri.app/develop/plugins/develop-mobile/).

## JS

```ts
import { transcribe } from "vox-tauri-stt/guest-js"; // path alias or copy
await transcribe();
```
