//! `mobile-utils.ts` emit + reference detection for the RN target.
//!
//! Vox source declares `import std.mobile` and then calls `mobile.notify(...)`,
//! `mobile.transcribe_microphone()`, etc. inside component handlers. The
//! shared HIR-to-TS emit produces the call sites verbatim, expecting a `mobile`
//! binding to exist in scope.
//!
//! On the web target, `mobile-utils.ts` provides that binding via Tauri's
//! `invoke` / `listen`. On the RN target this module provides the equivalent
//! re-export, routing every method through `@vox/runtime-rn::voxRuntime` so
//! Vox source remains target-agnostic.
//!
//! Method names: Vox uses snake_case (`take_photo`, `transcribe_microphone`)
//! and the JS `voxRuntime` interface uses camelCase (`takePhoto`,
//! `transcribeMicrophone`). The wrappers below bridge that one-time at the
//! emit boundary so no caller needs to know.

use vox_compiler::hir::{HirExpr, HirModule, HirReactiveMember, HirStmt};

/// Walk every component's view, prelude, derived expressions, and effects to
/// see if the bare identifier `mobile` appears anywhere. Used to decide
/// whether `mobile-utils.ts` is needed and to know which component files
/// should `import { mobile } from "./mobile-utils"`.
pub fn any_component_uses_mobile(hir: &HirModule) -> bool {
    hir.components.iter().any(component_uses_mobile)
}

/// Check a single component for any reference to the `mobile` identifier.
pub fn component_uses_mobile(rc: &vox_compiler::hir::HirReactiveComponent) -> bool {
    for m in &rc.members {
        match m {
            HirReactiveMember::State(s) => {
                if expr_uses_mobile(&s.init) {
                    return true;
                }
            }
            HirReactiveMember::Derived(d) => {
                if expr_uses_mobile(&d.expr) {
                    return true;
                }
            }
            HirReactiveMember::Effect(e) => {
                if expr_uses_mobile(&e.body) {
                    return true;
                }
            }
            HirReactiveMember::OnMount(o) => {
                if expr_uses_mobile(&o.body) {
                    return true;
                }
            }
            HirReactiveMember::OnCleanup(o) => {
                if expr_uses_mobile(&o.body) {
                    return true;
                }
            }
            HirReactiveMember::Stmt(s) => {
                if stmt_uses_mobile(s) {
                    return true;
                }
            }
        }
    }
    if let Some(v) = &rc.view {
        if expr_uses_mobile(v) {
            return true;
        }
    }
    false
}

fn stmt_uses_mobile(stmt: &HirStmt) -> bool {
    match stmt {
        HirStmt::Let { value, .. } => expr_uses_mobile(value),
        HirStmt::Assign { target, value, .. } => {
            expr_uses_mobile(target) || expr_uses_mobile(value)
        }
        HirStmt::Return { value, .. } => value.as_ref().is_some_and(expr_uses_mobile),
        HirStmt::Expr { expr, .. } => expr_uses_mobile(expr),
        HirStmt::While { condition, body, .. } => {
            expr_uses_mobile(condition) || body.iter().any(stmt_uses_mobile)
        }
        HirStmt::Loop { body, .. } => body.iter().any(stmt_uses_mobile),
        _ => false,
    }
}

fn expr_uses_mobile(expr: &HirExpr) -> bool {
    match expr {
        HirExpr::Ident(name, _) => name == "mobile",
        HirExpr::Call(callee, args, _, _) => {
            expr_uses_mobile(callee) || args.iter().any(|a| expr_uses_mobile(&a.value))
        }
        HirExpr::MethodCall(receiver, _, args, _, _) => {
            expr_uses_mobile(receiver) || args.iter().any(|a| expr_uses_mobile(&a.value))
        }
        HirExpr::FieldAccess(object, _, _) => expr_uses_mobile(object),
        HirExpr::Binary(_, l, r, _) => expr_uses_mobile(l) || expr_uses_mobile(r),
        HirExpr::Unary(_, operand, _) => expr_uses_mobile(operand),
        HirExpr::ObjectLit(fields, _) => fields.iter().any(|(_, v)| expr_uses_mobile(v)),
        HirExpr::ListLit(elements, _) | HirExpr::TupleLit(elements, _) => {
            elements.iter().any(expr_uses_mobile)
        }
        HirExpr::Block(stmts, _) => stmts.iter().any(stmt_uses_mobile),
        HirExpr::If(cond, then_b, else_b, _) => {
            expr_uses_mobile(cond)
                || then_b.iter().any(stmt_uses_mobile)
                || else_b
                    .as_ref()
                    .is_some_and(|b| b.iter().any(stmt_uses_mobile))
        }
        HirExpr::Match(subj, arms, _) => {
            expr_uses_mobile(subj) || arms.iter().any(|a| expr_uses_mobile(&a.body))
        }
        HirExpr::For(_, _, iter, body, _, _) => {
            expr_uses_mobile(iter) || expr_uses_mobile(body)
        }
        HirExpr::Lambda(_, _, body, _, _) => expr_uses_mobile(body),
        HirExpr::Jsx(el) => {
            el.attributes.iter().any(|a| expr_uses_mobile(&a.value))
                || el.children.iter().any(expr_uses_mobile)
        }
        HirExpr::JsxSelfClosing(sc) => sc.attributes.iter().any(|a| expr_uses_mobile(&a.value)),
        HirExpr::JsxFragment(children, _) => children.iter().any(expr_uses_mobile),
        HirExpr::Spawn(target, _) => expr_uses_mobile(target),
        _ => false,
    }
}

/// Emit `mobile-utils.ts` content for the RN target.
///
/// Re-exports a `mobile` namespace whose method names match the snake_case
/// Vox source convention, each method delegating to `@vox/runtime-rn`.
pub fn emit_mobile_utils_rn() -> String {
    r#"// AUTO-GENERATED by Vox (RN target). Bridges `std.mobile` snake_case method
// names to the camelCase `@vox/runtime-rn::voxRuntime` API so emitted handlers
// can call `mobile.notify(...)`, `mobile.transcribe_microphone()`, etc.
import { voxRuntime } from "@vox/runtime-rn";

export const mobile = {
  notify(title: string, body: string): Promise<void> {
    return voxRuntime.notify(title, body);
  },
  vibrate(): Promise<void> {
    return voxRuntime.vibrate();
  },
  take_photo(): Promise<string> {
    return voxRuntime.takePhoto();
  },
  transcribe(audio_bytes: Uint8Array, lang_hint?: string): Promise<string> {
    return voxRuntime.transcribe(audio_bytes, lang_hint);
  },
  transcribe_microphone(): Promise<string> {
    return voxRuntime.transcribeMicrophone();
  },
};
"#
    .to_string()
}
