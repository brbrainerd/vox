//! Required runtime capability ids for packaging (Tauri / mobile), derived from HIR.
//!
//! Ids must match rows in `contracts/capability/runtime-capabilities.v1.yaml`.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::hir::nodes::effect::HirEffectKind;
use crate::hir::{HirCapability, HirEndpointFn, HirExpr, HirFn, HirModule, HirStmt};

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
    if m.starts_with("read") || m == "open" || m.contains("read_") || m.ends_with("_read") {
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

fn is_speech_module(name: &str) -> bool {
    name == "Speech"
}

fn is_speech_method(method: &str) -> bool {
    // TODO(capabilities): `transcribe(path)` is file-based and does not use the microphone at
    // runtime; only `transcribe_microphone()` truly requires RECORD_AUDIO /
    // NSMicrophoneUsageDescription.  A future change may want to derive the STT-plugin need
    // separately from the microphone permission so that file-only transcription paths avoid
    // triggering the microphone capability (and any associated App Store review scrutiny).
    matches!(method, "transcribe" | "transcribe_microphone")
}

/// Usage flags accumulated during the HIR body walk for capability derivation.
#[derive(Default)]
struct UsageFlags {
    fs_read: bool,
    fs_write: bool,
    microphone: bool,
}

fn collect_usage_from_expr(expr: &HirExpr, read: &mut bool, write: &mut bool, mic: &mut bool) {
    match expr {
        HirExpr::MethodCall(obj, method, args, _, _) => {
            if let HirExpr::Ident(module_name, _) = obj.as_ref() {
                if is_fs_module(module_name)
                    && let Some(id) = fs_method_rw(method)
                {
                    if id == "fs.read" {
                        *read = true;
                    } else {
                        *write = true;
                    }
                }
                if is_speech_module(module_name) && is_speech_method(method) {
                    *mic = true;
                }
            }
            collect_usage_from_expr(obj, read, write, mic);
            for a in args {
                collect_usage_from_expr(&a.value, read, write, mic);
            }
        }
        HirExpr::Call(callee, args, _, _) => {
            collect_usage_from_expr(callee, read, write, mic);
            for a in args {
                collect_usage_from_expr(&a.value, read, write, mic);
            }
        }
        HirExpr::Binary(_, l, r, _) => {
            collect_usage_from_expr(l, read, write, mic);
            collect_usage_from_expr(r, read, write, mic);
        }
        HirExpr::Unary(_, o, _) => collect_usage_from_expr(o, read, write, mic),
        HirExpr::If(c, t, e, _) => {
            collect_usage_from_expr(c, read, write, mic);
            for s in t {
                collect_usage_from_stmt(s, read, write, mic);
            }
            if let Some(els) = e {
                for s in els {
                    collect_usage_from_stmt(s, read, write, mic);
                }
            }
        }
        HirExpr::Block(stmts, _) => {
            for s in stmts {
                collect_usage_from_stmt(s, read, write, mic);
            }
        }
        HirExpr::For(_, _, it, body, _, _) => {
            collect_usage_from_expr(it, read, write, mic);
            collect_usage_from_expr(body, read, write, mic);
        }
        HirExpr::Lambda(_, _, body, _, _) => collect_usage_from_expr(body, read, write, mic),
        HirExpr::With(l, r, _) => {
            collect_usage_from_expr(l, read, write, mic);
            collect_usage_from_expr(r, read, write, mic);
        }
        HirExpr::Match(subj, arms, _) => {
            collect_usage_from_expr(subj, read, write, mic);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    collect_usage_from_expr(g, read, write, mic);
                }
                collect_usage_from_expr(&arm.body, read, write, mic);
            }
        }
        HirExpr::FieldAccess(o, _, _) => collect_usage_from_expr(o, read, write, mic),
        HirExpr::ListLit(elems, _) | HirExpr::TupleLit(elems, _) => {
            for e in elems {
                collect_usage_from_expr(e, read, write, mic);
            }
        }
        HirExpr::ObjectLit(fields, _) => {
            for (_, v) in fields {
                collect_usage_from_expr(v, read, write, mic);
            }
        }
        HirExpr::Spawn(inner, _) => collect_usage_from_expr(inner, read, write, mic),
        HirExpr::JsxFragment(children, _) => {
            for c in children {
                collect_usage_from_expr(c, read, write, mic);
            }
        }
        HirExpr::Index(o, i, _) => {
            collect_usage_from_expr(o, read, write, mic);
            collect_usage_from_expr(i, read, write, mic);
        }
        HirExpr::AsyncView(v) => {
            collect_usage_from_expr(v.source.as_ref(), read, write, mic);
            if let Some(a) = &v.fetching_arm {
                collect_usage_from_expr(a, read, write, mic);
            }
            if let Some(a) = &v.empty_arm {
                collect_usage_from_expr(a, read, write, mic);
            }
            if let Some(a) = &v.error_arm {
                collect_usage_from_expr(a, read, write, mic);
            }
            if let Some(a) = &v.ok_arm {
                collect_usage_from_expr(a, read, write, mic);
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

fn collect_usage_from_stmt(stmt: &HirStmt, read: &mut bool, write: &mut bool, mic: &mut bool) {
    match stmt {
        HirStmt::Let { value, .. } | HirStmt::Expr { expr: value, .. } => {
            collect_usage_from_expr(value, read, write, mic);
        }
        HirStmt::Assign { target, value, .. } => {
            collect_usage_from_expr(target, read, write, mic);
            collect_usage_from_expr(value, read, write, mic);
        }
        HirStmt::Return { value, .. } => {
            if let Some(e) = value {
                collect_usage_from_expr(e, read, write, mic);
            }
        }
        HirStmt::While {
            condition, body, ..
        } => {
            collect_usage_from_expr(condition, read, write, mic);
            for s in body {
                collect_usage_from_stmt(s, read, write, mic);
            }
        }
        HirStmt::Loop { body, .. } => {
            for s in body {
                collect_usage_from_stmt(s, read, write, mic);
            }
        }
        HirStmt::Break { .. } | HirStmt::Continue { .. } => {}
    }
}

fn walk_fn_body_for_usage(body: &[HirStmt], flags: &mut UsageFlags) {
    for s in body {
        collect_usage_from_stmt(
            s,
            &mut flags.fs_read,
            &mut flags.fs_write,
            &mut flags.microphone,
        );
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
    let mut usage = UsageFlags::default();

    for f in &m.functions {
        for cap in effective_fn_capabilities(f) {
            if cap == HirCapability::Fs {
                fs_declared = true;
            }
            if let Some(id) = hir_capability_to_packaging_id(&cap) {
                ids.insert(id.to_string());
            }
        }
        walk_fn_body_for_usage(&f.body, &mut usage);
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
        walk_fn_body_for_usage(&f.body, &mut usage);
    }

    if usage.fs_read {
        ids.insert("fs.read".to_string());
    }
    if usage.fs_write {
        ids.insert("fs.write".to_string());
    }
    if fs_declared && !usage.fs_read && !usage.fs_write {
        ids.insert("fs.read".to_string());
        ids.insert("fs.write".to_string());
    }
    if usage.microphone {
        ids.insert("microphone".to_string());
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

    #[test]
    fn microphone_capability_emitted_when_speech_used() {
        let res = crate::pipeline::run_frontend_str(
            "fn note() -> Result[str] { Speech.transcribe_microphone() }",
            "t.vox",
        )
        .expect("frontend ok");
        let caps = project_required_capabilities(&res.hir).capability_ids;
        assert!(caps.iter().any(|c| c == "microphone"), "{caps:?}");
    }

    #[test]
    fn no_microphone_capability_without_speech() {
        let res = crate::pipeline::run_frontend_str("fn f() { }", "t.vox").expect("frontend ok");
        let caps = project_required_capabilities(&res.hir).capability_ids;
        assert!(!caps.iter().any(|c| c == "microphone"), "{caps:?}");
    }
}
