# Language: JSON parse + access stdlib — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans.

> **amended (2026-05-08):** Server (Rust) target complete; **TS codegen lowering deferred** (Part B). The Json opaque type is registered in `typeck/builtins.rs` with all 11 accessor signatures (get_str/get_int/get_float/get_bool/get_object/get_array, is_null, length, at, keys, to_string). `std.json.parse(str) → Result[Json]` added to `builtin_registry::std_namespace_method_ty` and wired through `std_namespace_runtime_call` to a new `vox_actor_runtime::builtins::vox_json_parse`. Runtime ships an opaque `VoxJson(serde_json::Value)` wrapper with inherent methods matching the typeck signatures — Rust codegen's existing `obj.method(args)` emit dispatches naturally to these. Method `key` parameters are typed as `String` (not `&str`) so they accept the codegen's owned-string args without specialization. Existing `JsonModule.parse` left intact for backward compat. **TS deferred** because no `std.*` namespace has TS-side dispatch today (mirrors the regex plan deferral) — needs a separate `@vox/runtime/json` helper + std namespace dispatch infra. Reference doc + Part D app integration also deferred.

**Goal:** First-class JSON parsing and value access in Vox: `std.json.parse(s: str) to Result[Json]` plus typed accessors on the resulting value (`get_str`, `get_int`, `get_object`, `get_array`, `is_null`).

**Why now:** Blocks the vox-mental-tracker app's Phase 2 voice flow and Phase 4 export bundle assembly. Today, Vox endpoints emit JSON via raw string concatenation and consumers can't read it back inside Vox at all — every cross-endpoint data path either re-classifies inputs from scratch or routes through TS. JSON is also the on-disk shape of `payload_json` in the HealthEventLog, so any Vox-side filter or rollup that needs to peek into `mood_score` etc. is currently impossible.

**Architecture:**
- Add a `Json` opaque type (HIR-level `Ty::Named("Json")`, codegen-defined runtime).
- TS codegen: `parse` lowers to `JSON.parse` wrapped in `Result.Ok/Err`; accessors lower to `typeof x === "string"` / `Array.isArray(x)` / property reads.
- Rust codegen: backed by `serde_json::Value`; accessors map to `as_str`, `as_i64`, `as_object`, `as_array`, `is_null`.
- Stdlib registration via the existing `builtin_registry` pattern (see how `Speech` / `HTTP` modules are added in `typeck/builtins.rs`).

**Tech Stack:** Rust (compiler builtin registration + codegen), `serde_json`, native `JSON.parse`.

**Out of scope (defer):**
- Schema-typed JSON (`std.json.parse_typed[T](s)` returning a struct) — useful but depends on the struct-types plan landing first; tracked separately.
- JSON Pointer / JSONPath traversal.
- Streaming / incremental JSON.
- Mutation of parsed values (`set_field`).

---

## File Structure

**Created:**
- `examples/golden/json_stdlib.vox` — parse, then walk a small object.
- `docs/src/reference/stdlib-json.md` — user-facing reference.

**Modified:**
- `crates/vox-compiler/src/typeck/builtins.rs` — register `std.json` namespace; register methods on the `Json` type.
- `crates/vox-compiler/src/builtin_registry/**` — add entries for `parse`, `get_str`, `get_int`, `get_float`, `get_bool`, `get_object`, `get_array`, `is_null`, `length` (on arrays), `keys` (on objects).
- `crates/vox-compiler/src/codegen_ts/**` — lowering for each builtin (most are 1-2 lines).
- `crates/vox-compiler/src/codegen_rust/**` — lowering using `serde_json::Value`.

---

## Part A — Type registration

### Task A1: Add `Json` to the type system

- [ ] **Step 1:** Decide whether `Json` is its own primitive `Ty::Json` or a `Ty::Named("Json")` with builtin methods. Recommendation: `Ty::Named("Json")` — keeps the type system small; `builtins.rs` already has the methods-on-named-type pattern (see `Str`, `HTTPModule`, `SpeechModule`).
- [ ] **Step 2:** Register `"Json"` in the methods map with the accessor signatures listed below.

Method signatures (returned by accessors that may fail to find a typed value, use `Result` to surface "wrong shape" errors):

| Method | Signature |
| --- | --- |
| `get_str(key: str) to Result[str]` | object-only; `Err` if key missing or not a string |
| `get_int(key: str) to Result[int]` | object-only; `Err` if missing or not integer |
| `get_float(key: str) to Result[float]` | object-only |
| `get_bool(key: str) to Result[bool]` | object-only |
| `get_object(key: str) to Result[Json]` | object-only |
| `get_array(key: str) to Result[Json]` | object-only |
| `is_null() to bool` | any |
| `length() to int` | array-only; 0 otherwise |
| `at(index: int) to Result[Json]` | array-only |
| `keys() to List[str]` | object-only; empty otherwise |
| `to_string() to str` | re-serialize |

### Task A2: Add `std.json.parse` namespace builtin

- [ ] **Step 1:** Match the `Speech.transcribe` registration pattern in `typeck/builtins.rs`: top-level binding `"std.json"` of type `Ty::Named("JsonModule")`, then `parse(s: str) to Result[Json]` on `JsonModule`.

---

## Part B — TS codegen

### Task B1: Lower `std.json.parse`

- [ ] **Step 1:** Emit `(() => { try { return Ok(JSON.parse(s)); } catch (e) { return Err(String(e)); } })()` (or the project's standard Result wrapper).

### Task B2: Lower accessors

- [ ] For each accessor, emit a one-shot inline check or a small helper module. Recommendation: emit `import { jsonGetStr, jsonGetInt, ... } from "@vox/runtime/json"` once per file that uses any of these, then call the helper. Helper module lives in the existing TS runtime crate / package (see how `mobile.notify` is wired).

### Task B3: Golden snapshot

- [ ] Add `examples/golden/json_stdlib.vox` covering parse + nested `get_object` + `get_str`. Run the existing golden snapshot harness; commit the emitted TS.

---

## Part C — Rust codegen

### Task C1: Lower `std.json.parse`

- [ ] **Step 1:** Emit `serde_json::from_str::<serde_json::Value>(s).map_err(|e| e.to_string())` wrapped in the Vox `Result` type.

### Task C2: Lower accessors

- [ ] Each accessor is 2-5 lines using `as_str` / `as_i64` / `as_f64` / `as_bool` / `as_object` / `as_array` / `is_null`, mapping `None` to a `Result::Err("not <kind>")` and `Some(v)` to `Ok(v)` (cloning where needed since accessors borrow).

### Task C3: Golden snapshot

- [ ] Mirror the TS golden in `tests/golden_codegen_rust.rs`.

---

## Part D — Integration

### Task D1: Wire into vox-mental-tracker voice flow

- [ ] In `apps/vox-mental-tracker/src/main.vox`, the `VoicePage` save handler can now do:
  ```
  match std.json.parse(parsed_preview) {
      Ok(j) => {
          let kind = j.get_str("kind")?
          let payload = j.get_object("payload")?.to_string()
          let conf = j.get_float("confidence")?
          // ... save via record_event
      }
      Error(e) => status = "parse failed: " + e
  }
  ```
- [ ] Remove the planned `voice_*` extractor stubs.

### Task D2: Wire into export bundle hash

- [ ] `export_health_json_bundle` currently only hashes the CSV header; the full row-level hash is deferred to TS. With `Json` available we can read structured `recorded_at_monotonic` etc. from inside Vox, but full hashing of the row dump still belongs to the TS materializer pipeline (separate plan).

---

## Verification

- [ ] `cargo nextest run -p vox-compiler` passes.
- [ ] `vox check examples/golden/json_stdlib.vox` passes.
- [ ] `vox build examples/golden/json_stdlib.vox` produces TS and Rust that compile.
- [ ] After D1, `vox check apps/vox-mental-tracker/src/main.vox` still passes and the VoicePage flow round-trips end-to-end (see app Phase 2 plan).
