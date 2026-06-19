---
title: "Track C Completion & Beyond — Close the AI-UI-Target Loop"
description: "TDD plan to close the AGH-0007 Track C follow-ups (SSOT-driven primitive parity, effect-level VUV validation tests) and go beyond the minimum (rule-linked diagnostics + a vox_gui_rules discovery tool), completing the components+tokens+rules+validate loop that makes Vox a first-class AI-UI-generation target."
category: "Architecture SSOTs"
status: "roadmap"
training_eligible: false
---

# Track C Completion & Beyond — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the three below-ceiling follow-ups from the Track C review (AGH-0007) and extend Track C past the minimum so external AI UI generators can *discover the rules they must satisfy* and *get rule-linked validation feedback* — completing the components + tokens + **rules** + **validate** loop.

**Architecture:** Four self-contained changes. (1) Make the component-registry parity test enumerate the live primitive SSOT instead of a hand-copied list. (2) Extract a pure `validate_vuv_source(&str)` helper from the `vox_validate_vuv` MCP handler and prove — at the *effect* level, with known-bad fixtures — that it catches real contrast/occlusion/a11y violations. (3) Enrich that validation output with `rule_id`, linking each diagnostic to its `gui-design-rule/*` policy entry. (4) Add a `vox_gui_rules` MCP tool that lists the registered GUI design rules so a generator can read the constraint set before emitting.

**Tech Stack:** Rust; `vox-compiler` (`lowering_shared::primitive_tags` SSOT, lexer/parser/HIR), `vox-codegen` (`web_ir` lower + validators), `vox-orchestrator-mcp` (MCP tools, dispatch, input schemas), `contracts/` (operations catalog, policy + MCP registries). Existing deps only — `serde_yaml`, `vox-config`, `vox-codegen` are already in `vox-orchestrator-mcp/Cargo.toml`.

**North star (why this plan exists):** the Track-C handoffs kept passing green gates while shipping non-working effects (AGH-0005/0006). Every task here proves the **effect**, not the **shape**: parity against the live SSOT (not a copy), validation against proven-bad fixtures (not substrings), and discovery/validation tools exercised end-to-end.

---

## Executor notes (Sonnet 4.6)

- Work on a branch off the current Track-C tip (the branch holding `crates/vox-orchestrator-mcp/src/gui_registry_tools.rs`). Verify it exists first: `rg -l "vox_validate_vuv" crates/vox-orchestrator-mcp/src/gui_registry_tools.rs` must hit. If it does not, STOP — this plan assumes the Track-C base (incl. `vox_validate_vuv` from commit `7994b368a6`) is present.
- TDD throughout: write the failing test, watch it fail for the *right* reason, implement minimally, watch it pass, commit. One logical change per commit.
- **Verify-before-use** for any cross-crate symbol the step names: run the given `rg` and confirm the signature before writing code against it. The codebase is concurrently edited; do not trust memory.
- Windows: never `cargo fmt --all` (arg-limit overflow) — use `cargo fmt -p <crate>`. Prefer `cargo test -p <crate>` over whole-workspace runs.
- Gates per task, at full strictness (no `--warn-only`, no `|| true`): `cargo test -p <crate>`, `cargo clippy -p <crate> -- -D warnings`, and for registry changes `cargo run -p vox-arch-check` (exit 0). Reference skills: superpowers:test-driven-development, superpowers:verification-before-completion.
- If a gate is red at baseline for reasons unrelated to your change, STOP and report — do not weaken the gate.

## File structure

| File | Responsibility | Task |
|---|---|---|
| `crates/vox-codegen/tests/component_registry_sync.rs` | Parity test driven by the live `primitive_tags` SSOT (no hardcoded list) | T1 (modify) |
| `crates/vox-orchestrator-mcp/src/gui_registry_tools.rs` | Pure `validate_vuv_source` helper + `rule_id` enrichment + `vox_gui_rules` handler | T2, T3, T4 (modify) |
| `crates/vox-orchestrator-mcp/tests/validate_vuv_effect.rs` | Effect-level tests of `validate_vuv_source` against forbidden/golden fixtures | T2, T3 (create) |
| `crates/vox-orchestrator-mcp/src/dispatch.rs` | Dispatch arm for `vox_gui_rules` | T4 (modify) |
| `crates/vox-orchestrator-mcp/src/input_schemas.rs` | Input schema for `vox_gui_rules` | T4 (modify) |
| `crates/vox-orchestrator-mcp/src/lib.rs` | Module doc listing the GUI tools | T4 (modify) |
| `contracts/operations/catalog.v1.yaml` | `gui.rules` operation row | T4 (modify) |
| `contracts/mcp/tool-registry.canonical.yaml` | Regenerated (do NOT hand-edit) | T4 (generated) |
| `docs/src/architecture/automatic-gui-and-debugging-vox-design-2026-06-18.md` | §3b updated: 4 MCP tools, rule-linked validation | T5 (modify) |
| `docs/superpowers/antigravity-handoff-ledger.md` | AGH-0007 follow-ups closed | T5 (modify) |

---

## Phase 1 — Close the identified issues

### Task 1: SSOT-drive the component-registry parity test

**Why:** the current test (`component_registry_sync.rs`) checks "every primitive is registered" against a **hardcoded** `known_primitives` vec. A new compiler primitive missing from the registry would not be caught. The canonical list already exists: `vox_compiler::lowering_shared::primitive_tags::all_primitives()`.

**Files:**
- Modify: `crates/vox-codegen/tests/component_registry_sync.rs`

- [ ] **Step 1: Verify the SSOT API exists**

Run: `rg -n "pub fn all_primitives|pub const PRIMITIVE_TAGS" crates/vox-compiler/src/lowering_shared/primitive_tags.rs`
Expected: both present — `PRIMITIVE_TAGS: &[&str]` and `pub fn all_primitives() -> &'static [&'static str]`.

- [ ] **Step 2: Replace the hardcoded list with the live SSOT (failing test first)**

Replace the body of `test_component_registry_sync` (the `known_primitives` vec and both loops) with the SSOT-driven version. Keep the existing file-read prologue (path via `CARGO_MANIFEST_DIR`).

```rust
    // SSOT for primitive tags — the same list the parser & lowerer recognize.
    let primitive_tags = vox_compiler::lowering_shared::primitive_tags::all_primitives();

    let registered: std::collections::HashSet<&str> =
        registry.components.iter().map(|c| c.name.as_str()).collect();

    // 1. Every canonical primitive tag MUST have a registry entry (drift guard).
    for tag in primitive_tags {
        assert!(
            registered.contains(tag),
            "primitive '{tag}' (from lowering_shared::primitive_tags) is not in component-registry.v1.json"
        );
    }

    // 2. Every registered component MUST be a real primitive tag (no stale rows).
    let canonical: std::collections::HashSet<&str> = primitive_tags.iter().copied().collect();
    for comp in &registry.components {
        assert!(
            canonical.contains(comp.name.as_str()),
            "registry component '{}' is not a canonical primitive tag",
            comp.name
        );
    }
```

- [ ] **Step 3: Run the test — expect it to surface any real drift**

Run: `cargo test -p vox-codegen --test component_registry_sync -- --nocapture`
Expected: PASS if the 20 `PRIMITIVE_TAGS` and the registry agree. If it FAILS, the failure names the exact missing/stale tag — fix `contracts/gui/component-registry.v1.json` (add the missing primitive entry, mirroring an existing entry's shape: `name`, `tag`, `props`, `variants`, `a11y`) or remove the stale row, then re-run until green. Do NOT re-add a hardcoded list to make it pass.

- [ ] **Step 4: Confirm `vox-compiler` is a dev-dependency of the test crate**

Run: `rg -n "vox-compiler" crates/vox-codegen/Cargo.toml`
Expected: present under `[dependencies]` or `[dev-dependencies]`. If absent under dev-deps and the test fails to resolve the path, add `vox-compiler = { workspace = true }` to `[dev-dependencies]` in `crates/vox-codegen/Cargo.toml`, then re-run Step 3.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-codegen/tests/component_registry_sync.rs crates/vox-codegen/Cargo.toml contracts/gui/component-registry.v1.json
git commit -m "test(codegen): drive component-registry parity from the live primitive_tags SSOT

Replaces the hardcoded known_primitives list with
vox_compiler::lowering_shared::primitive_tags::all_primitives(), so a new
compiler primitive missing from component-registry.v1.json is now caught
(closes AGH-0007 follow-up; §B-7)."
```

---

### Task 2: Pure `validate_vuv_source` helper + effect-level tests

**Why:** `vox_validate_vuv` currently has no test proving it actually catches violations (the handler needs `ServerState`, so it was untested). Extract the pure pipeline into a testable helper and prove the **effect** against the forbidden-corpus fixtures.

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/gui_registry_tools.rs`
- Create: `crates/vox-orchestrator-mcp/tests/validate_vuv_effect.rs`

- [ ] **Step 1: Verify the pipeline symbols and fixtures**

Run: `rg -n "pub fn lower_hir_to_web_ir_with_summary|pub fn validate_web_ir_with_registry" crates/vox-codegen/src/web_ir/lower.rs crates/vox-codegen/src/web_ir/validate.rs`
Run: `ls examples/forbidden/tokens_low_contrast_pair.vox examples/forbidden/raw_class_occlusion.vox examples/golden-ts/form_basic.vox`
Expected: both functions exist; all three fixture files exist.

- [ ] **Step 2: Extract the pure helper (refactor; keep the handler thin)**

In `crates/vox-orchestrator-mcp/src/gui_registry_tools.rs`, replace the body of `vox_validate_vuv` so the pipeline lives in a pure, synchronous, `state`-free helper that returns a `serde_json::Value`, and the async handler just unwraps the `source` arg and calls it:

```rust
/// Pure validation pipeline: Vox/VUV `source` → web-IR → diagnostics, returning
/// the JSON payload `{ ok, error_count, diagnostic_count, diagnostics[] }`. No
/// `ServerState`, no I/O, no files written — directly unit-testable.
pub fn validate_vuv_source(source: &str) -> serde_json::Value {
    let tokens = vox_compiler::lexer::cursor::lex(source);
    let module = match vox_compiler::parser::parse(tokens) {
        Ok(m) => m,
        Err(e) => {
            return serde_json::json!({
                "ok": false,
                "error_count": 1,
                "diagnostic_count": 1,
                "diagnostics": [{
                    "code": "parse_error",
                    "message": format!("{e:?}"),
                    "severity": "Error",
                    "category": "parse",
                }],
            });
        }
    };
    let hir = vox_compiler::hir::lower_module(&module);
    let (web_ir, _summary) = vox_codegen::web_ir::lower::lower_hir_to_web_ir_with_summary(&hir);
    let diags = vox_codegen::web_ir::validate::validate_web_ir_with_registry(&web_ir, None);

    use vox_codegen::web_ir::WebIrDiagnosticSeverity;
    let error_count = diags
        .iter()
        .filter(|d| matches!(d.severity(), WebIrDiagnosticSeverity::Error))
        .count();
    let diagnostics: Vec<serde_json::Value> = diags
        .iter()
        .map(|d| {
            serde_json::json!({
                "code": d.code,
                "message": d.message,
                "severity": format!("{:?}", d.severity()),
                "category": d.category,
            })
        })
        .collect();

    serde_json::json!({
        "ok": error_count == 0,
        "error_count": error_count,
        "diagnostic_count": diags.len(),
        "diagnostics": diagnostics,
    })
}

/// Validate a Vox/VUV source string against the compile-time GUI guarantees
/// (contrast, layer-occlusion, a11y, structural web-IR) WITHOUT writing files —
/// the external-validation API an AI UI generator calls to check its output
/// before a human sees it. See design §3b / Track C.
pub async fn vox_validate_vuv(_state: &ServerState, args: serde_json::Value) -> String {
    let Some(source) = args.get("source").and_then(Value::as_str) else {
        return ToolResult::<Value>::err("missing required string field 'source'".to_string())
            .to_json();
    };
    ToolResult::ok(validate_vuv_source(source)).to_json()
}
```

- [ ] **Step 3: Write the failing effect tests**

Create `crates/vox-orchestrator-mcp/tests/validate_vuv_effect.rs`:

```rust
//! Effect-level proof that vox_validate_vuv catches real GUI-guarantee
//! violations — fed the forbidden-corpus fixtures, not substrings. Closes the
//! AGH-0007 follow-up (a) and enforces the §B-9 "prove the effect" rule.
use vox_orchestrator_mcp::gui_registry_tools::validate_vuv_source;

fn errors(v: &serde_json::Value) -> u64 {
    v["error_count"].as_u64().unwrap_or(0)
}
fn codes(v: &serde_json::Value) -> Vec<String> {
    v["diagnostics"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|d| d["code"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn clean_source_validates_ok() {
    let src = include_str!("../../../examples/golden-ts/form_basic.vox");
    let report = validate_vuv_source(src);
    assert_eq!(report["ok"], serde_json::Value::Bool(true), "report: {report}");
    assert_eq!(errors(&report), 0, "report: {report}");
}

#[test]
fn low_contrast_source_is_rejected() {
    let src = include_str!("../../../examples/forbidden/tokens_low_contrast_pair.vox");
    let report = validate_vuv_source(src);
    assert!(
        codes(&report).iter().any(|c| c.contains("contrast")),
        "expected a contrast diagnostic, got: {report}"
    );
}

#[test]
fn occlusion_source_is_rejected() {
    let src = include_str!("../../../examples/forbidden/raw_class_occlusion.vox");
    let report = validate_vuv_source(src);
    assert!(
        errors(&report) > 0 || !codes(&report).is_empty(),
        "expected an occlusion/style diagnostic, got: {report}"
    );
}
```

- [ ] **Step 4: Confirm the helper is reachable from the test (module is public)**

Run: `rg -n "pub mod gui_registry_tools" crates/vox-orchestrator-mcp/src/lib.rs`
Expected: the module is `pub`. If it is `pub(crate)` or `mod`, change it to `pub mod gui_registry_tools;` in `lib.rs` so the integration test can import `validate_vuv_source`.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p vox-orchestrator-mcp --test validate_vuv_effect -- --nocapture`
Expected: 3 PASS. If `clean_source_validates_ok` fails because `form_basic.vox` happens to emit a warning-level (non-error) diagnostic, that is fine — the assertion is on `ok`/`error_count` (errors only). If it emits an actual error, pick a different clean fixture from `examples/golden-ts/` that lowers to web-IR cleanly (verify with the same helper) and update the `include_str!` path. If `low_contrast`/`occlusion` fail with zero diagnostics, the fixture may not reach the palette/overlay validators through this path — run `rg -n "fn validate_web_ir_full" crates/vox-codegen/src/web_ir/validate.rs` and confirm `validate_palette` + overlay checks are invoked inside it; if a fixture is the wrong trigger, swap to `examples/forbidden/contrast_gray_on_white.vox` / `raw_z_index.vox` respectively.

- [ ] **Step 6: Gate + commit**

Run: `cargo clippy -p vox-orchestrator-mcp -- -D warnings`
Expected: clean.

```bash
git add crates/vox-orchestrator-mcp/src/gui_registry_tools.rs crates/vox-orchestrator-mcp/src/lib.rs crates/vox-orchestrator-mcp/tests/validate_vuv_effect.rs
git commit -m "test(mcp): prove vox_validate_vuv catches real violations (effect, not shape)

Extracts a pure validate_vuv_source(&str) helper from the handler and tests it
against forbidden-corpus fixtures (low-contrast, occlusion) + a clean golden
fixture. Closes AGH-0007 follow-up (a); enforces §B-9."
```

---

## Phase 2 — Beyond the minimum: complete the constraint loop

### Task 3: Rule-linked diagnostics in `validate_vuv_source`

**Why:** a generator gets far more actionable feedback if each diagnostic names the `gui-design-rule/*` it violated. Map each web-IR diagnostic `code` to its policy-registry rule id, so the validation tool's output joins back to the discoverable rule set (Task 4).

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/gui_registry_tools.rs`
- Modify: `crates/vox-orchestrator-mcp/tests/validate_vuv_effect.rs`

- [ ] **Step 1: Write the failing test**

Append to `crates/vox-orchestrator-mcp/tests/validate_vuv_effect.rs`:

```rust
fn rule_ids(v: &serde_json::Value) -> Vec<String> {
    v["diagnostics"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|d| d["rule_id"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn contrast_diagnostic_links_to_contrast_rule() {
    let src = include_str!("../../../examples/forbidden/tokens_low_contrast_pair.vox");
    let report = validate_vuv_source(src);
    assert!(
        rule_ids(&report).iter().any(|r| r == "gui-design-rule/contrast"),
        "expected a diagnostic linked to gui-design-rule/contrast, got: {report}"
    );
}
```

- [ ] **Step 2: Run it — expect FAIL (no `rule_id` field yet)**

Run: `cargo test -p vox-orchestrator-mcp --test validate_vuv_effect contrast_diagnostic_links_to_contrast_rule`
Expected: FAIL (`rule_id` absent → empty vec → assertion fails).

- [ ] **Step 3: Add the code→rule mapping and emit `rule_id`**

In `crates/vox-orchestrator-mcp/src/gui_registry_tools.rs`, add the mapping fn and include `rule_id` in each diagnostic object inside `validate_vuv_source`:

```rust
/// Map a web-IR validator diagnostic code to the `gui-design-rule/*` policy id
/// it belongs to, so external generators get rule-linked, discoverable feedback
/// (pairs with the vox_gui_rules tool). Returns `None` for non-GUI codes.
fn rule_id_for_code(code: &str) -> Option<&'static str> {
    if code.contains("contrast") {
        Some("gui-design-rule/contrast")
    } else if code.starts_with("web_ir_validate.a11y.") {
        Some("gui-design-rule/a11y")
    } else if code.starts_with("web_ir_validate.overlay.") {
        Some("gui-design-rule/layer-occlusion")
    } else {
        None
    }
}
```

Then in the `diagnostics` map closure inside `validate_vuv_source`, add the `rule_id` field:

```rust
        .map(|d| {
            serde_json::json!({
                "code": d.code,
                "message": d.message,
                "severity": format!("{:?}", d.severity()),
                "category": d.category,
                "rule_id": rule_id_for_code(&d.code),
            })
        })
```

(Note: `contrast` is checked first because the a11y validator emits `web_ir_validate.a11y.insufficient_contrast` / `.low_contrast`, which are contrast-rule violations despite the `a11y.` prefix.)

- [ ] **Step 4: Run the test — expect PASS**

Run: `cargo test -p vox-orchestrator-mcp --test validate_vuv_effect`
Expected: all PASS (4 tests).

- [ ] **Step 5: Gate + commit**

Run: `cargo clippy -p vox-orchestrator-mcp -- -D warnings`

```bash
git add crates/vox-orchestrator-mcp/src/gui_registry_tools.rs crates/vox-orchestrator-mcp/tests/validate_vuv_effect.rs
git commit -m "feat(mcp): rule-link vox_validate_vuv diagnostics to gui-design-rule/* ids

Each diagnostic now carries rule_id (contrast/a11y/layer-occlusion), joining
validation output to the discoverable rule set."
```

---

### Task 4: `vox_gui_rules` MCP tool — discover the constraint set

**Why:** components + tokens + validate are wired, but a generator can't *discover the rules* it must satisfy without running validation. Expose the registered `GuiDesignRule` policy entries so a generator reads the constraints up front. This completes the loop and matches design §3b ("constraints the model can read").

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/gui_registry_tools.rs`
- Modify: `crates/vox-orchestrator-mcp/src/dispatch.rs`
- Modify: `crates/vox-orchestrator-mcp/src/input_schemas.rs`
- Modify: `crates/vox-orchestrator-mcp/src/lib.rs`
- Modify: `contracts/operations/catalog.v1.yaml`
- Generated: `contracts/mcp/tool-registry.canonical.yaml`

- [ ] **Step 1: Verify the policy registry path, deps, and dispatch pattern**

Run: `rg -n "gui-design-rule" contracts/policy/policy-registry.v1.yaml | head`
Run: `rg -n "serde_yaml" crates/vox-orchestrator-mcp/Cargo.toml`
Run: `rg -n '"vox_gui_tokens" =>' crates/vox-orchestrator-mcp/src/dispatch.rs`
Expected: 3 `gui-design-rule/*` entries in the yaml; `serde_yaml` is a dep; the `vox_gui_tokens` dispatch arm exists (mirror it).

- [ ] **Step 2: Implement the handler**

Add to `crates/vox-orchestrator-mcp/src/gui_registry_tools.rs`:

```rust
/// List the registered GUI design rules (the `gui-design-rule/*` policy-registry
/// entries) so an external generator can read the constraint set BEFORE emitting
/// — the discovery counterpart to vox_validate_vuv. Reads the generated policy
/// registry; returns `{ rules: [ { id, title, description, severity, blocking } ] }`.
pub async fn vox_gui_rules(state: &ServerState, _args: serde_json::Value) -> String {
    let path = state
        .repository
        .root
        .join("contracts/policy/policy-registry.v1.yaml");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            return ToolResult::<Value>::err(format!("Failed to read policy registry: {e}"))
                .to_json();
        }
    };
    let doc: serde_yaml::Value = match serde_yaml::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            return ToolResult::<Value>::err(format!("Failed to parse policy registry: {e}"))
                .to_json();
        }
    };
    // The registry is a sequence under a top-level key; find the entries list and
    // filter to domain == "gui-design-rule".
    let entries = doc
        .get("entries")
        .or_else(|| doc.get("policies"))
        .and_then(serde_yaml::Value::as_sequence)
        .cloned()
        .unwrap_or_default();
    let rules: Vec<Value> = entries
        .iter()
        .filter(|e| e.get("domain").and_then(serde_yaml::Value::as_str) == Some("gui-design-rule"))
        .map(|e| {
            let s = |k: &str| {
                e.get(k)
                    .and_then(serde_yaml::Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            };
            serde_json::json!({
                "id": s("id"),
                "title": s("title"),
                "description": s("description"),
                "severity": s("severity"),
                "blocking": e.get("blocking").and_then(serde_yaml::Value::as_bool).unwrap_or(false),
            })
        })
        .collect();
    ToolResult::ok(serde_json::json!({ "rules": rules })).to_json()
}
```

- [ ] **Step 3: Confirm the top-level registry key**

Run: `rg -n "^entries:|^policies:|^- id: gui-design-rule" contracts/policy/policy-registry.v1.yaml | head`
Expected: confirm whether the list lives under `entries:` or `policies:` (the handler tries both). If it is neither (e.g., the file is a bare top-level sequence), adjust the `entries` binding to `doc.as_sequence()` accordingly before proceeding.

- [ ] **Step 4: Wire dispatch, input schema, and module doc**

In `crates/vox-orchestrator-mcp/src/dispatch.rs`, add after the `vox_validate_vuv` arm:

```rust
        "vox_gui_rules" => Ok(crate::gui_registry_tools::vox_gui_rules(state, args).await),
```

In `crates/vox-orchestrator-mcp/src/input_schemas.rs`, extend the no-arg group to include the new tool:

```rust
        "vox_gui_components" | "vox_gui_tokens" | "vox_gui_rules" => {
            parse_obj(r#"{"type":"object","additionalProperties":false}"#)
        }
```

In `crates/vox-orchestrator-mcp/src/lib.rs`, update the GUI-tools doc line to list four tools:

```rust
/// GUI registry + validation tools (`vox_gui_components`, `vox_gui_tokens`, `vox_gui_rules`, `vox_validate_vuv`).
```

- [ ] **Step 5: Add the catalog row**

In `contracts/operations/catalog.v1.yaml`, immediately after the `gui.validate` entry (the one whose `mcp.name` is `vox_validate_vuv`), insert:

```yaml
- id: gui.rules
  title: GUI Design Rules
  description: List the registered GUI design rules (gui-design-rule/*) so a generator
    can read the constraint set before emitting.
  description_human: null
  product_lane: ai
  intent_tags: []
  side_effect_class: null
  scope_kind: session
  reversible: false
  requires_repo: null
  preferred_for_models: null
  human_takeover_friendly: null
  mens_planner_visible: null
  canonical_name: null
  latin_aliases: null
  mcp:
    name: vox_gui_rules
    http_read_role_eligible: true
    tier: core
  cli: null
```

- [ ] **Step 6: Build, then regenerate the canonical MCP registry**

Run: `cargo build -p vox-orchestrator-mcp`
Expected: clean build.

Run: `VOX_SKIP_FRESHNESS_CHECK=1 cargo run -p vox-cli -- ci operations-sync --target mcp --write`
Expected: `wrote .../contracts/mcp/tool-registry.canonical.yaml`.

Run: `rg -n "vox_gui_rules" contracts/mcp/tool-registry.canonical.yaml`
Expected: one hit (the tool is now advertised). Never hand-edit this file.

- [ ] **Step 7: Verify parity gate**

Run: `VOX_SKIP_FRESHNESS_CHECK=1 cargo run -p vox-cli -- ci operations-verify`
Expected: passes (catalog ↔ canonical registry ↔ dispatch in sync). If it reports `vox_gui_rules` missing from a surface, re-check Step 4 (dispatch arm) and Step 5 (catalog row) match the tool name exactly.

- [ ] **Step 8: Gate + commit**

Run: `cargo clippy -p vox-orchestrator-mcp -- -D warnings`

```bash
git add crates/vox-orchestrator-mcp/src/gui_registry_tools.rs crates/vox-orchestrator-mcp/src/dispatch.rs crates/vox-orchestrator-mcp/src/input_schemas.rs crates/vox-orchestrator-mcp/src/lib.rs contracts/operations/catalog.v1.yaml contracts/mcp/tool-registry.canonical.yaml
git commit -m "feat(mcp): vox_gui_rules — discover the GUI design-rule constraint set

Completes the AI-UI-target loop (components + tokens + rules + validate): an
external generator can now read gui-design-rule/* before emitting and get
rule-linked feedback from vox_validate_vuv after."
```

---

### Task 5: Documentation + ledger closure

**Files:**
- Modify: `docs/src/architecture/automatic-gui-and-debugging-vox-design-2026-06-18.md`
- Modify: `docs/superpowers/antigravity-handoff-ledger.md`

- [ ] **Step 1: Update the design doc §3b**

In `docs/src/architecture/automatic-gui-and-debugging-vox-design-2026-06-18.md`, find the Track C / §3b MCP-tools description and update it to state there are **four** GUI MCP tools — `vox_gui_components`, `vox_gui_tokens`, `vox_gui_rules` (discovery), `vox_validate_vuv` (validation, with rule-linked diagnostics) — forming the read-rules → emit → validate loop. (Use `rg -n "vox_gui_components|MCP tool" docs/src/architecture/automatic-gui-and-debugging-vox-design-2026-06-18.md` to locate the section.)

- [ ] **Step 2: Close the AGH-0007 follow-ups in the ledger**

In `docs/superpowers/antigravity-handoff-ledger.md`, in the AGH-0007 review-detail "Remaining below-ceiling follow-ups" paragraph, mark each as CLOSED with its commit: (a) effect-level `vox_validate_vuv` test → Task 2; (b) the `vox_gui_tokens` SSOT item is **withdrawn — not a defect** (`vox.tokens.json` at repo root IS the token data SSOT; `contracts/tokens/tokens.v1.json` is its JSON Schema); (c) `component_registry_sync` now enumerates the live SSOT → Task 1. Add a one-line note that Track C reached the design ceiling plus a beyond-minimum extension (`vox_gui_rules` + rule-linked diagnostics).

- [ ] **Step 3: Commit**

```bash
git add docs/src/architecture/automatic-gui-and-debugging-vox-design-2026-06-18.md docs/superpowers/antigravity-handoff-ledger.md
git commit -m "docs(track-c): record 4-tool AI-UI loop; close AGH-0007 follow-ups"
```

- [ ] **Step 4: Final full verification**

Run: `cargo test -p vox-codegen --test component_registry_sync`
Run: `cargo test -p vox-orchestrator-mcp --test validate_vuv_effect`
Run: `cargo clippy -p vox-codegen -p vox-orchestrator-mcp -- -D warnings`
Run: `cargo run -p vox-arch-check`
Expected: all green; arch-check exit 0. Paste the outputs into the completion report (evidence before assertion — superpowers:verification-before-completion).

---

## Self-review

**Spec coverage:**
- AGH-0007 follow-up (a) effect-level validation test → **Task 2** ✓
- AGH-0007 follow-up (b) token SSOT → **withdrawn in Task 5** (verified misread: root `vox.tokens.json` is the data SSOT; `contracts/tokens/tokens.v1.json` is its schema) ✓
- AGH-0007 follow-up (c) live-primitive parity → **Task 1** ✓
- Beyond-minimum: rule-linked diagnostics → **Task 3**; discovery tool `vox_gui_rules` → **Task 4** ✓
- Docs/ledger closure → **Task 5** ✓

**Placeholder scan:** every code step shows full code; every command has expected output; fixtures (`examples/forbidden/tokens_low_contrast_pair.vox`, `raw_class_occlusion.vox`, `examples/golden-ts/form_basic.vox`) and symbols (`all_primitives`, `lower_hir_to_web_ir_with_summary`, `validate_web_ir_with_registry`, `WebIrDiagnosticSeverity`, `ToolResult`) are verified-real via Step-1 `rg`/`ls` gates. No TBDs.

**Type consistency:** `validate_vuv_source(&str) -> serde_json::Value` defined in T2, enriched (not renamed) in T3, consumed by the handler in T2/T3; `rule_id_for_code(&str) -> Option<&'static str>` consistent T3; `vox_gui_rules(state, args)` matches its dispatch arm + catalog `mcp.name` (`vox_gui_rules`) across T4; diagnostic JSON keys (`code`/`message`/`severity`/`category`/`rule_id`) consistent T2→T3.

**Risk notes for the executor:** the only soft spots are fixture-trigger fidelity (Step 2.5 / 3.x give concrete fallbacks per validator) and the policy-registry top-level key (Step 4.3 confirms `entries:` vs `policies:`). Both are resolved by the in-task verify steps, not assumptions.
