# Oratio HTTP examples

These snippets complement **`Speech.transcribe(path)`** on the Rust/server tier and the **`POST /api/audio/transcribe`** endpoint documented in **`docs/src/reference/oratio-speech.md`** and **`docs/src/reference/codex-http-api.md`**.

## Files

- **`codexAudioTranscribe.ts`** — minimal `fetch` examples for `/api/audio/status` and `/api/audio/transcribe` (JSON body with `{ "path": "..." }`).

The TS **`Speech.transcribe`** builtin throws by design in browser bundles; use **`@server`** Rust routes or HTTP as shown here.

For **on-device** microphone transcription in Capacitor shells, use **`std.mobile.transcribe_microphone()`** / **`Speech.transcribe_microphone()`**, which lower to **`mobile.transcribe_microphone()`** and expect a native **`VoxSherpaTranscribe`** plugin.
