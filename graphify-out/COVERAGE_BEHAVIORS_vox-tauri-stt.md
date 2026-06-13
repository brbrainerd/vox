# Semantic Behavior Map — `vox-tauri-stt`

A thin serde-only wire-contract crate for on-device speech-to-text (Android `SpeechRecognizer` / Apple `SFSpeechRecognizer`). The 5 extracted claims (3 tests) cover one data type (`TranscribeResult`), two string constants, and one guest-JS invoke-string contract. After dedup, four distinct symbols are proven. Every proof is either a happy-path serialization or a constant/invariant equality check; there are no validators, mutators, or integrity/security surfaces in the crate. The single meaningful semantic hole is the untested `None` branch of the `confidence` field's conditional serialization.

## `TranscribeResult::text`
- **Proven (happy):** Serializes to JSON as a string with the exact source value (`text: "hello"` → `v["text"] == "hello"`).
- Error path: none applicable (`String` is total). Edge/invariant: none (empty/Unicode strings not exercised, but no conditional logic on this field).

## `TranscribeResult::confidence`
- **Proven (happy):** `Option<f64>` serializes correctly when `Some(0.91)` → `v["confidence"] == 0.91`.
- **Not proven (edge):** The field carries `#[serde(skip_serializing_if = "Option::is_none")]`. The `None` branch — confidence omitted entirely from the JSON object — is never asserted. Round-trip deserialization of a payload lacking `confidence` is also untested.

## `PLUGIN_ID`
- **Proven (invariant):** Equals `"vox-stt"`, pinning the Tauri guest-JS plugin identifier. Constant; no error/edge modes.

## `TRANSCRIBE_COMMAND`
- **Proven (invariant):** Equals `"transcribe"`, pinning the invoke command name. Constant; no error/edge modes.

## guest-JS invocation contract
- **Proven (happy):** `guest-js/index.ts` invokes the plugin with the composed string `plugin:vox-stt|transcribe` (built from `PLUGIN_ID`/`TRANSCRIBE_COMMAND`), keeping the TypeScript facade and Rust constants in lockstep.
- Edge: relies on the file being present (`expect("read guest-js")`); a malformed/renamed guest file fails loudly, but no negative assertion that a *wrong* invoke string is rejected.

## Semantic gaps

Only one symbol has a genuine untested contract mode:

- **`TranscribeResult::confidence` — conditional-serialization `None` branch (most actionable).** The field's `skip_serializing_if = "Option::is_none"` is a real behavioral switch, yet only the `Some` arm is proven. A regression that drops the `skip_serializing_if` attribute (emitting `"confidence": null`) would pass all current tests while breaking the guest contract, which expects the field absent. Add a test asserting `TranscribeResult { text, confidence: None }` produces JSON with **no** `confidence` key, plus a deserialize round-trip from a confidence-less payload.

No other gaps of concern: `text` is a total `String` with no branch logic, and the constants / invoke-string are invariants with no failure mode beyond the already-asserted equality. The crate contains no validators (no rejection paths to test), no mutators (no failure paths), and no integrity/security surfaces.