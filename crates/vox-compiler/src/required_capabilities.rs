//! Required runtime capability ids for packaging (Tauri / mobile), derived from HIR.
//!
//! Ids must match rows in `contracts/capability/runtime-capabilities.v1.yaml`.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::hir::nodes::effect::HirEffectKind;
use crate::hir::{
    HirCapability, HirEndpointFn, HirExpr, HirFn, HirModule, HirStmt,
};

/// Version of [`RequiredRuntimeCapabilities`] JSON envelope.
pub const REQUIRED_CAPABILITIES_SCHEMA_VERSION: u32 = 1;

/// Sorted, deduplicated capability ids required by this module for packaging projections.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequiredRuntimeCapabilities {
    pub schema_version: u32,
    pub capability_ids: Vec<String>,
}

fn effect_kind_to_cap(eff: &HirEffectKind) -> HirCapability {
    match eff {
        HirEffectKind::Net => HirCapability::Net,
        HirEffectKind::Db => HirCapability::Db,
        HirEffectKind::Fs => HirCapability::Fs,
        HirEffectKind::Env => HirCapability::Env,
        HirEffectKind::Clock => HirCapability::Clock,
        HirEffectKind::Random => HirCapability::Random,
        HirEffectKind::Spawn => HirCapability::Spawn,
        HirEffectKind::GpuCompute => HirCapability::GpuCompute,
        HirEffectKind::Mutate => HirCapability::Mutate,
        HirEffectKind::Mcp(s) => HirCapability::Mcp(s.clone()),
    }
}

fn effective_fn_capabilities(f: &HirFn) -> impl Iterator<Item = HirCapability> + '_ {
    f.capabilities
        .iter()
        .filter(|c| !matches!(c, HirCapability::Nothing))
        .cloned()
}

fn effective_endpoint_capabilities(f: &HirEndpointFn) -> Vec<HirCapability> {
    if f.is_pure {
        return vec![];
    }
    f.effects.iter().map(effect_kind_to_cap).collect()
}

fn is_fs_module(name: &str) -> bool {
    matches!(name, "fs" | "filesystem" | "FS" | "Filesystem")
}

fn fs_method_rw(method: &str) -> Option<&'static str> {
    let m = method.to_ascii_lowercase();
    if m.starts_with("read")
        || m == "open"
        || m.contains("read_")
        || m.ends_with("_read")
    {
        return Some("fs.read");
    }
    if m.starts_with("write")
        || m.starts_with("append")
        || m.starts_with("create")
        || m == "remove"
        || m == "rename"
        || m == "copy"
        || m.contains("write_")
    {
        return Some("fs.write");
    }
    None
}

fn collect_fs_rw_from_expr(expr: &HirExpr, read: &mut bool, write: &mut bool) {
    match expr {
        HirExpr::MethodCall(obj, method, args, _, _) => {
            if let HirExpr::Ident(module_name, _) = obj.as_ref()
                && is_fs_module(module_name)
                && let Some(id) = fs_method_rw(method)
            {
                if id == "fs.read" {
                    *read = true;
                } else {
                    *write = true;
                }
            }
            collect_fs_rw_from_expr(obj, read, write);
            for a in args {
                collect_fs_rw_from_expr(&a.value, read, write);
            }
        }
        HirExpr::Call(callee, args, _, _) => {
            collect_fs_rw_from_expr(callee, read, write);
            for a in args {
                collect_fs_rw_from_expr(&a.value, read, write);
            }
        }
        HirExpr::Binary(_, l, r, _) => {
            collect_fs_rw_from_expr(l, read, write);
            collect_fs_rw_from_expr(r, read, write);
        }
        HirExpr::Unary(_, o, _) => collect_fs_rw_from_expr(o, read, write),
        HirExpr::If(c, t, e, _) => {
            collect_fs_rw_from_expr(c, read, write);
            for s in t {
                collect_fs_rw_from_stmt(s, read, write);
            }
            if let Some(els) = e {
                for s in els {
                    collect_fs_rw_from_stmt(s, read, write);
                }
            }
        }
        HirExpr::Block(stmts, _) => {
            for s in stmts {
                collect_fs_rw_from_stmt(s, read, write);
            }
        }
        HirExpr::For(_, _, it, body, _, _) => {
            collect_fs_rw_from_expr(it, read, write);
            collect_fs_rw_from_expr(body, read, write);
        }
        HirExpr::Lambda(_, _, body, _, _) => collect_fs_rw_from_expr(body, read, write),
        HirExpr::With(l, r, _) => {
            collect_fs_rw_from_expr(l, read, write);
            collect_fs_rw_from_expr(r, read, write);
        }
        HirExpr::Match(subj, arms, _) => {
            collect_fs_rw_from_expr(subj, read, write);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    collect_fs_rw_from_expr(g, read, write);
                }
                collect_fs_rw_from_expr(&arm.body, read, write);
            }
        }
        HirExpr::FieldAccess(o, _, _) => collect_fs_rw_from_expr(o, read, write),
        HirExpr::ListLit(elems, _) | HirExpr::TupleLit(elems, _) => {
            for e in elems {
                collect_fs_rw_from_expr(e, read, write);
            }
        }
        HirExpr::ObjectLit(fields, _) => {
            for (_, v) in fields {
                collect_fs_rw_from_expr(v, read, write);
            }
        }
        HirExpr::Spawn(inner, _) => collect_fs_rw_from_expr(inner, read, write),
        HirExpr::JsxFragment(children, _) => {
            for c in children {
                collect_fs_rw_from_expr(c, read, write);
            }
        }
        HirExpr::Index(o, i, _) => {
            collect_fs_rw_from_expr(o, read, write);
            collect_fs_rw_from_expr(i, read, write);
        }
        HirExpr::AsyncView(v) => {
            collect_fs_rw_from_expr(v.source.as_ref(), read, write);
            if let Some(a) = &v.fetching_arm {
                collect_fs_rw_from_expr(a, read, write);
            }
            if let Some(a) = &v.empty_arm {
                collect_fs_rw_from_expr(a, read, write);
            }
            if let Some(a) = &v.error_arm {
                collect_fs_rw_from_expr(a, read, write);
            }
            if let Some(a) = &v.ok_arm {
                collect_fs_rw_from_expr(a, read, write);
            }
        }
        HirExpr::IntLit(..)
        | HirExpr::FloatLit(..)
        | HirExpr::DecimalLit(..)
        | HirExpr::StringLit(..)
        | HirExpr::BoolLit(..)
        | HirExpr::Ident(..)
        | HirExpr::JsxSelfClosing(_)
        | HirExpr::Jsx(_)
        | HirExpr::Try(_)
        | HirExpr::WorkflowVersion(_) => {}
    }
}

fn collect_fs_rw_from_stmt(stmt: &HirStmt, read: &mut bool, write: &mut bool) {
    match stmt {
        HirStmt::Let { value, .. } | HirStmt::Expr { expr: value, .. } => {
            collect_fs_rw_from_expr(value, read, write);
        }
        HirStmt::Assign { target, value, .. } => {
            collect_fs_rw_from_expr(target, read, write);
            collect_fs_rw_from_expr(value, read, write);
        }
        HirStmt::Return { value, .. } => {
            if let Some(e) = value {
                collect_fs_rw_from_expr(e, read, write);
            }
        }
        HirStmt::While { condition, body, .. } => {
            collect_fs_rw_from_expr(condition, read, write);
            for s in body {
                collect_fs_rw_from_stmt(s, read, write);
            }
        }
        HirStmt::Loop { body, .. } => {
            for s in body {
                collect_fs_rw_from_stmt(s, read, write);
            }
        }
        HirStmt::Break { .. } | HirStmt::Continue { .. } => {}
    }
}

fn walk_fn_body_for_fs(body: &[HirStmt], read: &mut bool, write: &mut bool) {
    for s in body {
        collect_fs_rw_from_stmt(s, read, write);
    }
}

fn hir_capability_to_packaging_id(cap: &HirCapability) -> Option<&'static str> {
    match cap {
        HirCapability::Net => Some("net.http"),
        HirCapability::Fs => None,
        HirCapability::Nothing => None,
        // No YAML row yet for these — omit from required packaging set.
        HirCapability::Db
        | HirCapability::Env
        | HirCapability::Clock
        | HirCapability::Random
        | HirCapability::Spawn
        | HirCapability::GpuCompute
        | HirCapability::Mutate
        | HirCapability::Mcp(_) => None,
    }
}

/// Collect required packaging capability ids from a lowered module.
#[must_use]
pub fn project_required_capabilities(m: &HirModule) -> RequiredRuntimeCapabilities {
    let mut ids: BTreeSet<String> = BTreeSet::new();

    if m.deep_link.is_some() {
        ids.insert("deep_link".to_string());
    }
    if m.push.is_some() {
        ids.insert("notifications".to_string());
    }

    let mut fs_declared = false;
    let mut fs_read = false;
    let mut fs_write = false;

    for f in &m.functions {
        for cap in effective_fn_capabilities(f) {
            if cap == HirCapability::Fs {
                fs_declared = true;
            }
            if let Some(id) = hir_capability_to_packaging_id(&cap) {
                ids.insert(id.to_string());
            }
        }
        walk_fn_body_for_fs(&f.body, &mut fs_read, &mut fs_write);
    }

    for f in &m.endpoint_fns {
        for cap in effective_endpoint_capabilities(f) {
            if cap == HirCapability::Fs {
                fs_declared = true;
            }
            if let Some(id) = hir_capability_to_packaging_id(&cap) {
                ids.insert(id.to_string());
            }
        }
        walk_fn_body_for_fs(&f.body, &mut fs_read, &mut fs_write);
    }

    if fs_read {
        ids.insert("fs.read".to_string());
    }
    if fs_write {
        ids.insert("fs.write".to_string());
    }
    if fs_declared && !fs_read && !fs_write {
        ids.insert("fs.read".to_string());
        ids.insert("fs.write".to_string());
    }

    let mut capability_ids: Vec<String> = ids.into_iter().collect();
    capability_ids.sort();

    RequiredRuntimeCapabilities {
        schema_version: REQUIRED_CAPABILITIES_SCHEMA_VERSION,
        capability_ids,
    }
}

/// Canonical JSON bytes for stable hashing / parity tests.
pub fn canonical_required_capabilities_bytes(
    c: &RequiredRuntimeCapabilities,
) -> Result<Vec<u8>, serde_json::Error> {
    let mut v = serde_json::to_value(c)?;
    crate::canonical_json::sort_json_value_keys(&mut v);
    serde_json::to_vec(&v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_module_has_empty_capabilities() {
        let m = HirModule::default();
        let r = project_required_capabilities(&m);
        assert!(r.capability_ids.is_empty());
    }
}
