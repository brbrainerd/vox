//! # Vox Type Checker
//!
//! This crate implements the name resolution and bidirectional type checking
//! logic for the Vox compiler. It operates on the High-Level Intermediate
//! Representation (HIR).
//!
//! Key components:
//! - Unification-based type inference
//! - ADT and pattern matching validation
//! - Builtin type registration
//! - Diagnostic reporting for semantic errors

#![allow(clippy::collapsible_if)]

mod ast_decl_lints;
pub mod async_exhaustiveness;
mod async_handler_lint;
pub mod boilerplate_grafts;
pub mod contrast;
pub mod determinism_lint;
mod effect_deps_lint;
pub mod form_check;
pub mod layer;
pub mod semantic_ui;
mod stale_capture_lint;

pub use ast_decl_lints::lint_ast_declarations;
/// Automated fix suggestions for type-check diagnostics.
pub mod autofix;
/// Logic for registering and accessing builtin types and functions.
pub mod builtins;
// All type checking flows through `typecheck_hir` in the `checker` module
// (implemented in `checker/mod.rs`).
/// Central state machine for the type checking process.
pub mod checker;
/// CUDA availability gate for MENS training decorators (Mn-T5).
pub mod cuda_gate;
/// Diagnostic structures and error reporting for type checking.
pub mod diagnostics;
/// Effect propagation check: `caller.capabilities ⊇ callee.capabilities`.
pub mod effect_check;
/// Environment management for symbols, types, and scopes.
pub mod env;
/// Core logic for unification-based type inference.
pub mod infer;
pub mod policy;
/// Logic for registering declarations into the global environment.
pub mod registration;
/// State machine exhaustiveness and structural checks.
pub mod state_machine_check;
/// Representation of internal types used during inference and checking.
pub mod ty;
/// Logic for unification of types and solving constraints.
pub mod unify;

use crate::ast::decl::Module;
pub use crate::hir::HirModule;
use crate::hir::lower::lower_module;
pub use builtins::BuiltinTypes;
pub use checker::{Checker, typecheck_hir};
/// A single semantic diagnostic (error or warning) produced during type checking.
pub use diagnostics::{Diagnostic, DiagnosticCategory, DiagnosticFix, TypeckSeverity};
pub use env::TypeEnv;
pub use ty::ty_display;

/// Run the type Checker on a HirModule (replacement for the removed AST-only path).
///
/// # Parallelism
///
/// - **Phase 1 (sequential):** `typecheck_hir` — mutates `hir.inferred_types`; must finish
///   before any lint can read those types.
/// - **Phase 2 (parallel):** All subsequent read-only lint passes are dispatched in parallel
///   via [`rayon`], then merged in deterministic declaration order. The wall-clock time for
///   phase 2 is reduced from O(N·lint_count) to O(max(lint_time)) on multi-core machines.
#[must_use]
pub fn typecheck_hir_module(source: &str, hir: &mut HirModule) -> Vec<Diagnostic> {
    typecheck_hir_module_with_path(source, hir, None)
}

/// Like [`typecheck_hir_module`] but also resolves intra-project
/// `import "./foo.vox"` directives against disk, registering each imported
/// file's `pub fn`s into the type environment before checking the main
/// module. `source_path` is the absolute path of the file being checked —
/// imports are resolved relative to its parent directory.
///
/// Cycle-safe: a per-call visited set prevents re-entering a file already
/// in the middle of being typechecked. Idempotent re-imports of the same
/// file are no-ops.
///
/// See `docs/src/architecture/intra-project-imports-rfc-2026-05-23.md`.
#[must_use]
pub fn typecheck_hir_module_with_path(
    source: &str,
    hir: &mut HirModule,
    source_path: Option<&std::path::Path>,
) -> Vec<Diagnostic> {
    use rayon::prelude::*;

    // ── Phase 1: mutating type-inference pass (sequential) ────────────────
    let mut env = TypeEnv::new();
    let builtins = BuiltinTypes::register_all(&mut env);

    // Pre-register pub signatures from any intra-project local-file imports
    // so call sites in the main module type-check against real types
    // rather than `ImportPlaceholder`.
    if let Some(path) = source_path {
        let mut visited: std::collections::HashSet<std::path::PathBuf> =
            std::collections::HashSet::new();
        if let Ok(canon) = std::fs::canonicalize(path) {
            visited.insert(canon);
        }
        for imp in &hir.imports {
            if let Some(rel) = imp.local_file_path.as_ref() {
                resolve_imported_pubs_into_env(
                    &mut env,
                    rel,
                    path,
                    imp.local_file_alias.as_deref(),
                    &mut visited,
                );
            }
        }
    }
    let mut diags = typecheck_hir(hir, &mut env, &builtins, source);

    // ── Phase 2: read-only lint passes (parallel fan-out) ─────────────────
    // Each closure captures a shared reference to `hir` and `source`.  They
    // are all `Send` because `HirModule` and `&str` are `Sync`.  We collect
    // into independent `Vec<Diagnostic>` per pass and merge below.
    //
    // IMPORTANT: add new lint passes to this list rather than back to the
    // sequential chain above.
    type LintFn<'a> = Box<dyn Fn() -> Vec<Diagnostic> + Send + 'a>;

    let passes: Vec<LintFn<'_>> = vec![
        Box::new(|| effect_check::check_effect_compliance(hir, source)),
        Box::new(|| cuda_gate::check_training_cuda_tier(hir, source)),
        Box::new(|| state_machine_check::check_state_machines(hir, source)),
        Box::new(|| effect_deps_lint::check_effect_deps(hir, source)),
        Box::new(|| stale_capture_lint::check_stale_captures(hir, source)),
        // Task 6.1: reject non-deterministic stdlib calls inside `workflow` bodies (ADR-019 §5).
        Box::new(|| determinism_lint::check_workflow_determinism(hir, source)),
        Box::new(|| async_handler_lint::check_async_handlers(hir, source)),
        Box::new(|| form_check::check_forms(hir, source)),
        // GA-20 / CC-23: contrast-ratio validation for design token color pairs.
        Box::new(|| contrast::check_tokens(&hir.token_decls)),
        // GA-19: a11y label enforcement for semantic UI primitives.
        Box::new(|| semantic_ui::check_semantic_ui(&collect_semantic_ui_callsites(hir))),
        // GA-01: Async[T] view exhaustiveness (all four arms required).
        Box::new(|| {
            collect_async_views(hir)
                .into_iter()
                .filter_map(|v| async_exhaustiveness::check_async_view(&v))
                .collect()
        }),
    ];

    // Parallel execution: each pass runs on a rayon worker thread.
    let parallel_diags: Vec<Vec<Diagnostic>> = passes.into_par_iter().map(|pass| pass()).collect();

    for batch in parallel_diags {
        diags.extend(batch);
    }

    // ── Phase 3: per-item passes (still sequential — depend on iteration order) ──
    // GA-16/GA-06/GA-23/GA-26: per-endpoint decorator validation.
    for ep in &hir.endpoint_fns {
        if let Some(w) = &ep.webhook {
            diags.extend(boilerplate_grafts::check_webhook_decl(w));
        }
        if let Some(c) = &ep.cors {
            diags.extend(boilerplate_grafts::check_cors_policy(c));
        }
        if let Some(p) = &ep.pii {
            if let Some(d) = boilerplate_grafts::check_pii_with_net_effect(p, &ep.effects, &ep.name)
            {
                diags.push(d);
            }
        }
        if let Some(l) = &ep.layer {
            if let Some(d) = layer::check_system_overlay_reservation(l) {
                diags.push(d);
            }
        }
    }
    // GA-21 + AI fixtures: structural checks on every semantic-core function-like surface.
    let declared_type_names: std::collections::HashSet<&str> =
        hir.types.iter().map(|t| t.name.as_str()).collect();
    let ai_fixture_fn_sources = hir
        .functions
        .iter()
        .chain(hir.tests.iter())
        .chain(hir.examples.iter())
        .chain(hir.foralls.iter().map(|p| &p.func))
        .chain(hir.mcp_tools.iter().map(|t| &t.func))
        .chain(hir.mcp_resources.iter().map(|r| &r.func));
    for f in ai_fixture_fn_sources {
        if let Some(ao) = &f.ai_structured_output {
            let has_codec = declared_type_names.contains(ao.return_type.as_str());
            if let Some(d) = boilerplate_grafts::check_ai_return_shape(ao, has_codec) {
                diags.push(d);
            }
        }
        diags.extend(boilerplate_grafts::collect_ai_fixture_diagnostics(f));
        if let Some(crate::hir::nodes::boilerplate_grafts::HirAiFixture::Hole(hole)) = &f.ai_fixture
        {
            diags.push(boilerplate_grafts::check_unfilled_fixture_hole(hole));
            if let Some(stale) = boilerplate_grafts::check_fixture_hole_staleness(hole) {
                diags.push(stale);
            }
        }
        // GA-24: @embed dimension validity.
        if let Some(embed) = &f.embed {
            if let Some(d) = boilerplate_grafts::check_embed_dimensions(embed) {
                diags.push(d);
            }
        }
    }
    diags
}

/// Resolve one intra-project local-file import for typecheck. Loads the
/// referenced file from disk, lexes + parses + lowers it, then registers
/// only its `pub fn` signatures into `env`. Recursively resolves the
/// imported file's own local-file imports first so transitive pubs are
/// visible. Cycle-safe via `visited` (per-call set of canonicalized paths).
///
/// Errors (file not found, parse failure) are silent here — the eval-time
/// resolver already produces clear diagnostics for runtime failures, and
/// the typecheck pre-pass should not block check on transient IO issues.
/// A missing pub fn will surface naturally as an Unknown-name diagnostic
/// from the regular typecheck pass, which is the right place for that
/// signal.
fn resolve_imported_pubs_into_env(
    env: &mut env::TypeEnv,
    rel_path: &str,
    importer_path: &std::path::Path,
    alias: Option<&str>,
    visited: &mut std::collections::HashSet<std::path::PathBuf>,
) {
    let base_dir = importer_path.parent().unwrap_or(std::path::Path::new("."));
    let joined = base_dir.join(rel_path);
    let canonical = match std::fs::canonicalize(&joined) {
        Ok(c) => c,
        Err(_) => return,
    };
    if !visited.insert(canonical.clone()) {
        return;
    }
    let source = match std::fs::read_to_string(&canonical) {
        Ok(s) => s,
        Err(_) => return,
    };
    let tokens = crate::lexer::lex(&source);
    let module = match crate::parser::parse_script(tokens) {
        Ok(m) => m,
        Err(_) => return,
    };
    let imported_hir = crate::hir::lower::lower_module(&module);

    // Recurse into transitive imports first.
    for imp in &imported_hir.imports {
        if let Some(nested_rel) = imp.local_file_path.as_ref() {
            resolve_imported_pubs_into_env(
                env,
                nested_rel,
                &canonical,
                imp.local_file_alias.as_deref(),
                visited,
            );
        }
    }

    // Register pub fn signatures.
    if let Some(name) = alias {
        // Alias form: build a Ty::Record where each field is the pub fn's
        // signature. MethodCall typecheck routes `alias.field(args)` through
        // the Record-method dispatch arm in typeck/checker/expr.rs.
        let mut fields: Vec<(String, ty::Ty)> = Vec::new();
        for f in &imported_hir.functions {
            if !f.is_pub {
                continue;
            }
            let param_tys: Vec<ty::Ty> = f
                .params
                .iter()
                .map(|p| {
                    p.type_ann
                        .as_ref()
                        .map(|t| registration::resolve_hir_type(t, env))
                        .unwrap_or(ty::Ty::Infer)
                })
                .collect();
            let ret_ty = f
                .return_type
                .as_ref()
                .map(|t| registration::resolve_hir_type(t, env))
                .unwrap_or(ty::Ty::Unit);
            fields.push((f.name.clone(), ty::Ty::Fn(param_tys, Box::new(ret_ty))));
        }
        if env.lookup(name).is_none() {
            env.define(
                name.to_string(),
                env::Binding::new(ty::Ty::Record(fields), false, env::BindingKind::Variable),
            );
        }
    } else {
        for f in &imported_hir.functions {
            if !f.is_pub {
                continue;
            }
            if env.lookup(&f.name).is_some() {
                // Importer-defined name wins per RFC §3.
                continue;
            }
            registration::register_hir_function(env, f, None);
        }
        // Pub ADT variants: register the types so resolve_hir_type can find
        // them and so variant constructors are callable.
        for t in &imported_hir.types {
            if !t.is_pub {
                continue;
            }
            registration::register_hir_typedef(env, t);
        }
    }
}

/// Walk all statements in a function body looking for `Async[T]` view nodes.
fn collect_async_views(hir: &HirModule) -> Vec<crate::hir::nodes::async_view::HirAsyncView> {
    use crate::hir::HirExpr;
    use crate::hir::HirStmt;

    fn visit_expr(expr: &HirExpr, out: &mut Vec<crate::hir::nodes::async_view::HirAsyncView>) {
        match expr {
            HirExpr::AsyncView(v) => {
                // Check nested arms too before pushing this node.
                if let Some(arm) = &v.fetching_arm {
                    visit_expr(arm, out);
                }
                if let Some(arm) = &v.empty_arm {
                    visit_expr(arm, out);
                }
                if let Some(arm) = &v.error_arm {
                    visit_expr(arm, out);
                }
                if let Some(arm) = &v.ok_arm {
                    visit_expr(arm, out);
                }
                out.push(*v.clone());
            }
            HirExpr::Block(stmts, _) => {
                for s in stmts {
                    visit_stmt(s, out);
                }
            }
            HirExpr::If(cond, then_stmts, else_stmts, _) => {
                visit_expr(cond, out);
                for s in then_stmts {
                    visit_stmt(s, out);
                }
                if let Some(es) = else_stmts {
                    for s in es {
                        visit_stmt(s, out);
                    }
                }
            }
            HirExpr::Binary(_, l, r, _) => {
                visit_expr(l, out);
                visit_expr(r, out);
            }
            HirExpr::Unary(_, e, _) => visit_expr(e, out),
            HirExpr::Call(f, args, _, _) => {
                visit_expr(f, out);
                for a in args {
                    visit_expr(&a.value, out);
                }
            }
            HirExpr::MethodCall(recv, _, args, _, _) => {
                visit_expr(recv, out);
                for a in args {
                    visit_expr(&a.value, out);
                }
            }
            HirExpr::Lambda(_, _, body, _, _) => visit_expr(body, out),
            HirExpr::For(_, _, iter, body, key, _) => {
                visit_expr(iter, out);
                visit_expr(body, out);
                if let Some(k) = key {
                    visit_expr(k, out);
                }
            }
            HirExpr::Match(e, arms, _) => {
                visit_expr(e, out);
                for arm in arms {
                    visit_expr(&arm.body, out);
                }
            }
            HirExpr::Jsx(el) => {
                for a in &el.attributes {
                    visit_expr(&a.value, out);
                }
                for c in &el.children {
                    visit_expr(c, out);
                }
            }
            HirExpr::JsxSelfClosing(el) => {
                for a in &el.attributes {
                    visit_expr(&a.value, out);
                }
            }
            HirExpr::JsxFragment(children, _) => {
                for c in children {
                    visit_expr(c, out);
                }
            }
            HirExpr::Try(t) => visit_expr(&t.target, out),
            HirExpr::Index(a, b, _) => {
                visit_expr(a, out);
                visit_expr(b, out);
            }
            HirExpr::With(a, b, _) => {
                visit_expr(a, out);
                visit_expr(b, out);
            }
            HirExpr::FieldAccess(e, _, _) => visit_expr(e, out),
            HirExpr::Spawn(e, _) => visit_expr(e, out),
            HirExpr::ObjectLit(fields, _) => {
                for (_, v) in fields {
                    visit_expr(v, out);
                }
            }
            HirExpr::ListLit(items, _) | HirExpr::TupleLit(items, _) => {
                for v in items {
                    visit_expr(v, out);
                }
            }
            _ => {}
        }
    }

    fn visit_stmt(stmt: &HirStmt, out: &mut Vec<crate::hir::nodes::async_view::HirAsyncView>) {
        match stmt {
            HirStmt::Let { value, .. } => visit_expr(value, out),
            HirStmt::Assign { target, value, .. } => {
                visit_expr(target, out);
                visit_expr(value, out);
            }
            HirStmt::Expr { expr, .. } => visit_expr(expr, out),
            HirStmt::Return { value: Some(e), .. } => visit_expr(e, out),
            HirStmt::While {
                condition, body, ..
            } => {
                visit_expr(condition, out);
                for s in body {
                    visit_stmt(s, out);
                }
            }
            HirStmt::Loop { body, .. } => {
                for s in body {
                    visit_stmt(s, out);
                }
            }
            _ => {}
        }
    }

    let mut out = vec![];
    for f in &hir.functions {
        for s in &f.body {
            visit_stmt(s, &mut out);
        }
    }
    for comp in &hir.components {
        use crate::hir::nodes::HirReactiveMember;
        for m in &comp.members {
            match m {
                HirReactiveMember::State(s) => visit_expr(&s.init, &mut out),
                HirReactiveMember::Derived(d) => visit_expr(&d.expr, &mut out),
                HirReactiveMember::Effect(e) => visit_expr(&e.body, &mut out),
                HirReactiveMember::OnMount(m) => visit_expr(&m.body, &mut out),
                HirReactiveMember::OnCleanup(c) => visit_expr(&c.body, &mut out),
                HirReactiveMember::Stmt(s) => visit_stmt(s, &mut out),
            }
        }
        if let Some(view) = &comp.view {
            visit_expr(view, &mut out);
        }
    }
    out
}

/// Collect semantic UI primitive callsites from all JSX in the HIR.
///
/// Finds `<Dialog>`, `<Menu>`, `<Listbox>`, `<Combobox>`, `<Tabs>` elements and
/// records whether they carry a `label` attribute, so `check_semantic_ui` can
/// enforce the a11y requirement.
fn collect_semantic_ui_callsites(hir: &HirModule) -> Vec<semantic_ui::SemanticUiCallSite> {
    use crate::hir::HirExpr;
    use crate::hir::HirJsxAttr;
    use crate::hir::HirStmt;

    const PRIMITIVES: &[&str] = &["Dialog", "Menu", "Listbox", "Combobox", "Tabs"];

    fn check_attrs(attrs: &[HirJsxAttr]) -> bool {
        attrs.iter().any(|a| a.name == "label")
    }

    fn visit_expr(expr: &HirExpr, out: &mut Vec<semantic_ui::SemanticUiCallSite>) {
        match expr {
            HirExpr::Jsx(el) => {
                if PRIMITIVES.contains(&el.tag.as_str()) {
                    out.push(semantic_ui::SemanticUiCallSite {
                        primitive: el.tag.clone(),
                        has_label: check_attrs(&el.attributes),
                        span: el.span,
                    });
                }
                for a in &el.attributes {
                    visit_expr(&a.value, out);
                }
                for c in &el.children {
                    visit_expr(c, out);
                }
            }
            HirExpr::JsxSelfClosing(el) => {
                if PRIMITIVES.contains(&el.tag.as_str()) {
                    out.push(semantic_ui::SemanticUiCallSite {
                        primitive: el.tag.clone(),
                        has_label: check_attrs(&el.attributes),
                        span: el.span,
                    });
                }
                for a in &el.attributes {
                    visit_expr(&a.value, out);
                }
            }
            HirExpr::JsxFragment(children, _) => {
                for c in children {
                    visit_expr(c, out);
                }
            }
            HirExpr::Block(stmts, _) => {
                for s in stmts {
                    visit_stmt(s, out);
                }
            }
            HirExpr::If(cond, then_stmts, else_stmts, _) => {
                visit_expr(cond, out);
                for s in then_stmts {
                    visit_stmt(s, out);
                }
                if let Some(es) = else_stmts {
                    for s in es {
                        visit_stmt(s, out);
                    }
                }
            }
            HirExpr::For(_, _, iter, body, key, _) => {
                visit_expr(iter, out);
                visit_expr(body, out);
                if let Some(k) = key {
                    visit_expr(k, out);
                }
            }
            HirExpr::Lambda(_, _, body, _, _) => visit_expr(body, out),
            HirExpr::Match(e, arms, _) => {
                visit_expr(e, out);
                for arm in arms {
                    visit_expr(&arm.body, out);
                }
            }
            HirExpr::AsyncView(v) => {
                if let Some(arm) = &v.fetching_arm {
                    visit_expr(arm, out);
                }
                if let Some(arm) = &v.empty_arm {
                    visit_expr(arm, out);
                }
                if let Some(arm) = &v.error_arm {
                    visit_expr(arm, out);
                }
                if let Some(arm) = &v.ok_arm {
                    visit_expr(arm, out);
                }
            }
            HirExpr::Call(f, args, _, _) => {
                visit_expr(f, out);
                for a in args {
                    visit_expr(&a.value, out);
                }
            }
            HirExpr::MethodCall(recv, _, args, _, _) => {
                visit_expr(recv, out);
                for a in args {
                    visit_expr(&a.value, out);
                }
            }
            _ => {}
        }
    }

    fn visit_stmt(stmt: &HirStmt, out: &mut Vec<semantic_ui::SemanticUiCallSite>) {
        match stmt {
            HirStmt::Let { value, .. } => visit_expr(value, out),
            HirStmt::Assign { target, value, .. } => {
                visit_expr(target, out);
                visit_expr(value, out);
            }
            HirStmt::Expr { expr, .. } => visit_expr(expr, out),
            HirStmt::Return { value: Some(e), .. } => visit_expr(e, out),
            HirStmt::While {
                condition, body, ..
            } => {
                visit_expr(condition, out);
                for s in body {
                    visit_stmt(s, out);
                }
            }
            HirStmt::Loop { body, .. } => {
                for s in body {
                    visit_stmt(s, out);
                }
            }
            _ => {}
        }
    }

    let mut out = vec![];
    for f in &hir.functions {
        for s in &f.body {
            visit_stmt(s, &mut out);
        }
    }
    for comp in &hir.components {
        use crate::hir::nodes::HirReactiveMember;
        for m in &comp.members {
            match m {
                HirReactiveMember::State(s) => visit_expr(&s.init, &mut out),
                HirReactiveMember::Derived(d) => visit_expr(&d.expr, &mut out),
                HirReactiveMember::Effect(e) => visit_expr(&e.body, &mut out),
                HirReactiveMember::OnMount(m) => visit_expr(&m.body, &mut out),
                HirReactiveMember::OnCleanup(c) => visit_expr(&c.body, &mut out),
                HirReactiveMember::Stmt(s) => visit_stmt(s, &mut out),
            }
        }
        if let Some(view) = &comp.view {
            visit_expr(view, &mut out);
        }
    }
    out
}

/// Lower `module` to HIR and run the type Checker (replacement for the removed AST-only path).
#[must_use]
pub fn typecheck_ast_module(source: &str, module: &Module) -> Vec<Diagnostic> {
    let mut diags = ast_decl_lints::lint_ast_declarations(module, source);
    let mut hir = lower_module(module);
    diags.extend(typecheck_hir_module(source, &mut hir));
    diags
}

/// Type-check a parsed module. Pass `source` for diagnostic context (may be `""`).
///
/// This is the stable name used by the CLI, LSP, and integration tests; it delegates to the HIR pipeline.
#[must_use]
pub fn typecheck_module(module: &Module, source: &str) -> Vec<Diagnostic> {
    typecheck_ast_module(source, module)
}
