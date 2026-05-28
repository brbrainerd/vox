# Codegen TS bugs blocking vox-mental-tracker browser/Capacitor rendering

> **For agentic workers:** REQUIRED SUB-SKILLS: superpowers:systematic-debugging (root-cause each bug, then fix), superpowers:test-driven-development (golden snapshots per bug), superpowers:verification-before-completion.

**Goal:** Make the React/TSX emitted by `vox build apps/vox-mental-tracker/src/main.vox -o apps/vox-mental-tracker/dist` actually run in a browser.

**Why now:** Discovered while wiring the Playwright lane and the Vite scaffold for the tracker app. The Vox source (`main.vox`) `vox check`s clean and 34 vitest cases pass against the TS-side libraries — but the emitted dist/*.tsx has at least four issues that prevent any browser from running it. With these fixed, the tracker app boots end-to-end; without them, Phase 5's Playwright lane can only prove "wiring exists" by self-skipping.

This plan **does not** apply to vox-tracker specifically — every Vox app emitted today shares the same codegen path, so fixing each bug benefits the whole language surface. The repro steps reference the tracker because that's where they were caught; the fix locations are all under `crates/vox-compiler/src/codegen_ts/`.

---

## Bug A — `match` arms emit `case _:` literal patterns; bindings vanish

### Repro

`apps/vox-mental-tracker/dist/VoicePage.tsx:23` (after a fresh `vox build`):

```tsx
(() => (((_val) => { switch(_val) { case _: return (() => {
    set_transcript_raw(t);
    set_status("Transcript captured — tap Parse to classify.");
})(); case _: return (() => {
    set_status("Transcribe failed: " + e);
})(); } })(mobile.transcribe_microphone())));
```

### What's wrong

Vox source:
```vox
match Speech.transcribe_microphone() {
    Ok(t) => { transcript_raw = t; status = "..." }
    Error(e) => { status = "Transcribe failed: " + e }
}
```

Should compile to something that destructures `Ok(t)` and `Error(e)` (e.g. a tagged-union switch on `.kind` or `.tag`, with `t` and `e` bound from `.value`). Today the codegen emits:
- `case _:` (literal underscore identifier — JavaScript treats `_` as an identifier, the second `case _:` is an unreachable-code error in any modern compiler).
- `t` and `e` referenced in the bodies but never declared.

### Likely fix area

`crates/vox-compiler/src/codegen_ts/hir_emit/mod.rs` and `crates/vox-compiler/src/codegen_ts/jsx.rs` — wherever `HirExpr::Match` lowers to TS. Inspect the IIFE shape and the per-arm pattern emission. Tagged-union output (`switch(v.tag) { case "Ok": const t = v.value; ... }`) is the most TS-natural shape; alternatively a runtime helper `voxMatch(value, { Ok: (t) => ..., Error: (e) => ... })`.

### Test

Add `examples/golden/match_codegen_ts.vox` exercising `match Result[str] { Ok(x) => ..., Error(e) => ... }`. Snapshot the emitted TS and assert the snapshot compiles via `tsc --noEmit` on the snapshot in CI.

---

## Bug B — `Speech.method()` lowers to `mobile.method()`

### Repro

`apps/vox-mental-tracker/dist/VoicePage.tsx:32`:

```tsx
})(mobile.transcribe_microphone())));
```

Vox source: `Speech.transcribe_microphone()`.

### What's wrong

The `Speech` named binding is registered in `typeck/builtins.rs` (line ~404 of current main) as `Ty::Named("SpeechModule")`. Type-check passes. But the codegen TS lowering rewrites the call to `mobile.transcribe_microphone()` — wrong runtime namespace.

### Likely fix area

`crates/vox-compiler/src/codegen_ts/**` lookup of method calls on `Named("SpeechModule")` values. Probably an outdated mapping in a name-mapping table that pre-dated the Speech module rename.

### Test

`examples/golden/speech_call_ts.vox` calling `Speech.transcribe_microphone()`. Snapshot must contain `Speech.transcribe_microphone(` (or the agreed runtime symbol — never `mobile.`).

---

## Bug C — JSON-bearing string literals emitted with raw inner double quotes

### Repro

`apps/vox-mental-tracker/dist/Home.tsx:20`:

```tsx
const _ = record_event("mood_recorded", "{"mood_score":3}", str(ms), ...);
```

### What's wrong

The Vox source has `"{\"mood_score\":3}"` — escaped inner quotes. The codegen passes the unescaped Vox-internal string into a TS double-quoted string literal, breaking parse.

### Likely fix area

The TS literal emitter must JSON.stringify (or equivalent escape) string values that contain `"` characters. Look in `codegen_ts` for the function that wraps a string in `"..."` for emit; replace the wrap with `JSON.stringify(s)` — that handles `"`, `\`, control chars uniformly.

### Test

`examples/golden/string_literal_with_quotes.vox` — a function returning `"{\"k\":\"v\"}"`. Snapshot must produce a TS literal that parses (`tsc --noEmit` on the snapshot).

---

## Bug D — Endpoint calls + `std.*` references emit as bare identifiers (no imports)

### Repro

`dist/VoicePage.tsx:38, 50, 56`:

```tsx
const p = parse_voice(transcript_raw);          // no import
const ms = std.time.now_ms();                   // no import for std
record_event(parsed_kind, parsed_payload, ...) // no import
```

### What's wrong

Vox endpoints (`@endpoint(kind: query|mutation) fn ...`) are exposed via the generated `vox-client.ts` (or similar) — that file IS emitted. The component file references them but doesn't import them. Same for `std.*` builtins like `std.time.now_ms()` — these need a runtime helper import or a pure-JS replacement (`Date.now()`).

### Likely fix area

Two pieces:
1. The component emitter (`codegen_ts/component.rs`) needs to track every `@endpoint` it calls and emit `import { parse_voice, record_event, ... } from "./vox-client"` (or the right relative path) at file top.
2. `std.*` calls need either: (a) a runtime shim module imported once per file (`import * as std from "@vox/runtime/std"`), or (b) inlined replacements (`std.time.now_ms()` → `Date.now()`).

### Test

`examples/golden/endpoint_call_in_component.vox` — a component that calls an `@endpoint` and uses `std.time.now_ms()`. Snapshot must include the right `import` lines and the call sites.

---

## Sequence

These bugs are independent, but fixing them in the order **C → B → A → D** minimizes churn:

1. **C (string escapes)** is a one-line fix — `JSON.stringify` the literal — and unblocks any further snapshot diff signal because today the snapshots don't even parse as TS.
2. **B (Speech namespace)** is a small mapping fix.
3. **A (match arms)** is the meatiest — requires a tagged-union output shape and possibly a tiny runtime helper.
4. **D (imports)** depends on A's output shape decisions, since the runtime helper from A may itself need importing.

After all four land, `vox build apps/vox-mental-tracker/src/main.vox -o apps/vox-mental-tracker/dist` should produce TSX that:

- `tsc --noEmit -p apps/vox-mental-tracker/tsconfig.web.json` accepts (when that tsconfig is added by the app's Vite scaffold plan)
- Vite can bundle into a runnable `index.html` + JS that boots the Home page in a browser
- Playwright's `webServer` config can spin up against
- Capacitor `cap sync` can wrap into Android / iOS builds

## Out of scope here

- The vox-mental-tracker app's Vite scaffold itself (separate plan: `2026-05-08-app-vite-scaffold.md` — write after these bugs are landed).
- Replacing `Speech.transcribe_microphone()` with sherpa-onnx (Phase 3, blocked on platform PR #68).
- Any change to the `@endpoint`-to-API-client codegen pipeline beyond the missing-import fix in Bug D.

## Verification

- [ ] `cargo nextest run -p vox-compiler` — all golden snapshots pass.
- [ ] `vox build apps/vox-mental-tracker/src/main.vox -o /tmp/dist` — emits without errors.
- [ ] `tsc --noEmit` over the emitted /tmp/dist with a minimal tsconfig — exits 0.
- [ ] Once the Vite scaffold lands: `pnpm dev` boots the app at `http://127.0.0.1:5173/` and Home renders without console errors.
