//! Thin wrapper that wires `RenameRegistry` alias resolution into the Vox parser.
//!
//! ## Design choice: wrapper module (not direct integration)
//!
//! The existing `parse()` function returns `Result<Module, Vec<ParseError>>` and
//! discards all `ParseSeverity::Warning` entries on success. Threading a registry
//! through every call site in the recursive-descent parser would require adding
//! an `Option<&RenameRegistry>` field to `Parser`, plus guard-clauses at every
//! primitive-recognition branch — a large, risky diff across a complex module.
//!
//! Instead, this wrapper:
//!   1. Lexes and parses the source with the existing pipeline.
//!   2. Walks the resulting AST to find `Expr::Jsx` and `Expr::JsxSelfClosing`
//!      nodes whose tags are deprecated (i.e., `registry.resolve(tag)` returns
//!      `Some(entry)` and `entry.kind == RenameKind::Primitive`).
//!   3. Rewrites the tag name in-place to the canonical name.
//!   4. Appends a `Warning` with the mandated message format.
//!
//! The caller gets a `ParseResult` with `warnings()` and `uses_primitive()` methods.
//! This is acceptable per the VUV-9 Task 4 spec ("the goal is correctness, not
//! architectural purity").

use crate::ast::decl::{Decl, Module};
use crate::ast::decl::{
    FragmentDecl, ReactiveComponentDecl, ReactiveMemberDecl, ReactiveModuleDecl,
};
use crate::ast::expr::{Expr, JsxElement, JsxSelfClosingElement, StringPart};
use crate::ast::span::Span;
use crate::ast::stmt::Stmt;
use crate::lexer;
use crate::parser::descent::parse;
use crate::parser::error::ParseError;
use crate::parser::renames::{RenameEntry, RenameKind, RenameRegistry};

// ── Public types ─────────────────────────────────────────────────────────────

/// A deprecation warning emitted when a renamed primitive is used.
#[derive(Debug, Clone)]
pub struct Warning {
    /// Human-readable message. Format is fixed by VUV-9 Task 4 spec:
    /// `primitive \`{from}\` was renamed to \`{to}\` in {since}; use the new name (run \`vox migrate names\` to update)`
    pub message: String,
    /// Source span of the deprecated identifier.
    pub span: Span,
}

/// Result of [`parse_with_registry`].
#[derive(Debug)]
pub struct ParseResult {
    /// The rewritten AST (deprecated primitive tags replaced with canonical names).
    pub module: Module,
    /// One warning per deprecated primitive invocation site.
    warnings: Vec<Warning>,
    /// Canonical primitive names that appear in the parsed source (after rewriting).
    primitives_used: std::collections::HashSet<String>,
}

impl ParseResult {
    /// Deprecation warnings: one per deprecated primitive call site.
    pub fn warnings(&self) -> &[Warning] {
        &self.warnings
    }

    /// Returns `true` if the (rewritten) source contains a call to the primitive
    /// named `name`. Names are matched against canonical (post-rename) identifiers.
    pub fn uses_primitive(&self, name: &str) -> bool {
        self.primitives_used.contains(name)
    }
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Parse `source` with alias resolution from `registry`.
///
/// Deprecated primitive names are rewritten to their canonical equivalents and
/// a [`Warning`] is emitted for each rewrite site. The returned [`ParseResult`]
/// exposes the rewritten [`crate::Module`], the warnings, and a `uses_primitive` query.
///
/// Returns `Err(Vec<ParseError>)` if the source has actual parse errors.
pub fn parse_with_registry(
    source: &str,
    registry: &RenameRegistry,
) -> Result<ParseResult, Vec<ParseError>> {
    // Step 1: lex + parse.
    let tokens = lexer::lex(source);
    let mut module = parse(tokens)?;

    // Step 2 & 3: walk AST, rewrite deprecated primitive tags, collect warnings.
    let mut warnings: Vec<Warning> = Vec::new();
    let mut primitives_used: std::collections::HashSet<String> = std::collections::HashSet::new();

    walk_module(&mut module, registry, &mut warnings, &mut primitives_used);

    Ok(ParseResult {
        module,
        warnings,
        primitives_used,
    })
}

// ── AST walker ────────────────────────────────────────────────────────────────

fn walk_module(
    module: &mut Module,
    registry: &RenameRegistry,
    warnings: &mut Vec<Warning>,
    primitives_used: &mut std::collections::HashSet<String>,
) {
    for decl in &mut module.declarations {
        walk_decl(decl, registry, warnings, primitives_used);
    }
}

fn walk_decl(
    decl: &mut Decl,
    registry: &RenameRegistry,
    warnings: &mut Vec<Warning>,
    primitives_used: &mut std::collections::HashSet<String>,
) {
    match decl {
        Decl::Function(f) => {
            for stmt in &mut f.body {
                walk_stmt(stmt, registry, warnings, primitives_used);
            }
        }
        Decl::ReactiveComponent(rc) => {
            walk_reactive_component(rc, registry, warnings, primitives_used)
        }
        Decl::ReactiveModule(rm) => walk_reactive_module(rm, registry, warnings, primitives_used),
        Decl::Fragment(frag) => walk_fragment(frag, registry, warnings, primitives_used),
        Decl::Const(c) => {
            walk_expr(&mut c.value, registry, warnings, primitives_used);
        }
        // Other decl kinds do not contain view expressions.
        _ => {}
    }
}

fn walk_reactive_component(
    rc: &mut ReactiveComponentDecl,
    registry: &RenameRegistry,
    warnings: &mut Vec<Warning>,
    primitives_used: &mut std::collections::HashSet<String>,
) {
    if let Some(view) = &mut rc.view {
        walk_expr(view, registry, warnings, primitives_used);
    }
    for member in &mut rc.members {
        walk_reactive_member(member, registry, warnings, primitives_used);
    }
}

fn walk_reactive_module(
    rm: &mut ReactiveModuleDecl,
    registry: &RenameRegistry,
    warnings: &mut Vec<Warning>,
    primitives_used: &mut std::collections::HashSet<String>,
) {
    for member in &mut rm.members {
        walk_reactive_member(member, registry, warnings, primitives_used);
    }
}

fn walk_fragment(
    frag: &mut FragmentDecl,
    registry: &RenameRegistry,
    warnings: &mut Vec<Warning>,
    primitives_used: &mut std::collections::HashSet<String>,
) {
    walk_expr(&mut frag.body, registry, warnings, primitives_used);
}

fn walk_reactive_member(
    member: &mut ReactiveMemberDecl,
    registry: &RenameRegistry,
    warnings: &mut Vec<Warning>,
    primitives_used: &mut std::collections::HashSet<String>,
) {
    match member {
        ReactiveMemberDecl::State(s) => {
            walk_expr(&mut s.init, registry, warnings, primitives_used);
        }
        ReactiveMemberDecl::Derived(d) => {
            walk_expr(&mut d.expr, registry, warnings, primitives_used);
        }
        ReactiveMemberDecl::Effect(e) => {
            walk_expr(&mut e.body, registry, warnings, primitives_used);
        }
        ReactiveMemberDecl::OnMount(m) => {
            walk_expr(&mut m.body, registry, warnings, primitives_used);
        }
        ReactiveMemberDecl::OnCleanup(c) => {
            walk_expr(&mut c.body, registry, warnings, primitives_used);
        }
        ReactiveMemberDecl::Stmt(s) => {
            walk_stmt(s, registry, warnings, primitives_used);
        }
    }
}

fn walk_stmt(
    stmt: &mut Stmt,
    registry: &RenameRegistry,
    warnings: &mut Vec<Warning>,
    primitives_used: &mut std::collections::HashSet<String>,
) {
    match stmt {
        Stmt::Expr { expr, .. } => walk_expr(expr, registry, warnings, primitives_used),
        Stmt::Let { value, .. } => walk_expr(value, registry, warnings, primitives_used),
        Stmt::Return { value: Some(v), .. } => walk_expr(v, registry, warnings, primitives_used),
        Stmt::Assign { value, .. } => walk_expr(value, registry, warnings, primitives_used),
        Stmt::While {
            condition, body, ..
        } => {
            walk_expr(condition, registry, warnings, primitives_used);
            for s in body {
                walk_stmt(s, registry, warnings, primitives_used);
            }
        }
        Stmt::Loop { body, .. } => {
            for s in body {
                walk_stmt(s, registry, warnings, primitives_used);
            }
        }
        Stmt::Return { value: None, .. } | Stmt::Break { .. } | Stmt::Continue { .. } => {}
    }
}

fn walk_expr(
    expr: &mut Expr,
    registry: &RenameRegistry,
    warnings: &mut Vec<Warning>,
    primitives_used: &mut std::collections::HashSet<String>,
) {
    match expr {
        // ── JSX with children ────────────────────────────────────────────────
        Expr::Jsx(el) => {
            maybe_rewrite_tag_jsx(el, registry, warnings, primitives_used);
            // Clone children indices to avoid borrow conflicts.
            for child in el.children.iter_mut() {
                walk_expr(child, registry, warnings, primitives_used);
            }
        }
        // ── Self-closing JSX ─────────────────────────────────────────────────
        Expr::JsxSelfClosing(el) => {
            maybe_rewrite_tag_self_closing(el, registry, warnings, primitives_used);
        }
        // ── Fragment ──────────────────────────────────────────────────────────
        Expr::JsxFragment { children, .. } => {
            for child in children.iter_mut() {
                walk_expr(child, registry, warnings, primitives_used);
            }
        }
        // ── Composite expressions ─────────────────────────────────────────────
        Expr::Block { stmts, .. } => {
            for stmt in stmts.iter_mut() {
                walk_stmt(stmt, registry, warnings, primitives_used);
            }
        }
        Expr::If {
            condition,
            then_body,
            else_body,
            ..
        } => {
            walk_expr(condition, registry, warnings, primitives_used);
            for stmt in then_body.iter_mut() {
                walk_stmt(stmt, registry, warnings, primitives_used);
            }
            if let Some(else_stmts) = else_body {
                for stmt in else_stmts.iter_mut() {
                    walk_stmt(stmt, registry, warnings, primitives_used);
                }
            }
        }
        Expr::For { iterable, body, .. } => {
            walk_expr(iterable, registry, warnings, primitives_used);
            walk_expr(body, registry, warnings, primitives_used);
        }
        Expr::Call { callee, args, .. } => {
            walk_expr(callee, registry, warnings, primitives_used);
            for arg in args.iter_mut() {
                walk_expr(&mut arg.value, registry, warnings, primitives_used);
            }
        }
        Expr::MethodCall { object, args, .. } => {
            walk_expr(object, registry, warnings, primitives_used);
            for arg in args.iter_mut() {
                walk_expr(&mut arg.value, registry, warnings, primitives_used);
            }
        }
        Expr::Binary { left, right, .. } => {
            walk_expr(left, registry, warnings, primitives_used);
            walk_expr(right, registry, warnings, primitives_used);
        }
        Expr::Unary { operand, .. } => {
            walk_expr(operand, registry, warnings, primitives_used);
        }
        Expr::Spawn { target, .. } => {
            walk_expr(target, registry, warnings, primitives_used);
        }
        Expr::Lambda { body, .. } => {
            walk_expr(body, registry, warnings, primitives_used);
        }
        Expr::Match { subject, arms, .. } => {
            walk_expr(subject, registry, warnings, primitives_used);
            for arm in arms.iter_mut() {
                walk_expr(&mut arm.body, registry, warnings, primitives_used);
            }
        }
        Expr::Pipe { left, right, .. } => {
            walk_expr(left, registry, warnings, primitives_used);
            walk_expr(right, registry, warnings, primitives_used);
        }
        Expr::FieldAccess { object, .. } => {
            walk_expr(object, registry, warnings, primitives_used);
        }
        Expr::Index { object, index, .. } => {
            walk_expr(object, registry, warnings, primitives_used);
            walk_expr(index, registry, warnings, primitives_used);
        }
        Expr::With {
            operand, options, ..
        } => {
            walk_expr(operand, registry, warnings, primitives_used);
            walk_expr(options, registry, warnings, primitives_used);
        }
        Expr::Try { target, .. } => {
            walk_expr(target, registry, warnings, primitives_used);
        }
        Expr::ListLit { elements, .. } | Expr::TupleLit { elements, .. } => {
            for element in elements.iter_mut() {
                walk_expr(element, registry, warnings, primitives_used);
            }
        }
        Expr::ObjectLit { fields, .. } => {
            for field in fields.iter_mut() {
                walk_expr(&mut field.1, registry, warnings, primitives_used);
            }
        }
        Expr::StringInterp { parts, .. } => {
            for part in parts.iter_mut() {
                if let StringPart::Interpolation(e) = part {
                    walk_expr(e, registry, warnings, primitives_used);
                }
            }
        }
        // Leaf expressions: literals, identifiers — nothing to walk or rewrite.
        Expr::IntLit { .. }
        | Expr::FloatLit { .. }
        | Expr::StringLit { .. }
        | Expr::BoolLit { .. }
        | Expr::DecimalLit { .. }
        | Expr::Ident { .. }
        | Expr::SideEffect { .. }
        | Expr::WorkflowVersion(_) => {}
    }
}

// ── Tag rewrite helpers ───────────────────────────────────────────────────────

fn maybe_rewrite_tag_jsx(
    el: &mut JsxElement,
    registry: &RenameRegistry,
    warnings: &mut Vec<Warning>,
    primitives_used: &mut std::collections::HashSet<String>,
) {
    if let Some(entry) = registry.resolve(&el.tag)
        && entry.kind == RenameKind::Primitive
    {
        let old = std::mem::replace(&mut el.tag, entry.to.clone());
        warnings.push(make_warning(entry, el.span, &old));
    }
    // Record canonical name (post-rewrite) as used.
    primitives_used.insert(el.tag.clone());
}

fn maybe_rewrite_tag_self_closing(
    el: &mut JsxSelfClosingElement,
    registry: &RenameRegistry,
    warnings: &mut Vec<Warning>,
    primitives_used: &mut std::collections::HashSet<String>,
) {
    if let Some(entry) = registry.resolve(&el.tag)
        && entry.kind == RenameKind::Primitive
    {
        let old = std::mem::replace(&mut el.tag, entry.to.clone());
        warnings.push(make_warning(entry, el.span, &old));
    }
    primitives_used.insert(el.tag.clone());
}

/// Builds the mandated deprecation warning message.
///
/// Format (spec-locked by VUV-9 Task 4):
/// `primitive \`{from}\` was renamed to \`{to}\` in {since}; use the new name (run \`vox migrate names\` to update)`
fn make_warning(entry: &RenameEntry, span: Span, old_name: &str) -> Warning {
    Warning {
        message: format!(
            "primitive `{}` was renamed to `{}` in {}; use the new name (run `vox migrate names` to update)",
            old_name, entry.to, entry.since
        ),
        span,
    }
}
