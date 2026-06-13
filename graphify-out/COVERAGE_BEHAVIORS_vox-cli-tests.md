# Semantic Behavior Map — `vox-cli-tests`

Synthesized from 26 extracted Behavior claims across three test files: `tests/build_e2e.rs` (mobile/web/desktop codegen E2E), `tests/mobile_cross_compile.rs` (iOS build), and `tests/workflow_runtime_no_db.rs` (feature-boundary).

## Summary

The crate is almost entirely a **codegen-shape verification harness**: it asserts that the same Vox source lowers to the correct *leaf primitives* per target (React Native vs React DOM vs Tauri desktop) and that runtime-adapter contracts are honored. 22 of 26 claims are happy-path string/shape assertions; only 4 reach invariant strength (cross-target parity of validation keys, state-mutation lowering, fixture pass-all, and the workflow-runtime feature boundary) and **none assert an error or rejection path**. The proven surface is strong on "right output for valid input" and silent on "what happens for invalid input."

## Per-symbol proven behaviors

### Codegen — for-loop / list rendering
- for loops lower to `.map()` with item access, not DOM elements
- mobile bodies emit `<View>`/`<Text>`, never `<div>`/`<p>`
- `key={itemvar}` injected on the first body element *(key() attribute injection)*
- mobile output never uses `className`
- Strength: happy only.

### Codegen — `@form` declarations
- mobile form imports from `react-native`; emits `<TextInput>`/`<Pressable>`, not HTML form/input/button
- mobile uses `onChangeText`, not `onChange`
- **invariant:** mobile and web form outputs share identical validation error keys and messages
- Strength: happy + one invariant (parity), no negative/divergence proof.

### Codegen — `routes` → Expo Router
- layout file imports from `expo-router`
- `/` → `app/index.tsx`, `/detail/:id` → `app/detail/[id].tsx` with correct import paths
- `package.json` `main` = `expo-router/entry`; `app.json` registers `expo-router` plugin
- Strength: happy only (single route shape).

### Codegen — cross-target Counter (`mobile_and_web_emit_differ_in_leaf_shape_not_in_logic`)
- mobile uses `<View>`/`<Pressable>`/`StyleSheet.create()`; web uses `<div>`/`onClick`/`className`
- same source → RN for mobile, React DOM for web
- **invariant:** state mutations lower to `set_n(expr)` identically across targets
- Strength: happy + one invariant.

### Codegen — Tauri desktop / scheduled jobs
- `@scheduled` fns registered via `vox_workflow_runtime::scheduled::register`; `main.rs` calls `scheduled::start()`
- `main.rs` embeds/registers HirModule via `load_hir_module_from_embedded`
- Strength: happy only.

### Runtime adapter contract (`mobile_emit_uses_adapter_contract_not_direct_tauri`)
- `mobile.ts` imports `@vox/runtime`, never `@tauri-apps/api`
- calls `voxRuntime.onBackButton()` through the adapter
- Strength: happy only (negative-import assertion is a useful guardrail but still happy-path).

### Fixture discovery / build gates (`build_every_fixture_passes_fast_path`)
- **invariant:** all fixtures under `tests/fixtures/` with `main.vox` pass `vox build` fast-path gates
- at least one such fixture exists
- Strength: invariant (all-pass) + happy; **no rejection path.**

### iOS cross-compile (`vox_runtime_rn_cross_compiles_to_aarch64_ios`)
- `vox-runtime-rn` builds as `aarch64-apple-ios` static lib (macOS-host-gated)
- produces `libvox_runtime_rn.a`
- Strength: happy only (host-gated skip is not an error path).

### Feature boundary (`vox_workflow_runtime_compiles_without_default_features`)
- **invariant:** `vox-workflow-runtime` builds with no default features, proving `sql` is the sole `vox-db` dependency
- Strength: invariant (structural).

## Semantic gaps

Symbols whose contract has an obvious failure/empty/conflict mode but are proven only on the happy path:

1. **`build_every_fixture_passes_fast_path` (validator with no rejection test).** Proves every fixture *passes* the fast-path gates, but nothing proves a malformed fixture is *rejected*. A gate that only ever sees passing input could be a no-op and these tests would still be green. **Most actionable:** add a deliberately-broken fixture (or an isolated bad-input case) and assert the gate fails with a diagnostic.

2. **Cross-target form validation parity (invariant with no negative case).** It asserts mobile and web share validation keys/messages, but there is no test that validation *rejects* bad input on either target, nor a case where the targets are expected to diverge. The parity could hold trivially over an empty/identical-but-wrong set.

3. **`vox build` fast-path gates as a mutator/validator surface.** All proofs are "valid source → correct output." No proof that an invalid source produces an error rather than malformed-but-passing output (e.g., a DOM tag leaking into a mobile target, a missing `key`, a route collision).

4. **iOS cross-compile failure surface.** Only the successful `.a` artifact is proven, and only when a macOS host is present. No coverage of the missing-toolchain / link-failure path, so a regression that breaks the build on real CI hosts is invisible here.

5. **`key()` injection edge cases.** Injection is proven for the simple list case only — empty loop bodies, missing/duplicate item vars, nested loops, and pre-existing `key` attributes (conflict mode) are unproven.

6. **Scheduled-jobs wiring.** Registration + `start()` are proven present, but there is no error path for registration failure, duplicate schedules, or an empty `@scheduled` set.

7. **Expo Router route lowering edges.** Single route shape proven; nested/conflicting routes and missing dynamic params (`[id]`) are unproven.

The highest-leverage fixes are **#1 and #3** — the fixture/build-gate surface is a validator currently proven only to accept, which is exactly the shape that hides no-op or over-permissive gates.