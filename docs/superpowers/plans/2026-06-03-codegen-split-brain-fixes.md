# Codegen Split-Brain Fixes (P0–P2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix four confirmed correctness/data-integrity bugs in the multi-target codegen, delete verified dead code, and begin collapsing the duplicate HTTP-endpoint-contract representations onto `ContractIr` — without changing the healthy HIR-core + projection architecture.

**Architecture:** Vox lowers `.vox` → AST → `HirModule` (the single source of truth) → typed projections (`WebIR`, `ContractIr`, `AppContract`, `ShellProjection`, `RequiredRuntimeCapabilities`) → two emitter families (`codegen_rust` for Axum/Tauri, `codegen_ts` for web/React-Native). This plan touches the emitters and one projection; it does **not** redesign the IR layer. Each Phase below is an independently shippable PR.

**Tech Stack:** Rust (codegen as string emission), `cargo test` (inline `#[cfg(test)]` modules), `cargo run -p vox-arch-check` (layer guard), `cargo run -p vox-cli -- check` (Vox doctest of any fenced `vox` block — none added here).

**Source advisory:** [`docs/src/architecture/codegen-ssot-and-split-brain-audit-2026.md`](../../src/architecture/codegen-ssot-and-split-brain-audit-2026.md). Every finding below was adversarially verified against current code.

**Scope note (per writing-plans skill):** This spans four independent subsystems (Tauri Rust emit, GUI manifest, web scaffold, endpoint-contract projection). They are sequenced here as one document because you asked for one plan, but Phases 1–4 have no ordering dependency on each other and should land as **separate PRs**. Phase 5 (P2) depends on nothing here but is the largest.

---

## File structure

| File | Responsibility | Touched in |
|---|---|---|
| [`crates/vox-codegen/src/codegen_rust/emit/mod.rs`](../../../crates/vox-codegen/src/codegen_rust/emit/mod.rs) | Tauri workspace emit (`generate_tauri_workspace`, `emit_tauri_main_rs`, STT gating) | Phase 1, Phase 2 |
| [`crates/vox-codegen/src/codegen_rust/mod.rs`](../../../crates/vox-codegen/src/codegen_rust/mod.rs) | Inline `#[cfg(test)]` harness for Rust emit | Phase 1, Phase 2 |
| [`crates/vox-compiler/src/required_capabilities.rs`](../../../crates/vox-compiler/src/required_capabilities.rs) | Capability-id derivation from HIR (gains a `microphone` id) | Phase 2 |
| [`crates/vox-gui/src/commands/action_manifest.rs`](../../../crates/vox-gui/src/commands/action_manifest.rs) | GUI ActionManifest platform flags | Phase 3 |
| [`crates/vox-codegen/src/codegen_ts/scaffold.rs`](../../../crates/vox-codegen/src/codegen_ts/scaffold.rs) | One-shot web config scaffold (`package.json`) | Phase 4 |
| [`crates/vox-codegen/src/codegen_ts/component.rs`](../../../crates/vox-codegen/src/codegen_ts/component.rs) | Dead `generate_component*` removal (keep `ts_default_value`, `map_vox_type_to_ts`) | Phase 4 |
| [`crates/vox-codegen/src/codegen_ts/activity.rs`](../../../crates/vox-codegen/src/codegen_ts/activity.rs) | Orphan file (never declared in `mod.rs`) — delete | Phase 4 |
| [`crates/vox-compiler/src/app_contract.rs`](../../../crates/vox-compiler/src/app_contract.rs) | `AppContract` projection (derive endpoints from `ContractIr`) | Phase 5 |
| [`crates/vox-codegen/src/codegen_shared/route_ir.rs`](../../../crates/vox-codegen/src/codegen_shared/route_ir.rs) | Dead `RouteIR` (delete or wire) | Phase 5 |

**Test-harness pattern (reused throughout).** Inline tests in `codegen_rust/mod.rs` build a module and assert on emitted files:

```rust
let module = empty_module();                 // HirModule::default()
let out = pipeline::generate(&module, "pkg", RustAppShell::TauriApp).unwrap();
let main = out.files.get("src-tauri/src/main.rs").expect("src-tauri main.rs");
assert!(main.contains("…"), "{main}");
```

`HirFn` does **not** derive `Default`, so fixtures that need a function (e.g. a `@scheduled` fn) lower from source via the frontend instead of hand-building a `HirFn`:

```rust
// Pattern mirrored from crates/vox-compiler/tests/ir_emission_test.rs
let res = vox_compiler::pipeline::run_frontend_str(src).expect("frontend ok");
let module = res.hir;
```

---

## Phase 1 — Wire `@scheduled` into the Tauri target (P0, correctness)

**Decision recorded:** full support (not fail-fast). Generated Tauri apps must actually run `@scheduled` jobs, matching the Axum path.

**Background (verified):** The Axum path calls `emit_durable_boot_prelude(module, "vox_durable_db", true, BootPropagation::Expect)` then appends `emit_durable_boot_helpers(module)` ([`http.rs:322`](../../../crates/vox-codegen/src/codegen_rust/emit/http.rs:322), [`main_boot.rs:173`](../../../crates/vox-codegen/src/codegen_rust/emit/main_boot.rs:173), [`main_boot.rs:270`](../../../crates/vox-codegen/src/codegen_rust/emit/main_boot.rs:270)). `emit_tauri_main_rs` ([`mod.rs:289`](../../../crates/vox-codegen/src/codegen_rust/emit/mod.rs:289)) does neither, so `@scheduled` fns compile into `lib.rs` but are never registered — silently dead at runtime.

**Key design constraint:** Tauri's `fn main()` is **synchronous** (it ends in `tauri::Builder…run()`), but `emit_durable_boot_prelude` emits `.await` call sites (it assumes an async scope). The prelude must therefore run inside Tauri's async runtime, in `.setup()`, and the returned `scheduled_handle` must be kept alive for the app's lifetime. We run the prelude with `tauri::async_runtime::block_on(...)` inside `setup` (so registration completes before the window shows) and `std::mem::forget` the handle to keep the scheduler task alive.

### Task 1.1: Failing test — Tauri main registers a `@scheduled` fn

**Files:**
- Test: [`crates/vox-codegen/src/codegen_rust/mod.rs`](../../../crates/vox-codegen/src/codegen_rust/mod.rs) (inside the existing `#[cfg(test)] mod tests`)

- [ ] **Step 1: Write the failing test**

Add near the other `RustAppShell::TauriApp` tests (after `tauri_workspace_cargo_excludes_axum_from_root`):

```rust
#[test]
fn tauri_main_registers_scheduled_fns() {
    let src = r#"
@scheduled("1m")
fn heartbeat() { }
"#;
    let res = vox_compiler::pipeline::run_frontend_str(src).expect("frontend ok");
    let module = res.hir;
    let out = pipeline::generate(&module, "pkg", RustAppShell::TauriApp).unwrap();
    let main = out
        .files
        .get("src-tauri/src/main.rs")
        .expect("src-tauri main.rs");
    assert!(
        main.contains("vox_workflow_runtime::scheduled::register"),
        "Tauri main must register @scheduled fns: {main}"
    );
    assert!(
        main.contains("scheduled::start"),
        "Tauri main must start the scheduler: {main}"
    );
    assert!(
        main.contains("load_hir_module_from_embedded"),
        "Tauri main must embed + register the HirModule for workflow lookup: {main}"
    );
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p vox-codegen tauri_main_registers_scheduled_fns -- --nocapture`
Expected: FAIL — the emitted main contains none of those markers (and possibly a compile error if `run_frontend_str`'s exact name/return field differs — if so, confirm against `crates/vox-compiler/tests/ir_emission_test.rs` and adjust the two-line lowering preamble only).

- [ ] **Step 3: Commit the red test**

```bash
git add crates/vox-codegen/src/codegen_rust/mod.rs
git commit -m "test(codegen): Tauri main must register @scheduled fns (red)"
```

### Task 1.2: Emit the durable-boot prelude inside Tauri `setup`

**Files:**
- Modify: [`crates/vox-codegen/src/codegen_rust/emit/mod.rs:289-382`](../../../crates/vox-codegen/src/codegen_rust/emit/mod.rs:289) (`emit_tauri_main_rs`) and `generate_tauri_workspace` (to append helpers)

- [ ] **Step 1: Import the boot helpers**

At the top of `emit_tauri_main_rs` (or the module `use` block in `mod.rs`), ensure these are in scope (they are `pub` in `main_boot`):

```rust
use super::main_boot::{emit_durable_boot_prelude, emit_durable_boot_helpers, BootPropagation};
```

- [ ] **Step 2: Inject the prelude into the generated `setup` closure**

In `emit_tauri_main_rs`, the generated `fn main()` currently builds `tauri::Builder` with an optional `.setup` only when `has_tables`. Replace the builder assembly so the durable boot runs in `setup` whenever the module has `@scheduled` fns **or** tables. Compute once, before the builder string:

```rust
let has_scheduled = module
    .functions
    .iter()
    .any(|f| f.schedule_interval.is_some());
let needs_setup = has_tables || has_scheduled;
```

Then emit a single `.setup(|app| { … })` block when `needs_setup`. Inside it, after the existing DB-manage code (keep that), append the durable boot prelude wrapped so it runs to completion on Tauri's runtime. The prelude binds `vox_durable_db` itself (`include_db_connect = true`) and emits `.await.expect(...)` sites, so wrap it in `block_on`:

```rust
if needs_setup {
    out.push_str("        .setup(|app| {\n");
    if has_tables {
        // EXISTING Codex managed-state block stays here (mod.rs:367-374).
        out.push_str(r#"            // Secret-policy-compliant resolution (mirrors emit_db_setup):
            // resolve_canonical() reads VOX_DB_* / legacy TURSO_* / local file.
            // Codex::connect is async; .setup is sync, so block_on it.
            let db = tauri::async_runtime::block_on(async {
                let cfg = vox_db::DbConfig::resolve_canonical()
                    .expect("resolve Codex DB config (VOX_DB_URL+TOKEN, or VOX_DB_PATH)");
                vox_db::Codex::connect(cfg).await.expect("Failed to open Codex database")
            });
            app.manage(std::sync::Arc::new(db));
"#);
    }
    if has_scheduled {
        out.push_str("            tauri::async_runtime::block_on(async {\n");
        // Prelude binds `vox_durable_db` (Arc<vox_db::VoxDb>), registers the
        // process-global HirModule, and registers+starts every @scheduled fn.
        out.push_str(&emit_durable_boot_prelude(
            module,
            "vox_durable_db",
            /* include_db_connect = */ true,
            BootPropagation::Expect,
        ));
        // Keep the scheduler task alive for the app's lifetime.
        out.push_str("                std::mem::forget(scheduled_handle);\n");
        out.push_str("            });\n");
    }
    out.push_str("            Ok(())\n");
    out.push_str("        })\n");
}
```

Remove the old standalone `if has_tables { .setup… }` block (mod.rs:366-375) — it is now folded into the unified `needs_setup` block above.

- [ ] **Step 3: Append the embed helper after `main()` closes**

`emit_tauri_main_rs` returns `out` ending in `}\n` for `fn main()`. Before returning, append the helper (mirrors `http.rs`):

```rust
if has_scheduled {
    out.push_str("\n");
    out.push_str(&emit_durable_boot_helpers(module));
}
out
```

- [ ] **Step 4: Run the Task 1.1 test to verify it passes**

Run: `cargo test -p vox-codegen tauri_main_registers_scheduled_fns -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Run the full Rust-emit test module to catch regressions**

Run: `cargo test -p vox-codegen codegen_rust`
Expected: PASS — existing Tauri tests (`rust_app_shell_marker_tauri_in_main_rs` etc.) still pass because an `empty_module()` has no `@scheduled` fns, so `needs_setup` is false and their output is unchanged.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-codegen/src/codegen_rust/emit/mod.rs
git commit -m "feat(codegen): wire @scheduled durable boot into the Tauri target"
```

### Task 1.3: End-to-end guard — generated Tauri app with `@scheduled` compiles

**Files:**
- Test: [`crates/vox-cli-tests/tests/build_e2e.rs`](../../../crates/vox-cli-tests/tests/build_e2e.rs) (assert_cmd subprocess harness)

- [ ] **Step 1: Add an e2e test that builds a Tauri target and `cargo check`s it**

Mirror the existing `mobile_and_web_emit_differ_in_leaf_shape_not_in_logic` test in the same file for structure (temp dir, `vox build`, then assert). Build a `.vox` with one `@scheduled` fn to a desktop/Tauri target and assert the generated `src-tauri/src/main.rs` contains `scheduled::register`. (A full `cargo check` of the generated Tauri crate requires the Tauri toolchain; if CI lacks it, assert on the emitted file content instead and leave a `// TODO(ci): cargo check when tauri toolchain available` — string assertion is the deterministic guard.)

- [ ] **Step 2: Run it**

Run: `cargo test -p vox-cli-tests scheduled` (adjust filter to the new test name)
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/vox-cli-tests/tests/build_e2e.rs
git commit -m "test(e2e): generated Tauri app wires @scheduled jobs"
```

---

## Phase 2 — Gate the STT plugin on actual speech use (P0, bloat/permissions)

**Background (verified):** `emit_tauri_build_rs`, `emit_tauri_main_rs` (`.plugin(vox_tauri_stt::plugin::init())`, mod.rs:357), and `emit_tauri_default_capability_json` (mod.rs:384, `"vox-stt:default"`) are emitted **unconditionally**, unlike MCP emission which is gated (mod.rs:241). The plugin errors on every platform when unused. Speech is surfaced through the `Speech.transcribe`/`transcribe_microphone` builtins; the architecturally-correct gate is the capability projection (`bundle.capabilities`), consistent with [`runtime-capabilities.v1.yaml`](../../../contracts/capability/runtime-capabilities.v1.yaml).

### Task 2.1: Derive a `microphone` capability when speech is used

**Files:**
- Modify: [`crates/vox-compiler/src/required_capabilities.rs`](../../../crates/vox-compiler/src/required_capabilities.rs)
- Test: same file (inline `#[cfg(test)]`)

- [ ] **Step 1: Read the existing derivation**

Read `required_capabilities.rs` (esp. `project_required_capabilities`, line ~241) to see how existing ids (`net.http`, `deep_link`, `notifications`) are detected from HIR. Follow that exact pattern for the new id; do not invent a new mechanism.

- [ ] **Step 2: Write the failing test**

```rust
#[test]
fn microphone_capability_emitted_when_speech_used() {
    let src = r#"
fn note() -> Result[str] { Speech.transcribe_microphone() }
"#;
    let res = vox_compiler::pipeline::run_frontend_str(src).expect("frontend ok");
    let caps = project_required_capabilities(&res.hir);
    assert!(
        caps.iter().any(|c| c == "microphone"),
        "speech use must derive the microphone capability: {caps:?}"
    );
}

#[test]
fn no_microphone_capability_without_speech() {
    let res = vox_compiler::pipeline::run_frontend_str("fn f() { }").expect("frontend ok");
    let caps = project_required_capabilities(&res.hir);
    assert!(!caps.iter().any(|c| c == "microphone"), "{caps:?}");
}
```

(Adjust `project_required_capabilities`'s return-type access to match what the file actually returns — a `Vec<String>` or a wrapper with an ids accessor.)

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p vox-compiler microphone_capability`
Expected: FAIL (no `microphone` id derived yet).

- [ ] **Step 4: Implement detection following the existing pattern**

Add a HIR scan (reuse the same expr-walk the file already uses for other capabilities) that detects a call into the `Speech` namespace / `transcribe_microphone` builtin, and push the `"microphone"` id into the sorted set. Keep the output sorted/deduped as the existing code does.

- [ ] **Step 5: Run to verify pass**

Run: `cargo test -p vox-compiler microphone_capability`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-compiler/src/required_capabilities.rs
git commit -m "feat(capabilities): derive microphone capability from Speech use"
```

### Task 2.2: Gate the three STT emit sites on the capability

**Files:**
- Modify: [`crates/vox-codegen/src/codegen_rust/emit/mod.rs`](../../../crates/vox-codegen/src/codegen_rust/emit/mod.rs) (`generate_tauri_workspace`, `emit_tauri_main_rs`, `emit_tauri_build_rs`, `emit_tauri_default_capability_json`)
- Test: [`crates/vox-codegen/src/codegen_rust/mod.rs`](../../../crates/vox-codegen/src/codegen_rust/mod.rs)

- [ ] **Step 1: Write the failing tests AND fix the now-wrong existing test**

The existing `rust_app_shell_marker_tauri_in_main_rs` (mod.rs:457-460) asserts STT registration for an `empty_module()` — that becomes incorrect once gating lands. Update it to use a speech-using module, and add the negative case:

```rust
// Replace the STT assertion in rust_app_shell_marker_tauri_in_main_rs with
// the marker-only assertion (the `rust_app_shell=TauriApp` line), and move
// STT assertions into these two dedicated tests:

#[test]
fn tauri_emits_stt_only_when_speech_used() {
    let src = r#"fn note() -> Result[str] { Speech.transcribe_microphone() }"#;
    let module = vox_compiler::pipeline::run_frontend_str(src).expect("frontend ok").hir;
    let out = pipeline::generate(&module, "pkg", RustAppShell::TauriApp).unwrap();
    let main = out.files.get("src-tauri/src/main.rs").unwrap();
    let build = out.files.get("src-tauri/build.rs").unwrap();
    let cap = out.files.get("src-tauri/capabilities/default.json").unwrap();
    assert!(main.contains("vox_tauri_stt::plugin::init()"), "{main}");
    assert!(build.contains("\"vox-stt\""), "{build}");
    assert!(cap.contains("vox-stt:default"), "{cap}");
}

#[test]
fn tauri_omits_stt_without_speech() {
    let out = pipeline::generate(&empty_module(), "pkg", RustAppShell::TauriApp).unwrap();
    let main = out.files.get("src-tauri/src/main.rs").unwrap();
    let build = out.files.get("src-tauri/build.rs").unwrap();
    let cap = out.files.get("src-tauri/capabilities/default.json").unwrap();
    assert!(!main.contains("vox_tauri_stt"), "no STT plugin without speech: {main}");
    assert!(!build.contains("vox-stt"), "no STT ACL without speech: {build}");
    assert!(!cap.contains("vox-stt"), "no STT permission without speech: {cap}");
}
```

Also update `tauri_emit_registers_sherpa_acl_in_build_rs` (mod.rs:464) to lower the speech source instead of `empty_module()`.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p vox-codegen tauri_`
Expected: `tauri_omits_stt_without_speech` FAILS (STT currently always emitted).

- [ ] **Step 3: Implement the gate**

In `generate_tauri_workspace`, after `let bundle = project_bundle_from_hir(module);`, compute:

```rust
let needs_stt = bundle.capabilities.iter().any(|c| c == "microphone");
```

Thread `needs_stt` into `emit_tauri_main_rs`, `emit_tauri_build_rs`, and `emit_tauri_default_capability_json` (add a `bool` parameter to each). In each:
- `emit_tauri_main_rs`: only push the `.plugin(vox_tauri_stt::plugin::init())\n` line when `needs_stt`.
- `emit_tauri_build_rs`: emit the bare `tauri_build::build()` form when `!needs_stt`, and the InlinedPlugin ACL form only when `needs_stt`.
- `emit_tauri_default_capability_json`: include `"vox-stt:default"` in `permissions` only when `needs_stt` (otherwise just `["core:default"]`).
- `emit_cargo_toml_tauri_app`: gate the `vox-tauri-stt` dependency line on `needs_stt` too (otherwise an unused dep).

(Confirm `bundle.capabilities` is a `Vec<String>` of ids; if it is a wrapper, use its ids accessor.)

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p vox-codegen tauri_`
Expected: PASS (both new tests + updated existing ones).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-codegen/src/codegen_rust/emit/mod.rs crates/vox-codegen/src/codegen_rust/mod.rs
git commit -m "fix(codegen): emit STT plugin only when the module uses speech"
```

---

## Phase 3 — Fix ActionManifest mobile flag for CLI actions (P0, data integrity)

**Background (verified):** Every `ActionHandlerKind::Cli` action gets `platform: { desktop: true, mobile: true }` ([`action_manifest.rs:241-244`](../../../crates/vox-gui/src/commands/action_manifest.rs:241)), but CLI actions execute only via the Tauri `vox` sidecar, which does not exist on mobile. MCP actions correctly use `mobile: false` (action_manifest.rs:274-277).

### Task 3.1: CLI actions advertise `mobile: false`

**Files:**
- Modify: [`crates/vox-gui/src/commands/action_manifest.rs:241-244`](../../../crates/vox-gui/src/commands/action_manifest.rs:241)
- Test: same file (inline `#[cfg(test)]`) or the existing action_manifest test module

- [ ] **Step 1: Write the failing test**

Add a test that builds the manifest and asserts every CLI-kind action has `mobile == false`:

```rust
#[test]
fn cli_actions_are_not_advertised_on_mobile() {
    let manifest = build_action_manifest();
    for a in manifest.actions.iter().filter(|a| a.handler_kind == ActionHandlerKind::Cli) {
        assert!(
            !a.platform.mobile,
            "CLI action `{}` claims mobile support but only runs via the Tauri sidecar",
            a.id
        );
    }
}
```

(Match `build_action_manifest`'s real name/signature; if it needs inputs, follow the construction used by the existing tests in this file.)

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p vox-gui cli_actions_are_not_advertised_on_mobile`
Expected: FAIL.

- [ ] **Step 3: Implement the one-line fix**

At action_manifest.rs:241-244 change the CLI `ActionPlatform` to:

```rust
            platform: ActionPlatform {
                desktop: true,
                mobile: false,
            },
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p vox-gui cli_actions_are_not_advertised_on_mobile`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/src/commands/action_manifest.rs
git commit -m "fix(gui): CLI actions are not available on mobile (sidecar-only)"
```

---

## Phase 4 — Remove dead code & a bogus scaffold dependency (P0/P1)

### Task 4.1: Drop `react-router` from the web scaffold

**Files:**
- Modify: [`crates/vox-codegen/src/codegen_ts/scaffold.rs:104`](../../../crates/vox-codegen/src/codegen_ts/scaffold.rs:104)
- Test: same file (inline `#[cfg(test)]`)

**Background (verified):** The emitted `vox-app.tsx` ships a dependency-free history-API router and a test asserts it never imports `react-router` ([`web_entry.rs:397`](../../../crates/vox-codegen/src/codegen_ts/web_entry.rs:397)), yet `scaffold.rs:104` lists `"react-router": "^7.0.0"` as a runtime dependency.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn scaffold_package_json_has_no_react_router() {
    let files = web_config_files(); // the fn that returns the (name, contents) pairs
    let pkg = files.iter().find(|(n, _)| n == "package.json").map(|(_, c)| c).expect("package.json");
    assert!(!pkg.contains("react-router"), "scaffold must not depend on react-router: {pkg}");
}
```

(Use the real accessor name for the scaffold file list — `web_config_files` per scaffold.rs.)

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p vox-codegen scaffold_package_json_has_no_react_router`
Expected: FAIL.

- [ ] **Step 3: Delete the dependency line**

Remove this line from the `package.json` string literal at scaffold.rs:104:

```
    "react-router": "^7.0.0",
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p vox-codegen scaffold_package_json_has_no_react_router`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-codegen/src/codegen_ts/scaffold.rs
git commit -m "fix(codegen): drop unused react-router dep from web scaffold"
```

### Task 4.2: Delete the orphan `activity.rs`

**Files:**
- Delete: [`crates/vox-codegen/src/codegen_ts/activity.rs`](../../../crates/vox-codegen/src/codegen_ts/activity.rs)

**Background (verified):** `activity.rs` is not declared in [`codegen_ts/mod.rs`](../../../crates/vox-codegen/src/codegen_ts/mod.rs), so Cargo never compiles it. Nothing references `codegen_ts::activity`.

- [ ] **Step 1: Confirm it is undeclared and unreferenced**

Run: `cargo run -p vox-codegen 2>&1 | rg activity` (expect nothing) and grep the workspace:
Run: `git grep -n "codegen_ts::activity\|mod activity"`
Expected: no `mod activity;` line and no external references.

- [ ] **Step 2: Delete the file**

```bash
git rm crates/vox-codegen/src/codegen_ts/activity.rs
```

- [ ] **Step 3: Verify the crate still builds**

Run: `cargo build -p vox-codegen`
Expected: success, no change in output.

- [ ] **Step 4: Commit**

```bash
git commit -m "chore(codegen): delete orphan activity.rs (never compiled)"
```

### Task 4.3: Delete the dead classic-component emitter

**Files:**
- Modify: [`crates/vox-codegen/src/codegen_ts/component.rs`](../../../crates/vox-codegen/src/codegen_ts/component.rs)

**Background (verified):** `generate_component` and `generate_component_from_web_ir` have zero live callsites (the emit loop calls only `generate_reactive_component`). Their private helpers (`emit_component_stmt`/`expr`/`pattern`, `uses_mobile_ident_in_*`) are only used by them. **Keep** `ts_default_value` (used at emitter.rs:454) and `map_vox_type_to_ts`.

- [ ] **Step 1: Re-confirm zero live callers before deleting**

Run: `git grep -n "generate_component_from_web_ir\|generate_component\b"`
Expected: only definitions in `component.rs` (ignore the unrelated `vox-cli` `v0::generate_component`). If any live caller appears, STOP and reassess.

- [ ] **Step 2: Delete the dead functions and their private helpers**

Remove `generate_component`, `generate_component_from_web_ir`, and the private helpers only they use (`emit_component_stmt`, `emit_component_expr`, `emit_component_pattern`, `uses_mobile_ident_in_stmt`, `uses_mobile_ident_in_expr`). Remove the now-unused `use crate::codegen_ts::jsx::…` imports at the top of `component.rs`. **Do not** remove `ts_default_value` or `map_vox_type_to_ts`.

- [ ] **Step 3: Verify the crate builds and tests pass**

Run: `cargo build -p vox-codegen && cargo test -p vox-codegen`
Expected: success. If the compiler reports a now-unused import in `jsx.rs` or elsewhere, remove only the dead import (do **not** delete `jsx.rs` — it still re-exports `map_jsx_attr_name`/`map_jsx_tag` used by tests).

- [ ] **Step 4: Run the arch guard**

Run: `cargo run -p vox-arch-check`
Expected: PASS (no new orphan/inversion).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-codegen/src/codegen_ts/component.rs
git commit -m "chore(codegen): remove dead classic @component emit path"
```

---

## Phase 5 — Collapse endpoint contracts onto `ContractIr` (P2, SSOT)

**Background (verified):** Four structures re-derive HTTP-endpoint metadata from `HirModule.endpoint_fns`, but only three are live: `AppContract` (Axum, [`http.rs:374-399`](../../../crates/vox-codegen/src/codegen_rust/emit/http.rs:374)), `ContractIr` (TS zod/openapi/client), and `WebIR RouteNode` (validation-only). **`RouteIR` is dead** — `lower_module_routes` is called only from its own `#[cfg(test)]` block. The one field `ContractIr` lacks for `AppContract` is `wraps_db_transaction` (module-level: `!tables.is_empty()`), which must be preserved.

> Land 5.1 and 5.2 as **separate commits/PRs**; 5.1 is a safe deletion, 5.2 is a behavior-preserving refactor with a real risk (the transaction flag).

### Task 5.1: Delete the dead `RouteIR`

**Files:**
- Delete or empty: [`crates/vox-codegen/src/codegen_shared/route_ir.rs`](../../../crates/vox-codegen/src/codegen_shared/route_ir.rs)
- Modify: [`crates/vox-codegen/src/codegen_shared/mod.rs`](../../../crates/vox-codegen/src/codegen_shared/mod.rs) (remove the `pub mod route_ir;`)

- [ ] **Step 1: Confirm no non-test callers**

Run: `git grep -n "lower_module_routes\|route_ir::"`
Expected: references only inside `route_ir.rs` itself (definition + its `#[cfg(test)]`). If any production caller exists, STOP — switch to "wire it as SSOT" instead of deleting.

- [ ] **Step 2: Remove the module**

```bash
git rm crates/vox-codegen/src/codegen_shared/route_ir.rs
```

Remove `pub mod route_ir;` from `codegen_shared/mod.rs`.

- [ ] **Step 3: Build, test, arch-check**

Run: `cargo build -p vox-codegen && cargo test -p vox-codegen && cargo run -p vox-arch-check`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-codegen/src/codegen_shared/mod.rs
git commit -m "chore(codegen): remove dead RouteIR (only test callsite)"
```

### Task 5.2: Derive `AppContract` endpoints from `ContractIr`

**Files:**
- Modify: [`crates/vox-compiler/src/app_contract.rs`](../../../crates/vox-compiler/src/app_contract.rs) (`project_app_contract`)
- Test: same file (inline `#[cfg(test)]`)

- [ ] **Step 1: Read both projections side by side**

Read `app_contract.rs` (`project_app_contract`, `AppServerFnContract`, `AppMutationContract`) and `contract_ir/mod.rs` (`ContractEndpoint`, `project`). Confirm the field mapping: `AppServerFnContract.name`←`ContractEndpoint.name`, `.route_path`←`.path`, kind grouping←`ContractEndpoint.kind`. Note that `AppMutationContract.wraps_db_transaction` has no `ContractIr` source.

- [ ] **Step 2: Write a characterization test (golden) BEFORE refactoring**

Lock current behavior so the refactor can't change output:

```rust
#[test]
fn app_contract_endpoints_match_contract_ir() {
    let src = r#"
@table type Item { id: int, name: str }
@endpoint(kind: query)    fn list_items() -> [Item] { db.Item.all() }
@endpoint(kind: mutation) fn add_item(name: str) -> Item { db.Item.insert(Item { name }) }
"#;
    let hir = vox_compiler::pipeline::run_frontend_str(src).expect("frontend ok").hir;
    let app = project_app_contract(&hir);
    // server/query/mutation names + paths line up with ContractIr endpoints
    let cir = vox_compiler::contract_ir::project(&hir);
    let app_paths: std::collections::BTreeSet<_> =
        app.query_fns.iter().map(|f| (f.name.clone(), f.route_path.clone())).collect();
    let cir_query_paths: std::collections::BTreeSet<_> = cir
        .endpoints.iter().filter(|e| e.is_query()) // match the real kind predicate
        .map(|e| (e.name.clone(), e.path.clone())).collect();
    assert_eq!(app_paths, cir_query_paths);
    // transaction flag preserved for mutations touching tables
    assert!(app.mutation_fns.iter().any(|m| m.wraps_db_transaction));
}
```

(Adjust `is_query()`/kind predicate and field names to the real `ContractEndpoint` API found in Step 1.)

- [ ] **Step 3: Run the characterization test against current code**

Run: `cargo test -p vox-compiler app_contract_endpoints_match_contract_ir`
Expected: PASS now (it documents existing behavior). If it fails, the mapping assumption is wrong — fix the test to match reality before refactoring.

- [ ] **Step 4: Refactor `project_app_contract` to consume `ContractIr`**

Replace the three independent `module.endpoint_fns` iterations with a single `contract_ir::project(module)` call, mapping `ContractEndpoint` → `AppServerFnContract`/`AppMutationContract` by kind. Compute `wraps_db_transaction` exactly as today (`!module.tables.is_empty()` per the current logic at http.rs:458 / app_contract.rs:74) and attach it to mutations — this is the one value that does **not** come from `ContractIr`. Leave `server_config` and `http_routes` untouched.

- [ ] **Step 5: Run the characterization test + the Axum emit tests**

Run: `cargo test -p vox-compiler app_contract && cargo test -p vox-codegen http`
Expected: PASS — identical `AppContract` output, so the Axum emitter ([`http.rs`](../../../crates/vox-codegen/src/codegen_rust/emit/http.rs)) is unaffected.

- [ ] **Step 6: Run the projection parity guard**

Run: `cargo test -p vox-compiler projection_parity`
Expected: PASS (canonical bytes unchanged — see [`projection_parity_test.rs`](../../../crates/vox-compiler/tests/projection_parity_test.rs)).

- [ ] **Step 7: Commit**

```bash
git add crates/vox-compiler/src/app_contract.rs
git commit -m "refactor(contract): derive AppContract endpoints from ContractIr (single endpoint lens)"
```

> **Deferred (out of this plan):** retiring WebIR's validation-only `ServerFnContract`/`MutationContract` (replace with references to `ContractIr`) is a follow-on once 5.2 proves stable. It is validation-only, so lower risk but separate scope.

---

## Self-review

**Spec coverage** (against the prioritized list in the advisory):
- P0 #1 `@scheduled` on Tauri → Phase 1 (decision: full support). ✓
- P0 #2 ActionManifest mobile flag → Phase 3. ✓
- P0 #3 react-router scaffold dep → Task 4.1. ✓
- P0 #4 STT gating → Phase 2 (with the new `microphone` capability + the existing-test fix). ✓
- P1 #5 delete activity.rs → Task 4.2. ✓
- P1 #6 delete `generate_component*` → Task 4.3. ✓
- P2 #7 delete dead RouteIR → Task 5.1. ✓
- P2 #8 AppContract←ContractIr → Task 5.2. ✓
- Pruned items (correctly **not** in this plan): `schema/from_hir.rs` unification (verified as necessary divergence); reactive.rs parity-double-render removal (needs a corpus `parity_mismatch == 0` assertion first — a P3 prerequisite, not P0–P2); `jsx.rs` deletion (live re-exports — only trimmed opportunistically in 4.3).

**Placeholder scan:** The two genuinely unread specifics are flagged inline as *confirm-the-exact-API* steps with the file to check (`run_frontend_str` shape via `ir_emission_test.rs`; `bundle.capabilities`/`ContractEndpoint` accessors). These are verification steps, not hand-waved logic — the surrounding code is concrete.

**Type consistency:** `pipeline::generate(&module, name, RustAppShell::TauriApp) -> CodegenOutput { files: HashMap<String,String>, .. }` used consistently; `emit_durable_boot_prelude(module, &str, bool, BootPropagation)` and `emit_durable_boot_helpers(module)` match `main_boot.rs`; `f.schedule_interval: Option<String>` matches decl.rs:326; the `needs_stt` bool threaded into the four `emit_tauri_*` fns is named consistently.

---

## Execution handoff

Plan complete and saved to `docs/superpowers/plans/2026-06-03-codegen-split-brain-fixes.md`. Two execution options:

1. **Subagent-Driven (recommended)** — a fresh subagent per task, with review between tasks; fast iteration and isolation.
2. **Inline Execution** — execute tasks in this session with checkpoints for review.

Which approach?
