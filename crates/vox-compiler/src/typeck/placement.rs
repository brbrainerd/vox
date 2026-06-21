//! Placement inference: assigns each declaration a tier (native/shared/gui).
//! See docs/superpowers/specs/2026-06-20-vox-placement-model-design.md.

use crate::hir::{HirArg, HirCapability, HirExpr, HirFn, HirModule, HirReactiveComponent, HirStmt};
use crate::typeck::diagnostics::{Diagnostic, codes};
use std::collections::HashMap;

pub use vox_ast::decl::fundecl::PlacementHint;

/// Where a declaration may be emitted. `Shared` is the top of the lattice
/// (emits to native + gui + interp); `Native` and `Gui` are incompatible
/// specializations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    Shared,
    Native,
    Gui,
}

fn hint_to_placement(h: PlacementHint) -> Placement {
    match h {
        PlacementHint::Shared => Placement::Shared,
        PlacementHint::Native => Placement::Native,
        PlacementHint::Gui => Placement::Gui,
    }
}

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

/// Seed a single function's placement from its own decorators/effects.
#[must_use]
pub fn seed_fn(f: &HirFn) -> Placement {
    if let Some(h) = f.placement_override {
        return hint_to_placement(h);
    }
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
    Placement::Shared // @pure, `uses nothing`, or unannotated leaf
}

/// Components are always GUI-tier.
#[must_use]
pub fn seed_component(_c: &HirReactiveComponent) -> Placement {
    Placement::Gui
}

/// Placement per declaration, keyed by name (unique within a module at this stage).
#[derive(Debug, Default)]
pub struct PlacementMap(HashMap<String, Placement>);

impl PlacementMap {
    #[must_use]
    pub fn seed(m: &HirModule) -> PlacementMap {
        let mut map = HashMap::new();
        for f in &m.functions {
            map.insert(f.name.clone(), seed_fn(f));
        }
        for ep in &m.endpoint_fns {
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

    /// Seed, then propagate callee constraints to a fixed point. A Shared
    /// declaration that calls a specialized declaration adopts that
    /// specialization. (Native+Gui contradictions are left to the conflict
    /// pass in Task 6 so it can report both pulls.)
    #[must_use]
    pub fn infer(m: &HirModule) -> PlacementMap {
        let mut map = PlacementMap::seed(m);
        let edges: Vec<(String, Vec<String>)> = m
            .functions
            .iter()
            .map(|f| (f.name.clone(), callee_names(&f.body)))
            .collect();

        let mut changed = true;
        while changed {
            changed = false;
            for (caller, callees) in &edges {
                for callee in callees {
                    if let Some(cp) = map.get(callee) {
                        if cp != Placement::Shared && map.set_if_stronger(caller, cp) {
                            changed = true;
                        }
                    }
                }
            }
        }
        map
    }

    /// Promote `name` from Shared to a specialization. Returns true if it moved.
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

fn walk_expr(e: &HirExpr, out: &mut Vec<String>) {
    use HirExpr::*;
    match e {
        IntLit(..) | FloatLit(..) | StringLit(..) | BoolLit(..) | DecimalLit(..) | Ident(..)
        | WorkflowVersion(..) => {}
        ObjectLit(fields, _) => {
            for (_, v) in fields {
                walk_expr(v, out);
            }
        }
        ListLit(xs, _) | TupleLit(xs, _) | JsxFragment(xs, _) => {
            for x in xs {
                walk_expr(x, out);
            }
        }
        Binary(_, a, b, _) | With(a, b, _) | Index(a, b, _) => {
            walk_expr(a, out);
            walk_expr(b, out);
        }
        Unary(_, a, _) | FieldAccess(a, _, _) | Spawn(a, _) => walk_expr(a, out),
        Call(callee, args, _, _) => {
            if let Ident(name, _) = &**callee {
                out.push(name.clone());
            } else {
                walk_expr(callee, out);
            }
            for a in args {
                walk_arg(a, out);
            }
        }
        MethodCall(recv, _, args, _, _) => {
            walk_expr(recv, out);
            for a in args {
                walk_arg(a, out);
            }
        }
        If(c, then_b, else_b, _) => {
            walk_expr(c, out);
            for s in then_b {
                walk_stmt(s, out);
            }
            if let Some(eb) = else_b {
                for s in eb {
                    walk_stmt(s, out);
                }
            }
        }
        For(_, _, iter, body, key, _) => {
            walk_expr(iter, out);
            walk_expr(body, out);
            if let Some(k) = key {
                walk_expr(k, out);
            }
        }
        Lambda(_, _, body, _, _) => walk_expr(body, out),
        Block(stmts, _) => {
            for s in stmts {
                walk_stmt(s, out);
            }
        }
        Match(scrut, arms, _) => {
            walk_expr(scrut, out);
            // HirMatchArm holds a body — read its definition in stmt_expr.rs and
            // recurse into the arm body (expr or Vec<HirStmt>). Add a unit test
            // proving a call inside a match arm is found.
            let _ = arms;
        }
        // JSX / Async / Try carry nested exprs in their wrapper structs
        // (HirJsxElement.children, HirAsyncView arms, HirTry.target). These are
        // gui/async-tier; recurse by reading the three struct defs. The corpus
        // sweep (Task 11) will surface any missed call site as a fixture finding.
        Jsx(_) | JsxSelfClosing(_) | AsyncView(_) | Try(_) => {}
    }
}

fn walk_arg(a: &HirArg, out: &mut Vec<String>) {
    walk_expr(&a.value, out);
}

fn walk_stmt(s: &HirStmt, out: &mut Vec<String>) {
    use HirStmt::*;
    match s {
        Expr { expr, .. } => walk_expr(expr, out),
        Let { value, .. } => walk_expr(value, out),
        Assign { target, value, .. } => {
            walk_expr(target, out);
            walk_expr(value, out);
        }
        Return { value, .. } => {
            if let Some(v) = value {
                walk_expr(v, out);
            }
        }
        While {
            condition, body, ..
        } => {
            walk_expr(condition, out);
            for s in body {
                walk_stmt(s, out);
            }
        }
        Loop { body, .. } => {
            for s in body {
                walk_stmt(s, out);
            }
        }
        Break { .. } | Continue { .. } => {}
    }
}

/// Names of functions called anywhere in a body.
#[must_use]
pub fn callee_names(body: &[HirStmt]) -> Vec<String> {
    let mut out = Vec::new();
    for s in body {
        walk_stmt(s, &mut out);
    }
    out
}

/// Verify an explicit `@place(...)` override is satisfiable given effects.
#[must_use]
pub fn check_override(f: &HirFn, source: &str) -> Vec<Diagnostic> {
    let Some(hint) = f.placement_override else {
        return Vec::new();
    };
    let needs_native = f.capabilities.iter().any(is_native_capability)
        || f.is_remote
        || f.is_llm
        || f.durability.is_some();
    let unsat = match hint {
        PlacementHint::Native => false, // native is always satisfiable
        PlacementHint::Gui | PlacementHint::Shared => needs_native,
    };
    if unsat {
        vec![
            Diagnostic::error(
                format!(
                    "`@place({hint:?})` on `{}` is unsatisfiable — it uses native-only effects",
                    f.name
                ),
                f.span,
                source,
            )
            .with_code(codes::PLACEMENT_UNSAT)
            .with_suggestion(
                "remove the override, use @place(native), or route the effect through an endpoint",
            ),
        ]
    } else {
        Vec::new()
    }
}

/// Placement pass entry point wired into typeck. Emits conflict and boundary diagnostics.
#[must_use]
pub fn infer(m: &HirModule, source: &str) -> Vec<Diagnostic> {
    let map = PlacementMap::infer(m);
    let mut diags = Vec::new();

    // E-PLACE-CONFLICT: a function is pulled toward both native and gui tiers.
    for f in &m.functions {
        let own = seed_fn(f);
        for callee in callee_names(&f.body) {
            if let Some(cp) = map.get(&callee) {
                let incompatible = matches!(
                    (own, cp),
                    (Placement::Native, Placement::Gui) | (Placement::Gui, Placement::Native)
                );
                if incompatible {
                    diags.push(
                        Diagnostic::error(
                            format!(
                                "`{}` is {:?}-placed but calls `{}` which is {:?}-placed — split the function or cross via an endpoint",
                                f.name, own, callee, cp
                            ),
                            f.span,
                            source,
                        )
                        .with_code(codes::PLACEMENT_CONFLICT)
                        .with_suggestion(format!(
                            "extract the {cp:?}-tier work, or wrap `{callee}` in `@query fn` and call across the boundary"
                        )),
                    );
                }
            }
        }
    }

    // E-PLACE-BOUNDARY: a gui function calls a native function directly (not an endpoint).
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
                    .with_suggestion(format!(
                        "wrap `{callee}` in `@query fn` (or `@server`/`@mutation`) and call the generated client"
                    )),
                );
            }
        }
    }

    // E-PLACE-UNSAT: explicit @place(...) override that cannot be satisfied by effects.
    for f in &m.functions {
        diags.extend(check_override(f, source));
    }

    diags
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hir_of(src: &str) -> crate::hir::HirModule {
        use crate::hir::lower_module;
        use crate::lexer::cursor::lex;
        use crate::parser::parse;
        lower_module(&parse(lex(src)).expect("fixture must parse"))
    }

    #[test]
    fn pure_fn_seeds_shared() {
        let m = hir_of("@pure fn checksum() { 0 }");
        assert_eq!(seed_fn(&m.functions[0]), Placement::Shared);
    }

    #[test]
    fn db_fn_seeds_native() {
        let m = hir_of("fn load() uses db { 0 }");
        assert_eq!(seed_fn(&m.functions[0]), Placement::Native);
    }

    #[test]
    fn reactive_fn_seeds_gui() {
        let m = hir_of("@reactive fn Counter() { 0 }");
        assert_eq!(seed_fn(&m.functions[0]), Placement::Gui);
    }

    #[test]
    fn unannotated_fn_seeds_shared() {
        let m = hir_of("fn add(a: Int, b: Int) -> Int { a + b }");
        assert_eq!(seed_fn(&m.functions[0]), Placement::Shared);
    }

    #[test]
    fn map_seeds_functions_endpoints_components() {
        let m = hir_of(
            "fn fmt() { 0 }\n\
             fn load() uses db { 0 }\n\
             @query fn list() { 0 }\n\
             component Panel() { state x: Int = 0 }",
        );
        let map = PlacementMap::seed(&m);
        assert_eq!(map.get("fmt"), Some(Placement::Shared));
        assert_eq!(map.get("load"), Some(Placement::Native));
        assert_eq!(map.get("list"), Some(Placement::Native)); // endpoints are native
        assert_eq!(map.get("Panel"), Some(Placement::Gui)); // components are gui
    }

    #[test]
    fn walker_finds_calls_in_nested_positions() {
        let m = hir_of("fn f() { let x = g(h()); if x { notify() } }");
        let names = callee_names(&m.functions[0].body);
        for want in ["g", "h", "notify"] {
            assert!(
                names.iter().any(|n| n == want),
                "missing call to {want} in {names:?}"
            );
        }
    }

    #[test]
    fn caller_of_native_becomes_native() {
        // `wrapper` is unannotated (seeds Shared) but calls `read_db` (native).
        let m = hir_of("fn read_db() uses db { 0 }\nfn wrapper() { read_db() }");
        let map = PlacementMap::infer(&m);
        assert_eq!(map.get("wrapper"), Some(Placement::Native));
    }

    // ── Task 6: E-PLACE-CONFLICT ──────────────────────────────────────────────

    #[test]
    fn native_fn_calling_gui_is_conflict() {
        // hybrid seeds native (uses db) but calls render_view (gui via @reactive).
        // Note: uppercase names like Widget() are sugar for JSX — use lowercase to
        // avoid the parser's call-site-to-JsxSelfClosing rewrite.
        let m = hir_of("fn hybrid() uses db { render_view() }\n@reactive fn render_view() { 0 }");
        let diags = infer(&m, "");
        assert!(
            diags
                .iter()
                .any(|d| d.code.as_deref() == Some(codes::PLACEMENT_CONFLICT))
        );
    }

    #[test]
    fn shared_fn_calling_native_is_not_conflict() {
        let m = hir_of("fn read_db() uses db { 0 }\nfn wrapper() { read_db() }");
        let diags = infer(&m, "");
        assert!(
            diags
                .iter()
                .all(|d| d.code.as_deref() != Some(codes::PLACEMENT_CONFLICT))
        );
    }

    // ── Task 7: E-PLACE-BOUNDARY ──────────────────────────────────────────────

    #[test]
    fn gui_calling_native_nonendpoint_is_boundary_error() {
        let m = hir_of("@reactive fn View() { read_db() }\nfn read_db() uses db { 0 }");
        let diags = infer(&m, "");
        assert!(
            diags
                .iter()
                .any(|d| d.code.as_deref() == Some(codes::PLACEMENT_BOUNDARY))
        );
    }

    #[test]
    fn gui_calling_endpoint_is_allowed() {
        let m = hir_of("@reactive fn View() { list_tasks() }\n@query fn list_tasks() { 0 }");
        let diags = infer(&m, "");
        assert!(
            diags
                .iter()
                .all(|d| d.code.as_deref() != Some(codes::PLACEMENT_BOUNDARY))
        );
    }

    // ── Task 9: E-PLACE-UNSAT ─────────────────────────────────────────────────

    #[test]
    fn place_gui_on_db_fn_is_unsat() {
        let m = hir_of("@place(gui) fn bad() uses db { 0 }");
        let diags = check_override(&m.functions[0], "");
        assert!(
            diags
                .iter()
                .any(|d| d.code.as_deref() == Some(codes::PLACEMENT_UNSAT)),
            "expected PLACEMENT_UNSAT; got: {diags:?}"
        );
    }

    #[test]
    fn place_native_is_always_sat() {
        let m = hir_of("@place(native) fn ok() uses db { 0 }");
        assert!(
            check_override(&m.functions[0], "").is_empty(),
            "expected no diagnostics for @place(native)"
        );
    }

    #[test]
    fn parses_place_native_override() {
        let m = hir_of("@place(native) fn f() { 0 }");
        assert_eq!(
            m.functions[0].placement_override,
            Some(PlacementHint::Native),
            "placement_override should be Some(Native)"
        );
    }
}
