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
        Ty::Result(Box::new(Ty::Unit), Box::new(Ty::Str))
    } else {
        Ty::Result(Box::new(Ty::Str), Box::new(Ty::Str))
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
        // Static scraping (no browser): fetch + parse + CSS-select. Pure-Rust;
        // network builtins are not wasm-guarded here (they reuse the http path).
        BuiltinRegistryEntry {
            namespace: "Scrape",
            name: "fetch",
            arg_count: 1,
            signature: "fn(str) -> Result[str]",
            runtime_symbol: Some("vox_actor_runtime::builtins::vox_scrape_fetch"),
            arg_kinds: &[],
            returns_unit: false,
        },
        BuiltinRegistryEntry {
            namespace: "Scrape",
            name: "fetch_html",
            arg_count: 1,
            signature: "fn(str) -> Result[str]",
            runtime_symbol: Some("vox_actor_runtime::builtins::vox_scrape_fetch_html"),
            arg_kinds: &[],
            returns_unit: false,
        },
        BuiltinRegistryEntry {
            namespace: "Scrape",
            name: "select",
            arg_count: 2,
            signature: "fn(str, str) -> Result[str]",
            runtime_symbol: Some("vox_actor_runtime::builtins::vox_scrape_select"),
            arg_kinds: &[],
            returns_unit: false,
        },
        BuiltinRegistryEntry {
            namespace: "Scrape",
            name: "select_attr",
            arg_count: 3,
            signature: "fn(str, str, str) -> Result[str]",
            runtime_symbol: Some("vox_actor_runtime::builtins::vox_scrape_select_attr"),
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
    // Reliability gate (SSOT): for a namespace OWNED by the canonical
    // NAMESPACE_BUILTINS table, a method not listed there is rejected here.
    // Combined with the parity tests (which assert every listed method has
    // interp + codegen impls), this makes the table == the set of typecheckable
    // std.<ns>.<method> — so a method cannot be added to typecheck without the
    // interp↔codegen parity check covering it. Unowned namespaces are unaffected.
    if namespace_builtin_owned(namespace) && !namespace_builtin_listed(namespace, method) {
        return None;
    }
    Some(match (namespace, method) {
        ("fs", "read") | ("fs", "read_file") | ("fs", "read_to_string") => Ty::Fn(
            vec![Ty::Str],
            Box::new(Ty::Result(Box::new(Ty::Str), Box::new(Ty::Str))),
        ),
        ("fs", "read_bytes") => Ty::Fn(
            vec![Ty::Str],
            Box::new(Ty::Result(Box::new(Ty::Str), Box::new(Ty::Str))),
        ),
        ("fs", "write") | ("fs", "write_file") | ("fs", "write_to_file") => Ty::Fn(
            vec![Ty::Str, Ty::Str],
            Box::new(Ty::Result(Box::new(Ty::Unit), Box::new(Ty::Str))),
        ),
        ("fs", "cwd") => Ty::Fn(
            vec![],
            Box::new(Ty::Result(Box::new(Ty::Str), Box::new(Ty::Str))),
        ),
        ("fs", "walk") | ("fs", "list_recursive") => Ty::Fn(
            vec![Ty::Str],
            Box::new(Ty::Result(
                Box::new(Ty::List(Box::new(Ty::Str))),
                Box::new(Ty::Str),
            )),
        ),
        ("fs", "exists") => Ty::Fn(vec![Ty::Str], Box::new(Ty::Bool)),
        ("fs", "is_file") | ("fs", "is_dir") => Ty::Fn(vec![Ty::Str], Box::new(Ty::Bool)),
        ("fs", "canonicalize") => Ty::Fn(
            vec![Ty::Str],
            Box::new(Ty::Result(Box::new(Ty::Str), Box::new(Ty::Str))),
        ),
        ("fs", "list_dir") | ("fs", "glob") => Ty::Fn(
            vec![Ty::Str],
            Box::new(Ty::Result(
                Box::new(Ty::List(Box::new(Ty::Str))),
                Box::new(Ty::Str),
            )),
        ),
        ("fs", "list_dir_detailed") => Ty::Fn(
            vec![Ty::Str],
            Box::new(Ty::Result(
                Box::new(Ty::List(Box::new(file_record_ty()))),
                Box::new(Ty::Str),
            )),
        ),
        ("fs", "stat") => Ty::Fn(
            vec![Ty::Str],
            Box::new(Ty::Result(Box::new(file_record_ty()), Box::new(Ty::Str))),
        ),
        ("fs", "remove_dir_all") => Ty::Fn(
            vec![Ty::Str],
            Box::new(Ty::Result(Box::new(Ty::Unit), Box::new(Ty::Str))),
        ),
        // remove (delete file) + mkdir create/delete and return unit — they were
        // wrongly grouped with the read family (Result[str]); interp+codegen treat
        // them as unit/bool. (Wrong-shape parity fix surfaced by the shape test.)
        // `remove_dir_all` is already handled above; this arm covers the rest.
        ("fs", "remove") | ("fs", "mkdir") => Ty::Fn(
            vec![Ty::Str],
            Box::new(Ty::Result(Box::new(Ty::Unit), Box::new(Ty::Str))),
        ),
        ("fs", "copy") => Ty::Fn(
            vec![Ty::Str, Ty::Str],
            Box::new(Ty::Result(Box::new(Ty::Unit), Box::new(Ty::Str))),
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
        ("path", "resolve") => Ty::Fn(
            vec![Ty::Str],
            Box::new(Ty::Result(Box::new(Ty::Str), Box::new(Ty::Str))),
        ),
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
        // `process.run` is capture-and-guard: `Some({code, stdout, stderr})` on
        // spawn success, `None` on spawn failure. This matches the interpreter
        // (`eval/builtins.rs`) and the scripts that guard with `is null` then
        // read `.unwrap().code`. (Earlier this was mis-declared `Result[Int]`,
        // which disagreed with both the interpreter and codegen.)
        ("process", "run") => Ty::Fn(
            vec![Ty::Str, Ty::List(Box::new(Ty::Str))],
            Box::new(Ty::Option(Box::new(Ty::Record(vec![
                ("code".into(), Ty::Int),
                ("stdout".into(), Ty::Str),
                ("stderr".into(), Ty::Str),
            ])))),
        ),
        ("process", "run_ex") => Ty::Fn(
            vec![
                Ty::Str,
                Ty::List(Box::new(Ty::Str)),
                Ty::Str,
                Ty::List(Box::new(Ty::Str)),
            ],
            Box::new(Ty::Result(Box::new(Ty::Int), Box::new(Ty::Str))),
        ),
        ("process", "run_capture") => Ty::Fn(
            vec![Ty::Str, Ty::List(Box::new(Ty::Str))],
            Box::new(Ty::Result(
                Box::new(Ty::Record(vec![
                    ("exit".into(), Ty::Int),
                    ("stdout".into(), Ty::Str),
                    ("stderr".into(), Ty::Str),
                ])),
                Box::new(Ty::Str),
            )),
        ),
        ("process", "run_capture_ex") => Ty::Fn(
            vec![
                Ty::Str,
                Ty::List(Box::new(Ty::Str)),
                Ty::Str,
                Ty::List(Box::new(Ty::Str)),
            ],
            Box::new(Ty::Result(
                Box::new(Ty::Record(vec![
                    ("exit".into(), Ty::Int),
                    ("stdout".into(), Ty::Str),
                    ("stderr".into(), Ty::Str),
                ])),
                Box::new(Ty::Str),
            )),
        ),
        ("process", "run_capture_json") => Ty::Fn(
            vec![Ty::Str, Ty::List(Box::new(Ty::Str))],
            Box::new(Ty::Result(
                Box::new(Ty::Named("Json".into())),
                Box::new(Ty::Str),
            )),
        ),
        ("process", "run_capture_lines") => Ty::Fn(
            vec![Ty::Str, Ty::List(Box::new(Ty::Str))],
            Box::new(Ty::Result(
                Box::new(Ty::List(Box::new(Ty::Str))),
                Box::new(Ty::Str),
            )),
        ),
        ("process", "spawn_background") => Ty::Fn(
            vec![Ty::Str, Ty::List(Box::new(Ty::Str))],
            Box::new(Ty::Result(Box::new(Ty::Int), Box::new(Ty::Str))),
        ),
        ("process", "exec") => Ty::Fn(
            vec![Ty::Str, Ty::List(Box::new(Ty::Str))],
            Box::new(Ty::Result(Box::new(Ty::Unit), Box::new(Ty::Str))),
        ),
        ("process", "register_exit_command") => Ty::Fn(
            vec![Ty::Str, Ty::List(Box::new(Ty::Str))],
            Box::new(Ty::Result(Box::new(Ty::Unit), Box::new(Ty::Str))),
        ),
        ("process", "exit") => Ty::Fn(vec![Ty::Int], Box::new(Ty::Never)),
        ("csv", "parse") => Ty::Fn(
            vec![Ty::Str],
            Box::new(Ty::Result(
                Box::new(Ty::Named("Json".into())),
                Box::new(Ty::Str),
            )),
        ),
        ("csv", "parse_records") => Ty::Fn(
            vec![Ty::Str],
            Box::new(Ty::Result(
                Box::new(Ty::Named("Json".into())),
                Box::new(Ty::Str),
            )),
        ),
        ("csv", "render") => Ty::Fn(
            vec![Ty::List(Box::new(Ty::List(Box::new(Ty::Str))))],
            Box::new(Ty::Result(Box::new(Ty::Str), Box::new(Ty::Str))),
        ),
        ("toml", "parse") => Ty::Fn(
            vec![Ty::Str],
            Box::new(Ty::Result(
                Box::new(Ty::Named("Json".into())),
                Box::new(Ty::Str),
            )),
        ),
        ("toml", "render") => Ty::Fn(
            vec![Ty::GenericParam(0)],
            Box::new(Ty::Result(Box::new(Ty::Str), Box::new(Ty::Str))),
        ),
        ("yaml", "parse") => Ty::Fn(
            vec![Ty::Str],
            Box::new(Ty::Result(
                Box::new(Ty::Named("Json".into())),
                Box::new(Ty::Str),
            )),
        ),
        ("yaml", "render") => Ty::Fn(
            vec![Ty::GenericParam(0)],
            Box::new(Ty::Result(Box::new(Ty::Str), Box::new(Ty::Str))),
        ),
        ("io", "open") => Ty::Fn(
            vec![Ty::Str],
            Box::new(Ty::Result(
                Box::new(Ty::Named("Json".into())),
                Box::new(Ty::Str),
            )),
        ),
        ("io", "save") => Ty::Fn(
            vec![Ty::Str, Ty::GenericParam(0)],
            Box::new(Ty::Result(Box::new(Ty::Unit), Box::new(Ty::Str))),
        ),
        ("json", "render") => Ty::Fn(
            vec![Ty::GenericParam(0)],
            Box::new(Ty::Result(Box::new(Ty::Str), Box::new(Ty::Str))),
        ),
        ("json", "parse") => Ty::Fn(
            vec![Ty::Str],
            Box::new(Ty::Result(
                Box::new(Ty::Named("Json".into())),
                Box::new(Ty::Str),
            )),
        ),
        ("json", "read_str") => Ty::Fn(
            vec![Ty::Str, Ty::Str],
            Box::new(Ty::Result(Box::new(Ty::Str), Box::new(Ty::Str))),
        ),
        ("json", "read_f64") => Ty::Fn(
            vec![Ty::Str, Ty::Str],
            Box::new(Ty::Result(Box::new(Ty::Float), Box::new(Ty::Str))),
        ),
        ("json", "quote") => Ty::Fn(vec![Ty::Str], Box::new(Ty::Str)),
        ("http", "get_text") => Ty::Fn(
            vec![Ty::Str],
            Box::new(Ty::Result(Box::new(Ty::Str), Box::new(Ty::Str))),
        ),
        ("http", "post_json") => Ty::Fn(
            vec![Ty::Str, Ty::Str],
            Box::new(Ty::Result(Box::new(Ty::Str), Box::new(Ty::Str))),
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
            Box::new(Ty::Result(
                Box::new(Ty::Named("Regex".into())),
                Box::new(Ty::Str),
            )),
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
        // Interp/codegen parity: these fs aliases typecheck and run under --interp,
        // but previously fell through to invalid `::std::fs::read_file(...)` etc. in
        // native emit. Route them to the canonical runtime fns (same as read/write).
        ("fs", "read_file" | "read_to_string") if !args.is_empty() => Some(format!(
            "::vox_actor_runtime::builtins::vox_fs_read(({}).as_str())",
            args[0]
        )),
        ("fs", "write_file" | "write_to_file") if args.len() >= 2 => Some(format!(
            "::vox_actor_runtime::builtins::vox_fs_write(({}).as_str(), ({}).as_str())",
            args[0], args[1]
        )),
        ("fs", "cwd") => Some(
            "::std::env::current_dir().map(|p| p.to_string_lossy().to_string()).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)".to_string(),
        ),
        ("fs", "walk" | "list_recursive") if !args.is_empty() => Some(format!(
            "::vox_actor_runtime::builtins::vox_fs_glob(format!(\"{{}}/**/*\", {}).as_str())",
            args[0]
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
            "(vox_actor_runtime::builtins::vox_process_run_opt(({}).as_str(), {}.as_slice()))",
            args[0], args[1]
        )),
        ("process", "run_ex") if args.len() >= 4 => Some(format!(
            "(match vox_actor_runtime::builtins::vox_process_run_ex(({}).as_str(), {}.as_slice(), ({}).as_str(), {}.as_slice()) {{ Ok(c) => Ok(c as i64), Err(m) => Err(m) }})",
            args[0], args[1], args[2], args[3]
        )),
        ("process", "run_capture") if args.len() >= 2 => Some(format!(
            "(match vox_actor_runtime::builtins::vox_process_run_capture(({}).as_str(), {}.as_slice()) {{ Ok(p) => Ok(serde_json::json!({{ \"exit\": p.exit as i64, \"stdout\": p.stdout, \"stderr\": p.stderr }})), Err(m) => Err(m) }})",
            args[0], args[1]
        )),
        ("process", "run_capture_ex") if args.len() >= 4 => Some(format!(
            "(match vox_actor_runtime::builtins::vox_process_run_capture_ex(({}).as_str(), {}.as_slice(), ({}).as_str(), {}.as_slice()) {{ Ok(p) => Ok(serde_json::json!({{ \"exit\": p.exit as i64, \"stdout\": p.stdout, \"stderr\": p.stderr }})), Err(m) => Err(m) }})",
            args[0], args[1], args[2], args[3]
        )),
        ("process", "run_capture_json") if args.len() >= 2 => Some(format!(
            "(match ::vox_actor_runtime::builtins::vox_process_run_capture_json(({}).as_str(), {}.as_slice()) {{ Ok(v) => Ok(v), Err(m) => Err(m) }})",
            args[0], args[1]
        )),
        ("process", "run_capture_lines") if args.len() >= 2 => Some(format!(
            "(match ::vox_actor_runtime::builtins::vox_process_run_capture_lines(({}).as_str(), {}.as_slice()) {{ Ok(v) => Ok(v), Err(m) => Err(m) }})",
            args[0], args[1]
        )),
        ("process", "spawn_background") if args.len() >= 2 => Some(format!(
            "(match vox_actor_runtime::builtins::vox_process_spawn_background(({}).as_str(), {}.as_slice()) {{ Ok(id) => Ok(id), Err(m) => Err(m) }})",
            args[0], args[1]
        )),
        ("process", "exec") if args.len() >= 2 => Some(format!(
            "(match vox_actor_runtime::builtins::vox_process_exec(({}).as_str(), {}.as_slice()) {{ Ok(()) => Ok(()), Err(m) => Err(m) }})",
            args[0], args[1]
        )),
        ("process", "register_exit_command") if args.len() >= 2 => Some(format!(
            "(match vox_actor_runtime::builtins::vox_process_register_exit_command(({}).as_str(), {}.as_slice()) {{ Ok(()) => Ok(()), Err(m) => Err(m) }})",
            args[0], args[1]
        )),
        ("process", "exit") if !args.is_empty() => {
            Some(format!("{{ std::process::exit({} as i32) }}", args[0]))
        }
        ("fs", "list_dir_detailed") if !args.is_empty() => Some(format!(
            "(match ::vox_actor_runtime::builtins::vox_fs_list_dir_detailed(({}).as_str()) {{ Ok(rows) => Ok(rows.into_iter().map(|r| serde_json::json!({{ \"name\": r.name, \"path\": r.path, \"size\": r.size, \"modified_ms\": r.modified_ms, \"is_dir\": r.is_dir, \"is_file\": r.is_file, \"is_symlink\": r.is_symlink }})).collect::<Vec<serde_json::Value>>()), Err(m) => Err(m) }})",
            args[0]
        )),
        ("fs", "stat") if !args.is_empty() => Some(format!(
            "(match ::vox_actor_runtime::builtins::vox_fs_stat(({}).as_str()) {{ Ok(r) => Ok(serde_json::json!({{ \"name\": r.name, \"path\": r.path, \"size\": r.size, \"modified_ms\": r.modified_ms, \"is_dir\": r.is_dir, \"is_file\": r.is_file, \"is_symlink\": r.is_symlink }})), Err(m) => Err(m) }})",
            args[0]
        )),
        ("fs", "list_dir") if !args.is_empty() => Some(format!(
            "(match vox_actor_runtime::builtins::vox_list_dir(({}).as_str()) {{ Ok(v) => Ok(v), Err(m) => Err(m) }})",
            args[0]
        )),
        ("fs", "glob") if !args.is_empty() => Some(format!(
            "(match vox_actor_runtime::builtins::vox_fs_glob(({}).as_str()) {{ Ok(v) => Ok(v), Err(m) => Err(m) }})",
            args[0]
        )),
        ("fs", "remove_dir_all") if !args.is_empty() => Some(format!(
            "(match vox_actor_runtime::builtins::vox_fs_remove_dir_all(({}).as_str()) {{ Ok(()) => Ok(()), Err(m) => Err(m) }})",
            args[0]
        )),
        ("fs", "copy") if args.len() >= 2 => Some(format!(
            "(match vox_actor_runtime::builtins::vox_fs_copy(({}).as_str(), ({}).as_str()) {{ Ok(()) => Ok(()), Err(m) => Err(m) }})",
            args[0], args[1]
        )),
        ("path", "join_many") if !args.is_empty() => Some(format!(
            "vox_actor_runtime::builtins::vox_path_join_many({}.as_slice())",
            args[0]
        )),
        ("json", "read_str") if args.len() >= 2 => Some(format!(
            "(match vox_actor_runtime::builtins::vox_json_read_str(({}).as_str(), ({}).as_str()) {{ Ok(s) => Ok(s), Err(m) => Err(m) }})",
            args[0], args[1]
        )),
        ("json", "read_f64") if args.len() >= 2 => Some(format!(
            "(match vox_actor_runtime::builtins::vox_json_read_f64(({}).as_str(), ({}).as_str()) {{ Ok(v) => Ok(v), Err(m) => Err(m) }})",
            args[0], args[1]
        )),
        ("json", "quote") if !args.is_empty() => Some(format!(
            "vox_actor_runtime::builtins::vox_json_quote(({}).as_str())",
            args[0]
        )),
        ("http", "get_text") if !args.is_empty() => Some(format!(
            "({{ #[cfg(target_arch = \"wasm32\")] {{ Err(\"std.http.get_text is not supported in WASI scripts\".to_string()) }} #[cfg(not(target_arch = \"wasm32\"))] {{ match vox_actor_runtime::builtins::vox_http_get_text(({}).as_str()) {{ Ok(s) => Ok(s), Err(m) => Err(m) }} }} }})",
            args[0]
        )),
        ("http", "post_json") if args.len() >= 2 => Some(format!(
            "({{ #[cfg(target_arch = \"wasm32\")] {{ Err(\"std.http.post_json is not supported in WASI scripts\".to_string()) }} #[cfg(not(target_arch = \"wasm32\"))] {{ match vox_actor_runtime::builtins::vox_http_post_json(({}).as_str(), ({}).as_str()) {{ Ok(s) => Ok(s), Err(m) => Err(m) }} }} }})",
            args[0], args[1]
        )),
        ("regex", "compile") if !args.is_empty() => Some(format!(
            "(match vox_actor_runtime::builtins::vox_regex_compile(({}).as_str()) {{ Ok(r) => Ok(r), Err(m) => Err(m) }})",
            args[0]
        )),
        ("csv", "parse") if !args.is_empty() => Some(format!(
            "(match ::vox_actor_runtime::builtins::vox_csv_parse(({}).as_str()) {{ Ok(v) => Ok(v), Err(m) => Err(m) }})",
            args[0]
        )),
        ("csv", "parse_records") if !args.is_empty() => Some(format!(
            "(match ::vox_actor_runtime::builtins::vox_csv_parse_records(({}).as_str()) {{ Ok(v) => Ok(v), Err(m) => Err(m) }})",
            args[0]
        )),
        ("csv", "render") if !args.is_empty() => Some(format!(
            "(match ::vox_actor_runtime::builtins::vox_csv_render({}.as_slice()) {{ Ok(s) => Ok(s), Err(m) => Err(m) }})",
            args[0]
        )),
        ("toml", "parse") if !args.is_empty() => Some(format!(
            "(match ::vox_actor_runtime::builtins::vox_toml_parse(({}).as_str()) {{ Ok(v) => Ok(v), Err(m) => Err(m) }})",
            args[0]
        )),
        ("toml", "render") if !args.is_empty() => Some(format!(
            "(match ::vox_actor_runtime::builtins::vox_toml_render(&({})) {{ Ok(s) => Ok(s), Err(m) => Err(m) }})",
            args[0]
        )),
        ("yaml", "parse") if !args.is_empty() => Some(format!(
            "(match ::vox_actor_runtime::builtins::vox_yaml_parse(({}).as_str()) {{ Ok(v) => Ok(v), Err(m) => Err(m) }})",
            args[0]
        )),
        ("yaml", "render") if !args.is_empty() => Some(format!(
            "(match ::vox_actor_runtime::builtins::vox_yaml_render(&({})) {{ Ok(s) => Ok(s), Err(m) => Err(m) }})",
            args[0]
        )),
        ("io", "open") if !args.is_empty() => Some(format!(
            "(match ::vox_actor_runtime::builtins::vox_io_open(({}).as_str()) {{ Ok(v) => Ok(v), Err(m) => Err(m) }})",
            args[0]
        )),
        ("io", "save") if args.len() >= 2 => Some(format!(
            "(match ::vox_actor_runtime::builtins::vox_io_save(({}).as_str(), &({})) {{ Ok(()) => Ok(()), Err(m) => Err(m) }})",
            args[0], args[1]
        )),
        ("json", "render") if !args.is_empty() => Some(format!(
            "(match vox_actor_runtime::builtins::vox_json_render(&({})) {{ Ok(s) => Ok(s), Err(m) => Err(m) }})",
            args[0]
        )),
        ("json", "parse") if !args.is_empty() => Some(format!(
            "(match vox_actor_runtime::builtins::vox_json_parse(({}).as_str()) {{ Ok(j) => Ok(j), Err(m) => Err(m) }})",
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

    #[test]
    fn scrape_builtins_map_to_vox_runtime() {
        let scrape: Vec<_> = builtin_registry_entries()
            .iter()
            .copied()
            .filter(|e| e.namespace == "Scrape")
            .collect();
        assert_eq!(
            scrape.len(),
            4,
            "Scrape registry size drift (update typeck builtins.rs + runtime if intentional)"
        );
        for e in scrape {
            let sym = e
                .runtime_symbol
                .unwrap_or_else(|| panic!("Scrape.{} missing runtime_symbol", e.name));
            assert!(
                sym.starts_with("vox_actor_runtime::builtins::vox_scrape_"),
                "unexpected symbol for Scrape.{}: {sym}",
                e.name
            );
            // All Scrape fns return Result[str].
            assert!(!e.returns_unit, "Scrape.{} must return Result[str]", e.name);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Canonical surface-flagged registry of `std.<ns>.<method>` builtins.
//
// SSOT for the interp↔codegen↔typecheck parity guard. Each entry declares which
// SURFACES the builtin must be implemented on, so the parity tests below can
// assert coverage without false-positives on native/codegen-only builtins
// (http/crypto have no interpreter network/crypto stack — they are RUST-only).
//
// Adding a `std.<ns>.<method>` to the typechecker (std_namespace_method_ty) WITHOUT
// adding it here, or here without the declared impls, fails the parity tests.
// ─────────────────────────────────────────────────────────────────────────────

/// Surface bitflags for [`NAMESPACE_BUILTINS`].
pub mod surface {
    /// Tree-walking interpreter (`vox run --interp`) via `call_builtin_method`.
    pub const INTERP: u8 = 0b01;
    /// Native Rust codegen (`--mode script` / compiled) via `std_namespace_runtime_call`.
    pub const RUST: u8 = 0b10;
    /// Both interpreter and Rust codegen.
    pub const IR: u8 = INTERP | RUST;
}

/// `(namespace, method, arg_count, surfaces)` — the canonical set of
/// `std.<ns>.<method>` builtins and the surfaces each must support.
pub const NAMESPACE_BUILTINS: &[(&str, &str, usize, u8)] = &[
    // fs — file system (interp + native)
    ("fs", "read", 1, surface::IR),
    ("fs", "read_file", 1, surface::IR),
    ("fs", "read_to_string", 1, surface::IR),
    ("fs", "read_bytes", 1, surface::IR),
    ("fs", "write", 2, surface::IR),
    ("fs", "write_file", 2, surface::IR),
    ("fs", "write_to_file", 2, surface::IR),
    ("fs", "cwd", 0, surface::IR),
    ("fs", "walk", 1, surface::IR),
    ("fs", "list_recursive", 1, surface::IR),
    ("fs", "exists", 1, surface::IR),
    ("fs", "is_file", 1, surface::IR),
    ("fs", "is_dir", 1, surface::IR),
    ("fs", "canonicalize", 1, surface::IR),
    ("fs", "list_dir", 1, surface::IR),
    ("fs", "glob", 1, surface::IR),
    ("fs", "list_dir_detailed", 1, surface::IR),
    ("fs", "stat", 1, surface::IR),
    ("fs", "remove_dir_all", 1, surface::IR),
    ("fs", "copy", 2, surface::IR),
    ("fs", "remove", 1, surface::IR),
    ("fs", "mkdir", 1, surface::IR),
    // path
    ("path", "join", 2, surface::IR),
    ("path", "join_many", 1, surface::IR),
    ("path", "basename", 1, surface::IR),
    ("path", "dirname", 1, surface::IR),
    ("path", "extension", 1, surface::IR),
    ("path", "parent", 1, surface::IR),
    ("path", "file_name", 1, surface::IR),
    ("path", "stem", 1, surface::IR),
    ("path", "is_absolute", 1, surface::IR),
    ("path", "resolve", 1, surface::IR),
    // env
    ("env", "get", 1, surface::IR),
    ("env", "args", 0, surface::IR),
    ("env", "set", 2, surface::IR),
    // regex
    ("regex", "replace", 3, surface::IR),
    ("regex", "find", 2, surface::IR),
    ("regex", "is_match", 2, surface::IR),
    ("regex", "captures", 2, surface::IR),
    // Present on all surfaces, but its return SHAPE differs (typeck Result[Regex]
    // vs interp/codegen str) — a wrong-shape issue the presence checker can't catch.
    ("regex", "compile", 1, surface::IR),
    // process
    ("process", "which", 1, surface::IR),
    ("process", "run", 2, surface::IR),
    ("process", "run_ex", 4, surface::IR),
    ("process", "run_capture", 2, surface::IR),
    ("process", "run_capture_ex", 4, surface::IR),
    ("process", "run_capture_json", 2, surface::IR),
    ("process", "run_capture_lines", 2, surface::IR),
    ("process", "spawn_background", 2, surface::IR),
    ("process", "exec", 2, surface::IR),
    ("process", "register_exit_command", 2, surface::IR),
    ("process", "exit", 1, surface::IR),
    // structured formats
    ("csv", "parse", 1, surface::IR),
    ("csv", "parse_records", 1, surface::IR),
    ("csv", "render", 1, surface::IR),
    ("toml", "parse", 1, surface::IR),
    ("toml", "render", 1, surface::IR),
    ("yaml", "parse", 1, surface::IR),
    ("yaml", "render", 1, surface::IR),
    ("io", "open", 1, surface::IR),
    ("io", "save", 2, surface::IR),
    ("json", "parse", 1, surface::IR),
    ("json", "render", 1, surface::IR),
    ("json", "read_str", 2, surface::IR),
    ("json", "read_f64", 2, surface::IR),
    ("json", "quote", 1, surface::IR),
    // logging
    ("log", "debug", 1, surface::IR),
    ("log", "info", 1, surface::IR),
    ("log", "warn", 1, surface::IR),
    ("log", "error", 1, surface::IR),
    // time
    ("time", "now_ms", 0, surface::IR),
    // agentos
    ("agentos", "mutation_kind_for_tool", 1, surface::IR),
    // crypto + http — NATIVE/codegen only. The tree-walking interpreter has no
    // crypto/network stack; scripts needing these run via --mode script.
    ("crypto", "hash_fast", 1, surface::RUST),
    ("crypto", "hash_secure", 1, surface::RUST),
    ("crypto", "uuid", 0, surface::RUST),
    ("http", "get_text", 1, surface::RUST),
    ("http", "post_json", 2, surface::RUST),
];

/// True if `namespace` is owned by the canonical [`NAMESPACE_BUILTINS`] table
/// (i.e. its full method set is declared there). Used by the typecheck gate.
#[must_use]
pub fn namespace_builtin_owned(namespace: &str) -> bool {
    NAMESPACE_BUILTINS
        .iter()
        .any(|(ns, _, _, _)| *ns == namespace)
}

/// True if `(namespace, method)` is listed in [`NAMESPACE_BUILTINS`].
#[must_use]
pub fn namespace_builtin_listed(namespace: &str, method: &str) -> bool {
    NAMESPACE_BUILTINS
        .iter()
        .any(|(ns, m, _, _)| *ns == namespace && *m == method)
}

#[cfg(test)]
mod namespace_builtin_parity_tests {
    use super::*;
    use crate::eval::builtins::call_builtin_method;
    use crate::eval::value::VoxValue;

    fn ns_receiver(ns: &str) -> VoxValue {
        VoxValue::Object(vec![(
            "__namespace__".to_string(),
            VoxValue::Str(ns.to_string()),
        )])
    }

    /// Every INTERP-surface builtin must dispatch in the tree-walking interpreter.
    /// A missing arm makes `call_builtin_method` return None ("Method not found"
    /// at runtime) — the regression we are guarding against (now_ms/path.basename).
    #[test]
    fn interpreter_dispatches_every_interp_builtin() {
        // Robust probe: a missing method returns Ok(None) ("Method not found").
        // An arm that exists but panics on our type-agnostic dummy args (e.g.
        // process.run expects a list arg) unwinds — we catch it and treat it as
        // PRESENT, since dispatch was reached. Only a clean None is a real gap.
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let mut missing = Vec::new();
        for (ns, method, argc, surfaces) in NAMESPACE_BUILTINS {
            if surfaces & surface::INTERP == 0 {
                continue;
            }
            // process.exit calls std::process::exit — executing it would kill the
            // test harness (catch_unwind can't stop it). Its interp arm exists by
            // inspection; the codegen-surface test covers it safely.
            if (*ns, *method) == ("process", "exit") {
                continue;
            }
            let (ns, method, argc) = (*ns, *method, *argc);
            let probe = std::panic::catch_unwind(|| {
                let args: Vec<VoxValue> =
                    (0..argc).map(|_| VoxValue::Str("x".to_string())).collect();
                call_builtin_method(&ns_receiver(ns), method, args, None)
            });
            if matches!(probe, Ok(None)) {
                missing.push(format!("std.{ns}.{method}"));
            }
        }
        std::panic::set_hook(prev);
        assert!(
            missing.is_empty(),
            "interpreter missing dispatch for: {missing:#?}"
        );
    }

    /// Every RUST-surface builtin must have an explicit Rust-codegen lowering
    /// (no arm => fallthrough to invalid `::std::ns::method(...)`).
    #[test]
    fn codegen_lowers_every_rust_builtin() {
        let mut missing = Vec::new();
        for (ns, method, argc, surfaces) in NAMESPACE_BUILTINS {
            if surfaces & surface::RUST == 0 {
                continue;
            }
            let args: Vec<String> = (0..*argc).map(|i| format!("a{i}")).collect();
            if std_namespace_runtime_call(ns, method, &args).is_none() {
                missing.push(format!("std.{ns}.{method}"));
            }
        }
        assert!(
            missing.is_empty(),
            "codegen missing lowering for: {missing:#?}"
        );
    }

    /// The canonical list must be a subset of what the typechecker accepts —
    /// catches list entries that name a builtin the typechecker doesn't know.
    #[test]
    fn typecheck_knows_every_listed_builtin() {
        let mut missing = Vec::new();
        for (ns, method, _argc, _surfaces) in NAMESPACE_BUILTINS {
            if std_namespace_method_ty(ns, method).is_none() {
                missing.push(format!("std.{ns}.{method}"));
            }
        }
        assert!(
            missing.is_empty(),
            "typecheck missing signature for: {missing:#?}"
        );
    }

    /// The typecheck gate makes the table the SSOT: an unlisted method in an
    /// owned namespace is rejected, so typecheck cannot drift ahead of the table.
    #[test]
    fn typecheck_gate_enforces_table_as_ssot() {
        // Owned namespace + listed method → resolves.
        assert!(std_namespace_method_ty("fs", "read").is_some());
        // Owned namespace + UNlisted method → rejected by the gate.
        assert!(std_namespace_method_ty("fs", "totally_not_a_real_fs_method").is_none());
        assert!(std_namespace_method_ty("process", "no_such_method").is_none());
        // The owned-namespace set is exactly the table's namespaces.
        assert!(namespace_builtin_owned("fs"));
        assert!(!namespace_builtin_owned("mobile")); // mobile dispatches elsewhere — not gated.
    }

    /// Does the runtime VALUE's shape match the typecheck return Ty? Catches
    /// WRONG-SHAPE drift (e.g. typeck Option[str] but interp returns a bare str)
    /// that the presence checker cannot. On Err/None the inner is not observable,
    /// so we accept it; unmodelled Ty variants are permissive (never false-fail).
    fn shape_matches(ty: &Ty, val: &VoxValue) -> bool {
        match ty {
            Ty::Result(inner, _) => match val {
                VoxValue::Result(Ok(b)) => shape_matches(inner, &**b),
                VoxValue::Result(Err(_)) => true,
                _ => false,
            },
            Ty::Option(inner) => match val {
                VoxValue::Option(Some(b)) => shape_matches(inner, &**b),
                VoxValue::Option(None) => true,
                _ => false,
            },
            Ty::Int => matches!(val, VoxValue::Int(_)),
            Ty::Float => matches!(val, VoxValue::Float(_) | VoxValue::Int(_)),
            Ty::Str => matches!(val, VoxValue::Str(_)),
            Ty::Bool => matches!(val, VoxValue::Bool(_)),
            Ty::List(_) => matches!(val, VoxValue::List(_)),
            Ty::Record(_) => matches!(val, VoxValue::Object(_)),
            // unit = "no meaningful value"; interp variously returns Null/Bool(true) for
            // write/mkdir/etc. — all acceptable shapes for a unit return.
            Ty::Unit => true,
            // The interpreter models compiled regexes / matches as the dedicated
            // `VoxValue::Regex` / `VoxValue::Match` variants (it also tolerates the
            // older `Tagged` placeholder form).
            Ty::Named(n) if n == "Regex" => {
                matches!(val, VoxValue::Regex(_))
                    || matches!(val, VoxValue::Tagged { name, .. } if name == "Regex")
            }
            Ty::Named(n) if n == "Match" => {
                matches!(val, VoxValue::Match(_))
                    || matches!(val, VoxValue::Tagged { name, .. } if name == "Match")
            }
            // Json is dynamic; Map/Set/Named(other)/generics are not modelled here.
            _ => true,
        }
    }

    /// Behavioral shape parity: each pure/hermetic std.<ns>.<method> is invoked in
    /// the interpreter with valid inputs (so the success shape is observable) and
    /// its return shape is asserted against the typecheck return Ty. This catches
    /// the wrong-shape class (process.run_ex Result[int]-vs-record, regex.compile
    /// Regex-vs-str, path.parent Option-vs-bare) that presence parity can't.
    /// Scope: pure/string-in builtins (no process spawn). FS/process shape checks
    /// need fixtures — a follow-up slice.
    #[test]
    fn interpreter_return_shape_matches_typecheck() {
        fn s(x: &str) -> VoxValue {
            VoxValue::Str(x.to_string())
        }
        let obj_k1 = VoxValue::Object(vec![("k".to_string(), VoxValue::Int(1))]);
        let probes: Vec<(&str, &str, Vec<VoxValue>)> = vec![
            ("path", "join", vec![s("a"), s("b")]),
            (
                "path",
                "join_many",
                vec![VoxValue::List(vec![s("a"), s("b")])],
            ),
            ("path", "basename", vec![s("a/b.txt")]),
            ("path", "dirname", vec![s("a/b")]),
            ("path", "extension", vec![s("a.txt")]),
            ("path", "parent", vec![s("a/b")]),
            ("path", "file_name", vec![s("a/b")]),
            ("path", "stem", vec![s("a.txt")]),
            ("path", "is_absolute", vec![s("/a")]),
            ("path", "resolve", vec![s(".")]),
            ("regex", "replace", vec![s("a1"), s(r"\d"), s("X")]),
            ("regex", "find", vec![s("a1"), s(r"\d")]),
            ("regex", "is_match", vec![s("a1"), s(r"\d")]),
            ("regex", "captures", vec![s("a1"), s(r"(\d)")]),
            ("regex", "compile", vec![s(r"\d+")]),
            ("json", "parse", vec![s("{}")]),
            ("json", "render", vec![obj_k1.clone()]),
            ("json", "read_str", vec![s(r#"{"k":"v"}"#), s("k")]),
            ("json", "read_f64", vec![s(r#"{"n":1}"#), s("n")]),
            ("json", "quote", vec![s("a")]),
            ("csv", "parse", vec![s("a,b\n1,2")]),
            ("csv", "parse_records", vec![s("a,b\n1,2")]),
            (
                "csv",
                "render",
                vec![VoxValue::List(vec![VoxValue::List(vec![s("a"), s("b")])])],
            ),
            ("toml", "parse", vec![s("k = 1")]),
            ("toml", "render", vec![obj_k1.clone()]),
            ("yaml", "parse", vec![s("k: 1")]),
            ("yaml", "render", vec![obj_k1.clone()]),
            ("time", "now_ms", vec![]),
            ("env", "args", vec![]),
            ("env", "get", vec![s("PATH")]),
            ("agentos", "mutation_kind_for_tool", vec![s("read_file")]),
        ];
        let receiver = |ns: &str| {
            VoxValue::Object(vec![(
                "__namespace__".to_string(),
                VoxValue::Str(ns.to_string()),
            )])
        };
        let mut mismatches = Vec::new();
        for (ns, method, args) in probes {
            let ret = match std_namespace_method_ty(ns, method) {
                Some(Ty::Fn(_, ret)) => *ret,
                other => {
                    mismatches.push(format!("std.{ns}.{method}: no Fn typeck sig ({other:?})"));
                    continue;
                }
            };
            match call_builtin_method(&receiver(ns), method, args, None) {
                Some(val) => {
                    if !shape_matches(&ret, &val) {
                        mismatches.push(format!(
                            "std.{ns}.{method}: typeck {ret:?} but interp returned {val:?}"
                        ));
                    }
                }
                None => mismatches.push(format!("std.{ns}.{method}: interp returned None")),
            }
        }
        assert!(
            mismatches.is_empty(),
            "interp return-shape mismatches vs typecheck:\n{}",
            mismatches.join("\n")
        );
    }

    /// Return-shape parity for FS + process builtins, using real fixtures (a temp
    /// dir/file and the `rustc` command, present in any test env) so the success
    /// path is exercised and the inner shape (Result[Record], Result[list], etc.)
    /// is observed. Locks in fs.stat/list_dir_detailed records, run_ex Result[int],
    /// run_capture records, etc. If a process spawn fails (cmd absent) the result is
    /// still Result/Option-shaped, so the test never false-fails — it just observes
    /// less inner detail.
    #[test]
    fn interpreter_return_shape_matches_typecheck_fs_process() {
        fn s(x: &str) -> VoxValue {
            VoxValue::Str(x.to_string())
        }
        let receiver = |ns: &str| {
            VoxValue::Object(vec![(
                "__namespace__".to_string(),
                VoxValue::Str(ns.to_string()),
            )])
        };

        // Hermetic fixture: a unique temp dir + a file with content.
        let tmp = std::env::temp_dir().join(format!("vox_shape_parity_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&tmp);
        let file = tmp.join("f.txt");
        let _ = std::fs::write(&file, "hello");
        let dir_s = tmp.to_string_lossy().to_string();
        let file_s = file.to_string_lossy().to_string();
        let glob_s = format!("{dir_s}/*");
        let args_list = || VoxValue::List(vec![s("--version")]);

        let probes: Vec<(&str, &str, Vec<VoxValue>)> = vec![
            ("fs", "read", vec![s(&file_s)]),
            ("fs", "read_file", vec![s(&file_s)]),
            ("fs", "read_to_string", vec![s(&file_s)]),
            ("fs", "read_bytes", vec![s(&file_s)]),
            (
                "fs",
                "write",
                vec![s(&tmp.join("w.txt").to_string_lossy()), s("x")],
            ),
            (
                "fs",
                "write_file",
                vec![s(&tmp.join("w2.txt").to_string_lossy()), s("x")],
            ),
            ("fs", "exists", vec![s(&file_s)]),
            ("fs", "is_file", vec![s(&file_s)]),
            ("fs", "is_dir", vec![s(&dir_s)]),
            ("fs", "canonicalize", vec![s(&dir_s)]),
            ("fs", "list_dir", vec![s(&dir_s)]),
            ("fs", "glob", vec![s(&glob_s)]),
            ("fs", "list_dir_detailed", vec![s(&dir_s)]),
            ("fs", "stat", vec![s(&file_s)]),
            ("fs", "walk", vec![s(&dir_s)]),
            ("fs", "list_recursive", vec![s(&dir_s)]),
            (
                "fs",
                "copy",
                vec![s(&file_s), s(&tmp.join("c.txt").to_string_lossy())],
            ),
            ("fs", "mkdir", vec![s(&tmp.join("sub").to_string_lossy())]),
            ("fs", "cwd", vec![]),
            ("process", "which", vec![s("rustc")]),
            ("process", "run", vec![s("rustc"), args_list()]),
            (
                "process",
                "run_ex",
                vec![s("rustc"), args_list(), s("."), VoxValue::List(vec![])],
            ),
            ("process", "run_capture", vec![s("rustc"), args_list()]),
            (
                "process",
                "run_capture_ex",
                vec![s("rustc"), args_list(), s("."), VoxValue::List(vec![])],
            ),
            ("process", "run_capture_json", vec![s("rustc"), args_list()]),
            (
                "process",
                "run_capture_lines",
                vec![s("rustc"), args_list()],
            ),
        ];

        let mut mismatches = Vec::new();
        for (ns, method, args) in probes {
            let ret = match std_namespace_method_ty(ns, method) {
                Some(Ty::Fn(_, ret)) => *ret,
                other => {
                    mismatches.push(format!("std.{ns}.{method}: no Fn typeck sig ({other:?})"));
                    continue;
                }
            };
            match call_builtin_method(&receiver(ns), method, args, None) {
                Some(val) => {
                    if !shape_matches(&ret, &val) {
                        mismatches.push(format!(
                            "std.{ns}.{method}: typeck {ret:?} but interp returned {val:?}"
                        ));
                    }
                }
                None => mismatches.push(format!("std.{ns}.{method}: interp returned None")),
            }
        }
        let _ = std::fs::remove_dir_all(&tmp);
        assert!(
            mismatches.is_empty(),
            "interp FS/process return-shape mismatches vs typecheck:\n{}",
            mismatches.join("\n")
        );
    }
}
