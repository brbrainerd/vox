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

/// Classify a `Speech.*` method.
///
/// Both methods need the STT plugin (`speech`); only microphone capture additionally needs the OS
/// microphone permission (RECORD_AUDIO / NSMicrophoneUsageDescription). `transcribe(path)` is
/// file-based and must NOT trigger the microphone permission (App Store review friction).
fn speech_method_uses_microphone(method: &str) -> Option<bool> {
    match method {
        "transcribe" => Some(false),
        "transcribe_microphone" => Some(true),
        _ => None,
    }
}

/// Usage flags accumulated during the HIR body walk for capability derivation.
#[derive(Default)]
struct UsageFlags {
    fs_read: bool,
    fs_write: bool,
    /// Module uses ANY `Speech.*` method → needs the STT plugin.
    speech: bool,
    /// Module uses `Speech.transcribe_microphone()` → needs the OS microphone permission.
    microphone: bool,
}

fn collect_usage_from_expr(expr: &HirExpr, flags: &mut UsageFlags) {
    match expr {
        HirExpr::MethodCall(obj, method, args, _, _) => {
            if let HirExpr::Ident(module_name, _) = obj.as_ref() {
                if is_fs_module(module_name)
                    && let Some(id) = fs_method_rw(method)
                {
                    if id == "fs.read" {
                        flags.fs_read = true;
                    } else {
                        flags.fs_write = true;
                    }
                }
                if is_speech_module(module_name)
                    && let Some(uses_mic) = speech_method_uses_microphone(method)
                {
                    flags.speech = true;
                    if uses_mic {
                        flags.microphone = true;
                    }
                }
            }
            collect_usage_from_expr(obj, flags);
            for a in args {
                collect_usage_from_expr(&a.value, flags);
            }
        }
        HirExpr::Call(callee, args, _, _) => {
            collect_usage_from_expr(callee, flags);
            for a in args {
                collect_usage_from_expr(&a.value, flags);
            }
        }
        HirExpr::Binary(_, l, r, _) => {
            collect_usage_from_expr(l, flags);
            collect_usage_from_expr(r, flags);
        }
        HirExpr::Unary(_, o, _) => collect_usage_from_expr(o, flags),
        HirExpr::If(c, t, e, _) => {
            collect_usage_from_expr(c, flags);
            for s in t {
                collect_usage_from_stmt(s, flags);
            }
            if let Some(els) = e {
                for s in els {
                    collect_usage_from_stmt(s, flags);
                }
            }
        }
        HirExpr::Block(stmts, _) => {
            for s in stmts {
                collect_usage_from_stmt(s, flags);
            }
        }
        HirExpr::For(_, _, it, body, _, _) => {
            collect_usage_from_expr(it, flags);
            collect_usage_from_expr(body, flags);
        }
        HirExpr::Lambda(_, _, body, _, _) => collect_usage_from_expr(body, flags),
        HirExpr::With(l, r, _) => {
            collect_usage_from_expr(l, flags);
            collect_usage_from_expr(r, flags);
        }
        HirExpr::Match(subj, arms, _) => {
            collect_usage_from_expr(subj, flags);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    collect_usage_from_expr(g, flags);
                }
                collect_usage_from_expr(&arm.body, flags);
            }
        }
        HirExpr::FieldAccess(o, _, _) => collect_usage_from_expr(o, flags),
        HirExpr::ListLit(elems, _) | HirExpr::TupleLit(elems, _) => {
            for e in elems {
                collect_usage_from_expr(e, flags);
            }
        }
        HirExpr::ObjectLit(fields, _) => {
            for (_, v) in fields {
                collect_usage_from_expr(v, flags);
            }
        }
        HirExpr::Spawn(inner, _) => collect_usage_from_expr(inner, flags),
        HirExpr::Try(t) => collect_usage_from_expr(t.target.as_ref(), flags),
        HirExpr::JsxFragment(children, _) => {
            for c in children {
                collect_usage_from_expr(c, flags);
            }
        }
        HirExpr::Jsx(el) => {
            for a in &el.attributes {
                collect_usage_from_expr(&a.value, flags);
            }
            for c in &el.children {
                collect_usage_from_expr(c, flags);
            }
        }
        HirExpr::JsxSelfClosing(el) => {
            for a in &el.attributes {
                collect_usage_from_expr(&a.value, flags);
            }
        }
        HirExpr::Index(o, i, _) => {
            collect_usage_from_expr(o, flags);
            collect_usage_from_expr(i, flags);
        }
        HirExpr::AsyncView(v) => {
            collect_usage_from_expr(v.source.as_ref(), flags);
            if let Some(a) = &v.fetching_arm {
                collect_usage_from_expr(a, flags);
            }
            if let Some(a) = &v.empty_arm {
                collect_usage_from_expr(a, flags);
            }
            if let Some(a) = &v.error_arm {
                collect_usage_from_expr(a, flags);
            }
            if let Some(a) = &v.ok_arm {
                collect_usage_from_expr(a, flags);
            }
        }
        HirExpr::IntLit(..)
        | HirExpr::FloatLit(..)
        | HirExpr::DecimalLit(..)
        | HirExpr::StringLit(..)
        | HirExpr::BoolLit(..)
        | HirExpr::Ident(..)
        | HirExpr::WorkflowVersion(_) => {}
    }
}

fn collect_usage_from_stmt(stmt: &HirStmt, flags: &mut UsageFlags) {
    match stmt {
        HirStmt::Let { value, .. } | HirStmt::Expr { expr: value, .. } => {
            collect_usage_from_expr(value, flags);
        }
        HirStmt::Assign { target, value, .. } => {
            collect_usage_from_expr(target, flags);
            collect_usage_from_expr(value, flags);
        }
        HirStmt::Return { value, .. } => {
            if let Some(e) = value {
                collect_usage_from_expr(e, flags);
            }
        }
        HirStmt::While {
            condition, body, ..
        } => {
            collect_usage_from_expr(condition, flags);
            for s in body {
                collect_usage_from_stmt(s, flags);
            }
        }
        HirStmt::Loop { body, .. } => {
            for s in body {
                collect_usage_from_stmt(s, flags);
            }
        }
        HirStmt::Break { .. } | HirStmt::Continue { .. } => {}
    }
}

fn walk_fn_body_for_usage(body: &[HirStmt], flags: &mut UsageFlags) {
    for s in body {
        collect_usage_from_stmt(s, flags);
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
    if usage.speech {
        ids.insert("speech".to_string());
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
        // `transcribe_microphone` derives BOTH speech (STT plugin) and microphone (OS mic permission).
        assert!(caps.iter().any(|c| c == "microphone"), "{caps:?}");
        assert!(caps.iter().any(|c| c == "speech"), "{caps:?}");
    }

    #[test]
    fn try_postfix_microphone_call_derives_speech_and_microphone() {
        let res = crate::pipeline::run_frontend_str(
            "fn f() -> Result[str] { Speech.transcribe_microphone()? }",
            "t.vox",
        )
        .expect("frontend ok");
        let caps = project_required_capabilities(&res.hir).capability_ids;
        // A try-postfix (`?`) call must still derive its capabilities.
        assert!(caps.iter().any(|c| c == "speech"), "{caps:?}");
        assert!(caps.iter().any(|c| c == "microphone"), "{caps:?}");
    }

    #[test]
    fn try_postfix_fs_read_call_derives_fs_read() {
        let res = crate::pipeline::run_frontend_str(
            "fn f(p: str) -> Result[str] { fs.read(p)? }",
            "t.vox",
        )
        .expect("frontend ok");
        let caps = project_required_capabilities(&res.hir).capability_ids;
        // A try-postfix (`?`) fs read must derive the fs.read capability.
        assert!(caps.iter().any(|c| c == "fs.read"), "{caps:?}");
    }

    #[test]
    fn file_transcribe_derives_speech_but_not_microphone() {
        let res = crate::pipeline::run_frontend_str(
            "fn note() -> Result[str] { Speech.transcribe(\"/tmp/a.wav\") }",
            "t.vox",
        )
        .expect("frontend ok");
        let caps = project_required_capabilities(&res.hir).capability_ids;
        // File-based transcription needs the STT plugin (`speech`) but NOT the OS mic permission.
        assert!(caps.iter().any(|c| c == "speech"), "{caps:?}");
        assert!(!caps.iter().any(|c| c == "microphone"), "{caps:?}");
    }

    #[test]
    fn jsx_attribute_microphone_call_derives_speech_and_microphone() {
        // Self-closing view-call (`<Recorder audio={...} />`) sugars to `HirExpr::JsxSelfClosing`.
        // The capability walker must descend into attribute value expressions, so an effectful
        // `Speech.transcribe_microphone()` inside a prop derives both `speech` and `microphone`.
        let res = crate::pipeline::run_frontend_str(
            "fn render() -> Result[str] { Recorder(audio=Speech.transcribe_microphone()) }",
            "t.vox",
        )
        .expect("frontend ok");
        let caps = project_required_capabilities(&res.hir).capability_ids;
        assert!(caps.iter().any(|c| c == "speech"), "{caps:?}");
        assert!(caps.iter().any(|c| c == "microphone"), "{caps:?}");
    }

    #[test]
    fn jsx_child_microphone_call_derives_speech_and_microphone() {
        // Element with children (`<Wrapper>{ Speech.transcribe_microphone() }</Wrapper>`) sugars to
        // `HirExpr::Jsx`. The walker must descend into both attributes AND children.
        let res = crate::pipeline::run_frontend_str(
            "fn render() -> Result[str] { Wrapper() { Speech.transcribe_microphone() } }",
            "t.vox",
        )
        .expect("frontend ok");
        let caps = project_required_capabilities(&res.hir).capability_ids;
        assert!(caps.iter().any(|c| c == "speech"), "{caps:?}");
        assert!(caps.iter().any(|c| c == "microphone"), "{caps:?}");
    }

    #[test]
    fn no_speech_or_microphone_capability_without_speech() {
        let res = crate::pipeline::run_frontend_str("fn f() { }", "t.vox").expect("frontend ok");
        let caps = project_required_capabilities(&res.hir).capability_ids;
        assert!(!caps.iter().any(|c| c == "microphone"), "{caps:?}");
        assert!(!caps.iter().any(|c| c == "speech"), "{caps:?}");
    }
}
