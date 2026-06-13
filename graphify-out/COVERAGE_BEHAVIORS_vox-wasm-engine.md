# Semantic Behavior Map — `vox-wasm-engine`

Synthesized from 21 extracted Behavior claims across four test files (`wasm_engine_smoke.rs`, `wasm_env_integration.rs`, `wasm_exec_smoke.rs`, `wasm_fuel_integration.rs`), covering 10 distinct symbols.

## Summary

The crate's proven behavior is heavily skewed to the happy path. Construction (`WasmHost::new` / `with_fuel`), execution of a minimal WASI module, outcome accessors, builder methods, and small-input env forwarding are all confirmed to *work*. The **only error-path proof in the entire surface** is fuel exhaustion via `WasmHost::execute` (both host-budget and per-execution `fuel_override`). Every capability/security-shaped symbol — preopen modes, env forwarding, module loading — is proven only structurally or on clean inputs, never on its rejection/conflict/enforcement mode. The most actionable holes are the filesystem preopen surface (modes set but enforcement never tested) and `WasmHost::new` having no malformed-module rejection.

## Per-symbol behaviors

### `WasmHost::new`
- Constructs successfully without panic or error. *(happy only)*
- No error/edge proof. **No malformed-module / invalid-config rejection tested.**

### `WasmHost::with_fuel`
- Creates a host that runs a minimal WASI module successfully when the fuel budget is sufficient. *(happy only)*
- No edge proof (e.g. zero or extreme fuel budget at construction).

### `WasmHost::execute`
- Runs a minimal WASI module and returns a successful outcome. *(happy)*
- **Error path:** returns an error containing a fuel/trap hint when the module exhausts its fuel budget on an infinite loop. *(error — the one real failure proof)*
- **Error path:** returns an error with fuel/trap hint when `fuel_override` exhausts the budget. *(error)*
- Gap: no proof for non-fuel traps (unreachable, missing import, bad WASI version) or a normal non-zero exit.

### `WasmRunOutcome` (`success`, `exit_code`, `stdout_str`, `stderr_str`)
- `success()` returns `true` when `exit_code == 0` (incl. minimal module exiting 0) and `false` when non-zero. *(happy, both branches)*
- `exit_code` reflects the module's `proc_exit()` value. *(happy)*
- `stdout_str()` decodes stdout bytes to UTF-8. *(happy)*
- `stderr_str()` returns a non-empty string when stderr bytes are present. *(happy)*
- Gap: decode helpers never exercised on invalid/non-UTF-8 bytes.

### `Preopen::read_only` / `PreopenMode::ReadOnly`
- Builder sets mode to `ReadOnly` and preserves the guest path argument. *(happy, structural only)*
- **No enforcement proof:** no test that a guest write is actually denied.

### `Preopen::read_write` / `PreopenMode::ReadWrite`
- Builder sets mode to `ReadWrite`. *(happy, structural only)*
- **No enforcement proof:** no test that writes succeed, and no path-confinement/escape test.

### `WasmExecOpts::with_args`
- Populates `args` with the provided arguments. *(happy)*
- Initializes `preopens` as empty by default. *(happy, invariant-ish)*
- Gap: no integration proof that args reach the guest.

### `WasmExecOpts::env`
- Forwarding 0 / 1 / 2 clean entries → guest observes exactly that many env vars (no leakage of extra vars). *(happy, small inputs)*
- Gap: no proof of key collision/override semantics, empty values, or rejection of malformed/duplicate keys.

### `WasmExecOpts::fuel_override`
- Overrides the host fuel budget and causes early exhaustion on an infinite loop. *(happy, drives the execute error path)*
- Gap: no edge proof for `fuel_override = 0` or an override exceeding the host budget.

## Semantic gaps

The following symbols are proven **only on the happy path** despite contracts that clearly have a failure / empty / conflict / enforcement mode. Ranked by actionability:

1. **`Preopen::read_only` / `PreopenMode::ReadOnly` (security/capability surface).** The mode is set on the struct but nothing proves a `ReadOnly` preopen actually *denies a guest write*. This is the highest-value missing test — a validator that is never shown to reject.
2. **`Preopen::read_write` / `PreopenMode::ReadWrite`.** No proof writes are permitted, and no sandbox-confinement / path-escape test for a writable mount.
3. **`WasmExecOpts::env` (input-conflict surface).** Forwarding is proven to *not leak* on 0–2 clean entries, but key-collision/override behavior, empty values, and malformed/duplicate keys are untested.
4. **`WasmHost::new` (loader/validator).** Proven only to construct; no malformed/invalid wasm-module rejection path.
5. **`WasmHost::execute` non-fuel failures.** Only fuel exhaustion is proven; other traps (unreachable, missing import, bad WASI version) and ordinary non-zero exits are unproven.
6. **`WasmRunOutcome::stdout_str` / `stderr_str`.** UTF-8 decode proven only on valid bytes; behavior on non-UTF-8 output is unspecified by test.
7. **`WasmExecOpts::fuel_override` edges.** `0` and over-budget values untested.
8. **`WasmExecOpts::with_args` end-to-end.** Args set on the struct but never shown to reach the guest (the env surface has this integration proof; args does not).