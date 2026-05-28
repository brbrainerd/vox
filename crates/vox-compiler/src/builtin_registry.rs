//! Shared builtin registry to reduce drift between type registration and Rust emit mapping.

use crate::typeck::ty::Ty;

/// Parameter kind for registry-driven codegen (`str` lowers with `.as_str()`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinArgKind {
    Str,
    Bool,
    Int,
}

/// Builtin callable shape shared by typecheck and codegen lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinRegistryEntry {
    pub namespace: &'static str,
    pub name: &'static str,
    pub arg_count: usize,
    pub signature: &'static str,
    /// Fully-qualified runtime function symbol, if implemented by `vox-actor-runtime`.
    pub runtime_symbol: Option<&'static str>,
    /// Non-empty when arguments are not all `str` (must match [`Self::arg_count`]).
    pub arg_kinds: &'static [BuiltinArgKind],
    /// `true` → runtime returns `Result<(), String>` (Vox `Result[unit]`).
    pub returns_unit: bool,
}

/// Parameter types for a registry entry (shared by typecheck namespace methods).
#[must_use]
pub fn builtin_entry_param_tys(entry: BuiltinRegistryEntry) -> Option<Vec<Ty>> {
    if !entry.arg_kinds.is_empty() {
        if entry.arg_kinds.len() != entry.arg_count {
            return None;
        }
        return Some(
            entry
                .arg_kinds
                .iter()
                .map(|k| match k {
                    BuiltinArgKind::Str => Ty::Str,
                    BuiltinArgKind::Bool => Ty::Bool,
                    BuiltinArgKind::Int => Ty::Int,
                })
                .collect(),
        );
    }
    match entry.arg_count {
        0 => Some(vec![]),
        1 => Some(vec![Ty::Str]),
        2 => Some(vec![Ty::Str, Ty::Str]),
        3 => Some(vec![Ty::Str, Ty::Str, Ty::Str]),
        _ => None,
    }
}

/// Result type for a registry entry (`Result[str]` or `Result[unit]`).
#[must_use]
pub fn builtin_entry_result_ty(entry: BuiltinRegistryEntry) -> Ty {
    if entry.returns_unit {
        Ty::Result(Box::new(Ty::Unit))
    } else {
        Ty::Result(Box::new(Ty::Str))
    }
}

/// Stable subset of builtins with shared registry ownership.
#[must_use]
pub fn builtin_registry_entries() -> &'static [BuiltinRegistryEntry] {
    &[
        BuiltinRegistryEntry {
            namespace: "std",
            name: "uuid",
            arg_count: 0,
            signature: "fn() -> str",
            runtime_symbol: Some("vox_actor_runtime::builtins::vox_uuid"),
            arg_kinds: &[],
            returns_unit: false,
        },
        BuiltinRegistryEntry {
            namespace: "std",
            name: "now_ms",
            arg_count: 0,
            signature: "fn() -> int",
            runtime_symbol: Some("vox_actor_runtime::builtins::vox_now_ms"),
            arg_kinds: &[],
            returns_unit: false,
        },
        BuiltinRegistryEntry {
            namespace: "std",
            name: "hash_fast",
            arg_count: 1,
            signature: "fn(str) -> str",
            runtime_symbol: Some("vox_actor_runtime::builtins::vox_hash_fast"),
            arg_kinds: &[],
            returns_unit: false,
        },
        BuiltinRegistryEntry {
            namespace: "std",
            name: "hash_secure",
            arg_count: 1,
            signature: "fn(str) -> str",
            runtime_symbol: Some("vox_actor_runtime::builtins::vox_hash_secure"),
            arg_kinds: &[],
            returns_unit: false,
        },
        BuiltinRegistryEntry {
            namespace: "std.http",
            name: "get_text",
            arg_count: 1,
            signature: "fn(str) -> Result[str]",
            runtime_symbol: Some("vox_actor_runtime::builtins::vox_http_get_text"),
            arg_kinds: &[],
            returns_unit: false,
        },
        BuiltinRegistryEntry {
            namespace: "std.http",
            name: "post_json",
            arg_count: 2,
            signature: "fn(str, str) -> Result[str]",
            runtime_symbol: Some("vox_actor_runtime::builtins::vox_http_post_json"),
            arg_kinds: &[],
            returns_unit: false,
        },
        BuiltinRegistryEntry {
            namespace: "OpenClaw",
            name: "list_skills",
            arg_count: 0,
            signature: "fn() -> Result[str]",
            runtime_symbol: Some("vox_actor_runtime::builtins::vox_openclaw_list_skills"),
            arg_kinds: &[],
            returns_unit: false,
        },
        BuiltinRegistryEntry {
            namespace: "OpenClaw",
            name: "call",
            arg_count: 2,
            signature: "fn(str, str) -> Result[str]",
            runtime_symbol: Some("vox_actor_runtime::builtins::vox_openclaw_call"),
            arg_kinds: &[],
            returns_unit: false,
        },
        BuiltinRegistryEntry {
            namespace: "OpenClaw",
            name: "subscribe",
            arg_count: 1,
            signature: "fn(str) -> Result[str]",
            runtime_symbol: Some("vox_actor_runtime::builtins::vox_openclaw_subscribe"),
            arg_kinds: &[],
            returns_unit: false,
        },
        BuiltinRegistryEntry {
            namespace: "OpenClaw",
            name: "unsubscribe",
            arg_count: 1,
            signature: "fn(str) -> Result[str]",
            runtime_symbol: Some("vox_actor_runtime::builtins::vox_openclaw_unsubscribe"),
            arg_kinds: &[],
            returns_unit: false,
        },
        BuiltinRegistryEntry {
            namespace: "OpenClaw",
            name: "notify",
            arg_count: 2,
            signature: "fn(str, str) -> Result[str]",
            runtime_symbol: Some("vox_actor_runtime::builtins::vox_openclaw_notify"),
            arg_kinds: &[],
            returns_unit: false,
        },
        // Chromium / CDP — native scripts only (see codegen `wasm32` guard).
        BuiltinRegistryEntry {
            namespace: "Browser",
            name: "open",
            arg_count: 2,
            signature: "fn(str, bool) -> Result[str]",
            runtime_symbol: Some("vox_actor_runtime::builtins::vox_browser_open"),
            arg_kinds: &[BuiltinArgKind::Str, BuiltinArgKind::Bool],
            returns_unit: false,
        },
        BuiltinRegistryEntry {
            namespace: "Browser",
            name: "close",
            arg_count: 1,
            signature: "fn(str) -> Result[unit]",
            runtime_symbol: Some("vox_actor_runtime::builtins::vox_browser_close"),
            arg_kinds: &[],
            returns_unit: true,
        },
        BuiltinRegistryEntry {
            namespace: "Browser",
            name: "goto",
            arg_count: 2,
            signature: "fn(str, str) -> Result[unit]",
            runtime_symbol: Some("vox_actor_runtime::builtins::vox_browser_goto"),
            arg_kinds: &[],
            returns_unit: true,
        },
        BuiltinRegistryEntry {
            namespace: "Browser",
            name: "click",
            arg_count: 2,
            signature: "fn(str, str) -> Result[unit]",
            runtime_symbol: Some("vox_actor_runtime::builtins::vox_browser_click"),
            arg_kinds: &[],
            returns_unit: true,
        },
        BuiltinRegistryEntry {
            namespace: "Browser",
            name: "fill",
            arg_count: 3,
            signature: "fn(str, str, str) -> Result[unit]",
            runtime_symbol: Some("vox_actor_runtime::builtins::vox_browser_fill"),
            arg_kinds: &[],
            returns_unit: true,
        },
        BuiltinRegistryEntry {
            namespace: "Browser",
            name: "wait_for",
            arg_count: 3,
            signature: "fn(str, str, int) -> Result[unit]",
            runtime_symbol: Some("vox_actor_runtime::builtins::vox_browser_wait_for"),
            arg_kinds: &[
                BuiltinArgKind::Str,
                BuiltinArgKind::Str,
                BuiltinArgKind::Int,
            ],
            returns_unit: true,
        },
        BuiltinRegistryEntry {
            namespace: "Browser",
            name: "text",
            arg_count: 2,
            signature: "fn(str, str) -> Result[str]",
            runtime_symbol: Some("vox_actor_runtime::builtins::vox_browser_text"),
            arg_kinds: &[],
            returns_unit: false,
        },
        BuiltinRegistryEntry {
            namespace: "Browser",
            name: "html",
            arg_count: 2,
            signature: "fn(str, str) -> Result[str]",
            runtime_symbol: Some("vox_actor_runtime::builtins::vox_browser_html"),
            arg_kinds: &[],
            returns_unit: false,
        },
        BuiltinRegistryEntry {
            namespace: "Browser",
            name: "screenshot",
            arg_count: 2,
            signature: "fn(str, str) -> Result[str]",
            runtime_symbol: Some("vox_actor_runtime::builtins::vox_browser_screenshot"),
            arg_kinds: &[],
            returns_unit: false,
        },
        BuiltinRegistryEntry {
            namespace: "std.mobile",
            name: "notify",
            arg_count: 2,
            signature: "fn(str, str) -> Result[unit]",
            runtime_symbol: None,
            arg_kinds: &[],
            returns_unit: true,
        },
        BuiltinRegistryEntry {
            namespace: "std.mobile",
            name: "get_location",
            arg_count: 0,
            signature: "fn() -> Result[str]",
            runtime_symbol: None,
            arg_kinds: &[],
            returns_unit: false,
        },
        BuiltinRegistryEntry {
            namespace: "std.mobile",
            name: "vibrate",
            arg_count: 0,
            signature: "fn() -> Result[unit]",
            runtime_symbol: None,
            arg_kinds: &[],
            returns_unit: true,
        },
        BuiltinRegistryEntry {
            namespace: "std.mobile",
            name: "take_photo",
            arg_count: 0,
            signature: "fn() -> Result[str]",
            runtime_symbol: None,
            arg_kinds: &[],
            returns_unit: false,
        },
        BuiltinRegistryEntry {
            namespace: "std.mobile",
            name: "take_photo_from_gallery",
            arg_count: 0,
            signature: "fn() -> Result[str]",
            runtime_symbol: None,
            arg_kinds: &[],
            returns_unit: false,
        },
        BuiltinRegistryEntry {
            namespace: "std.mobile",
            name: "transcribe_microphone",
            arg_count: 0,
            signature: "fn() -> Result[str]",
            runtime_symbol: None,
            arg_kinds: &[],
            returns_unit: false,
        },
        BuiltinRegistryEntry {
            namespace: "std.mobile",
            name: "accelerometer",
            arg_count: 0,
            signature: "fn() -> Result[str]",
            runtime_symbol: None,
            arg_kinds: &[],
            returns_unit: false,
        },
        BuiltinRegistryEntry {
            namespace: "std.mobile",
            name: "platform",
            arg_count: 0,
            signature: "fn() -> str",
            runtime_symbol: None,
            arg_kinds: &[],
            returns_unit: false,
        },
        BuiltinRegistryEntry {
            namespace: "std.mobile",
            name: "has_camera",
            arg_count: 0,
            signature: "fn() -> bool",
            runtime_symbol: None,
            arg_kinds: &[],
            returns_unit: false,
        },
        BuiltinRegistryEntry {
            namespace: "std.mobile",
            name: "copy_to_clipboard",
            arg_count: 1,
            signature: "fn(str) -> unit",
            runtime_symbol: None,
            arg_kinds: &[],
            returns_unit: true,
        },
        BuiltinRegistryEntry {
            namespace: "std.mobile",
            name: "read_clipboard",
            arg_count: 0,
            signature: "fn() -> Result[str]",
            runtime_symbol: None,
            arg_kinds: &[],
            returns_unit: false,
        },
        BuiltinRegistryEntry {
            namespace: "std.mobile",
            name: "biometric_auth",
            arg_count: 1,
            signature: "fn(str) -> Result[bool]",
            runtime_symbol: None,
            arg_kinds: &[],
            returns_unit: false,
        },
        BuiltinRegistryEntry {
            namespace: "std.mobile",
            name: "read_contacts",
            arg_count: 0,
            signature: "fn() -> Result[str]", // Resolving contacts as JSON str representation
            runtime_symbol: None,
            arg_kinds: &[],
            returns_unit: false,
        },
        BuiltinRegistryEntry {
            namespace: "std.mobile",
            name: "share_text",
            arg_count: 1,
            signature: "fn(str) -> Result[bool]",
            runtime_symbol: None,
            arg_kinds: &[],
            returns_unit: false,
        },
        BuiltinRegistryEntry {
            namespace: "std.mobile",
            name: "store_file",
            arg_count: 2,
            signature: "fn(str, str) -> Result[bool]",
            runtime_symbol: None,
            arg_kinds: &[],
            returns_unit: false,
        },
        BuiltinRegistryEntry {
            namespace: "std.mobile",
            name: "read_file",
            arg_count: 1,
            signature: "fn(str) -> Result[str]",
            runtime_symbol: None,
            arg_kinds: &[],
            returns_unit: false,
        },
        BuiltinRegistryEntry {
            namespace: "std.mobile.push",
            name: "register",
            arg_count: 0,
            signature: "fn() -> Result[str]",
            runtime_symbol: None,
            arg_kinds: &[],
            returns_unit: false,
        },
        BuiltinRegistryEntry {
            namespace: "std.mobile.push",
            name: "on_message",
            arg_count: 1,
            signature: "fn(fn(str)) -> unit",
            runtime_symbol: None,
            arg_kinds: &[],
            returns_unit: true,
        },
    ]
}

#[must_use]
pub fn lookup_builtin(
    namespace: &str,
    name: &str,
    arg_count: usize,
) -> Option<BuiltinRegistryEntry> {
    builtin_registry_entries()
        .iter()
        .copied()
        .find(|e| e.namespace == namespace && e.name == name && e.arg_count == arg_count)
}

fn file_record_ty() -> Ty {
    Ty::Record(vec![
        ("name".into(), Ty::Str),
        ("path".into(), Ty::Str),
        ("size".into(), Ty::Int),
        ("modified_ms".into(), Ty::Int),
        ("is_dir".into(), Ty::Bool),
        ("is_file".into(), Ty::Bool),
        ("is_symlink".into(), Ty::Bool),
    ])
}

/// `std.<field>` type members on the root namespace.
#[must_use]
pub fn std_root_field_ty(field: &str) -> Option<Ty> {
    Some(match field {
        "fs" => Ty::Named("StdFsNs".into()),
        "path" => Ty::Named("StdPathNs".into()),
        "env" => Ty::Named("StdEnvNs".into()),
        "process" => Ty::Named("StdProcessNs".into()),
        "csv" => Ty::Named("StdCsvNs".into()),
        "toml" => Ty::Named("StdTomlNs".into()),
        "yaml" => Ty::Named("StdYamlNs".into()),
        "io" => Ty::Named("StdIoNs".into()),
        "json" => Ty::Named("StdJsonNs".into()),
        "http" => Ty::Named("StdHttpNs".into()),
        "crypto" => Ty::Named("StdCryptoNs".into()),
        "time" => Ty::Named("StdTimeNs".into()),
        "log" => Ty::Named("StdLogNs".into()),
        "mobile" => Ty::Named("StdMobileNs".into()),
        "regex" => Ty::Named("StdRegexNs".into()),
        "agentos" => Ty::Named("StdAgentosNs".into()),
        "uuid" => Ty::Fn(vec![], Box::new(Ty::Str)),
        "now_ms" => Ty::Fn(vec![], Box::new(Ty::Int)),
        "hash_fast" | "hash_secure" => Ty::Fn(vec![Ty::Str], Box::new(Ty::Str)),
        "args" => Ty::List(Box::new(Ty::Str)),
        _ => return None,
    })
}

/// `std.<namespace>.<method>` signatures used by type checking.
#[must_use]
pub fn std_namespace_method_ty(namespace: &str, method: &str) -> Option<Ty> {
    Some(match (namespace, method) {
        ("fs", "read")
        | ("fs", "read_file")
        | ("fs", "read_to_string")
        | ("fs", "remove")
        | ("fs", "mkdir") => Ty::Fn(vec![Ty::Str], Box::new(Ty::Result(Box::new(Ty::Str)))),
        ("fs", "read_bytes") => Ty::Fn(vec![Ty::Str], Box::new(Ty::Result(Box::new(Ty::Str)))),
        ("fs", "write") | ("fs", "write_file") | ("fs", "write_to_file") => Ty::Fn(
            vec![Ty::Str, Ty::Str],
            Box::new(Ty::Result(Box::new(Ty::Unit))),
        ),
        ("fs", "cwd") => Ty::Fn(vec![], Box::new(Ty::Result(Box::new(Ty::Str)))),
        ("fs", "walk") | ("fs", "list_recursive") => Ty::Fn(
            vec![Ty::Str],
            Box::new(Ty::Result(Box::new(Ty::List(Box::new(Ty::Str))))),
        ),
        ("fs", "exists") => Ty::Fn(vec![Ty::Str], Box::new(Ty::Bool)),
        ("fs", "is_file") | ("fs", "is_dir") => Ty::Fn(vec![Ty::Str], Box::new(Ty::Bool)),
        ("fs", "canonicalize") => Ty::Fn(vec![Ty::Str], Box::new(Ty::Result(Box::new(Ty::Str)))),
        ("fs", "list_dir") | ("fs", "glob") => Ty::Fn(
            vec![Ty::Str],
            Box::new(Ty::Result(Box::new(Ty::List(Box::new(Ty::Str))))),
        ),
        ("fs", "list_dir_detailed") => Ty::Fn(
            vec![Ty::Str],
            Box::new(Ty::Result(Box::new(Ty::List(Box::new(file_record_ty()))))),
        ),
        ("fs", "stat") => Ty::Fn(
            vec![Ty::Str],
            Box::new(Ty::Result(Box::new(file_record_ty()))),
        ),
        ("fs", "remove_dir_all") => Ty::Fn(vec![Ty::Str], Box::new(Ty::Result(Box::new(Ty::Unit)))),
        ("fs", "copy") => Ty::Fn(
            vec![Ty::Str, Ty::Str],
            Box::new(Ty::Result(Box::new(Ty::Unit))),
        ),
        ("path", "join") => Ty::Fn(vec![Ty::Str, Ty::Str], Box::new(Ty::Str)),
        ("path", "join_many") => Ty::Fn(vec![Ty::List(Box::new(Ty::Str))], Box::new(Ty::Str)),
        ("path", "basename") | ("path", "dirname") | ("path", "extension") => {
            Ty::Fn(vec![Ty::Str], Box::new(Ty::Str))
        }
        // Phase K typeck signatures (2026-05-23) — match the actor-runtime
        // Option-returning wrappers landed in vox-actor-runtime. Strict-Option
        // discipline keeps the interp + script surfaces aligned.
        ("path", "parent") | ("path", "file_name") | ("path", "stem") => {
            Ty::Fn(vec![Ty::Str], Box::new(Ty::Option(Box::new(Ty::Str))))
        }
        ("path", "is_absolute") => Ty::Fn(vec![Ty::Str], Box::new(Ty::Bool)),
        ("path", "resolve") => Ty::Fn(vec![Ty::Str], Box::new(Ty::Result(Box::new(Ty::Str)))),
        ("env", "get") => Ty::Fn(vec![Ty::Str], Box::new(Ty::Option(Box::new(Ty::Str)))),
        ("env", "args") => Ty::Fn(vec![], Box::new(Ty::List(Box::new(Ty::Str)))),
        ("env", "set") => Ty::Fn(vec![Ty::Str, Ty::Str], Box::new(Ty::Unit)),
        // Match the typeck/builtins.rs RegexModule shape AND the eval
        // behavior (bare Str; compile errors silently discarded). The
        // actor-runtime wrapper landed in Phase K returns Result, but
        // since corpus convention is "discard the error" we keep typeck
        // aligned to Str — change all three sides together if this
        // semantics ever evolves.
        // regex.replace(haystack, pattern, replacement) → str
        ("regex", "replace") => Ty::Fn(vec![Ty::Str, Ty::Str, Ty::Str], Box::new(Ty::Str)),
        // regex.find(haystack, pattern) → Option[str]  (first match substring)
        ("regex", "find") => Ty::Fn(
            vec![Ty::Str, Ty::Str],
            Box::new(Ty::Option(Box::new(Ty::Str))),
        ),
        // regex.is_match(haystack, pattern) → bool
        ("regex", "is_match") => Ty::Fn(vec![Ty::Str, Ty::Str], Box::new(Ty::Bool)),
        // regex.captures(haystack, pattern) → Option[list[str]]
        ("regex", "captures") => Ty::Fn(
            vec![Ty::Str, Ty::Str],
            Box::new(Ty::Option(Box::new(Ty::List(Box::new(Ty::Str))))),
        ),
        ("process", "which") => Ty::Fn(vec![Ty::Str], Box::new(Ty::Option(Box::new(Ty::Str)))),
        ("process", "run") => Ty::Fn(
            vec![Ty::Str, Ty::List(Box::new(Ty::Str))],
            Box::new(Ty::Result(Box::new(Ty::Int))),
        ),
        ("process", "run_ex") => Ty::Fn(
            vec![
                Ty::Str,
                Ty::List(Box::new(Ty::Str)),
                Ty::Str,
                Ty::List(Box::new(Ty::Str)),
            ],
            Box::new(Ty::Result(Box::new(Ty::Int))),
        ),
        ("process", "run_capture") => Ty::Fn(
            vec![Ty::Str, Ty::List(Box::new(Ty::Str))],
            Box::new(Ty::Result(Box::new(Ty::Record(vec![
                ("exit".into(), Ty::Int),
                ("stdout".into(), Ty::Str),
                ("stderr".into(), Ty::Str),
            ])))),
        ),
        ("process", "run_capture_ex") => Ty::Fn(
            vec![
                Ty::Str,
                Ty::List(Box::new(Ty::Str)),
                Ty::Str,
                Ty::List(Box::new(Ty::Str)),
            ],
            Box::new(Ty::Result(Box::new(Ty::Record(vec![
                ("exit".into(), Ty::Int),
                ("stdout".into(), Ty::Str),
                ("stderr".into(), Ty::Str),
            ])))),
        ),
        ("process", "run_capture_json") => Ty::Fn(
            vec![Ty::Str, Ty::List(Box::new(Ty::Str))],
            Box::new(Ty::Result(Box::new(Ty::Named("Json".into())))),
        ),
        ("process", "run_capture_lines") => Ty::Fn(
            vec![Ty::Str, Ty::List(Box::new(Ty::Str))],
            Box::new(Ty::Result(Box::new(Ty::List(Box::new(Ty::Str))))),
        ),
        ("process", "spawn_background") => Ty::Fn(
            vec![Ty::Str, Ty::List(Box::new(Ty::Str))],
            Box::new(Ty::Result(Box::new(Ty::Int))),
        ),
        ("process", "exec") => Ty::Fn(
            vec![Ty::Str, Ty::List(Box::new(Ty::Str))],
            Box::new(Ty::Result(Box::new(Ty::Unit))),
        ),
        ("process", "register_exit_command") => Ty::Fn(
            vec![Ty::Str, Ty::List(Box::new(Ty::Str))],
            Box::new(Ty::Result(Box::new(Ty::Unit))),
        ),
        ("process", "exit") => Ty::Fn(vec![Ty::Int], Box::new(Ty::Never)),
        ("csv", "parse") => Ty::Fn(
            vec![Ty::Str],
            Box::new(Ty::Result(Box::new(Ty::Named("Json".into())))),
        ),
        ("csv", "parse_records") => Ty::Fn(
            vec![Ty::Str],
            Box::new(Ty::Result(Box::new(Ty::Named("Json".into())))),
        ),
        ("csv", "render") => Ty::Fn(
            vec![Ty::List(Box::new(Ty::List(Box::new(Ty::Str))))],
            Box::new(Ty::Result(Box::new(Ty::Str))),
        ),
        ("toml", "parse") => Ty::Fn(
            vec![Ty::Str],
            Box::new(Ty::Result(Box::new(Ty::Named("Json".into())))),
        ),
        ("toml", "render") => Ty::Fn(
            vec![Ty::GenericParam(0)],
            Box::new(Ty::Result(Box::new(Ty::Str))),
        ),
        ("yaml", "parse") => Ty::Fn(
            vec![Ty::Str],
            Box::new(Ty::Result(Box::new(Ty::Named("Json".into())))),
        ),
        ("yaml", "render") => Ty::Fn(
            vec![Ty::GenericParam(0)],
            Box::new(Ty::Result(Box::new(Ty::Str))),
        ),
        ("io", "open") => Ty::Fn(
            vec![Ty::Str],
            Box::new(Ty::Result(Box::new(Ty::Named("Json".into())))),
        ),
        ("io", "save") => Ty::Fn(
            vec![Ty::Str, Ty::GenericParam(0)],
            Box::new(Ty::Result(Box::new(Ty::Unit))),
        ),
        ("json", "render") => Ty::Fn(
            vec![Ty::GenericParam(0)],
            Box::new(Ty::Result(Box::new(Ty::Str))),
        ),
        ("json", "parse") => Ty::Fn(
            vec![Ty::Str],
            Box::new(Ty::Result(Box::new(Ty::Named("Json".into())))),
        ),
        ("json", "read_str") => Ty::Fn(
            vec![Ty::Str, Ty::Str],
            Box::new(Ty::Result(Box::new(Ty::Str))),
        ),
        ("json", "read_f64") => Ty::Fn(
            vec![Ty::Str, Ty::Str],
            Box::new(Ty::Result(Box::new(Ty::Float))),
        ),
        ("json", "quote") => Ty::Fn(vec![Ty::Str], Box::new(Ty::Str)),
        ("http", "get_text") => Ty::Fn(vec![Ty::Str], Box::new(Ty::Result(Box::new(Ty::Str)))),
        ("http", "post_json") => Ty::Fn(
            vec![Ty::Str, Ty::Str],
            Box::new(Ty::Result(Box::new(Ty::Str))),
        ),
        ("crypto", "hash_fast") | ("crypto", "hash_secure") => {
            Ty::Fn(vec![Ty::Str], Box::new(Ty::Str))
        }
        ("crypto", "uuid") => Ty::Fn(vec![], Box::new(Ty::Str)),
        ("time", "now_ms") => Ty::Fn(vec![], Box::new(Ty::Int)),
        ("log", "debug") | ("log", "info") | ("log", "warn") | ("log", "error") => {
            Ty::Fn(vec![Ty::Str], Box::new(Ty::Unit))
        }
        ("regex", "compile") => Ty::Fn(
            vec![Ty::Str],
            Box::new(Ty::Result(Box::new(Ty::Named("Regex".into())))),
        ),
        ("agentos", "mutation_kind_for_tool") => Ty::Fn(vec![Ty::Str], Box::new(Ty::Str)),
        _ => return None,
    })
}

/// Shared runtime call lowering for `std.<namespace>.<method>` in Rust codegen.
#[must_use]
pub fn std_namespace_runtime_call(
    namespace: &str,
    method: &str,
    args: &[String],
) -> Option<String> {
    match (namespace, method) {
        ("crypto", "hash_fast") if !args.is_empty() => Some(format!(
            "vox_actor_runtime::builtins::vox_hash_fast(&{})",
            args[0]
        )),
        ("crypto", "hash_secure") if !args.is_empty() => Some(format!(
            "vox_actor_runtime::builtins::vox_hash_secure(&{})",
            args[0]
        )),
        ("crypto", "uuid") => Some("vox_actor_runtime::builtins::vox_uuid()".to_string()),
        ("time", "now_ms") => Some("vox_actor_runtime::builtins::vox_now_ms()".to_string()),
        ("log", "debug") if !args.is_empty() => Some(format!(
            "vox_actor_runtime::builtins::vox_log_debug(({}).as_str())",
            args[0]
        )),
        ("log", "info") if !args.is_empty() => Some(format!(
            "vox_actor_runtime::builtins::vox_log_info(({}).as_str())",
            args[0]
        )),
        ("log", "warn") if !args.is_empty() => Some(format!(
            "vox_actor_runtime::builtins::vox_log_warn(({}).as_str())",
            args[0]
        )),
        ("log", "error") if !args.is_empty() => Some(format!(
            "vox_actor_runtime::builtins::vox_log_error(({}).as_str())",
            args[0]
        )),
        ("fs", "read") if !args.is_empty() => Some(format!(
            "::vox_actor_runtime::builtins::vox_fs_read(({}).as_str())",
            args[0]
        )),
        ("fs", "write") if args.len() >= 2 => Some(format!(
            "::vox_actor_runtime::builtins::vox_fs_write(({}).as_str(), ({}).as_str())",
            args[0], args[1]
        )),
        ("fs", "exists") if !args.is_empty() => {
            Some(format!("std::path::Path::new(&{}).exists()", args[0]))
        }
        ("fs", "is_file") if !args.is_empty() => {
            Some(format!("std::path::Path::new(&{}).is_file()", args[0]))
        }
        ("fs", "is_dir") if !args.is_empty() => {
            Some(format!("std::path::Path::new(&{}).is_dir()", args[0]))
        }
        ("fs", "canonicalize") if !args.is_empty() => Some(format!(
            "::std::fs::canonicalize({}).map(|p| p.to_string_lossy().to_string()).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)",
            args[0]
        )),
        ("fs", "remove") if !args.is_empty() => Some(format!(
            "::std::fs::remove_file({}).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)",
            args[0]
        )),
        ("fs", "read_bytes") if !args.is_empty() => Some(format!(
            "::std::fs::read({}).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)",
            args[0]
        )),
        ("fs", "mkdir") if !args.is_empty() => Some(format!(
            "::vox_actor_runtime::builtins::vox_fs_mkdir(({}).as_str())",
            args[0]
        )),
        ("fs", "glob") if !args.is_empty() => Some(format!(
            "::vox_actor_runtime::builtins::vox_fs_glob(({}).as_str())",
            args[0]
        )),
        ("path", "join") if args.len() >= 2 => Some(format!(
            "std::path::Path::new(&{}).join(&{}).to_string_lossy().to_string()",
            args[0], args[1]
        )),
        ("path", "basename") if !args.is_empty() => Some(format!(
            "std::path::Path::new(&{}).file_name().unwrap_or_default().to_string_lossy().to_string()",
            args[0]
        )),
        ("path", "dirname") if !args.is_empty() => Some(format!(
            "std::path::Path::new(&{}).parent().unwrap_or(std::path::Path::new(\".\")).to_string_lossy().to_string()",
            args[0]
        )),
        ("path", "extension") if !args.is_empty() => Some(format!(
            "std::path::Path::new(&{}).extension().unwrap_or_default().to_string_lossy().to_string()",
            args[0]
        )),
        // Phase K codegen wire-up (2026-05-23) — dispatches the remaining
        // path/env/regex primitives landed in actor-runtime so --mode script
        // reaches feature parity with --mode interp. These map to the
        // `vox_*` functions in `crates/vox-actor-runtime/src/builtins/mod.rs`
        // (Phase K block right after `vox_fs_mkdir`).
        ("path", "parent") if !args.is_empty() => Some(format!(
            "(vox_actor_runtime::builtins::vox_path_parent(({}).as_str()))",
            args[0]
        )),
        ("path", "file_name") if !args.is_empty() => Some(format!(
            "(vox_actor_runtime::builtins::vox_path_file_name(({}).as_str()))",
            args[0]
        )),
        ("path", "stem") if !args.is_empty() => Some(format!(
            "(vox_actor_runtime::builtins::vox_path_stem(({}).as_str()))",
            args[0]
        )),
        ("path", "is_absolute") if !args.is_empty() => Some(format!(
            "(vox_actor_runtime::builtins::vox_path_is_absolute(({}).as_str()))",
            args[0]
        )),
        ("path", "resolve") if !args.is_empty() => Some(format!(
            "(vox_actor_runtime::builtins::vox_path_resolve(({}).as_str()))",
            args[0]
        )),
        ("env", "get") if !args.is_empty() => Some(format!(
            "(vox_actor_runtime::builtins::vox_env_get(({}).as_str()))",
            args[0]
        )),
        ("env", "args") => Some("vox_actor_runtime::builtins::vox_env_args()".to_string()),
        ("env", "set") if args.len() >= 2 => Some(format!(
            "{{ vox_actor_runtime::builtins::vox_env_set(({}).as_str(), ({}).as_str()); }}",
            args[0], args[1]
        )),
        // regex.replace(haystack, pattern, replacement) → str
        ("regex", "replace") if args.len() >= 3 => Some(format!(
            "(vox_actor_runtime::builtins::vox_regex_replace(({}).as_str(), ({}).as_str(), ({}).as_str()).unwrap_or_default())",
            args[0], args[1], args[2]
        )),
        // regex.find(haystack, pattern) → Option[str]
        ("regex", "find") if args.len() >= 2 => Some(format!(
            "(vox_actor_runtime::builtins::vox_regex_find(({}).as_str(), ({}).as_str()).ok().flatten())",
            args[0], args[1]
        )),
        // regex.is_match(haystack, pattern) → bool
        ("regex", "is_match") if args.len() >= 2 => Some(format!(
            "(vox_actor_runtime::builtins::vox_regex_is_match(({}).as_str(), ({}).as_str()))",
            args[0], args[1]
        )),
        // regex.captures(haystack, pattern) → Option[list[str]]
        ("regex", "captures") if args.len() >= 2 => Some(format!(
            "(vox_actor_runtime::builtins::vox_regex_captures(({}).as_str(), ({}).as_str()))",
            args[0], args[1]
        )),
        ("process", "which") if !args.is_empty() => Some(format!(
            "(vox_actor_runtime::builtins::vox_process_which(({}).as_str()))",
            args[0]
        )),
        ("process", "run") if args.len() >= 2 => Some(format!(
            "(match vox_actor_runtime::builtins::vox_process_run(({}).as_str(), {}.as_slice()) {{ Ok(c) => Ok(c as i64), Err(m) => Error(m) }})",
            args[0], args[1]
        )),
        ("process", "run_ex") if args.len() >= 4 => Some(format!(
            "(match vox_actor_runtime::builtins::vox_process_run_ex(({}).as_str(), {}.as_slice(), ({}).as_str(), {}.as_slice()) {{ Ok(c) => Ok(c as i64), Err(m) => Error(m) }})",
            args[0], args[1], args[2], args[3]
        )),
        ("process", "run_capture") if args.len() >= 2 => Some(format!(
            "(match vox_actor_runtime::builtins::vox_process_run_capture(({}).as_str(), {}.as_slice()) {{ Ok(p) => Ok(serde_json::json!({{ \"exit\": p.exit as i64, \"stdout\": p.stdout, \"stderr\": p.stderr }})), Err(m) => Error(m) }})",
            args[0], args[1]
        )),
        ("process", "run_capture_ex") if args.len() >= 4 => Some(format!(
            "(match vox_actor_runtime::builtins::vox_process_run_capture_ex(({}).as_str(), {}.as_slice(), ({}).as_str(), {}.as_slice()) {{ Ok(p) => Ok(serde_json::json!({{ \"exit\": p.exit as i64, \"stdout\": p.stdout, \"stderr\": p.stderr }})), Err(m) => Error(m) }})",
            args[0], args[1], args[2], args[3]
        )),
        ("process", "run_capture_json") if args.len() >= 2 => Some(format!(
            "(match ::vox_actor_runtime::builtins::vox_process_run_capture_json(({}).as_str(), {}.as_slice()) {{ Ok(v) => Ok(v), Err(m) => Error(m) }})",
            args[0], args[1]
        )),
        ("process", "run_capture_lines") if args.len() >= 2 => Some(format!(
            "(match ::vox_actor_runtime::builtins::vox_process_run_capture_lines(({}).as_str(), {}.as_slice()) {{ Ok(v) => Ok(v), Err(m) => Error(m) }})",
            args[0], args[1]
        )),
        ("process", "spawn_background") if args.len() >= 2 => Some(format!(
            "(match vox_actor_runtime::builtins::vox_process_spawn_background(({}).as_str(), {}.as_slice()) {{ Ok(id) => Ok(id), Err(m) => Error(m) }})",
            args[0], args[1]
        )),
        ("process", "exec") if args.len() >= 2 => Some(format!(
            "(match vox_actor_runtime::builtins::vox_process_exec(({}).as_str(), {}.as_slice()) {{ Ok(()) => Ok(()), Err(m) => Error(m) }})",
            args[0], args[1]
        )),
        ("process", "register_exit_command") if args.len() >= 2 => Some(format!(
            "(match vox_actor_runtime::builtins::vox_process_register_exit_command(({}).as_str(), {}.as_slice()) {{ Ok(()) => Ok(()), Err(m) => Error(m) }})",
            args[0], args[1]
        )),
        ("process", "exit") if !args.is_empty() => {
            Some(format!("{{ std::process::exit({} as i32) }}", args[0]))
        }
        ("fs", "list_dir_detailed") if !args.is_empty() => Some(format!(
            "(match ::vox_actor_runtime::builtins::vox_fs_list_dir_detailed(({}).as_str()) {{ Ok(rows) => Ok(rows.into_iter().map(|r| serde_json::json!({{ \"name\": r.name, \"path\": r.path, \"size\": r.size, \"modified_ms\": r.modified_ms, \"is_dir\": r.is_dir, \"is_file\": r.is_file, \"is_symlink\": r.is_symlink }})).collect::<Vec<serde_json::Value>>()), Err(m) => Error(m) }})",
            args[0]
        )),
        ("fs", "stat") if !args.is_empty() => Some(format!(
            "(match ::vox_actor_runtime::builtins::vox_fs_stat(({}).as_str()) {{ Ok(r) => Ok(serde_json::json!({{ \"name\": r.name, \"path\": r.path, \"size\": r.size, \"modified_ms\": r.modified_ms, \"is_dir\": r.is_dir, \"is_file\": r.is_file, \"is_symlink\": r.is_symlink }})), Err(m) => Error(m) }})",
            args[0]
        )),
        ("fs", "list_dir") if !args.is_empty() => Some(format!(
            "(match vox_actor_runtime::builtins::vox_list_dir(({}).as_str()) {{ Ok(v) => Ok(v), Err(m) => Error(m) }})",
            args[0]
        )),
        ("fs", "glob") if !args.is_empty() => Some(format!(
            "(match vox_actor_runtime::builtins::vox_fs_glob(({}).as_str()) {{ Ok(v) => Ok(v), Err(m) => Error(m) }})",
            args[0]
        )),
        ("fs", "remove_dir_all") if !args.is_empty() => Some(format!(
            "(match vox_actor_runtime::builtins::vox_fs_remove_dir_all(({}).as_str()) {{ Ok(()) => Ok(()), Err(m) => Error(m) }})",
            args[0]
        )),
        ("fs", "copy") if args.len() >= 2 => Some(format!(
            "(match vox_actor_runtime::builtins::vox_fs_copy(({}).as_str(), ({}).as_str()) {{ Ok(()) => Ok(()), Err(m) => Error(m) }})",
            args[0], args[1]
        )),
        ("path", "join_many") if !args.is_empty() => Some(format!(
            "vox_actor_runtime::builtins::vox_path_join_many({}.as_slice())",
            args[0]
        )),
        ("json", "read_str") if args.len() >= 2 => Some(format!(
            "(match vox_actor_runtime::builtins::vox_json_read_str(({}).as_str(), ({}).as_str()) {{ Ok(s) => Ok(s), Err(m) => Error(m) }})",
            args[0], args[1]
        )),
        ("json", "read_f64") if args.len() >= 2 => Some(format!(
            "(match vox_actor_runtime::builtins::vox_json_read_f64(({}).as_str(), ({}).as_str()) {{ Ok(v) => Ok(v), Err(m) => Error(m) }})",
            args[0], args[1]
        )),
        ("json", "quote") if !args.is_empty() => Some(format!(
            "vox_actor_runtime::builtins::vox_json_quote(({}).as_str())",
            args[0]
        )),
        ("http", "get_text") if !args.is_empty() => Some(format!(
            "({{ #[cfg(target_arch = \"wasm32\")] {{ Error(\"std.http.get_text is not supported in WASI scripts\".to_string()) }} #[cfg(not(target_arch = \"wasm32\"))] {{ match vox_actor_runtime::builtins::vox_http_get_text(({}).as_str()) {{ Ok(s) => Ok(s), Err(m) => Error(m) }} }} }})",
            args[0]
        )),
        ("http", "post_json") if args.len() >= 2 => Some(format!(
            "({{ #[cfg(target_arch = \"wasm32\")] {{ Error(\"std.http.post_json is not supported in WASI scripts\".to_string()) }} #[cfg(not(target_arch = \"wasm32\"))] {{ match vox_actor_runtime::builtins::vox_http_post_json(({}).as_str(), ({}).as_str()) {{ Ok(s) => Ok(s), Err(m) => Error(m) }} }} }})",
            args[0], args[1]
        )),
        ("regex", "compile") if !args.is_empty() => Some(format!(
            "(match vox_actor_runtime::builtins::vox_regex_compile(({}).as_str()) {{ Ok(r) => Ok(r), Err(m) => Error(m) }})",
            args[0]
        )),
        ("csv", "parse") if !args.is_empty() => Some(format!(
            "(match ::vox_actor_runtime::builtins::vox_csv_parse(({}).as_str()) {{ Ok(v) => Ok(v), Err(m) => Error(m) }})",
            args[0]
        )),
        ("csv", "parse_records") if !args.is_empty() => Some(format!(
            "(match ::vox_actor_runtime::builtins::vox_csv_parse_records(({}).as_str()) {{ Ok(v) => Ok(v), Err(m) => Error(m) }})",
            args[0]
        )),
        ("csv", "render") if !args.is_empty() => Some(format!(
            "(match ::vox_actor_runtime::builtins::vox_csv_render({}.as_slice()) {{ Ok(s) => Ok(s), Err(m) => Error(m) }})",
            args[0]
        )),
        ("toml", "parse") if !args.is_empty() => Some(format!(
            "(match ::vox_actor_runtime::builtins::vox_toml_parse(({}).as_str()) {{ Ok(v) => Ok(v), Err(m) => Error(m) }})",
            args[0]
        )),
        ("toml", "render") if !args.is_empty() => Some(format!(
            "(match ::vox_actor_runtime::builtins::vox_toml_render(&({})) {{ Ok(s) => Ok(s), Err(m) => Error(m) }})",
            args[0]
        )),
        ("yaml", "parse") if !args.is_empty() => Some(format!(
            "(match ::vox_actor_runtime::builtins::vox_yaml_parse(({}).as_str()) {{ Ok(v) => Ok(v), Err(m) => Error(m) }})",
            args[0]
        )),
        ("yaml", "render") if !args.is_empty() => Some(format!(
            "(match ::vox_actor_runtime::builtins::vox_yaml_render(&({})) {{ Ok(s) => Ok(s), Err(m) => Error(m) }})",
            args[0]
        )),
        ("io", "open") if !args.is_empty() => Some(format!(
            "(match ::vox_actor_runtime::builtins::vox_io_open(({}).as_str()) {{ Ok(v) => Ok(v), Err(m) => Error(m) }})",
            args[0]
        )),
        ("io", "save") if args.len() >= 2 => Some(format!(
            "(match ::vox_actor_runtime::builtins::vox_io_save(({}).as_str(), &({})) {{ Ok(()) => Ok(()), Err(m) => Error(m) }})",
            args[0], args[1]
        )),
        ("json", "render") if !args.is_empty() => Some(format!(
            "(match vox_actor_runtime::builtins::vox_json_render(&({})) {{ Ok(s) => Ok(s), Err(m) => Error(m) }})",
            args[0]
        )),
        ("json", "parse") if !args.is_empty() => Some(format!(
            "(match vox_actor_runtime::builtins::vox_json_parse(({}).as_str()) {{ Ok(j) => Ok(j), Err(m) => Error(m) }})",
            args[0]
        )),
        ("agentos", "mutation_kind_for_tool") if !args.is_empty() => Some(format!(
            "vox_actor_runtime::builtins::vox_foundation::primitives::agentos_mutation_kind_for_tool(({}).as_str())",
            args[0]
        )),
        _ => None,
    }
}

#[cfg(test)]
mod agentos_std_surface_tests {
    use super::{std_namespace_method_ty, std_namespace_runtime_call, std_root_field_ty};
    use crate::typeck::ty::Ty;

    #[test]
    fn std_agentos_namespace_and_method_wired() {
        assert!(matches!(
            std_root_field_ty("agentos"),
            Some(Ty::Named(n)) if n == "StdAgentosNs"
        ));
        assert!(std_namespace_method_ty("agentos", "mutation_kind_for_tool").is_some());
        let rust = std_namespace_runtime_call(
            "agentos",
            "mutation_kind_for_tool",
            &["tool_arg".to_string()],
        )
        .expect("runtime lowering");
        assert!(
            rust.contains("vox_foundation::primitives::agentos_mutation_kind_for_tool"),
            "{rust}"
        );
    }
}

#[cfg(test)]
mod browser_registry_tests {
    use super::builtin_registry_entries;

    #[test]
    fn browser_builtins_map_to_vox_runtime() {
        let browser: Vec<_> = builtin_registry_entries()
            .iter()
            .copied()
            .filter(|e| e.namespace == "Browser")
            .collect();
        assert_eq!(
            browser.len(),
            9,
            "Browser registry size drift (update typeck + runtime if intentional)"
        );
        for e in browser {
            let sym = e
                .runtime_symbol
                .unwrap_or_else(|| panic!("Browser.{} missing runtime_symbol", e.name));
            assert!(
                sym.starts_with("vox_actor_runtime::builtins::vox_browser_"),
                "unexpected symbol for Browser.{}: {sym}",
                e.name
            );
        }
    }
}
