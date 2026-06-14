//! Shared HIR → TypeScript / JSX emission for reactive components, activities, and routes.
//!
//! **Migration (Web IR, ADR 012):** Structural JSX and route/view parity are owned by
//! [`super::web_ir`] (`lower`, `validate`, `emit_tsx`). This module is the **compatibility**
//! string emitter still used by Path C reactive codegen, routes, activities, and by Web IR lowering
//! where it needs HIR-shaped expressions (`emit_hir_expr`, attribute values). Prefer
//! [`super::web_ir::emit_tsx`] for new preview/parity work; keep changes here in sync with
//! `compat` so AST JSX (`super::jsx`) and HIR paths share one attribute/type matrix.
//!
//! **Compatibility tags (OP-S029):** grep/CI anchors pairing this module with `super::jsx` (OP-S031) and
//! reactive view emit ([`crate::codegen_ts::reactive`], OP-S037). Attribute semantics and DOM/event name
//! mapping stay in `compat`; do not fork the matrix into JSX or Web IR without updating all three.

mod async_walker;
pub mod compat;
mod state_deps;

use async_walker::stmt_has_async_call;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use vox_compiler::hir::*;

use super::builtin_registry::{BuiltinLowering, BuiltinRegistry};
pub use compat::{map_hir_type_to_ts, map_jsx_attr_name, map_jsx_tag, ts_string_literal};
pub(crate) use state_deps::extract_state_deps_with_diagnostics;

static EMPTY_ASYNC_FNS: OnceLock<HashSet<String>> = OnceLock::new();
static EMPTY_ENDPOINT_PARAMS: OnceLock<HashMap<String, Vec<String>>> = OnceLock::new();
static BUILTIN_REGISTRY: OnceLock<BuiltinRegistry> = OnceLock::new();

fn registry() -> &'static BuiltinRegistry {
    BUILTIN_REGISTRY.get_or_init(BuiltinRegistry::standard)
}

/// Emission context threaded through HIR → TS lowering.
///
/// `state_names` drives `set_x()` rewriting for reactive state. `async_fn_names` holds the names
/// of `@endpoint` functions whose call sites should receive `await` (only meaningful in event-
/// handler bodies where the handler arrow will be emitted as `async`).
///
/// `endpoint_params` maps each `@query`/`@mutation`/`@server` fn name to its ordered parameter
/// names. The generated `vox-client.ts` exposes these fns as taking a single named-args object
/// (`record_event({ event_kind, payload_json, ... })`); when this map is populated, a positional
/// call to such a fn (`record_event("mood", ...)`) is rewritten to that object form so the call
/// site matches the client signature. Empty in non-handler/non-component contexts, where bare
/// positional emit is retained.
///
/// `local_exec_db` is set ONLY when emitting a `@mutation`/`@query` body that
/// runs on-device (the RN local-execution path). It gates the rewrite of `db`
/// operations to `voxRuntime.recordMutation`/`replayTable` and the real `?`
/// (Try) propagation. It is `false` for every web/component context, so those
/// emit paths are byte-identical.
#[derive(Clone)]
pub struct EmitCtx<'a> {
    pub state_names: &'a HashSet<String>,
    pub async_fn_names: &'a HashSet<String>,
    pub endpoint_params: &'a HashMap<String, Vec<String>>,
    pub local_exec_db: bool,
    /// Set when emitting a component event-handler body (an `on_click`/`on_press`
    /// lambda) that calls an async client fn (`@query`/`@mutation`) or an async
    /// mobile call (`Speech.transcribe_microphone()`). Promotes the handler arrow
    /// to `async`, blocks to awaited async IIFEs, and `match` dispatch arrows to
    /// `async` — the same async machinery `local_exec_db` uses — WITHOUT the
    /// db-op / `?`-propagation lowering. Lets handler bodies legally `await` so a
    /// `match record_event(...) { Ok(..) => .. }` discriminates on the resolved
    /// value, not the pending Promise.
    pub handler_await: bool,
    /// Name of the endpoint fn whose body is currently being emitted (the
    /// tracing label passed to `recordMutation`); empty outside local-exec.
    pub current_fn: &'a str,
}

impl<'a> EmitCtx<'a> {
    /// Standard context: no async fn names (non-handler expression contexts).
    pub fn new(state_names: &'a HashSet<String>) -> Self {
        Self {
            state_names,
            async_fn_names: EMPTY_ASYNC_FNS.get_or_init(HashSet::new),
            endpoint_params: EMPTY_ENDPOINT_PARAMS.get_or_init(HashMap::new),
            local_exec_db: false,
            handler_await: false,
            current_fn: "",
        }
    }

    /// Handler context: calls to names in `async_fn_names` get `await` + the handler arrow is `async`.
    pub fn with_async(
        state_names: &'a HashSet<String>,
        async_fn_names: &'a HashSet<String>,
    ) -> Self {
        Self {
            async_fn_names,
            ..Self::new(state_names)
        }
    }

    /// Non-handler context (state inits, effects, on-mount) that still knows
    /// endpoint parameter names, so a multi-arg endpoint call outside a handler
    /// is rewritten to the named-object form. No `await` is added (that's the
    /// handler context's job).
    pub fn with_endpoints(
        state_names: &'a HashSet<String>,
        endpoint_params: &'a HashMap<String, Vec<String>>,
    ) -> Self {
        Self {
            endpoint_params,
            ..Self::new(state_names)
        }
    }

    /// Handler context that also knows endpoint parameter names, enabling the
    /// positional→named-object rewrite for `vox-client` endpoint calls.
    pub fn with_async_and_endpoints(
        state_names: &'a HashSet<String>,
        async_fn_names: &'a HashSet<String>,
        endpoint_params: &'a HashMap<String, Vec<String>>,
    ) -> Self {
        Self {
            async_fn_names,
            endpoint_params,
            ..Self::new(state_names)
        }
    }

    /// On-device local-execution context for a `@mutation`/`@query` body: db ops
    /// lower to `voxRuntime` journal calls and `?` propagates Result errors.
    pub fn local_exec(
        state_names: &'a HashSet<String>,
        async_fn_names: &'a HashSet<String>,
        endpoint_params: &'a HashMap<String, Vec<String>>,
        current_fn: &'a str,
    ) -> Self {
        Self {
            async_fn_names,
            endpoint_params,
            local_exec_db: true,
            current_fn,
            ..Self::new(state_names)
        }
    }

    /// Clone this context but force a plain (non-async) variant, preserving the
    /// `local_exec_db` flag, endpoint params, and current fn. Used by
    /// inner-scope rebuilds (blocks, lambdas) so the local-execution lowering
    /// survives nesting.
    pub fn to_plain(&self) -> Self {
        EmitCtx {
            state_names: self.state_names,
            async_fn_names: EMPTY_ASYNC_FNS.get_or_init(HashSet::new),
            endpoint_params: self.endpoint_params,
            local_exec_db: self.local_exec_db,
            handler_await: self.handler_await,
            current_fn: self.current_fn,
        }
    }

    /// Clone this context as an async event-handler body context: preserves
    /// `async_fn_names` + `endpoint_params` (so calls still `await` and rewrite to
    /// the named-args form) and sets `handler_await`, WITHOUT the `local_exec_db`
    /// db/`?` lowering. Used when an `on_click`/`on_press` lambda body calls an
    /// async client fn — the arrow is emitted `async` so those awaits are legal.
    pub fn to_handler(&self) -> Self {
        EmitCtx {
            handler_await: true,
            local_exec_db: false,
            ..self.clone()
        }
    }
}

/// Unwrap a single-expression block used as a JSX / attribute value (matches AST `unwrap_block`).
#[must_use]
pub(crate) fn unwrap_inline_hir_block_expr(expr: &HirExpr) -> &HirExpr {
    if let HirExpr::Block(stmts, _) = expr {
        if stmts.len() == 1 {
            if let HirStmt::Expr { expr: inner, .. } = &stmts[0] {
                return inner;
            }
        }
    }
    expr
}

/// If `stmts` is a single pure expression statement, return its emitted string so the caller can
/// use it directly (as an inline ternary branch or JSX child) instead of a void IIFE.
///
/// A single-expression block is always safe to inline: it produces a value, never void.
/// Multi-statement blocks still fall back to IIFEs.
fn extract_single_jsx_expr(stmts: &[HirStmt], ctx: &EmitCtx<'_>) -> Option<String> {
    if stmts.len() != 1 {
        return None;
    }
    if let HirStmt::Expr { expr, .. } = &stmts[0] {
        // Unwrap a single-expression block `{...}` that JSX expression children produce.
        let inner = unwrap_inline_hir_block_expr(expr);
        return Some(emit_hir_expr(inner, ctx));
    }
    None
}

/// Expand `bind={…}` into (`value` expr string, `onChange` handler string), aligned with
/// [`crate::codegen_ts::jsx::expand_bind_attribute`] and [`super::web_ir::lower::lower_jsx_attr_pair`].
#[must_use]
pub(crate) fn expand_bind_hir_attribute(expr: &HirExpr, ctx: &EmitCtx<'_>) -> (String, String) {
    let e = unwrap_inline_hir_block_expr(expr);
    match e {
        HirExpr::Ident(name, _) => {
            let setter = format!("set_{name}");
            let value = emit_hir_expr(e, ctx);
            (value, format!("(e) => {setter}(e.target.value)"))
        }
        HirExpr::FieldAccess(obj, field, _) => {
            let value_str = emit_hir_expr(e, ctx);
            let obj_str = emit_hir_expr(obj, ctx);
            let setter = match obj.as_ref() {
                HirExpr::Ident(obj_name, _) => format!("set_{obj_name}"),
                _ => format!("set_{}", emit_hir_expr(obj, ctx)),
            };
            let onchange = format!("(e) => {setter}({{...{obj_str}, {field}: e.target.value}})");
            (value_str, onchange)
        }
        _ => {
            let val = emit_hir_expr(e, ctx);
            (val, "(e) => {}".to_string())
        }
    }
}

#[inline]
fn map_vox_react_hook_callee(name: &str) -> &str {
    match name {
        "use_state" => "useState",
        "use_effect" => "useEffect",
        "use_memo" => "useMemo",
        "use_ref" => "useRef",
        "use_callback" => "useCallback",
        other => other,
    }
}

/// Wrap a child expression so TSX matches [`super::web_ir::emit_tsx`] [`DomNode::Expr`] (`{ts}`).
///
/// JSX subtree roots (elements) start with `<` and must not get an extra `{...}` layer.
pub(crate) fn wrap_jsx_hir_child_expr(emit: String) -> String {
    let t = emit.trim_start();
    if t.starts_with('<') {
        emit
    } else {
        format!("{{{emit}}}")
    }
}

/// Bug D: inline-replace `std.<namespace>.<method>(args)` calls with native JS equivalents
/// when the receiver is the literal `std` global. Returns `Some(emit)` if the call matched a
/// known `std.*` shim; `None` otherwise.
///
/// Browser-side components have no `std` runtime — these replacements emit code that uses
/// the platform's native APIs (`Date.now()` for `std.time.now_ms()`, etc.).
fn lower_std_namespace_call(
    obj: &HirExpr,
    method: &str,
    args: &[vox_compiler::hir::HirArg],
    ctx: &EmitCtx<'_>,
) -> Option<String> {
    // Receiver must be `std.<ns>` (FieldAccess(Ident("std"), ns)).
    let HirExpr::FieldAccess(root, ns, _) = obj else {
        return None;
    };
    let HirExpr::Ident(root_name, _) = root.as_ref() else {
        return None;
    };
    if root_name != "std" {
        return None;
    }
    let args_str: Vec<String> = args.iter().map(|a| emit_hir_expr(&a.value, ctx)).collect();
    match (ns.as_str(), method) {
        ("time", "now_ms") => Some("Date.now()".to_string()),
        ("time", "now_iso") => Some("new Date().toISOString()".to_string()),
        ("json", "stringify") => Some(format!("JSON.stringify({})", args_str.join(", "))),
        ("json", "parse") => Some(format!("JSON.parse({})", args_str.join(", "))),
        // `std.crypto.uuid()` mints record ids inside on-device mutation bodies
        // → the runtime's `uuid()`. Only reachable in local-exec contexts (no
        // component handler calls std.crypto), so it never affects web output.
        ("crypto", "uuid") => Some("voxRuntime.uuid()".to_string()),
        _ => None,
    }
}

/// Emit a HIR expression as TypeScript/JSX.
///
/// Pass [`EmitCtx::new`] for non-handler contexts; [`EmitCtx::with_async`] inside event handlers
/// so that calls to `@endpoint` functions receive `await`.
///
/// **Phase:** compat-legacy (OP-0138). Prefer [`super::web_ir::emit_tsx`] for structural parity and
/// preview emit; keep this in sync with `compat`.
#[must_use]
pub fn emit_hir_expr(expr: &HirExpr, ctx: &EmitCtx<'_>) -> String {
    match expr {
        HirExpr::IntLit(v, _) => v.to_string(),
        HirExpr::FloatLit(v, _) => v.to_string(),
        HirExpr::StringLit(v, _) => compat::ts_string_literal(v),
        HirExpr::BoolLit(v, _) => v.to_string(),
        HirExpr::Ident(name, _) => name.clone(),
        HirExpr::Binary(op, left, right, _) => {
            let l = emit_hir_expr(left, ctx);
            let r = emit_hir_expr(right, ctx);
            let op_str = match op {
                HirBinOp::Add => "+",
                HirBinOp::Sub => "-",
                HirBinOp::Mul => "*",
                HirBinOp::Div => "/",
                HirBinOp::Lt => "<",
                HirBinOp::Gt => ">",
                HirBinOp::Lte => "<=",
                HirBinOp::Gte => ">=",
                HirBinOp::And => "&&",
                HirBinOp::Or => "||",
                HirBinOp::Is => "===",
                HirBinOp::Isnt => "!==",
                HirBinOp::Mod => "%",
                HirBinOp::Pipe => "|>",
            };
            if matches!(op, HirBinOp::Pipe) {
                format!("{r}({l})")
            } else {
                format!("{l} {op_str} {r}")
            }
        }
        HirExpr::Unary(op, expr, _) => {
            let e = emit_hir_expr(expr, ctx);
            match op {
                HirUnOp::Not => format!("!{e}"),
                HirUnOp::Neg => format!("-{e}"),
            }
        }
        HirExpr::Block(stmts, _) => {
            // Inline single-JSX/if blocks so JSX child `{if ...}` emits as a ternary, not an IIFE.
            if let Some(inline) = extract_single_jsx_expr(stmts, ctx) {
                return inline;
            }
            // Handler bodies preserve `async_fn_names` (so nested calls keep their
            // `await`); local-exec inner scope uses `to_plain` (db ops lower via
            // the `local_exec_db` flag, which `to_plain` retains).
            let plain_ctx = if ctx.handler_await {
                ctx.clone()
            } else {
                ctx.to_plain()
            };
            if ctx.local_exec_db || ctx.handler_await {
                // On-device block-as-value: an `async` IIFE so arm/db awaits are
                // legal, and the trailing expression is `return`ed so the block
                // produces its value (e.g. a match-arm body `{ let x = …; <expr> }`).
                let mut out = String::from("(await (async () => {\n");
                let n = stmts.len();
                for (i, stmt) in stmts.iter().enumerate() {
                    if i + 1 == n {
                        if let HirStmt::Expr { expr, .. } = stmt {
                            out.push_str(&format!(
                                "    return {};\n",
                                emit_hir_expr(expr, &plain_ctx)
                            ));
                            continue;
                        }
                    }
                    out.push_str(&emit_hir_stmt(stmt, &plain_ctx, 2));
                }
                out.push_str("  })())");
                return out;
            }
            // Non-handler blocks: a plain void IIFE (no async promotion).
            let mut out = String::new();
            out.push_str("(() => {\n");
            for stmt in stmts {
                out.push_str(&emit_hir_stmt(stmt, &plain_ctx, 2));
            }
            out.push_str("  })()");
            out
        }
        HirExpr::Jsx(el) => {
            // VUV: resolve UI primitives + universal style kwargs into a single className expr.
            let view = transform_hir_view_kwargs(&el.tag, &el.attributes, ctx);
            let mut attrs = Vec::new();
            if let Some(class_expr) = &view.class_expr {
                attrs.push(format!("className={{{class_expr}}}"));
            }
            if let Some(style_props) = &view.style_expr {
                attrs.push(format!("style={{{{ {style_props} }}}}"));
            }
            for attr in &view.passthrough {
                if attr.name == "bind" {
                    let (value_str, onchange_str) = expand_bind_hir_attribute(&attr.value, ctx);
                    attrs.push(format!("value={{{value_str}}}"));
                    attrs.push(format!("onChange={{{onchange_str}}}"));
                    continue;
                }
                let name = map_jsx_attr_name(&attr.name);
                let val = emit_hir_expr_attr_value(&attr.value, ctx, name);
                attrs.push(format!("{name}={{{val}}}"));
            }
            let mut children = Vec::new();
            for child in &el.children {
                let c = emit_hir_expr(child, ctx);
                children.push(wrap_jsx_hir_child_expr(c));
            }
            format!(
                "<{} {}\n>\n  {}\n</{}>",
                view.html_tag,
                attrs.join(" "),
                children.join("\n  "),
                view.html_tag
            )
        }
        HirExpr::JsxSelfClosing(el) => {
            let view = transform_hir_view_kwargs(&el.tag, &el.attributes, ctx);
            let mut attrs = Vec::new();
            if let Some(class_expr) = &view.class_expr {
                attrs.push(format!("className={{{class_expr}}}"));
            }
            if let Some(style_props) = &view.style_expr {
                attrs.push(format!("style={{{{ {style_props} }}}}"));
            }
            for attr in &view.passthrough {
                if attr.name == "bind" {
                    let (value_str, onchange_str) = expand_bind_hir_attribute(&attr.value, ctx);
                    attrs.push(format!("value={{{value_str}}}"));
                    attrs.push(format!("onChange={{{onchange_str}}}"));
                    continue;
                }
                let name = map_jsx_attr_name(&attr.name);
                let val = emit_hir_expr_attr_value(&attr.value, ctx, name);
                attrs.push(format!("{name}={{{val}}}"));
            }
            format!("<{} {} />", view.html_tag, attrs.join(" "))
        }
        HirExpr::JsxFragment(children, _) => {
            let mut child_strs = Vec::new();
            for child in children {
                let c = emit_hir_expr(child, ctx);
                child_strs.push(wrap_jsx_hir_child_expr(c));
            }
            format!("<>\n  {}\n</>", child_strs.join("\n  "))
        }
        HirExpr::ObjectLit(fields, _) => {
            let pairs: Vec<String> = fields
                .iter()
                .map(|(k, v)| format!("{k}: {}", emit_hir_expr(v, ctx)))
                .collect();
            format!("{{ {} }}", pairs.join(", "))
        }
        HirExpr::ListLit(elems, _) | HirExpr::TupleLit(elems, _) => {
            let items: Vec<String> = elems.iter().map(|e| emit_hir_expr(e, ctx)).collect();
            format!("[{}]", items.join(", "))
        }
        HirExpr::Call(callee, args, _, _) => {
            let args_str: Vec<String> = args.iter().map(|a| emit_hir_expr(&a.value, ctx)).collect();
            // §1.A.2: if this call is to an @endpoint fn (async), emit `await call(...)`.
            // Also consult the builtin registry so Vox primitives like `str(x)` → `String(x)`.
            if let HirExpr::Ident(name, _) = callee.as_ref() {
                // Check builtin registry first (e.g. `str` → `String`, `len` → `__vox_len`).
                if let Some(lowering) = registry().lookup_function(name, args_str.len()) {
                    let call_expr = match lowering {
                        BuiltinLowering::FunctionRename(ts_name) => {
                            format!("{ts_name}({})", args_str.join(", "))
                        }
                        BuiltinLowering::Inline(s) => s.to_string(),
                        _ => {
                            let callee_str = map_vox_react_hook_callee(name).to_string();
                            format!("{callee_str}({})", args_str.join(", "))
                        }
                    };
                    // Even a builtin might be async (unlikely, but respect the context).
                    if ctx.async_fn_names.contains(name.as_str()) {
                        return format!("await {call_expr}");
                    }
                    return call_expr;
                }
                // Endpoint fns are exposed by `vox-client.ts` as taking a single
                // named-args object. Rewrite a positional call to that object
                // form so the call site matches the generated client signature.
                // Only when the fn has ≥1 param (zero-arg endpoints stay bare)
                // and the args aren't already a single object literal.
                if let Some(params) = ctx.endpoint_params.get(name) {
                    let already_object =
                        args.len() == 1 && matches!(args[0].value, HirExpr::ObjectLit(_, _));
                    if !params.is_empty() && !already_object {
                        let fields: Vec<String> = params
                            .iter()
                            .zip(args_str.iter())
                            .map(|(p, a)| format!("{p}: {a}"))
                            .collect();
                        let call_expr = format!("{name}({{ {} }})", fields.join(", "));
                        if ctx.async_fn_names.contains(name.as_str()) {
                            return format!("await {call_expr}");
                        }
                        return call_expr;
                    }
                }
                let callee_str = map_vox_react_hook_callee(name).to_string();
                let call_expr = format!("{callee_str}({})", args_str.join(", "));
                if ctx.async_fn_names.contains(name.as_str()) {
                    return format!("await {call_expr}");
                }
                call_expr
            } else {
                let callee_str = emit_hir_expr(callee, ctx);
                format!("{callee_str}({})", args_str.join(", "))
            }
        }
        HirExpr::MethodCall(obj, method, args, plan, _) => {
            // On-device local execution: a `db.<Table>.<op>(...)` call (carries a
            // db query plan) lowers to a `voxRuntime` journal call instead of the
            // host SQL plan. Gated on `local_exec_db` so web/component emit is
            // untouched.
            if ctx.local_exec_db {
                if let Some(p) = plan {
                    return emit_local_db_op(p, args, ctx);
                }
            }
            // Bug D: inline-replace `std.<ns>.<method>(...)` calls with their JS equivalents
            // so emitted component files don't reference an unresolved `std` global.
            if let Some(replacement) = lower_std_namespace_call(obj, method, args, ctx) {
                return replacement;
            }
            // `Speech.<method>` STT namespace. Mirrors the AST/web emit in
            // `codegen_ts/component.rs`: on-device microphone transcription
            // routes through the `mobile` binding (which the RN target imports
            // from `./mobile-utils`, web from Tauri); file-path transcription
            // is backend-only. Keeps a single source of truth for what `Speech`
            // means across both emit stacks.
            if let HirExpr::Ident(ns, _) = obj.as_ref() {
                if ns == "Speech" && method == "transcribe_microphone" && args.is_empty() {
                    // `mobile.transcribe_microphone(): Promise<string>` — await it in
                    // an async handler so a `match Speech.transcribe_microphone() {…}`
                    // discriminates on the resolved string, not the pending Promise.
                    if ctx.handler_await {
                        return "await mobile.transcribe_microphone()".to_string();
                    }
                    return "mobile.transcribe_microphone()".to_string();
                }
                if ns == "Speech" && method == "transcribe" {
                    let path_js = args
                        .first()
                        .map(|a| emit_hir_expr(&a.value, ctx))
                        .unwrap_or_else(|| "\"\"".to_string());
                    return format!(
                        "((path: string) => {{ throw new Error(\"Speech.transcribe is backend-only (Vox Oratio / Candle Whisper). Use a @server fn or POST /api/audio/transcribe; see examples/oratio/codexAudioTranscribe.ts.\"); }})({path_js} as string)"
                    );
                }
            }
            // §1.A.4: if the direct receiver is an async call (`fetch_user().trim()`),
            // emit_hir_expr will produce `await fetch_user()`. Wrap in parens so the chain
            // resolves the settled value: `(await fetch_user()).trim()`.
            // Use HIR-level structural analysis (same logic as handler detection) rather than
            // inspecting the emitted string — avoids fragile `starts_with("await ")` heuristic.
            let needs_parens = async_walker::expr_has_async_call(obj, ctx.async_fn_names);
            let raw_obj = emit_hir_expr(obj, ctx);
            let obj_str = if needs_parens {
                format!("({raw_obj})")
            } else {
                raw_obj
            };
            let args_str: Vec<String> = args.iter().map(|a| emit_hir_expr(&a.value, ctx)).collect();
            // Map Vox snake_case Str methods to JS String.prototype names where they differ.
            // char_at/index_of return Optional in Vox; JS returns "" / -1, so we wrap.
            // For other methods, consult the builtin registry (§A3: centralized lowering).
            let mut base = match method.as_str() {
                "char_at" => format!(
                    "((__i) => {{ const __c = ({}).charAt(__i); return __c === \"\" ? null : __c; }})({})",
                    obj_str,
                    args_str.first().map(String::as_str).unwrap_or("0")
                ),
                "index_of" => format!(
                    "((__n) => {{ const __i = ({}).indexOf(__n); return __i < 0 ? null : __i; }})({})",
                    obj_str,
                    args_str.first().map(String::as_str).unwrap_or("\"\"")
                ),
                _ => {
                    // Consult the builtin registry. We pass "" as the type hint when no type
                    // info is available; the registry falls back to a name-only scan.
                    match registry().lookup_method("", method, args_str.len()) {
                        Some(BuiltinLowering::Property(p)) => format!("{obj_str}.{p}"),
                        Some(BuiltinLowering::MethodRename(m)) => {
                            format!("{obj_str}.{m}({})", args_str.join(", "))
                        }
                        Some(BuiltinLowering::Inline(s)) => s.to_string(),
                        Some(BuiltinLowering::FunctionRename(f)) => {
                            format!("{f}({})", args_str.join(", "))
                        }
                        None => format!("{obj_str}.{method}({})", args_str.join(", ")),
                    }
                }
            };
            if let Some(p) = plan {
                if p.capabilities.requires_sync {
                    base.push_str(".sync()");
                }
                if let Some(mode) = p.capabilities.retrieval_mode {
                    let m = match mode {
                        vox_compiler::hir::HirDbRetrievalMode::Fts => "fts",
                        vox_compiler::hir::HirDbRetrievalMode::Vector => "vector",
                        vox_compiler::hir::HirDbRetrievalMode::Hybrid => "hybrid",
                    };
                    base.push_str(&format!(".using(\"{m}\")"));
                }
                if let Some(topic) = &p.capabilities.live_topic {
                    base.push_str(&format!(".live(\"{}\")", topic.replace('\"', "\\\"")));
                }
                if let Some(scope) = &p.capabilities.orchestration_scope {
                    base.push_str(&format!(".scope(\"{}\")", scope.replace('\"', "\\\"")));
                }
            }
            base
        }
        HirExpr::FieldAccess(obj, field, _) => {
            let obj_str = emit_hir_expr(obj, ctx);
            format!("{obj_str}.{field}")
        }
        HirExpr::If(cond, then_stmts, else_stmts, _) => {
            let c = emit_hir_expr(cond, ctx);

            // Fast path: single JSX expression in both branches → emit as inline ternary.
            // This avoids void IIFEs like `(() => { <Comp />; })()` which render nothing.
            if let Some(then_jsx) = extract_single_jsx_expr(then_stmts, ctx) {
                let else_jsx = else_stmts
                    .as_ref()
                    .and_then(|s| extract_single_jsx_expr(s, ctx))
                    .unwrap_or_else(|| "null".to_string());
                return format!("({c} ? {then_jsx} : {else_jsx})");
            }

            let mut then_out = String::new();
            for s in then_stmts {
                then_out.push_str(&emit_hir_stmt(s, ctx, 0));
            }
            let mut else_out = String::new();
            if let Some(estmts) = else_stmts {
                for s in estmts {
                    else_out.push_str(&emit_hir_stmt(s, ctx, 0));
                }
            }
            // An `if` used as a value emits each branch as an IIFE. When a branch
            // body contains `await` (a nested `@query`/`@mutation` in an async
            // handler, or an on-device db op under `local_exec_db`), that IIFE must
            // be `async` and awaited — otherwise `await` sits in a sync function.
            // Branches with no `await` stay byte-identical to the sync form.
            let arm_iife = |body: &str| {
                if body.contains("await ") {
                    format!("await (async () => {{ {body} }})()")
                } else {
                    format!("(() => {{ {body} }})()")
                }
            };
            format!(
                "(({c}) ? {} : {})",
                arm_iife(&then_out),
                arm_iife(&else_out)
            )
        }
        HirExpr::For(name, index, iterable, body, key_expr, _) => {
            let iter = emit_hir_expr(iterable, ctx);
            let mut b = emit_hir_expr(body, ctx);
            // Default index name when the user wrote `for x in arr` (no index binding).
            // The leading underscore signals "unused" by JS convention and avoids clashing
            // with a user-named `i` in an outer scope.
            let idx = index.as_deref().unwrap_or("_i");
            // Inject the `key` prop into the first JSX element in the body.
            if let Some(k) = key_expr {
                let key_str = emit_hir_expr(k, ctx);
                let key_attr = format!(" key={{{key_str}}}");
                b = inject_key_into_jsx(b, &key_attr);
            }
            // Add explicit type annotations on the callback params so tsc `strict` mode
            // does not raise "Parameter implicitly has an 'any' type" when the iterable is
            // typed `any` (e.g. a component prop with no further narrowing).
            format!("{iter}.map(({name}: any, {idx}: number) => ({b}))")
        }
        HirExpr::Lambda(params, _, body, _, _) => {
            let param_names: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
            // An event-handler lambda whose body calls an async client fn
            // (`@query`/`@mutation`) or an async mobile call must be emitted as an
            // `async` arrow so those calls can `await` — otherwise a
            // `match record_event(...) { Ok(..) => .. }` discriminates on the
            // pending Promise (hits `default`) and `let p = parse_voice(...); p.kind`
            // reads a field off a Promise. See `EmitCtx::handler_await`.
            if !ctx.handler_await && async_walker::expr_has_async_call(body, ctx.async_fn_names) {
                let hctx = ctx.to_handler();
                if let HirExpr::Block(stmts, _) = body.as_ref() {
                    // Emit the block's statements directly into the async arrow so
                    // top-level awaits are legal (no wrapping void IIFE).
                    let mut out = format!("(async ({}) => {{\n", param_names.join(", "));
                    for stmt in stmts {
                        out.push_str(&emit_hir_stmt(stmt, &hctx, 2));
                    }
                    out.push_str("  })");
                    return out;
                }
                let b = emit_hir_expr(body, &hctx);
                return format!("(async ({}) => ({}))", param_names.join(", "), b);
            }
            // Sync lambda: strip async context — the lambda has its own scope — but
            // keep `endpoint_params` so endpoint calls inside an event-handler
            // lambda are still rewritten to the named-object form `vox-client`
            // expects.
            let lambda_ctx = ctx.to_plain();
            let b = emit_hir_expr(body, &lambda_ctx);
            format!("(({}) => ({}))", param_names.join(", "), b)
        }
        HirExpr::Match(subject, arms, _) => {
            // ADT-shaped values (Result/Option/user-defined) carry a `_tag` discriminator
            // (see crates/vox-compiler/src/codegen_ts/adt.rs). Dispatch on `_val._tag` and
            // bind constructor fields by destructuring; fall back to wildcard / literal /
            // ident-bind cases for non-ADT subjects.
            // In on-device (local-exec) bodies a match arm can contain `await`
            // (a nested db op), so the dispatch arrow must be `async` and its
            // result awaited; otherwise the plain sync IIFE is emitted.
            let (match_afn, match_aw) = if ctx.local_exec_db || ctx.handler_await {
                ("async ", "await ")
            } else {
                ("", "")
            };
            let s = emit_hir_expr(subject, ctx);
            let all_constructor_or_wild = arms.iter().all(|a| {
                matches!(
                    &a.pattern,
                    HirPattern::Constructor(_, _, _) | HirPattern::Wildcard(_)
                )
            });
            if all_constructor_or_wild
                && arms
                    .iter()
                    .any(|a| matches!(&a.pattern, HirPattern::Constructor(_, _, _)))
            {
                let mut arms_out = Vec::new();
                let mut has_default = false;
                for arm in arms {
                    let body = emit_hir_expr(&arm.body, ctx);
                    match &arm.pattern {
                        HirPattern::Constructor(name, fields, _) => {
                            // Fields bind by name where the inner pattern is `Ident`; nested
                            // patterns (rare for Result/Option) fall back to wildcard names.
                            let binders: Vec<String> = fields
                                .iter()
                                .enumerate()
                                .map(|(i, p)| match p {
                                    HirPattern::Ident(n, _) => n.clone(),
                                    HirPattern::Wildcard(_) => format!("_f{i}"),
                                    _ => format!("_f{i}"),
                                })
                                .collect();
                            // Constructor field order in HIR matches the ADT decl, but ADT codegen
                            // uses named fields (e.g. `{ _tag: "Ok", value }`). For the built-in
                            // `Result`/`Option`, the single payload field is conventionally
                            // accessed positionally — synthesize `_p0/_p1/...` accessors that work
                            // both for built-ins (positional) and user ADTs (named).
                            let destructure = if binders.is_empty() {
                                String::new()
                            } else {
                                let parts: Vec<String> = binders
                                    .iter()
                                    .enumerate()
                                    .map(|(i, b)| format!("const {b} = (_val as any)._p{i} ?? (_val as any).value;"))
                                    .collect();
                                parts.join(" ")
                            };
                            arms_out.push(format!(
                                "case \"{name}\": {{ {destructure} return {body}; }}"
                            ));
                        }
                        HirPattern::Wildcard(_) => {
                            has_default = true;
                            arms_out.push(format!("default: return {body};"));
                        }
                        _ => unreachable!(),
                    }
                }
                if !has_default {
                    arms_out.push("default: return undefined;".to_string());
                }
                format!(
                    "({match_aw}({match_afn}(_val) => {{ switch((_val as any)._tag) {{ {} }} }})({s}))",
                    arms_out.join(" ")
                )
            } else {
                let mut arms_out = Vec::new();
                for arm in arms {
                    let body = emit_hir_expr(&arm.body, ctx);
                    match &arm.pattern {
                        HirPattern::Literal(_, _) => {
                            let pat = emit_hir_pattern(&arm.pattern);
                            arms_out.push(format!("case {pat}: return {body};"));
                        }
                        HirPattern::Wildcard(_) => {
                            arms_out.push(format!("default: return {body};"));
                        }
                        HirPattern::Ident(name, _) => {
                            arms_out.push(format!(
                                "default: {{ const {name} = _val; return {body}; }}"
                            ));
                        }
                        HirPattern::Tuple(_, _) | HirPattern::Constructor(_, _, _) => {
                            let pat = emit_hir_pattern(&arm.pattern);
                            arms_out.push(format!("case {pat}: return {body};"));
                        }
                    }
                }
                format!(
                    "({match_aw}({match_afn}(_val) => {{ switch(_val) {{ {} }} }})({s}))",
                    arms_out.join(" ")
                )
            }
        }
        HirExpr::Try(h) => {
            let target = emit_hir_expr(h.target.as_ref(), ctx);
            if ctx.local_exec_db {
                // Real `?` propagation for on-device endpoint bodies: `__voxTry`
                // returns the Ok payload, or throws a sentinel (caught at the fn
                // top) that early-returns the Error variant. The target is a
                // Result value (db ops emit `Ok(await …)`); `await` settles any
                // inner promise before unwrapping.
                format!("__voxTry(await {target})")
            } else {
                // Web/component path: no Result runtime; emit the unwrapped target
                // (unchanged legacy behavior).
                target
            }
        }
        HirExpr::DecimalLit(v, _) => compat::ts_string_literal(v),

        HirExpr::Spawn(target, _) => {
            let t = emit_hir_expr(target, ctx);
            format!("new {t}()")
        }
        HirExpr::With(base, _, _) => emit_hir_expr(base, ctx),
        HirExpr::Index(object, index, _) => {
            let obj_str = emit_hir_expr(object, ctx);
            let idx_str = emit_hir_expr(index, ctx);
            format!("{obj_str}[{idx_str}]")
        }
        HirExpr::AsyncView(v) => {
            let source_tsx = emit_hir_expr(&v.source, ctx);
            let fetching_tsx = v
                .fetching_arm
                .as_deref()
                .map(|e| emit_hir_expr(e, ctx))
                .unwrap_or_else(|| "null".to_string());
            let empty_tsx = v
                .empty_arm
                .as_deref()
                .map(|e| emit_hir_expr(e, ctx))
                .unwrap_or_else(|| "null".to_string());
            let error_binding = v.error_binding.as_deref().unwrap_or("_err");
            let error_tsx = v
                .error_arm
                .as_deref()
                .map(|e| emit_hir_expr(e, ctx))
                .unwrap_or_else(|| "null".to_string());
            let ok_binding = v.ok_binding.as_deref().unwrap_or("_data");
            let ok_tsx = v
                .ok_arm
                .as_deref()
                .map(|e| emit_hir_expr(e, ctx))
                .unwrap_or_else(|| "null".to_string());
            // Use `crate::web_ir::…` which resolves in both embedded (standalone=OFF,
            // production `vox build`) and standalone compile modes. The previous
            // `vox_codegen::web_ir::…` path only resolved in standalone, silently
            // falling back to bare `source_tsx` in production.
            crate::web_ir::async_state::emit_async_view_tsx(
                &source_tsx,
                &fetching_tsx,
                &empty_tsx,
                error_binding,
                &error_tsx,
                ok_binding,
                &ok_tsx,
            )
        }
        // WorkflowVersion is not representable as a TS expression in this emit path
        HirExpr::WorkflowVersion(_) => String::new(),
    }
}

/// **Phase:** compat-legacy (OP-0138).
#[must_use]
pub(crate) fn emit_hir_expr_attr_value(
    expr: &HirExpr,
    ctx: &EmitCtx<'_>,
    attr_name: &str,
) -> String {
    let is_event_handler = attr_name.starts_with("on")
        && attr_name.len() > 2
        && attr_name
            .chars()
            .nth(2)
            .map(|c| c.is_uppercase())
            .unwrap_or(false);
    if is_event_handler {
        // A handler written as a lambda — `on_click={fn() {…}}` lowers to a Block
        // wrapping a single Lambda statement (or, rarely, a bare Lambda). Emit that
        // lambda AS the handler: the shared Lambda lowering makes it `async () =>`
        // when its body awaits an `@query`/`@mutation`. Without this it would be
        // emitted as an expression statement inside an extra `() => { … }` wrapper
        // — defined but never invoked, so the handler silently did nothing.
        if let Some(lambda) = single_lambda_handler(expr) {
            return emit_hir_expr(lambda, ctx);
        }
        if let HirExpr::Block(stmts, _) = expr {
            // A handler written as a bare statement block — `on_click={ stmt; stmt }`.
            // Detect if any stmt (transitively, not across lambdas) calls an async
            // client fn; if so emit `async () => {` with the handler-await context so
            // those calls `await` and nested `match`/blocks dispatch async.
            let needs_async = !ctx.async_fn_names.is_empty()
                && stmts
                    .iter()
                    .any(|s| stmt_has_async_call(s, ctx.async_fn_names));
            let handler_ctx = if needs_async {
                ctx.to_handler()
            } else {
                ctx.to_plain()
            };
            let stmts_str = stmts
                .iter()
                .map(|s| emit_hir_stmt(s, &handler_ctx, 2))
                .collect::<String>();
            let async_kw = if needs_async { "async " } else { "" };
            return format!("{async_kw}() => {{\n{}}}", stmts_str);
        }
    }
    emit_hir_expr(expr, ctx)
}

/// If `expr` is a handler written as a lambda — either a bare `HirExpr::Lambda`
/// or a single-statement `Block` wrapping one (`on_click={fn() {…}}`) — return a
/// reference to that lambda so it can be emitted directly as the event handler.
fn single_lambda_handler(expr: &HirExpr) -> Option<&HirExpr> {
    match expr {
        HirExpr::Lambda(..) => Some(expr),
        HirExpr::Block(stmts, _) if stmts.len() == 1 => match &stmts[0] {
            HirStmt::Expr { expr: inner, .. } if matches!(inner, HirExpr::Lambda(..)) => {
                Some(inner)
            }
            _ => None,
        },
        _ => None,
    }
}

/// **Phase:** compat-legacy (OP-0138).
#[must_use]
/// React `useEffect` / cleanup callbacks must return `undefined | (() => void)`,
/// never a `Promise`. When an effect / `on mount:` body awaits an async endpoint
/// call, the emitted statements contain `await`, which is only legal inside an
/// `async` function. Wrap such a body in a fire-and-forget
/// `void (async () => { ... })()` so the awaits are legal while the surrounding
/// callback stays synchronous. Bodies with no `await` are returned unchanged.
///
/// Shared by the web (reactive) and RN component emitters so both wrap async
/// lifecycle bodies identically.
pub fn wrap_effect_body_if_async(stmts_str: &str, indent: usize) -> std::borrow::Cow<'_, str> {
    if stmts_str.contains("await ") {
        let pad = "  ".repeat(indent);
        std::borrow::Cow::Owned(format!(
            "{pad}void (async () => {{\n{stmts_str}{pad}}})();\n"
        ))
    } else {
        std::borrow::Cow::Borrowed(stmts_str)
    }
}

pub fn emit_block_stmts(expr: &HirExpr, ctx: &EmitCtx<'_>, indent: usize) -> String {
    match expr {
        HirExpr::Block(stmts, _) => stmts
            .iter()
            .map(|s| emit_hir_stmt(s, ctx, indent))
            .collect(),
        _ => {
            let e = emit_hir_expr(expr, ctx);
            let pad = "  ".repeat(indent);
            format!("{pad}{e};\n")
        }
    }
}

/// When `expr` is a call to a `vox-client` endpoint fn whose result is
/// *discarded* (`let _ = …`) or used as a bare statement, return the fn name.
/// Such a call is fire-and-forget: a bare/discarded Promise becomes an
/// *unhandled rejection* if it fails — e.g. an RN button handler whose
/// `record_event(...)` POSTs to a server that isn't on the device. We wrap
/// those in `.catch(...)` so a failed background call logs instead of crashing
/// the app / spamming the dev log.
///
/// This applies even when the endpoint is async: in an `async` event handler
/// (per the handler-await lowering) an awaited-then-discarded call still leaks
/// its rejection past the handler. Since the result is unused, fire-and-forget
/// with `.catch` is both correct and safe. (Calls whose result is *used* are
/// awaited normally — they never reach this helper.)
fn floating_endpoint_call_name<'a>(expr: &'a HirExpr, ctx: &EmitCtx<'_>) -> Option<&'a str> {
    if let HirExpr::Call(callee, _, _, _) = expr {
        if let HirExpr::Ident(name, _) = callee.as_ref() {
            if ctx.endpoint_params.contains_key(name) {
                return Some(name.as_str());
            }
        }
    }
    None
}

/// Like `floating_endpoint_call_name`, but for the explicit DISCARD form
/// (`let _ = record_event(...)`). A discarded endpoint call is *always* a
/// fire-and-forget — the binding is thrown away, so there is nothing to await.
/// Unlike the bare-statement case, this must fire even inside a `handler_await`
/// context (where the name lives in `async_fn_names`): awaiting a discarded
/// promise only serves to let a rejection escape as an unhandled rejection in
/// the async handler, which is the bug this guards against.
fn discarded_endpoint_call_name<'a>(expr: &'a HirExpr, ctx: &EmitCtx<'_>) -> Option<&'a str> {
    if let HirExpr::Call(callee, _, _, _) = expr {
        if let HirExpr::Ident(name, _) = callee.as_ref() {
            if ctx.endpoint_params.contains_key(name) {
                return Some(name.as_str());
            }
        }
    }
    None
}

/// Render a fire-and-forget endpoint call with an attached `.catch` so it can
/// never surface as an unhandled promise rejection.
fn emit_floating_endpoint_call(expr: &HirExpr, ctx: &EmitCtx<'_>, name: &str, pad: &str) -> String {
    // Render the call WITHOUT `await` even in a handler context: the discard
    // path can reach here with `name` in `async_fn_names`, but a fire-and-forget
    // promise must stay un-awaited so its rejection is caught by `.catch`, not
    // surfaced as an unhandled rejection. Shadow the async set with one that
    // omits this name so `emit_hir_expr` emits the bare promise.
    let async_without_name: HashSet<String> = ctx
        .async_fn_names
        .iter()
        .filter(|n| n.as_str() != name)
        .cloned()
        .collect();
    let no_await_ctx = EmitCtx {
        async_fn_names: &async_without_name,
        ..ctx.clone()
    };
    let call = emit_hir_expr(expr, &no_await_ctx);
    format!(
        "{pad}void Promise.resolve({call}).catch((__e) => {{ console.error(\"[vox] endpoint '{name}' failed (fire-and-forget):\", __e); }});\n"
    )
}

/// Lower a `db.<Table>.<op>(...)` call to a `voxRuntime` journal call for the
/// on-device local-execution path. Insert/All/Count are wired to the append-only
/// seam; richer ops (Get/Filter/Delete/raw) are deferred — endpoints that use
/// them are classified non-local-executable (see `is_endpoint_locally_executable`)
/// and never reach this with those ops, but we emit an explicit throw as a
/// belt-and-suspenders so output is never silently wrong.
fn emit_local_db_op(plan: &HirDbQueryPlan, args: &[HirArg], ctx: &EmitCtx<'_>) -> String {
    let table = &plan.table;
    let fname = ctx.current_fn;
    match plan.op {
        HirDbTableOp::Insert => {
            let row = args
                .first()
                .map(|a| emit_hir_expr(&a.value, ctx))
                .unwrap_or_else(|| "{}".to_string());
            format!("Ok(await voxRuntime.recordMutation(\"{fname}\", \"{table}\", {row}))")
        }
        // Cast the journal rows to the table's row type so endpoint bodies and
        // helpers field-access them with full type-safety. vox-client.ts emits a
        // matching `type {table} = {...}` alias for every table reached here.
        HirDbTableOp::All => {
            format!("Ok((await voxRuntime.replayTable(\"{table}\")) as {table}[])")
        }
        HirDbTableOp::Count => {
            format!("Ok(__vox_len(await voxRuntime.replayTable(\"{table}\")))")
        }
        HirDbTableOp::Get
        | HirDbTableOp::Delete
        | HirDbTableOp::FilterRecord
        | HirDbTableOp::UnsafeQueryRawClause => format!(
            "(() => {{ throw new VoxRuntimeError(\"UnsupportedOnPlatform\", \"on-device db op not yet supported for table '{table}'\"); }})()"
        ),
    }
}

/// **Phase:** compat-legacy (OP-0138).
#[must_use]
pub(crate) fn emit_hir_stmt(stmt: &HirStmt, ctx: &EmitCtx<'_>, indent: usize) -> String {
    let pad = "  ".repeat(indent);
    match stmt {
        HirStmt::Let {
            pattern,
            value,
            mutable,
            ..
        } => {
            // `let _ = record_event(...)` is the idiomatic "fire and forget a
            // mutation" form. When the value is an un-awaited endpoint call and
            // the binding is discarded, emit the catch-guarded form instead of a
            // dead `const _ = <promise>`.
            let is_discard = matches!(pattern, HirPattern::Wildcard(_))
                || matches!(pattern, HirPattern::Ident(n, _) if n == "_");
            if is_discard {
                if let Some(name) = discarded_endpoint_call_name(value, ctx) {
                    return emit_floating_endpoint_call(value, ctx, name, &pad);
                }
            }
            let keyword = if *mutable { "let" } else { "const" };
            let pat = emit_hir_pattern(pattern);
            let val = emit_hir_expr(value, ctx);
            format!("{pad}{keyword} {pat} = {val};\n")
        }
        HirStmt::Assign { target, value, .. } => {
            if let HirExpr::Ident(name, _) = target {
                if ctx.state_names.contains(name) {
                    let val = emit_hir_expr(value, ctx);
                    return format!("{pad}set_{name}({val});\n");
                }
            }
            format!(
                "{pad}{} = {};\n",
                emit_hir_expr(target, ctx),
                emit_hir_expr(value, ctx)
            )
        }
        HirStmt::Expr { expr, .. } => {
            // A bare `record_event(...)` statement is a fire-and-forget call;
            // guard it so a failed background request can't become an unhandled
            // rejection.
            if let Some(name) = floating_endpoint_call_name(expr, ctx) {
                return emit_floating_endpoint_call(expr, ctx, name, &pad);
            }
            format!("{pad}{};\n", emit_hir_expr(expr, ctx))
        }
        HirStmt::Return { value, .. } => {
            if let Some(v) = value {
                format!("{pad}return {};\n", emit_hir_expr(v, ctx))
            } else {
                format!("{pad}return;\n")
            }
        }
        HirStmt::While {
            condition, body, ..
        } => {
            let cond = emit_hir_expr(condition, ctx);
            let mut out = format!("{pad}while ({cond}) {{\n");
            for s in body {
                out.push_str(&emit_hir_stmt(s, ctx, indent + 2));
            }
            out.push_str(&format!("{pad}}}\n"));
            out
        }
        HirStmt::Loop { body, .. } => {
            let mut out = format!("{pad}while (true) {{\n");
            for s in body {
                out.push_str(&emit_hir_stmt(s, ctx, indent + 2));
            }
            out.push_str(&format!("{pad}}}\n"));
            out
        }
        HirStmt::Break { .. } => format!("{pad}break;\n"),
        HirStmt::Continue { .. } => format!("{pad}continue;\n"),
    }
}

/// **Phase:** compat-legacy (OP-0138).
#[must_use]
pub(crate) fn emit_hir_pattern(pattern: &HirPattern) -> String {
    match pattern {
        HirPattern::Ident(name, _) => name.clone(),
        HirPattern::Tuple(elems, _) => {
            let s: Vec<String> = elems.iter().map(emit_hir_pattern).collect();
            format!("[{}]", s.join(", "))
        }
        HirPattern::Literal(lit, _) => match lit.as_ref() {
            HirExpr::IntLit(v, _) => v.to_string(),
            HirExpr::FloatLit(v, _) => v.to_string(),
            HirExpr::StringLit(s, _) => compat::ts_string_literal(s),
            HirExpr::BoolLit(b, _) => b.to_string(),
            _ => "_".to_string(),
        },
        HirPattern::Wildcard(_) => "_".to_string(),
        _ => "_".to_string(),
    }
}

/// Emit a mobile native bridge function through Tauri `invoke` (Rust commands on the native side).
///
/// **Phase:** mobile-integration (OP-M042).
#[must_use]
pub fn emit_mobile_bridge_fn(f: &HirFn) -> String {
    let mut out = String::new();
    let name = &f.name;
    let params: Vec<String> = f
        .params
        .iter()
        .map(|p| {
            let ty = p
                .type_ann
                .as_ref()
                .map_or("any".to_string(), map_hir_type_to_ts);
            format!("{}: {}", p.name, ty)
        })
        .collect();
    let ret_ty = f
        .return_type
        .as_ref()
        .map_or("Promise<void>".to_string(), |ty| {
            format!("Promise<{}>", map_hir_type_to_ts(ty))
        });

    out.push_str(&format!(
        "export async function {name}({}): {ret_ty} {{\n",
        params.join(", ")
    ));
    let args_obj: Vec<String> = f
        .params
        .iter()
        .map(|p| format!("{}: {}", p.name, p.name))
        .collect();
    out.push_str(&format!(
        "  return await invoke('vox_mobile_bridge', {{ fn: '{name}', args: {{ {} }} }});\n",
        args_obj.join(", ")
    ));
    out.push_str("}\n");
    out
}
/// Emit the `std.mobile` Web API implementation.
///
/// Provides Tier-1 browser-native implementations of all `mobile.*` methods.
/// Tier-2 (Tauri mobile + `invoke` commands) is used when `target` is `android` / `ios` / `native`.
#[must_use]
pub fn emit_mobile_web_api_utils(target: Option<&str>) -> String {
    let mut is_native = false;
    if let Some(t) = target {
        if t == "ios" || t == "android" || t == "native" {
            is_native = true;
        }
    }

    if is_native {
        return r#"// std.mobile — Tauri native implementation (invoke + event bridge)
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

async function __mi<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return invoke<T>(cmd, args ?? {});
}

export const mobile = {
  async take_photo(): Promise<{ Ok?: string; Error?: string }> {
    try {
      const image = await __mi<{ dataUrl: string }>('mobile_camera_capture', { quality: 90, source: 'camera' });
      return { Ok: image.dataUrl };
    } catch (e: any) { return { Error: e?.message ?? "Camera failed" }; }
  },
  async take_photo_from_gallery(): Promise<{ Ok?: string; Error?: string }> {
    try {
      const image = await __mi<{ dataUrl: string }>('mobile_camera_capture', { quality: 90, source: 'gallery' });
      return { Ok: image.dataUrl };
    } catch (e: any) { return { Error: e?.message ?? "Gallery failed" }; }
  },
  notify(title: string, body: string): void {
    void __mi('mobile_notify', { title, body }).catch(() => console.log("Notify", title, body));
  },
  vibrate(duration_ms: number = 200): void {
    void __mi('mobile_haptics_vibrate', { duration_ms });
  },
  async get_location(): Promise<{ Ok?: string; Error?: string }> {
    try {
      const pos = await __mi<{ lat: number; lng: number; accuracy?: number }>('mobile_geolocation_current');
      return { Ok: JSON.stringify({ lat: pos.lat, lng: pos.lng, accuracy: pos.accuracy ?? 0 }) };
    } catch (e: any) { return { Error: e?.message ?? "Geolocation failed" }; }
  },
  async accelerometer(): Promise<{ Ok?: string; Error?: string }> {
    try {
      const v = await __mi<string>('mobile_accelerometer_sample');
      return { Ok: v };
    } catch (e: any) { return { Error: e?.message ?? "Accelerometer unavailable" }; }
  },
  platform(): string {
    return typeof (globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ !== "undefined" ? "tauri" : "web";
  },
  has_camera(): boolean { return true; },
  copy_to_clipboard(text: string): void { void __mi('mobile_clipboard_write', { text }); },
  async read_clipboard(): Promise<{ Ok?: string; Error?: string }> {
    try {
      const value = await __mi<string>('mobile_clipboard_read');
      return { Ok: value };
    } catch (e: any) { return { Error: e?.message ?? "Clipboard failed" }; }
  },
  useWaitUntilSync(): boolean { return false; },
  async biometric_auth(prompt: string): Promise<{ Ok?: boolean; Error?: string }> {
    try { const ok = await __mi<boolean>('mobile_biometric_auth', { prompt }); return { Ok: ok }; }
    catch (e: any) { return { Error: e?.message ?? "Biometrics failed" }; }
  },
  async read_contacts(): Promise<{ Ok?: string; Error?: string }> {
    try { const v = await __mi<string>('mobile_read_contacts'); return { Ok: v }; }
    catch (e: any) { return { Error: e?.message ?? "Contacts unavailable" }; }
  },
  async share_text(text: string): Promise<{ Ok?: boolean; Error?: string }> {
    try { await __mi('mobile_share_text', { text }); return { Ok: true }; }
    catch (e: any) { return { Error: e?.message ?? "Share failed" }; }
  },
  async store_file(name: string, base64: string): Promise<{ Ok?: boolean; Error?: string }> {
    try { await __mi('mobile_store_file', { name, base64 }); return { Ok: true }; }
    catch (e: any) { return { Error: e?.message ?? "Store failed" }; }
  },
  async read_file(name: string): Promise<{ Ok?: string; Error?: string }> {
    try { const v = await __mi<string>('mobile_read_file', { name }); return { Ok: v }; }
    catch (e: any) { return { Error: e?.message ?? "Read failed" }; }
  },
  push: {
    async register(): Promise<{ Ok?: string; Error?: string }> {
      try {
        const token = await __mi<string>('mobile_push_register');
        return { Ok: token };
      } catch (e: any) { return { Error: String(e) }; }
    },
    on_message(fn: (msg: string) => void): void {
      void listen<string>('vox-push-notification', (e) => { fn(e.payload); });
      void listen<string>('vox-push-action', (e) => { fn(e.payload); });
    }
  }
};
"#.to_string();
    }

    r#"// std.mobile — Web API implementation generated by Vox compiler
// Works on desktop browsers and mobile browsers (iOS Safari, Android Chrome).
// For app-store distribution, use `vox compile --target mobile-android|mobile-ios` (Tauri).

export const mobile = {
  async take_photo(): Promise<{ Ok?: string; Error?: string }> {
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ video: { facingMode: "environment" } });
      const video = document.createElement("video");
      video.srcObject = stream;
      await video.play();
      const canvas = document.createElement("canvas");
      canvas.width = video.videoWidth;
      canvas.height = video.videoHeight;
      canvas.getContext("2d")!.drawImage(video, 0, 0);
      stream.getTracks().forEach(t => t.stop());
      return { Ok: canvas.toDataURL("image/jpeg") };
    } catch (e: any) {
      return { Error: e?.message ?? "Camera unavailable" };
    }
  },

  async take_photo_from_gallery(): Promise<{ Ok?: string; Error?: string }> {
    return new Promise(resolve => {
      const input = document.createElement("input");
      input.type = "file";
      input.accept = "image/*";
      input.onchange = () => {
        const file = input.files?.[0];
        if (!file) return resolve({ Error: "No file selected" });
        const reader = new FileReader();
        reader.onload = () => resolve({ Ok: reader.result as string });
        reader.onerror = () => resolve({ Error: "Read error" });
        reader.readAsDataURL(file);
      };
      input.click();
    });
  },

  notify(title: string, body: string): void {
    if ("Notification" in window && Notification.permission === "granted") {
      new Notification(title, { body });
    } else if ("Notification" in window && Notification.permission !== "denied") {
      Notification.requestPermission().then(p => {
        if (p === "granted") new Notification(title, { body });
      });
    }
  },

  vibrate(duration_ms: number = 200): void {
    if ("vibrate" in navigator) navigator.vibrate(duration_ms);
  },

  async get_location(): Promise<{ Ok?: string; Error?: string }> {
    return new Promise(resolve => {
      if (!("geolocation" in navigator)) return resolve({ Error: "Geolocation unavailable" });
      navigator.geolocation.getCurrentPosition(
        pos => resolve({ Ok: JSON.stringify({ lat: pos.coords.latitude, lng: pos.coords.longitude, accuracy: pos.coords.accuracy }) }),
        err => resolve({ Error: err.message })
      );
    });
  },

  async accelerometer(): Promise<{ Ok?: string; Error?: string }> {
    return new Promise((resolve, reject) => {
      const handler = (e: DeviceMotionEvent) => {
        window.removeEventListener("devicemotion", handler);
        const a = e.accelerationIncludingGravity;
        resolve({ Ok: JSON.stringify({ x: a?.x ?? 0, y: a?.y ?? 0, z: a?.z ?? 0 }) });
      };
      window.addEventListener("devicemotion", handler, { once: true });
      setTimeout(() => resolve({ Error: "Timeout" }), 2000);
    });
  },

  platform(): string {
    const ua = navigator.userAgent;
    if (/android/i.test(ua)) return "android";
    if (/iphone|ipad|ipod/i.test(ua)) return "ios";
    if (typeof (window as any).__TAURI__ !== "undefined") return "desktop";
    return "web";
  },

  has_camera(): boolean {
    return !!(navigator.mediaDevices && navigator.mediaDevices.getUserMedia);
  },

  copy_to_clipboard(text: string): void {
    navigator.clipboard?.writeText(text);
  },

  async read_clipboard(): Promise<{ Ok?: string; Error?: string }> {
    try {
      const t = await navigator.clipboard.readText();
      return { Ok: t };
    } catch (e: any) {
      return { Error: e?.message ?? "Clipboard unavailable" };
    }
  },

  useWaitUntilSync(): boolean {
    if (typeof window !== "undefined" && (window as any).React) {
        const [syncing, setSyncing] = (window as any).React.useState(false);
        (window as any).React.useEffect(() => {
            if ("serviceWorker" in navigator) {
                // Future integration with Workbox-Window events
                const handleOffline = () => setSyncing(true);
                const handleOnline = () => setSyncing(false);
                window.addEventListener("offline", handleOffline);
                window.addEventListener("online", handleOnline);
                setSyncing(!navigator.onLine);
                return () => {
                    window.removeEventListener("offline", handleOffline);
                    window.removeEventListener("online", handleOnline);
                };
            }
        }, []);
        return syncing;
    }
    return false;
  },

  async biometric_auth(prompt: string): Promise<{ Ok?: boolean; Error?: string }> {
    if (!window.PublicKeyCredential) return { Error: "WebAuthn not supported" };
    try {
      const challenge = new Uint8Array(32);
      crypto.getRandomValues(challenge);
      await navigator.credentials.get({
        publicKey: { challenge, userVerification: "required" }
      });
      return { Ok: true };
    } catch (e: any) {
      return { Error: e?.message ?? "Biometric auth failed" };
    }
  },

  async read_contacts(): Promise<{ Ok?: string; Error?: string }> {
    if (!("contacts" in navigator && "ContactsManager" in window)) {
      return { Error: "Contacts API not supported" };
    }
    try {
      const props = ["name", "email", "tel"];
      const opts = { multiple: true };
      const contacts = await (navigator as any).contacts.select(props, opts);
      return { Ok: JSON.stringify(contacts) };
    } catch (e: any) {
      return { Error: e?.message ?? "Failed to read contacts" };
    }
  },

  async share_text(text: string): Promise<{ Ok?: boolean; Error?: string }> {
    if (!navigator.share) return { Error: "Web Share API not supported" };
    try {
      await navigator.share({ text });
      return { Ok: true };
    } catch (e: any) {
      return { Error: e?.message ?? "Failed to share" };
    }
  },

  async store_file(name: string, base64: string): Promise<{ Ok?: boolean; Error?: string }> {
    try {
      // Very simple local persistence fallback via generic web API. For real mobile files, use the Tauri FS plugin.
      localStorage.setItem(`vox-file-${name}`, base64);
      return { Ok: true };
    } catch (e: any) {
      return { Error: e?.message ?? "Failed to store file" };
    }
  },

  async read_file(name: string): Promise<{ Ok?: string; Error?: string }> {
    try {
      const val = localStorage.getItem(`vox-file-${name}`);
      if (val !== null) return { Ok: val };
      return { Error: "File not found" };
    } catch (e: any) {
      return { Error: e?.message ?? "Failed to read file" };
    }
  },
  
  push: {
    async register(): Promise<{ Ok?: string; Error?: string }> { return { Error: "Push APIs require physical device or Service Worker implementation" }; },
    on_message(fn: (msg: string) => void): void { }
  }
};
"#.to_string()
}

#[cfg(test)]
mod hir_emit_if_tests {
    use super::*;

    fn span() -> vox_compiler::ast::span::Span {
        vox_compiler::ast::span::Span { start: 0, end: 0 }
    }

    fn jsx_self_closing(name: &str) -> HirExpr {
        HirExpr::JsxSelfClosing(HirJsxSelfClosing {
            tag: name.to_string(),
            attributes: vec![],
            span: span(),
        })
    }

    fn expr_stmt(expr: HirExpr) -> HirStmt {
        HirStmt::Expr { expr, span: span() }
    }

    #[test]
    fn if_with_jsx_branches_emits_ternary_not_iife() {
        let cond = HirExpr::BoolLit(true, span());
        let then_stmts = vec![expr_stmt(jsx_self_closing("SpeakTab"))];
        let else_stmts = vec![expr_stmt(jsx_self_closing("CommandTab"))];

        let if_expr = HirExpr::If(Box::new(cond), then_stmts, Some(else_stmts), span());

        let out = emit_hir_expr(&if_expr, &EmitCtx::new(&HashSet::new()));

        assert!(
            out.contains("? <SpeakTab") || out.contains("?<SpeakTab"),
            "expected ternary but got: {out}"
        );
        assert!(
            !out.contains("(() => {"),
            "void IIFE should not appear for single-JSX branches, but got: {out}"
        );
    }

    #[test]
    fn if_with_nested_jsx_if_emits_nested_ternary() {
        let inner_cond = HirExpr::BoolLit(false, span());
        let inner_then = vec![expr_stmt(jsx_self_closing("NetworkTab"))];
        let inner_else = vec![expr_stmt(jsx_self_closing("ForgeTab"))];
        let nested_if = HirExpr::If(Box::new(inner_cond), inner_then, Some(inner_else), span());

        let outer_cond = HirExpr::BoolLit(true, span());
        let outer_then = vec![expr_stmt(jsx_self_closing("SpeakTab"))];
        let outer_else = vec![expr_stmt(nested_if)];
        let outer_if = HirExpr::If(Box::new(outer_cond), outer_then, Some(outer_else), span());

        let out = emit_hir_expr(&outer_if, &EmitCtx::new(&HashSet::new()));

        assert!(
            out.contains("<SpeakTab") && out.contains("<NetworkTab") && out.contains("<ForgeTab"),
            "all three branches should appear: {out}"
        );
        assert!(
            !out.contains("(() => {"),
            "no void IIFEs in nested ternary: {out}"
        );
    }
}

#[cfg(test)]
mod async_emit_tests {
    use super::*;
    use vox_compiler::ast::span::Span;
    use vox_compiler::hir::{HirArg, HirExpr, HirMatchArm, HirPattern, HirStmt};

    fn span() -> Span {
        Span { start: 0, end: 0 }
    }

    fn ident(name: &str) -> HirExpr {
        HirExpr::Ident(name.to_string(), span())
    }

    fn call(name: &str, args: Vec<HirExpr>) -> HirExpr {
        HirExpr::Call(
            Box::new(ident(name)),
            args.into_iter()
                .map(|v| HirArg {
                    name: None,
                    value: v,
                })
                .collect(),
            false,
            span(),
        )
    }

    fn string_lit(s: &str) -> HirExpr {
        HirExpr::StringLit(s.to_string(), span())
    }

    fn let_stmt(var: &str, value: HirExpr) -> HirStmt {
        HirStmt::Let {
            pattern: HirPattern::Ident(var.to_string(), span()),
            type_ann: None,
            value,
            mutable: false,
            span: span(),
        }
    }

    fn expr_stmt(expr: HirExpr) -> HirStmt {
        HirStmt::Expr { expr, span: span() }
    }

    fn assign_stmt(target: HirExpr, value: HirExpr) -> HirStmt {
        HirStmt::Assign {
            target,
            value,
            span: span(),
        }
    }

    fn async_names(names: &[&str]) -> HashSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    fn handler_block(stmts: Vec<HirStmt>) -> HirExpr {
        HirExpr::Block(stmts, span())
    }

    // Test 1: endpoint called in a let binding inside a handler gets `await`
    #[test]
    fn endpoint_called_in_let_binding_inside_handler_gets_await() {
        let state_names: HashSet<String> = HashSet::new();
        let async_fns = async_names(&["parse_voice"]);
        let ctx = EmitCtx::with_async(&state_names, &async_fns);

        // `let p = parse_voice("hi")`
        let stmt = let_stmt("p", call("parse_voice", vec![string_lit("hi")]));
        let block = handler_block(vec![stmt]);

        let out = emit_hir_expr_attr_value(&block, &ctx, "onClick");
        assert!(
            out.contains("await parse_voice"),
            "expected `await parse_voice` in let binding, got: {out}"
        );
        assert!(
            out.starts_with("async "),
            "handler should be async, got: {out}"
        );
    }

    // Test 2: endpoint called as method chain receiver gets `(await …)`
    #[test]
    fn endpoint_called_as_method_chain_gets_await() {
        let state_names: HashSet<String> = HashSet::new();
        let async_fns = async_names(&["fetch_user"]);
        let ctx = EmitCtx::with_async(&state_names, &async_fns);

        // `fetch_user().trim()` as expression stmt
        let method_call = HirExpr::MethodCall(
            Box::new(call("fetch_user", vec![])),
            "trim".to_string(),
            vec![],
            None,
            span(),
        );
        let stmt = expr_stmt(method_call);
        let block = handler_block(vec![stmt]);

        let out = emit_hir_expr_attr_value(&block, &ctx, "onClick");
        assert!(
            out.contains("(await fetch_user())"),
            "expected `(await fetch_user()).trim()`, got: {out}"
        );
        assert!(
            out.starts_with("async "),
            "handler should be async, got: {out}"
        );
    }

    // Test 3: endpoint nested in assignment gets `await`
    #[test]
    fn nested_endpoint_in_assignment_gets_await() {
        let state_names: HashSet<String> = HashSet::new();
        let async_fns = async_names(&["save"]);
        let ctx = EmitCtx::with_async(&state_names, &async_fns);

        // `result = save("data")`
        let stmt = assign_stmt(ident("result"), call("save", vec![string_lit("data")]));
        let block = handler_block(vec![stmt]);

        let out = emit_hir_expr_attr_value(&block, &ctx, "onClick");
        assert!(
            out.contains("await save("),
            "expected `await save(` in assignment, got: {out}"
        );
        assert!(
            out.starts_with("async "),
            "handler should be async, got: {out}"
        );
    }

    // Test 4: pure sync handler must NOT get `async`
    #[test]
    fn handler_with_no_async_call_is_not_async() {
        let state_names: HashSet<String> = HashSet::new();
        let async_fns = async_names(&["fetch_user", "save"]);
        let ctx = EmitCtx::with_async(&state_names, &async_fns);

        // `console.log("clicked")` — no async calls
        let stmt = expr_stmt(HirExpr::MethodCall(
            Box::new(ident("console")),
            "log".to_string(),
            vec![HirArg {
                name: None,
                value: string_lit("clicked"),
            }],
            None,
            span(),
        ));
        let block = handler_block(vec![stmt]);

        let out = emit_hir_expr_attr_value(&block, &ctx, "onClick");
        assert!(
            !out.starts_with("async "),
            "pure sync handler must not be async, got: {out}"
        );
        assert!(
            !out.contains("await "),
            "no await in sync handler, got: {out}"
        );
    }

    fn lambda(body_stmts: Vec<HirStmt>) -> HirExpr {
        HirExpr::Lambda(
            vec![],
            None,
            Box::new(handler_block(body_stmts)),
            false,
            span(),
        )
    }

    // Test 5: a handler written as a lambda — `on_click={fn() { … }}` lowers to a
    // Block wrapping the Lambda. It must be emitted AS the handler (a single async
    // arrow that awaits), NOT as a discarded expression statement inside an extra
    // sync `() => {}` wrapper (the old bug that left voice handlers inert).
    #[test]
    fn lambda_wrapped_handler_emits_single_async_arrow() {
        let state_names: HashSet<String> = HashSet::new();
        let async_fns = async_names(&["parse_voice"]);
        let ctx = EmitCtx::with_async(&state_names, &async_fns);

        // `{ fn() { let p = parse_voice("hi") } }`
        let inner = lambda(vec![let_stmt(
            "p",
            call("parse_voice", vec![string_lit("hi")]),
        )]);
        let block = handler_block(vec![expr_stmt(inner)]);

        let out = emit_hir_expr_attr_value(&block, &ctx, "onClick");
        assert!(
            out.starts_with("(async ()"),
            "lambda handler must emit a single async arrow, got: {out}"
        );
        assert!(
            out.contains("await parse_voice"),
            "awaited call expected, got: {out}"
        );
        // No sync outer wrapper that would discard the lambda unexecuted.
        assert!(
            !out.trim_start().starts_with("() => {"),
            "lambda must not be double-wrapped + discarded, got: {out}"
        );
    }

    // Test 6: a `match` whose scrutinee is an async call, inside a lambda handler,
    // awaits the scrutinee — so it discriminates on the resolved value, not the
    // pending Promise (the chained-Save data-loss bug).
    #[test]
    fn match_scrutinee_async_call_in_lambda_handler_is_awaited() {
        let state_names: HashSet<String> = HashSet::new();
        let async_fns = async_names(&["save_event"]);
        let ctx = EmitCtx::with_async(&state_names, &async_fns);

        // `{ fn() { match save_event() { _ => {} } } }`
        let arm = HirMatchArm {
            pattern: HirPattern::Wildcard(span()),
            guard: None,
            body: Box::new(handler_block(vec![])),
            span: span(),
        };
        let m = HirExpr::Match(Box::new(call("save_event", vec![])), vec![arm], span());
        let block = handler_block(vec![expr_stmt(lambda(vec![expr_stmt(m)]))]);

        let out = emit_hir_expr_attr_value(&block, &ctx, "onClick");
        assert!(
            out.contains("await save_event"),
            "match scrutinee must be awaited, got: {out}"
        );
        assert!(
            out.starts_with("(async ()"),
            "handler must be async, got: {out}"
        );
    }
}

// ── VUV view-call lowering at HIR emit time ─────────────────────────────────
//
// The legacy reactive emit path (used when web_ir bridge falls back to parity-mismatch) sends
// HirExpr::Jsx through emit_hir_expr. Without primitive resolution here, view-call kwargs leak
// as raw JSX attributes (`<row pad_x={4}>`) instead of Tailwind classes. This module mirrors
// `web_ir::primitives::apply_primitive_emission` for HIR.

pub(crate) struct ViewCallHir {
    pub(crate) html_tag: String,
    pub(crate) class_expr: Option<String>,
    /// Inline React `style` object expression string (without outer `{{ }}`), e.g.
    /// `"paddingTop: 'env(safe-area-inset-top)'"`. Present when `safe_area` kwarg is set.
    pub(crate) style_expr: Option<String>,
    pub(crate) passthrough: Vec<HirJsxAttr>,
}

const HIR_PRIMITIVE_CONSUMED_PROPS: &[&str] = &[
    "size", "weight", "align", "wrap", "scroll", "bleed", "variant", "level", "surface", "z",
];

pub(crate) fn transform_hir_view_kwargs(
    tag: &str,
    attrs: &[HirJsxAttr],
    ctx: &EmitCtx<'_>,
) -> ViewCallHir {
    // Collect static-literal per-primitive kwargs (size/weight/align/wrap/variant/level/surface)
    // so primitives::resolve can apply their per-primitive logic (e.g. size="xs" → text-xs).
    // Dynamic per-primitive kwargs (rare) are dropped — they'd require a runtime helper.
    let mut static_per_primitive: Vec<(String, String)> = Vec::new();
    for attr in attrs {
        if HIR_PRIMITIVE_CONSUMED_PROPS.contains(&attr.name.as_str()) {
            if let HirExpr::StringLit(v, _) = unwrap_inline_hir_block_expr(&attr.value) {
                static_per_primitive.push((attr.name.clone(), v.clone()));
            } else if let HirExpr::BoolLit(v, _) = unwrap_inline_hir_block_expr(&attr.value) {
                static_per_primitive.push((attr.name.clone(), v.to_string()));
            } else if let HirExpr::IntLit(v, _) = unwrap_inline_hir_block_expr(&attr.value) {
                static_per_primitive.push((attr.name.clone(), v.to_string()));
            }
        }
    }
    let primitive_emission = super::web_ir::primitives::resolve(tag, &static_per_primitive);
    let html_tag = primitive_emission
        .as_ref()
        .map(|e| e.html_tag.to_string())
        // Non-primitive fallback: route through map_jsx_tag so snake_case SVG forms
        // (radial_gradient → radialGradient, fe_gaussian_blur → feGaussianBlur, etc.)
        // emit canonical camelCase. Plain HTML/SVG tags pass through unchanged.
        .unwrap_or_else(|| map_jsx_tag(tag).to_string());
    // Author kwarg names — used to suppress primitive base classes on the same Tailwind axis.
    let author_kwargs: Vec<&str> = attrs.iter().map(|a| a.name.as_str()).collect();
    let mut class_pieces: Vec<String> = primitive_emission
        .as_ref()
        .map(|e| {
            e.base_classes
                .iter()
                .filter(|c| {
                    !super::web_ir::primitives::primitive_base_class_overridden(c, &author_kwargs)
                })
                .map(|c| format!("\"{c}\""))
                .collect()
        })
        .unwrap_or_default();
    let mut passthrough: Vec<HirJsxAttr> = Vec::with_capacity(attrs.len());
    let mut safe_area_style: Option<String> = None;

    for attr in attrs {
        let name = attr.name.as_str();
        if name == "class" || name == "className" {
            let val = emit_hir_expr_attr_value(&attr.value, ctx, name);
            class_pieces.push(val);
            continue;
        }
        if HIR_PRIMITIVE_CONSUMED_PROPS.contains(&name) {
            // Already folded into primitive_emission above.
            continue;
        }
        // D1: safe_area kwarg → inline style CSS env() vars (not expressible as Tailwind classes).
        if name == "safe_area" {
            if let HirExpr::StringLit(v, _) = unwrap_inline_hir_block_expr(&attr.value) {
                let props = super::web_ir::primitives::safe_area_to_style_props(v);
                if !props.is_empty() {
                    safe_area_style = Some(props);
                }
            }
            continue;
        }
        if let Some(piece) = hir_kwarg_to_class_expr(name, &attr.value, ctx) {
            class_pieces.push(piece);
            continue;
        }
        if super::web_ir::primitives::UNIVERSAL_STYLE_KWARGS.contains(&name) {
            // Recognized style kwarg with no class to emit (e.g. `border=false`).
            // Drop instead of passing through to prevent invalid JSX attrs.
            continue;
        }
        passthrough.push(attr.clone());
    }

    let class_expr = if class_pieces.is_empty() {
        None
    } else if class_pieces.len() == 1 {
        Some(class_pieces.into_iter().next().unwrap())
    } else {
        Some(format!(
            "[{}].filter(Boolean).join(\" \")",
            class_pieces.join(", ")
        ))
    };

    ViewCallHir {
        html_tag,
        class_expr,
        style_expr: safe_area_style,
        passthrough,
    }
}

fn hir_kwarg_to_class_expr(kwarg: &str, expr: &HirExpr, ctx: &EmitCtx<'_>) -> Option<String> {
    match unwrap_inline_hir_block_expr(expr) {
        HirExpr::StringLit(value, _) => {
            let classes = super::web_ir::primitives::resolve_universal_kwarg(kwarg, value)?;
            if classes.is_empty() {
                return None;
            }
            Some(format!("\"{}\"", classes.join(" ")))
        }
        HirExpr::BoolLit(value, _) => {
            let v = value.to_string();
            let classes = super::web_ir::primitives::resolve_universal_kwarg(kwarg, &v)?;
            if classes.is_empty() {
                return None;
            }
            Some(format!("\"{}\"", classes.join(" ")))
        }
        HirExpr::IntLit(value, _) => {
            let v = value.to_string();
            let classes = super::web_ir::primitives::resolve_universal_kwarg(kwarg, &v)?;
            if classes.is_empty() {
                return None;
            }
            Some(format!("\"{}\"", classes.join(" ")))
        }
        HirExpr::FloatLit(value, _) => {
            let v = value.to_string();
            let classes = super::web_ir::primitives::resolve_universal_kwarg(kwarg, &v)?;
            if classes.is_empty() {
                return None;
            }
            Some(format!("\"{}\"", classes.join(" ")))
        }
        HirExpr::If(cond, then_stmts, else_stmts, _) => {
            let then_expr = single_trailing_hir_expr(then_stmts)?;
            let else_stmts = else_stmts.as_ref()?;
            let else_expr = single_trailing_hir_expr(else_stmts)?;
            let then_class = hir_kwarg_to_class_expr(kwarg, then_expr, ctx)?;
            let else_class = hir_kwarg_to_class_expr(kwarg, else_expr, ctx)?;
            let cond_str = emit_hir_expr(cond, ctx);
            Some(format!("({cond_str} ? {then_class} : {else_class})"))
        }
        _ if super::web_ir::primitives::UNIVERSAL_STYLE_KWARGS.contains(&kwarg) => None,
        _ => None,
    }
}

fn single_trailing_hir_expr(body: &[HirStmt]) -> Option<&HirExpr> {
    if body.len() != 1 {
        return None;
    }
    if let HirStmt::Expr { expr, .. } = &body[0] {
        Some(expr)
    } else {
        None
    }
}

/// Inject a `key` attribute string into the first JSX element opening tag in `jsx`.
///
/// Scans past the tag name and attribute list, correctly skipping over `{...}`
/// JSX expression values (which may contain `/` or `>`) and `"..."` / `'...'`
/// string literals. Inserts `key_attr` (e.g. ` key={expr}`) immediately before
/// the closing `>` or `/>` of the opening tag.
///
/// Falls back to returning the original string unchanged if no opening tag is
/// found or parsing fails to locate the end of the opening tag.
fn inject_key_into_jsx(jsx: String, key_attr: &str) -> String {
    let Some(lt_pos) = jsx.find('<') else {
        return jsx;
    };
    let chars: Vec<char> = jsx[lt_pos..].chars().collect();
    let n = chars.len();
    // Skip past '<' and the tag name (first char after '<' begins the tag name).
    let mut i = 1; // skip '<'
    while i < n
        && chars[i] != '>'
        && chars[i] != ' '
        && chars[i] != '\n'
        && chars[i] != '\t'
        && chars[i] != '/'
    {
        i += 1;
    }
    // Scan through attributes, properly balancing {}, [], (), and skipping strings.
    while i < n {
        match chars[i] {
            '>' => {
                // End of opening tag — insert key here.
                let insert_at = lt_pos + chars[..i].iter().collect::<String>().len();
                return format!("{}{}{}", &jsx[..insert_at], key_attr, &jsx[insert_at..]);
            }
            '/' if i + 1 < n && chars[i + 1] == '>' => {
                // Self-closing `/>` — insert key before it.
                let insert_at = lt_pos + chars[..i].iter().collect::<String>().len();
                return format!("{}{}{}", &jsx[..insert_at], key_attr, &jsx[insert_at..]);
            }
            '{' => {
                // JSX expression value: skip balanced braces.
                let mut depth = 1usize;
                i += 1;
                while i < n && depth > 0 {
                    match chars[i] {
                        '{' => depth += 1,
                        '}' => depth -= 1,
                        _ => {}
                    }
                    i += 1;
                }
            }
            '"' | '\'' => {
                // String literal attribute value: skip to matching close quote.
                let q = chars[i];
                i += 1;
                while i < n && chars[i] != q {
                    if chars[i] == '\\' {
                        i += 1; // skip escaped char
                    }
                    i += 1;
                }
                i += 1; // skip closing quote
            }
            _ => {
                i += 1;
            }
        }
    }
    jsx
}

#[cfg(test)]
mod inject_key_tests {
    use super::inject_key_into_jsx;

    // Basic case: simple element, no tricky chars.
    #[test]
    fn basic_element_gets_key_before_close() {
        let jsx = "<div >children</div>".to_string();
        let result = inject_key_into_jsx(jsx, " key={x}");
        assert_eq!(result, "<div  key={x}>children</div>");
    }

    // Self-closing element: key inserted before `/>`.
    #[test]
    fn self_closing_element_gets_key_before_slash_gt() {
        let jsx = "<img src={item.src} />".to_string();
        let result = inject_key_into_jsx(jsx, " key={item.id}");
        // Key must appear before `/>` and the src attribute must be unchanged.
        assert!(
            result.contains("src={item.src}"),
            "src attr must be unchanged: {result}"
        );
        assert!(
            result.contains("key={item.id}"),
            "key attr must be present: {result}"
        );
        assert!(
            result.ends_with("/>"),
            "must still be self-closing: {result}"
        );
        // Key must appear AFTER the src attribute (not inside it).
        let key_pos = result.find("key={item.id}").unwrap();
        let src_end = result.find("src={item.src}").unwrap() + "src={item.src}".len();
        assert!(key_pos > src_end, "key must come after src: {result}");
    }

    // Attribute value contains `/` — the old naive scanner would misplace the key.
    #[test]
    fn slash_inside_jsx_expression_attr_value_not_confused_with_close() {
        // Simulates className containing a Tailwind fraction like "w-1/2".
        let jsx =
            "<div className={[\"w-1/2\", \"bg-blue-500/50\"].filter(Boolean).join(\" \")}>\n  {x}\n</div>"
                .to_string();
        let result = inject_key_into_jsx(jsx, " key={item.id}");
        // Key must be at the end of the opening tag, after the className attribute.
        assert!(
            result.contains("className={[\"w-1/2\", \"bg-blue-500/50\"].filter(Boolean).join(\" \")} key={item.id}>"),
            "key must be after closing `}}` of className, got: {result}"
        );
        // The className string literals must not be mutated.
        assert!(
            result.contains("\"w-1/2\""),
            "fraction class must be intact: {result}"
        );
    }

    // Attribute value contains `>` inside a string — old scanner would misplace key.
    #[test]
    fn gt_inside_string_attr_value_not_confused_with_opening_tag_close() {
        let jsx = "<div title={\"a > b\"}>\n  {child}\n</div>".to_string();
        let result = inject_key_into_jsx(jsx, " key={k}");
        assert!(
            result.contains("title={\"a > b\"} key={k}>"),
            "key must follow the title attr, got: {result}"
        );
    }

    // Multi-line opening tag (the actual format the HIR emitter produces).
    #[test]
    fn multiline_opening_tag_newline_before_gt() {
        // HIR emitter format: `<div attrs\n>\n  children\n</div>`
        let jsx = "<div className={[\"bg-background\", \"p-4\"].filter(Boolean).join(\" \")}\n>\n  <p>text</p>\n</div>".to_string();
        let result = inject_key_into_jsx(jsx, " key={item.id}");
        // Key inserted before the `>` that follows the `\n`.
        assert!(
            result.contains(".join(\" \")} key={item.id}>")
                || result.contains(".join(\" \")}\n key={item.id}>"),
            "key must be before `>` that closes opening tag, got: {result}"
        );
        // The Tailwind classes must be intact.
        assert!(
            result.contains("\"bg-background\""),
            "classes intact: {result}"
        );
    }

    // No `<` in input — returns unchanged.
    #[test]
    fn no_jsx_returns_unchanged() {
        let jsx = "items.map((item) => item.name)".to_string();
        let result = inject_key_into_jsx(jsx.clone(), " key={x}");
        assert_eq!(result, jsx);
    }
}
