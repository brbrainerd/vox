# Vox Placement Model Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give every Vox declaration a compiler-computed *placement* (`native` / `shared` / `gui`), inferred from the effect system and overridable with `@place(...)`, so the perf-vs-GUI boundary is a checked contract instead of convention.

**Architecture:** A new post-typeck pass `typeck::placement` reads `HirFn`/`HirReactiveComponent` effect data, seeds each declaration's placement, propagates along the call graph to a fixed point, then emits blocking diagnostics for conflicts (`E-PLACE-CONFLICT`), illegal gui→native calls (`E-PLACE-BOUNDARY`), and unsatisfiable `@place` overrides (`E-PLACE-UNSAT`). The dormant `@native` token is retired in favor of `@place(native)`. Enforcement is blocking from day one; broken golden fixtures are fixed in the same body of work (Task 11).

**Tech Stack:** Rust (`vox-compiler`, `vox-code-audit`), the existing `feature_matrix` parity machinery, `vox-code-audit` detector ABI.

**Spec:** `docs/superpowers/specs/2026-06-20-vox-placement-model-design.md`

**Out of scope (separate sub-specs):** speed tiers / cargo broker (sub-spec 2); behavioral differential oracle + Vox Axis Parity panel (sub-spec 3).

---

## File Structure

| File | Responsibility | New/Modify |
|------|----------------|-----------|
| `crates/vox-compiler/src/typeck/placement.rs` | The placement pass: `Placement` enum, `PlacementMap`, `infer()`, seed/propagate/conflict/boundary | **Create** |
| `crates/vox-compiler/src/typeck/diagnostics.rs` | Add 3 placement diagnostic codes + register them | Modify |
| `crates/vox-compiler/src/typeck/mod.rs` | Declare `pub mod placement;`; wire pass into the parallel lint `passes` vec | Modify |
| `crates/vox-compiler/src/hir/nodes/decl.rs` | Add `placement_override: Option<PlacementHint>` field to `HirFn` | Modify |
| `crates/vox-compiler/src/lexer/token.rs` | Add `@place` token; **remove** `@native` token | Modify |
| `crates/vox-compiler/src/parser/descent/decl/head_fn.rs` | Parse `@place(...)`; **remove** `AtNative` arm | Modify |
| `crates/vox-compiler/src/feature_matrix.rs` | Add `DecoratorFeature::Place`; **remove** `Native`; parity test | Modify |
| `crates/vox-compiler/src/language_surface.rs` | Swap `@native` → `@place` in the two decorator arrays | Modify |
| `crates/vox-compiler/src/lexer/cursor.rs` | Update the lexer test string that referenced `@native` | Modify |
| `crates/vox-code-audit/src/detectors/retired_decorator.rs` | Add `@native` → `@place(native)` to the retired-decorator table | Modify |
| `AGENTS.md` | Add `@native` row to the Retired Surfaces table | Modify |

---

## Task 1: Placement diagnostic codes

**Files:**
- Modify: `crates/vox-compiler/src/typeck/diagnostics.rs` (constants near line 688; array near lines 800–848)
- Test: same file's `#[cfg(test)] mod tests`

- [ ] **Step 1: Write the failing test**

Add to the test module in `crates/vox-compiler/src/typeck/diagnostics.rs`:

```rust
#[test]
fn placement_codes_are_registered() {
    for code in [
        codes::PLACEMENT_CONFLICT,
        codes::PLACEMENT_BOUNDARY,
        codes::PLACEMENT_UNSAT,
    ] {
        assert!(
            codes::ALL_COMPILER_DIAGNOSTIC_CODES.contains(&code),
            "{code} must be registered in ALL_COMPILER_DIAGNOSTIC_CODES"
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-compiler placement_codes_are_registered`
Expected: FAIL — `no associated item named PLACEMENT_CONFLICT found`.

- [ ] **Step 3: Add the constants and register them**

In the `codes` module of `diagnostics.rs`, alongside `PARITY_FRONTEND_ONLY` (near line 688) add:

```rust
/// A declaration is pulled toward both native-only and gui-only tiers.
pub const PLACEMENT_CONFLICT: &str = "vox/placement/conflict";
/// A `gui` declaration calls a `native` declaration without crossing an endpoint.
pub const PLACEMENT_BOUNDARY: &str = "vox/placement/boundary";
/// An explicit `@place(...)` override cannot be satisfied by the declaration's effects.
pub const PLACEMENT_UNSAT: &str = "vox/placement/unsat";
```

Then add these three string literals to the `ALL_COMPILER_DIAGNOSTIC_CODES` array (in the `vox/*` section near lines 845–847):

```rust
    "vox/placement/conflict",
    "vox/placement/boundary",
    "vox/placement/unsat",
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-compiler placement_codes_are_registered`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-compiler/src/typeck/diagnostics.rs
git commit -m "feat(placement): register placement diagnostic codes"
```

---

## Task 2: `Placement` type + seed inference (single declaration)

**Files:**
- Create: `crates/vox-compiler/src/typeck/placement.rs`
- Modify: `crates/vox-compiler/src/typeck/mod.rs` (add `pub mod placement;` near line 43)
- Test: in `placement.rs`

This task introduces the type and the *seed* step only (no call-graph propagation yet). Seed rules map a single `HirFn`'s own data to a placement.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-compiler/src/typeck/placement.rs` with only this test (and `use` lines) so it compiles to a failing state:

```rust
//! Placement inference: assigns each declaration a tier (native/shared/gui).
//! See docs/superpowers/specs/2026-06-20-vox-placement-model-design.md.

use crate::hir::nodes::decl::{HirCapability, HirFn};

/// Where a declaration may be emitted. `Shared` is the top of the lattice
/// (emits to native + gui + interp); `Native` and `Gui` are the two
/// incompatible specializations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    Shared,
    Native,
    Gui,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hir::nodes::decl::HirFn;

    fn bare_fn(name: &str) -> HirFn {
        HirFn::test_stub(name)
    }

    #[test]
    fn pure_fn_seeds_shared() {
        let mut f = bare_fn("checksum");
        f.is_pure = true;
        assert_eq!(seed_fn(&f), Placement::Shared);
    }

    #[test]
    fn db_capability_seeds_native() {
        let mut f = bare_fn("load_user");
        f.capabilities = vec![HirCapability::Db];
        assert_eq!(seed_fn(&f), Placement::Native);
    }

    #[test]
    fn reactive_fn_seeds_gui() {
        let mut f = bare_fn("Counter");
        f.is_reactive = true;
        assert_eq!(seed_fn(&f), Placement::Gui);
    }

    #[test]
    fn unannotated_fn_seeds_shared() {
        let f = bare_fn("add");
        assert_eq!(seed_fn(&f), Placement::Shared);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

First add `pub mod placement;` to `crates/vox-compiler/src/typeck/mod.rs` near line 43 (after `pub mod layer;`).
Run: `cargo test -p vox-compiler placement::tests`
Expected: FAIL — `cannot find function seed_fn` and `no function test_stub`.

- [ ] **Step 3: Add the `HirFn::test_stub` helper**

In `crates/vox-compiler/src/hir/nodes/decl.rs`, inside `impl HirFn`, add a test-only constructor (place it behind `#[cfg(test)]` if the surrounding impl is not test-gated, otherwise add a normal `#[doc(hidden)]` constructor). It must populate every required field with empty/false defaults and the given name. Read the existing `HirFn` struct (lines 328–427) and set: `is_pure: false`, `is_reactive: false`, `is_remote: false`, `is_llm: false`, `capabilities: vec![]`, `durability: None`, empty `params`/`body`, `Span::default()` (or the zero span used elsewhere in this file's tests), `id: DefId::default()` or the test-id constructor used elsewhere.

```rust
impl HirFn {
    /// Minimal stub for unit tests — empty body, no effects.
    #[doc(hidden)]
    pub fn test_stub(name: &str) -> HirFn {
        HirFn {
            name: name.to_string(),
            is_pure: false,
            is_reactive: false,
            is_remote: false,
            is_llm: false,
            capabilities: Vec::new(),
            durability: None,
            params: Vec::new(),
            body: Vec::new(),
            // ...fill remaining fields per the struct definition at lines 328–427,
            // using the same zero/default values the existing tests in this file use.
            ..Default::default()
        }
    }
}
```

> If `HirFn` does not derive `Default`, do not add `..Default::default()`; instead set every field explicitly. Check the struct first.

- [ ] **Step 4: Implement `seed_fn`**

Add to `placement.rs` (above the test module):

```rust
/// Native-tier capabilities — touching any of these forces the native tier.
fn is_native_capability(cap: &HirCapability) -> bool {
    matches!(
        cap,
        HirCapability::Net
            | HirCapability::Db
            | HirCapability::Fs
            | HirCapability::Env
            | HirCapability::Clock
            | HirCapability::Random
            | HirCapability::Spawn
            | HirCapability::GpuCompute
            | HirCapability::Vcs
            | HirCapability::Mcp(_)
    )
}

/// Seed a single function's placement from its own decorators/effects,
/// before any call-graph propagation.
#[must_use]
pub fn seed_fn(f: &HirFn) -> Placement {
    if f.is_reactive {
        return Placement::Gui;
    }
    if f.capabilities.iter().any(is_native_capability)
        || f.is_remote
        || f.is_llm
        || f.durability.is_some()
    {
        return Placement::Native;
    }
    // @pure, `uses nothing`, or unannotated leaf → shared.
    Placement::Shared
}
```

> Note: `HirCapability::Mutate` and `HirCapability::Nothing` are intentionally NOT native — local mutation and explicit purity do not require the native tier.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p vox-compiler placement::tests`
Expected: PASS (4 tests).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-compiler/src/typeck/placement.rs crates/vox-compiler/src/typeck/mod.rs crates/vox-compiler/src/hir/nodes/decl.rs
git commit -m "feat(placement): Placement type + per-declaration seed inference"
```

---

## Task 3: JSX-bearing components seed `gui`

**Files:**
- Modify: `crates/vox-compiler/src/typeck/placement.rs`
- Test: in `placement.rs`

`HirReactiveComponent` (decl.rs:640–652) has `view: Option<HirExpr>`; components live in `hir.components`. A component always seeds `gui`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn reactive_component_seeds_gui() {
    use crate::hir::nodes::decl::HirReactiveComponent;
    let c = HirReactiveComponent::test_stub("Panel");
    assert_eq!(seed_component(&c), Placement::Gui);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-compiler reactive_component_seeds_gui`
Expected: FAIL — `cannot find function seed_component` / `test_stub`.

- [ ] **Step 3: Add `HirReactiveComponent::test_stub` and `seed_component`**

Add a `#[doc(hidden)] pub fn test_stub(name: &str)` to `impl HirReactiveComponent` in `decl.rs` (mirror Task 2's approach against the struct at lines 640–652: empty `members`, `view: None`, empty `styles`, `layer: None`). Then in `placement.rs`:

```rust
use crate::hir::nodes::decl::HirReactiveComponent;

/// Components are always GUI-tier.
#[must_use]
pub fn seed_component(_c: &HirReactiveComponent) -> Placement {
    Placement::Gui
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-compiler reactive_component_seeds_gui`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-compiler/src/typeck/placement.rs crates/vox-compiler/src/hir/nodes/decl.rs
git commit -m "feat(placement): components seed gui"
```

---

## Task 4: `PlacementMap` + module-level seed pass

**Files:**
- Modify: `crates/vox-compiler/src/typeck/placement.rs`
- Test: in `placement.rs`

Build the keyed map over a whole `HirModule` (functions + endpoint_fns + components), seed-only.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn map_seeds_every_declaration() {
    use crate::hir::nodes::decl::HirModule;
    let mut m = HirModule::test_stub();
    let mut pure_fn = HirFn::test_stub("fmt");
    pure_fn.is_pure = true;
    let mut db_fn = HirFn::test_stub("load");
    db_fn.capabilities = vec![HirCapability::Db];
    m.functions = vec![pure_fn, db_fn];

    let map = PlacementMap::seed(&m);
    assert_eq!(map.get("fmt"), Some(Placement::Shared));
    assert_eq!(map.get("load"), Some(Placement::Native));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-compiler map_seeds_every_declaration`
Expected: FAIL — `cannot find type PlacementMap` / `HirModule::test_stub`.

- [ ] **Step 3: Implement `PlacementMap` + `HirModule::test_stub`**

Add a `#[doc(hidden)] pub fn test_stub() -> HirModule` to `impl HirModule` in `decl.rs` (all vecs empty — read fields at lines 30–162; if `HirModule: Default`, use `HirModule::default()`). Then in `placement.rs`:

```rust
use std::collections::HashMap;
use crate::hir::nodes::decl::HirModule;

/// Placement per declaration, keyed by name. (Names are unique within a module
/// at this stage; switch to `DefId` keys if/when name collisions become real.)
#[derive(Debug, Default)]
pub struct PlacementMap(HashMap<String, Placement>);

impl PlacementMap {
    /// Seed every function, endpoint, and component. No propagation yet.
    #[must_use]
    pub fn seed(m: &HirModule) -> PlacementMap {
        let mut map = HashMap::new();
        for f in &m.functions {
            map.insert(f.name.clone(), seed_fn(f));
        }
        for ep in &m.endpoint_fns {
            // Endpoints (@server/@query/@mutation) are always native.
            map.insert(ep.name.clone(), Placement::Native);
        }
        for c in &m.components {
            map.insert(c.name.clone(), seed_component(c));
        }
        PlacementMap(map)
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<Placement> {
        self.0.get(name).copied()
    }
}
```

> Verify the field name on `HirEndpointFn` is `name` (decl.rs around line 48 / the `HirEndpointFn` definition). Adjust if it differs.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-compiler map_seeds_every_declaration`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-compiler/src/typeck/placement.rs crates/vox-compiler/src/hir/nodes/decl.rs
git commit -m "feat(placement): PlacementMap module-level seed"
```

---

## Task 5: Call-graph propagation to a fixed point

**Files:**
- Modify: `crates/vox-compiler/src/typeck/placement.rs`
- Test: in `placement.rs`

A `shared` function that *calls* a `native` function is pulled to `native`; one that calls a `gui` function is pulled to `gui`. Iterate to a fixed point. (We need the set of callee names per function — walk `HirStmt`/`HirExpr` for `Call`/`MethodCall` nodes.)

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn caller_of_native_becomes_native() {
    use crate::hir::nodes::decl::HirModule;
    let mut m = HirModule::test_stub();
    let mut leaf = HirFn::test_stub("read_db");
    leaf.capabilities = vec![HirCapability::Db];
    // `wrapper` is unannotated (would seed Shared) but calls read_db.
    let wrapper = HirFn::test_stub_calling("wrapper", &["read_db"]);
    m.functions = vec![leaf, wrapper];

    let map = PlacementMap::infer(&m);
    assert_eq!(map.get("wrapper"), Some(Placement::Native));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-compiler caller_of_native_becomes_native`
Expected: FAIL — `HirFn::test_stub_calling` and `PlacementMap::infer` missing.

- [ ] **Step 3: Add the callee-extraction helper and `infer`**

Add `HirFn::test_stub_calling(name, callees)` in `decl.rs` that builds a body of `HirStmt::Expr(HirExpr::Call { callee: Ident(c), .. })` for each callee — match the real `HirExpr::Call`/`Ident` shape (read `hir/nodes/stmt_expr.rs`). In `placement.rs`:

```rust
use crate::hir::nodes::stmt_expr::{HirExpr, HirStmt};

/// Collect the names of functions called in a body (Call/MethodCall on an Ident callee).
fn callee_names(body: &[HirStmt]) -> Vec<String> {
    let mut out = Vec::new();
    fn walk_expr(e: &HirExpr, out: &mut Vec<String>) {
        match e {
            HirExpr::Call { callee, args, .. } => {
                if let HirExpr::Ident { name, .. } = &**callee {
                    out.push(name.clone());
                }
                for a in args { walk_expr(a, out); }
            }
            // Recurse into the other expr variants that contain sub-exprs
            // (Binary, Unary, If, Match, Block, MethodCall, FieldAccess, ...).
            // Read hir/nodes/stmt_expr.rs and cover each arm holding HirExpr children.
            _ => {}
        }
    }
    fn walk_stmt(s: &HirStmt, out: &mut Vec<String>) {
        match s {
            HirStmt::Expr(e) | HirStmt::Return(Some(e)) => walk_expr(e, out),
            HirStmt::Let { value, .. } => walk_expr(value, out),
            // ...cover Assign, While, Loop bodies per the HirStmt definition.
            _ => {}
        }
    }
    for s in body { walk_stmt(s, &mut out); }
    out
}

impl PlacementMap {
    /// Seed, then propagate callee constraints to a fixed point.
    #[must_use]
    pub fn infer(m: &HirModule) -> PlacementMap {
        let mut map = PlacementMap::seed(m);
        // Build name -> callees once.
        let edges: Vec<(String, Vec<String>)> = m
            .functions
            .iter()
            .map(|f| (f.name.clone(), callee_names(&f.body)))
            .collect();

        let mut changed = true;
        while changed {
            changed = false;
            for (caller, callees) in &edges {
                let cur = map.get(caller).unwrap_or(Placement::Shared);
                for callee in callees {
                    if let Some(cp) = map.get(callee) {
                        let next = join(cur, cp);
                        if next != cur && map.set_if_stronger(caller, next) {
                            changed = true;
                        }
                    }
                }
            }
        }
        map
    }

    /// Returns true if it moved `name` to `p` (only when `p` is more specific than current).
    fn set_if_stronger(&mut self, name: &str, p: Placement) -> bool {
        let cur = self.0.get(name).copied().unwrap_or(Placement::Shared);
        if cur == Placement::Shared && p != Placement::Shared {
            self.0.insert(name.to_string(), p);
            true
        } else {
            false
        }
    }
}

/// Lattice join used during propagation: Shared yields to a specialization;
/// two equal specializations stay; Native+Gui is left for the conflict pass
/// (represented here by returning the *caller's* current value unchanged so the
/// conflict detector — Task 6 — can see both pulls).
fn join(caller: Placement, callee: Placement) -> Placement {
    match (caller, callee) {
        (Placement::Shared, p) => p,
        (p, _) => p, // keep caller; conflict detection handled separately
    }
}
```

> The `callee_names` walker MUST cover every `HirExpr`/`HirStmt` variant that holds sub-expressions — an incomplete walker silently misses calls (a false negative). Use the variant list in `feature_matrix.rs` `ExprFeature`/`StmtFeature` (lines 214–256) as the checklist.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-compiler caller_of_native_becomes_native`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-compiler/src/typeck/placement.rs crates/vox-compiler/src/hir/nodes/decl.rs
git commit -m "feat(placement): call-graph propagation to fixed point"
```

---

## Task 6: Conflict detection — `E-PLACE-CONFLICT`

**Files:**
- Modify: `crates/vox-compiler/src/typeck/placement.rs`
- Test: in `placement.rs`

A function whose own seed is one specialization but which calls into the *other* specialization is a conflict.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn native_fn_calling_gui_is_conflict() {
    use crate::hir::nodes::decl::HirModule;
    let mut m = HirModule::test_stub();
    let mut native_caller = HirFn::test_stub_calling("hybrid", &["Widget"]);
    native_caller.capabilities = vec![HirCapability::Db]; // seeds Native
    let mut gui_callee = HirFn::test_stub("Widget");
    gui_callee.is_reactive = true; // seeds Gui
    m.functions = vec![native_caller, gui_callee];

    let diags = infer(&m, "");
    assert!(
        diags.iter().any(|d| d.code.as_deref() == Some(codes::PLACEMENT_CONFLICT)),
        "expected a placement-conflict diagnostic"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-compiler native_fn_calling_gui_is_conflict`
Expected: FAIL — `cannot find function infer` (the pass entry point).

- [ ] **Step 3: Implement the pass entry `infer(&HirModule, &str) -> Vec<Diagnostic>`**

```rust
use crate::typeck::diagnostics::{codes, Diagnostic};

/// The placement pass entry point wired into typeck. Returns conflict +
/// boundary + unsat diagnostics (boundary/unsat added in Tasks 7 & 9).
#[must_use]
pub fn infer(m: &HirModule, source: &str) -> Vec<Diagnostic> {
    let map = PlacementMap::infer(m);
    let mut diags = Vec::new();

    for f in &m.functions {
        let own = seed_fn(f);
        let callees = callee_names(&f.body);
        for callee in &callees {
            if let Some(cp) = map.get(callee) {
                let incompatible = matches!(
                    (own, cp),
                    (Placement::Native, Placement::Gui) | (Placement::Gui, Placement::Native)
                );
                if incompatible {
                    diags.push(
                        Diagnostic::error(
                            format!(
                                "`{}` is {:?}-placed but calls `{}` which is {:?}-placed — split the function or cross the boundary via an endpoint",
                                f.name, own, callee, cp
                            ),
                            f.span,
                            source,
                        )
                        .with_code(codes::PLACEMENT_CONFLICT)
                        .with_suggestion(format!(
                            "extract the {cp:?}-tier work, or wrap `{callee}` in `@query fn` and call it across the boundary"
                        )),
                    );
                }
            }
        }
    }
    diags
}
```

> Confirm `Diagnostic::error(message, span, source_snippet)` signature against diagnostics.rs:1169–1180 and that `.with_code`/`.with_suggestion` exist (lines 544–558). The `code` field is `Option<String>` — `with_code` sets it.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-compiler native_fn_calling_gui_is_conflict`
Expected: PASS.

- [ ] **Step 5: Add a negative test (no false positive)**

```rust
#[test]
fn shared_fn_calling_native_is_not_conflict() {
    use crate::hir::nodes::decl::HirModule;
    let mut m = HirModule::test_stub();
    let wrapper = HirFn::test_stub_calling("wrapper", &["read_db"]); // seeds Shared
    let mut leaf = HirFn::test_stub("read_db");
    leaf.capabilities = vec![HirCapability::Db];
    m.functions = vec![wrapper, leaf];
    let diags = infer(&m, "");
    assert!(diags.iter().all(|d| d.code.as_deref() != Some(codes::PLACEMENT_CONFLICT)));
}
```

Run: `cargo test -p vox-compiler shared_fn_calling_native_is_not_conflict`
Expected: PASS (shared→native is fine; only native↔gui conflicts).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-compiler/src/typeck/placement.rs
git commit -m "feat(placement): E-PLACE-CONFLICT detection for native<->gui pulls"
```

---

## Task 7: Boundary check — `E-PLACE-BOUNDARY` (gui → native direct call)

**Files:**
- Modify: `crates/vox-compiler/src/typeck/placement.rs`
- Test: in `placement.rs`

A `gui` declaration that directly calls a `native` declaration which is **not** an endpoint is an illegal boundary crossing. (Endpoints — `hir.endpoint_fns` — are the legal crossing and are exempt as callees.)

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn gui_calling_native_nonendpoint_is_boundary_error() {
    use crate::hir::nodes::decl::HirModule;
    let mut m = HirModule::test_stub();
    let mut view = HirFn::test_stub_calling("View", &["read_db"]);
    view.is_reactive = true; // gui
    let mut leaf = HirFn::test_stub("read_db");
    leaf.capabilities = vec![HirCapability::Db]; // native, NOT an endpoint
    m.functions = vec![view, leaf];

    let diags = infer(&m, "");
    assert!(diags.iter().any(|d| d.code.as_deref() == Some(codes::PLACEMENT_BOUNDARY)));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-compiler gui_calling_native_nonendpoint_is_boundary_error`
Expected: FAIL — no boundary diagnostic emitted yet.

- [ ] **Step 3: Extend `infer` with the boundary check**

In `infer`, after building `map`, compute the set of endpoint names and emit a boundary diagnostic when a `gui`-placed function calls a `native`, non-endpoint function:

```rust
    let endpoint_names: std::collections::HashSet<&str> =
        m.endpoint_fns.iter().map(|e| e.name.as_str()).collect();

    for f in &m.functions {
        if map.get(&f.name) != Some(Placement::Gui) {
            continue;
        }
        for callee in callee_names(&f.body) {
            let callee_native = map.get(&callee) == Some(Placement::Native);
            if callee_native && !endpoint_names.contains(callee.as_str()) {
                diags.push(
                    Diagnostic::error(
                        format!(
                            "GUI function `{}` calls native function `{}` directly — cross the boundary via an endpoint",
                            f.name, callee
                        ),
                        f.span,
                        source,
                    )
                    .with_code(codes::PLACEMENT_BOUNDARY)
                    .with_suggestion(format!("wrap `{callee}` in `@query fn` (or `@server`/`@mutation`) and call the generated client")),
                );
            }
        }
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-compiler gui_calling_native_nonendpoint_is_boundary_error`
Expected: PASS.

- [ ] **Step 5: Add the negative test (endpoint crossing is legal)**

```rust
#[test]
fn gui_calling_endpoint_is_allowed() {
    use crate::hir::nodes::decl::{HirEndpointFn, HirModule};
    let mut m = HirModule::test_stub();
    let mut view = HirFn::test_stub_calling("View", &["list_tasks"]);
    view.is_reactive = true;
    m.functions = vec![view];
    m.endpoint_fns = vec![HirEndpointFn::test_stub("list_tasks")]; // legal crossing
    let diags = infer(&m, "");
    assert!(diags.iter().all(|d| d.code.as_deref() != Some(codes::PLACEMENT_BOUNDARY)));
}
```

Add `HirEndpointFn::test_stub(name)` in `decl.rs` (mirror Task 2; read the `HirEndpointFn` struct). Run: `cargo test -p vox-compiler gui_calling_endpoint_is_allowed`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-compiler/src/typeck/placement.rs crates/vox-compiler/src/hir/nodes/decl.rs
git commit -m "feat(placement): E-PLACE-BOUNDARY for direct gui->native calls"
```

---

## Task 8: Wire the placement pass into typeck

**Files:**
- Modify: `crates/vox-compiler/src/typeck/mod.rs` (passes vec, lines 124–162)
- Test: `crates/vox-compiler/tests/` integration test (new) OR an in-module test that runs the full `typecheck_hir_module_with_path`

- [ ] **Step 1: Write the failing integration test**

Create `crates/vox-compiler/tests/placement_pass_wired.rs`:

```rust
//! Proves the placement pass runs inside typeck and surfaces conflicts end-to-end.
use vox_compiler::pipeline::{compile_source, CompileOptions}; // adjust to the real entry

#[test]
fn gui_calling_native_directly_is_rejected_by_typeck() {
    let src = r#"
        @reactive fn View() { read_db() }
        fn read_db() uses db { 0 }
    "#;
    let result = compile_source(src, &CompileOptions::default());
    let diags = result.diagnostics(); // adjust to real accessor
    assert!(
        diags.iter().any(|d| d.code.as_deref() == Some("vox/placement/boundary")),
        "expected a placement boundary diagnostic from typeck; got: {diags:?}"
    );
}
```

> Adjust imports/accessors to the real public compile entry point — read `crates/vox-compiler/src/pipeline.rs` (lines 60–152) and `lib.rs` exports to find the function that returns diagnostics for a source string. If no string-level helper is public, use the same helper the existing `tests/` files use.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-compiler --test placement_pass_wired`
Expected: FAIL — no boundary diagnostic (pass not wired).

- [ ] **Step 3: Wire the pass**

In `crates/vox-compiler/src/typeck/mod.rs`, add a closure to the `passes` vec (near line 134, after the `effect_check` closure):

```rust
        Box::new(|| placement::infer(hir, source)),
```

Confirm the closure captures the same `hir: &HirModule` and `source: &str` bindings the sibling passes use (the `effect_check::check_effect_compliance(hir, source)` line is the template).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-compiler --test placement_pass_wired`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-compiler/src/typeck/mod.rs crates/vox-compiler/tests/placement_pass_wired.rs
git commit -m "feat(placement): wire placement pass into typeck"
```

---

## Task 9: `@place(...)` override + satisfiability — `E-PLACE-UNSAT`

**Files:**
- Modify: `crates/vox-compiler/src/lexer/token.rs` (add `@place` token)
- Modify: `crates/vox-compiler/src/parser/descent/decl/head_fn.rs` (parse `@place(...)`)
- Modify: `crates/vox-compiler/src/hir/nodes/decl.rs` (add `placement_override` field to `HirFn`)
- Modify: `crates/vox-compiler/src/typeck/placement.rs` (honor + verify override)
- Test: in `placement.rs` and a parser test

`@place(native|shared|gui)` is an argument-bearing decorator. **Model its lexing/parsing on the existing `@uses(...)` clause** (which also takes an identifier argument and sets `capabilities` on `HirFn`) — read how `uses` is tokenized and parsed before writing this task's parser code.

- [ ] **Step 1: Write the failing unit test (satisfiability)**

In `placement.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementHint { Shared, Native, Gui }

#[test]
fn place_gui_on_db_fn_is_unsat() {
    let mut f = HirFn::test_stub("bad");
    f.capabilities = vec![HirCapability::Db];      // wants native
    f.placement_override = Some(PlacementHint::Gui); // but forced gui
    let diags = check_override(&f, "");
    assert!(diags.iter().any(|d| d.code.as_deref() == Some(codes::PLACEMENT_UNSAT)));
}

#[test]
fn place_native_is_always_sat() {
    let mut f = HirFn::test_stub("ok");
    f.capabilities = vec![HirCapability::Db];
    f.placement_override = Some(PlacementHint::Native);
    assert!(check_override(&f, "").is_empty());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-compiler place_gui_on_db_fn_is_unsat`
Expected: FAIL — `placement_override` field and `check_override` missing.

- [ ] **Step 3: Add the field + satisfiability check**

Add to `HirFn` in `decl.rs`:

```rust
    /// Explicit `@place(...)` override, if present. `None` = inferred.
    #[serde(default)]
    pub placement_override: Option<crate::typeck::placement::PlacementHint>,
```

(Update `HirFn::test_stub` to set `placement_override: None`.) In `placement.rs`:

```rust
/// Verify an explicit `@place(...)` override is satisfiable given the fn's effects.
#[must_use]
pub fn check_override(f: &HirFn, source: &str) -> Vec<Diagnostic> {
    let Some(hint) = f.placement_override else { return Vec::new() };
    let needs_native = f.capabilities.iter().any(is_native_capability)
        || f.is_remote || f.is_llm || f.durability.is_some();
    let unsat = match hint {
        PlacementHint::Native => false,               // native is always satisfiable
        PlacementHint::Gui | PlacementHint::Shared => needs_native,
    };
    if unsat {
        vec![Diagnostic::error(
            format!("`@place({hint:?})` on `{}` is unsatisfiable — it uses native-only effects", f.name),
            f.span,
            source,
        )
        .with_code(codes::PLACEMENT_UNSAT)
        .with_suggestion("remove the override, use @place(native), or route the effect through an endpoint".to_string())]
    } else {
        Vec::new()
    }
}
```

Also make `seed_fn` honor the override first: `if let Some(h) = f.placement_override { return hint_to_placement(h); }` at the top of `seed_fn`, with a small `hint_to_placement` mapping. Call `check_override` for every function inside `infer` and extend `diags`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vox-compiler place_gui_on_db_fn_is_unsat place_native_is_always_sat`
Expected: PASS.

- [ ] **Step 5: Add the `@place` token + parser, with a parser test**

Add to `token.rs` (near the other `At*` tokens, ~line 179):

```rust
    #[token("@place")]
    AtPlace,
```

and its `Display` arm (~line 598): `Token::AtPlace => write!(f, "@place"),`.

In `head_fn.rs` (the decorator-parsing match near line 133), add an arm for `Token::AtPlace` that consumes `( ident )`, maps `native|shared|gui` → `PlacementHint`, and stores it so lowering sets `HirFn::placement_override`. Follow the `@uses(...)` argument-parsing path exactly. Add a parser test:

```rust
#[test]
fn parses_place_native_override() {
    let hir = lower_str("@place(native) fn f() { 0 }"); // use the test helper used by sibling parser tests
    assert_eq!(hir.functions[0].placement_override, Some(PlacementHint::Native));
}
```

Run: `cargo test -p vox-compiler parses_place_native_override`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-compiler/src/lexer/token.rs crates/vox-compiler/src/parser/descent/decl/head_fn.rs crates/vox-compiler/src/hir/nodes/decl.rs crates/vox-compiler/src/typeck/placement.rs
git commit -m "feat(placement): @place(...) override + E-PLACE-UNSAT satisfiability"
```

---

## Task 10: Feature-matrix integration — add `Place`, remove `Native`, parity test

**Files:**
- Modify: `crates/vox-compiler/src/feature_matrix.rs` (enum line 170, spelling line 339, counts in tests)
- Test: in `feature_matrix.rs` test module

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn place_decorator_is_registered_and_native_is_gone() {
    // @place exists as a feature...
    let _ = support(Feature::Decorator(DecoratorFeature::Place), Target::TypeScript);
    // ...and its spelling is correct.
    assert_eq!(DecoratorFeature::Place.lexer_spelling(), "@place");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-compiler place_decorator_is_registered_and_native_is_gone`
Expected: FAIL — `no variant Place`.

- [ ] **Step 3: Swap the enum variant and spelling**

In `feature_matrix.rs`: replace `Native,` (line 170) with `Place,`; replace `Native => "@native",` (line 339) with `Place => "@place",`. `is_ladder_proven_decorator` does not list `Native`, so no change there; `Place` will fall to `unverified()` via the `_` arm in `decorator_row` until a ladder fixture proves it (Task 11 adds the fixture; decision 5B).

Update the count assertions in the test module: `DecoratorFeature::ALL.len()` stays **56** (one variant swapped, not added) — verify `feature_all_has_expected_count` still passes. If `DecoratorFeature::ALL` is a hand-written array, replace `Native` with `Place` there too.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vox-compiler -- feature_matrix`
Expected: PASS (including `feature_all_has_expected_count`, `matrix_is_total`).

- [ ] **Step 5: Add the placement↔matrix parity test**

```rust
#[test]
fn placement_matches_feature_matrix_frontend_backend() {
    // A gui-only construct (JSX) must be frontend-only in the matrix;
    // a native-only construct (Spawn) must be backend-only. This guards that
    // per-declaration placement and per-feature matrix verdicts cannot drift.
    assert!(matches!(
        support(Feature::Expr(ExprFeature::Jsx), Target::RustAxum),
        Support::Unsupported(_)
    ));
    assert!(matches!(
        support(Feature::Expr(ExprFeature::Spawn), Target::TypeScript),
        Support::Unsupported(_)
    ));
}
```

Run: `cargo test -p vox-compiler placement_matches_feature_matrix_frontend_backend`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-compiler/src/feature_matrix.rs
git commit -m "feat(placement): add @place to feature matrix, remove @native variant"
```

---

## Task 11: Retire the `@native` token everywhere + corpus sweep (decision A + B)

**Files:**
- Modify: `crates/vox-compiler/src/lexer/token.rs` (remove `AtNative` token + Display arm)
- Modify: `crates/vox-compiler/src/parser/descent/decl/head_fn.rs` (remove `Token::AtNative` arm)
- Modify: `crates/vox-compiler/src/language_surface.rs` (lines 205, 228: `@native` → `@place`)
- Modify: `crates/vox-compiler/src/lexer/cursor.rs` (line 253 test string)
- Modify: `crates/vox-code-audit/src/detectors/retired_decorator.rs` (add `@native` → `@place(native)` row)
- Modify: `AGENTS.md` (Retired Surfaces table, after the `@py.import` row ~line 407)

- [ ] **Step 1: Write the failing detector test**

In `retired_decorator.rs` test module, add:

```rust
#[test]
fn flags_retired_native_decorator() {
    let src = SourceFile::vox_for_test("scripts/x.vox", "@native fn perf() { 0 }");
    let findings = RetiredDecoratorDetector::new().detect(&src, None);
    assert!(findings.iter().any(|f| f.message.contains("@native")
        && f.suggestion.as_deref().unwrap_or("").contains("@place(native)")));
}
```

> Match `SourceFile`'s real test constructor used by the sibling tests in this file.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-code-audit flags_retired_native_decorator`
Expected: FAIL — `@native` not in the retired table.

- [ ] **Step 3: Add `@native` to the retired-decorator table**

In `retired_decorator.rs`, add an entry mapping `@native` → suggestion `Replace @native with @place(native)` (mirror the existing `@component fn` / `@endpoint` entries' structure and severity `Error`).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-code-audit flags_retired_native_decorator`
Expected: PASS.

- [ ] **Step 5: Remove the token + parser arm + surface entries**

- `token.rs`: delete `#[token("@native")] AtNative,` (line 179) and `Token::AtNative => write!(f, "@native"),` (line 598).
- `head_fn.rs`: remove `Token::AtNative` from the match arm at line 133 (leave `Token::AtFuzz`).
- `language_surface.rs`: replace `"@native"` with `"@place"` in both arrays (lines 205, 228).
- `cursor.rs`: change the test string at line 253 from `... @native` to `... @place` and update the expected token assertion accordingly.

Run: `cargo build -p vox-compiler`
Expected: compiles (no remaining references to `AtNative`).

- [ ] **Step 6: Update AGENTS.md Retired Surfaces table**

After the `@py.import` row (~line 407) add:

```markdown
| `@native` (decorator) | `@place(native)` |
```

- [ ] **Step 7: Corpus sweep + triage (parallel sub-agents — decision B)**

Run the full corpus through the new pass and fix every diagnostic before merge:

Run: `cargo test -p vox-compiler` and `cargo run -q -p vox-cli -- check examples/golden/*.vox` (use the real golden-check command the canonical ladder uses; read `contracts/pipeline/canonical-ladder.v1.yaml`).

Dispatch parallel sub-agents (one per fixture cluster: `examples/golden/**`, `crates/vox-codegen/tests/`, ladder fixtures). Each sub-agent triages every new placement diagnostic:
- **True positive** (real cross-tier bug) → fix the fixture (`.vox`) and note it.
- **False positive** (inference wrong) → fix the rule in `placement.rs`, add a regression unit test, do NOT silence by editing the fixture.
- **False negative** watch: for each fixture with a known legitimate cross-tier call, confirm the pass *did* fire where expected.

Collect results; the milestone is **green corpus under blocking placement diagnostics**.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-compiler/src/lexer/token.rs crates/vox-compiler/src/parser/descent/decl/head_fn.rs crates/vox-compiler/src/language_surface.rs crates/vox-compiler/src/lexer/cursor.rs crates/vox-code-audit/src/detectors/retired_decorator.rs AGENTS.md examples/ crates/vox-codegen/tests/
git commit -m "feat(placement): retire @native token; corpus green under placement gate"
```

---

## Task 12: Pass codegen the placement map (forward-link to sub-specs 2/3)

**Files:**
- Modify: `crates/vox-compiler/src/typeck/placement.rs` (make `PlacementMap` + `Placement` public from a stable path)
- Modify: `crates/vox-compiler/src/lib.rs` (re-export `pub use typeck::placement::{Placement, PlacementMap};`)
- Test: in `placement.rs`

This is the minimal seam sub-specs 2 (speed) and 3 (oracle/dashboard) consume; we do NOT change emitters here.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn placement_map_is_publicly_constructible() {
    use vox_compiler::{Placement, PlacementMap};
    let _ = Placement::Shared;
    // compile-time check that the type is re-exported at the crate root.
    fn _takes(_m: &PlacementMap) {}
}
```

Place this in a new `crates/vox-compiler/tests/placement_public_api.rs`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-compiler --test placement_public_api`
Expected: FAIL — types not re-exported at crate root.

- [ ] **Step 3: Re-export from lib.rs**

Add to `crates/vox-compiler/src/lib.rs`:

```rust
pub use typeck::placement::{Placement, PlacementHint, PlacementMap};
```

Ensure `PlacementMap`, `Placement`, `PlacementHint` are `pub` in `placement.rs` (they are from Tasks 2/4/9).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-compiler --test placement_public_api`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-compiler/src/lib.rs crates/vox-compiler/tests/placement_public_api.rs
git commit -m "feat(placement): re-export Placement/PlacementMap for downstream sub-specs"
```

---

## Final verification

- [ ] Run `cargo test -p vox-compiler` — all placement unit + integration tests green.
- [ ] Run `cargo test -p vox-code-audit retired_decorator` — `@native` flagged.
- [ ] Run `cargo build -p vox-compiler` — no `AtNative`/`Native` references remain.
- [ ] Run `cargo run -q -p vox-arch-check` — no new layer violations.
- [ ] Run `cargo clippy -p vox-compiler -p vox-code-audit -- -D warnings` (per the pre-push clippy-gap note in AGENTS.md).
- [ ] Run `vox run scripts/fmt.vox` (never `cargo fmt --all` on Windows).
- [ ] Confirm the corpus sweep (Task 11 Step 7) left zero unresolved placement diagnostics.

## Notes for the implementer

- **Effect data is the source of truth.** Placement is only as accurate as `HirFn.capabilities` / `is_pure` / `is_reactive` / `durability`. If a capability is missing on a function that clearly does I/O, that's an upstream effect-inference gap — note it, don't paper over it in placement.
- **Walker completeness is a correctness invariant.** The `callee_names` walker (Task 5) MUST visit every sub-expression. An incomplete walker produces false negatives (missed boundary violations) that no test will catch unless you add one. Cross-check against the `ExprFeature`/`StmtFeature` variant lists.
- **Names vs DefIds.** `PlacementMap` is keyed by name for simplicity. If the corpus sweep surfaces shadowing/overload collisions, switch the key to `HirFn.id: DefId` (the field exists) — this is a localized change to `PlacementMap`.
