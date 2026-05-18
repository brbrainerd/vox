use crate::builtin_registry::{
    builtin_entry_param_tys, builtin_entry_result_ty, builtin_registry_entries,
};
use crate::typeck::env::{AdtDef, Binding, BindingKind, TypeEnv, VariantDef};
use crate::typeck::ty::Ty;

/// Pre-registered type signatures for the Vox standard library.
///
/// This populates the root scope of a `TypeEnv` with:
/// - Built-in types (Option, Result as ADTs with proper constructors)
/// - Standard library functions (print, str, int, float, len)
/// - React/frontend bindings (`use_state`, `use_effect`, `use_memo`, `use_ref`, `use_callback`)
/// - HTTP/network module bindings
/// - String, list, and record methods
pub struct BuiltinTypes {
    /// Method signatures: type_key → { method_name → return_type }
    methods: std::collections::HashMap<String, std::collections::HashMap<String, Ty>>,
}

impl BuiltinTypes {
    /// Populate the given TypeEnv with all built-in definitions.
    pub fn register_all(env: &mut TypeEnv) -> Self {
        let mut methods: std::collections::HashMap<String, std::collections::HashMap<String, Ty>> =
            std::collections::HashMap::new();

        // ── Built-in ADTs ─────────────────────────────────────

        // Option[T] = | Some(value: T) | None
        env.register_type(AdtDef {
            name: "Option".into(),
            variants: vec![
                VariantDef {
                    name: "Some".into(),
                    fields: vec![("value".into(), Ty::GenericParam(0))],
                },
                VariantDef {
                    name: "None".into(),
                    fields: vec![],
                },
            ],
            fields: vec![],
        });

        // Some(value: T) → Option[T]
        env.define(
            "Some".into(),
            Binding {
                ty: Ty::Fn(
                    vec![Ty::GenericParam(0)],
                    Box::new(Ty::Option(Box::new(Ty::GenericParam(0)))),
                ),
                mutable: false,
                kind: BindingKind::Constructor,
                is_deprecated: false,
            },
        );
        // None → Option[T]
        env.define(
            "None".into(),
            Binding {
                ty: Ty::Option(Box::new(Ty::GenericParam(0))),
                mutable: false,
                kind: BindingKind::Constructor,
                is_deprecated: false,
            },
        );

        // Result[T] = | Ok(value: T) | Error(message: str)
        env.register_type(AdtDef {
            name: "Result".into(),
            variants: vec![
                VariantDef {
                    name: "Ok".into(),
                    fields: vec![("value".into(), Ty::GenericParam(0))],
                },
                VariantDef {
                    name: "Error".into(),
                    fields: vec![("message".into(), Ty::Str)],
                },
            ],
            fields: vec![],
        });

        // Ok(value: T) → Result[T, E]
        // Polymorphic in both T and E so two-param Result[T, E] surfaces unify.
        env.define(
            "Ok".into(),
            Binding {
                ty: Ty::Fn(
                    vec![Ty::GenericParam(0)],
                    Box::new(Ty::Result(
                        Box::new(Ty::GenericParam(0)),
                        Box::new(Ty::GenericParam(1)),
                    )),
                ),
                mutable: false,
                kind: BindingKind::Constructor,
                is_deprecated: false,
            },
        );
        // Error(payload: E) → Result[T, E]
        // Polymorphic; before two-param Result landed this was fixed to
        // Error(str), which is why the marquee fixtures had to fall back
        // to string error payloads.
        env.define(
            "Error".into(),
            Binding {
                ty: Ty::Fn(
                    vec![Ty::GenericParam(1)],
                    Box::new(Ty::Result(
                        Box::new(Ty::GenericParam(0)),
                        Box::new(Ty::GenericParam(1)),
                    )),
                ),
                mutable: false,
                kind: BindingKind::Constructor,
                is_deprecated: false,
            },
        );
        // Err is an alias of Error for Rust-familiar source.
        env.define(
            "Err".into(),
            Binding {
                ty: Ty::Fn(
                    vec![Ty::GenericParam(1)],
                    Box::new(Ty::Result(
                        Box::new(Ty::GenericParam(0)),
                        Box::new(Ty::GenericParam(1)),
                    )),
                ),
                mutable: false,
                kind: BindingKind::Constructor,
                is_deprecated: false,
            },
        );

        // bool as an ADT
        env.define(
            "true".into(),
            Binding {
                ty: Ty::Bool,
                mutable: false,
                kind: BindingKind::Constructor,
                is_deprecated: false,
            },
        );
        env.define(
            "false".into(),
            Binding {
                ty: Ty::Bool,
                mutable: false,
                kind: BindingKind::Constructor,
                is_deprecated: false,
            },
        );

        // ── Standard library functions ────────────────────────

        // print(value: str) → Unit
        env.define(
            "print".into(),
            Binding {
                ty: Ty::Fn(vec![Ty::Str], Box::new(Ty::Unit)),
                mutable: false,
                kind: BindingKind::Function,
                is_deprecated: false,
            },
        );

        // assert(condition: bool) → Unit
        env.define(
            "assert".into(),
            Binding {
                ty: Ty::Fn(vec![Ty::Bool], Box::new(Ty::Unit)),
                mutable: false,
                kind: BindingKind::Function,
                is_deprecated: false,
            },
        );

        // std — namespace for `std.fs.*`, `std.path.*`, `std.env.*`, `std.process.*`,
        // `std.json.*`, `std.http.*`, `std.crypto.*`, `std.time.*`, `std.log.*`,
        // and direct hash/time helpers.
        env.define(
            "std".into(),
            Binding {
                ty: Ty::Named("StdNamespace".into()),
                mutable: false,
                kind: BindingKind::Import,
                is_deprecated: false,
            },
        );

        // str(value: any) → str
        env.define(
            "str".into(),
            Binding {
                ty: Ty::Fn(vec![Ty::GenericParam(0)], Box::new(Ty::Str)),
                mutable: false,
                kind: BindingKind::Function,
                is_deprecated: false,
            },
        );

        // int(value: any) → int
        env.define(
            "int".into(),
            Binding {
                ty: Ty::Fn(vec![Ty::GenericParam(0)], Box::new(Ty::Int)),
                mutable: false,
                kind: BindingKind::Function,
                is_deprecated: false,
            },
        );

        // float(value: any) → float
        env.define(
            "float".into(),
            Binding {
                ty: Ty::Fn(vec![Ty::GenericParam(0)], Box::new(Ty::Float)),
                mutable: false,
                kind: BindingKind::Function,
                is_deprecated: false,
            },
        );

        // len(collection: any) → int
        env.define(
            "len".into(),
            Binding {
                ty: Ty::Fn(vec![Ty::GenericParam(0)], Box::new(Ty::Int)),
                mutable: false,
                kind: BindingKind::Function,
                is_deprecated: false,
            },
        );

        // range(start: int, end: int) → List[int]
        env.define(
            "range".into(),
            Binding {
                ty: Ty::Fn(
                    vec![Ty::Int, Ty::Int],
                    Box::new(Ty::List(Box::new(Ty::Int))),
                ),
                mutable: false,
                kind: BindingKind::Function,
                is_deprecated: false,
            },
        );

        // null → Option[T]
        env.define(
            "null".into(),
            Binding {
                ty: Ty::Option(Box::new(Ty::GenericParam(0))),
                mutable: false,
                kind: BindingKind::Constructor,
                is_deprecated: false,
            },
        );

        // Unit → Ty::Unit. Used as the payload of `Ok(Unit)` and
        // `Result[Unit]` returns. eval/expr.rs already resolves the
        // ident; this is the typecheck-side binding so source like
        // `fn send() to Result[Unit] { return Ok(Unit) }` typechecks.
        env.define(
            "Unit".into(),
            Binding {
                ty: Ty::Unit,
                mutable: false,
                kind: BindingKind::Constructor,
                is_deprecated: false,
            },
        );

        // panic(msg: str) → Never. Canonical Vox surface but missing
        // from the typecheck builtin set; eval-side raises
        // EvalError::Panic when invoked.
        env.define(
            "panic".into(),
            Binding {
                ty: Ty::Fn(vec![Ty::Str], Box::new(Ty::Never)),
                mutable: false,
                kind: BindingKind::Function,
                is_deprecated: false,
            },
        );

        // ── Automation/Glue namespaces ────────────────────────

        // fs module
        env.define(
            "fs".into(),
            Binding {
                ty: Ty::Named("FsModule".into()),
                mutable: false,
                kind: BindingKind::Import,
                is_deprecated: false,
            },
        );

        // path module
        env.define(
            "path".into(),
            Binding {
                ty: Ty::Named("PathModule".into()),
                mutable: false,
                kind: BindingKind::Import,
                is_deprecated: false,
            },
        );

        // json module
        env.define(
            "json".into(),
            Binding {
                ty: Ty::Named("JsonModule".into()),
                mutable: false,
                kind: BindingKind::Import,
                is_deprecated: false,
            },
        );

        // process module
        env.define(
            "process".into(),
            Binding {
                ty: Ty::Named("ProcessModule".into()),
                mutable: false,
                kind: BindingKind::Import,
                is_deprecated: false,
            },
        );

        // env module
        env.define(
            "env".into(),
            Binding {
                ty: Ty::Named("EnvModule".into()),
                mutable: false,
                kind: BindingKind::Import,
                is_deprecated: false,
            },
        );

        // secrets module
        env.define(
            "secrets".into(),
            Binding {
                ty: Ty::Named("SecretsModule".into()),
                mutable: false,
                kind: BindingKind::Import,
                is_deprecated: false,
            },
        );

        // ── React/frontend bindings ───────────────────────────

        // use_state: fn(T) -> (T, fn(T) -> Unit)
        // use_state(initial: T) → (T, fn(T) → Unit)
        env.define(
            "use_state".into(),
            Binding {
                ty: Ty::Fn(
                    vec![Ty::GenericParam(0)],
                    Box::new(Ty::Tuple(vec![
                        Ty::GenericParam(0),
                        Ty::Fn(vec![Ty::GenericParam(0)], Box::new(Ty::Unit)),
                    ])),
                ),
                mutable: false,
                kind: BindingKind::Function,
                is_deprecated: false,
            },
        );

        // use_effect(fn() → Unit) → Unit
        env.define(
            "use_effect".into(),
            Binding {
                ty: Ty::Fn(vec![Ty::Fn(vec![], Box::new(Ty::Unit))], Box::new(Ty::Unit)),
                mutable: false,
                kind: BindingKind::Function,
                is_deprecated: false,
            },
        );

        env.define(
            "use_memo".into(),
            Binding {
                ty: Ty::Fn(
                    vec![Ty::Fn(vec![], Box::new(Ty::GenericParam(0)))],
                    Box::new(Ty::GenericParam(0)),
                ),
                mutable: false,
                kind: BindingKind::Function,
                is_deprecated: false,
            },
        );

        env.define(
            "use_ref".into(),
            Binding {
                ty: Ty::Fn(vec![Ty::GenericParam(0)], Box::new(Ty::GenericParam(0))),
                mutable: false,
                kind: BindingKind::Function,
                is_deprecated: false,
            },
        );

        env.define(
            "use_callback".into(),
            Binding {
                ty: Ty::Fn(vec![Ty::GenericParam(0)], Box::new(Ty::GenericParam(0))),
                mutable: false,
                kind: BindingKind::Function,
                is_deprecated: false,
            },
        );

        // DOM / synthetic types (field access is special-cased in `Checker`)
        env.define_type("KeyboardEvent".into(), Ty::Named("KeyboardEvent".into()));

        // ── HTTP/network module ───────────────────────────────

        // HTTP module binding
        env.define(
            "HTTP".into(),
            Binding {
                ty: Ty::Named("HTTPModule".into()),
                mutable: false,
                kind: BindingKind::Import,
                is_deprecated: false,
            },
        );

        // request binding removed from global scope
        // It is now injected into HTTP route scopes in check.rs

        // Claude LLM actor (built-in)
        env.define(
            "Claude".into(),
            Binding {
                ty: Ty::Named("ClaudeActor".into()),
                mutable: false,
                kind: BindingKind::Actor,
                is_deprecated: false,
            },
        );

        // Speech-to-text module (Oratio / Candle Whisper — codegen links `vox-oratio`)
        env.define(
            "Speech".into(),
            Binding {
                ty: Ty::Named("SpeechModule".into()),
                mutable: false,
                kind: BindingKind::Import,
                is_deprecated: false,
            },
        );

        // OpenClaw gateway module (WS-first runtime adapter).
        env.define(
            "OpenClaw".into(),
            Binding {
                ty: Ty::Named("OpenClawModule".into()),
                mutable: false,
                kind: BindingKind::Import,
                is_deprecated: false,
            },
        );

        // Chromium/CDP browser module (native runtime only).
        env.define(
            "Browser".into(),
            Binding {
                ty: Ty::Named("BrowserModule".into()),
                mutable: false,
                kind: BindingKind::Import,
                is_deprecated: false,
            },
        );

        // Mobile native bridge (std.mobile).
        env.define(
            "mobile".into(),
            Binding {
                ty: Ty::Named("StdMobileNs".into()),
                mutable: false,
                kind: BindingKind::Import,
                is_deprecated: false,
            },
        );

        // ── Method registrations ──────────────────────────────

        // List methods
        let mut list_methods = std::collections::HashMap::new();
        list_methods.insert(
            "append".into(),
            Ty::Fn(
                vec![Ty::GenericParam(0)],
                Box::new(Ty::List(Box::new(Ty::GenericParam(0)))),
            ),
        );
        list_methods.insert(
            "push".into(),
            Ty::Fn(
                vec![Ty::GenericParam(0)],
                Box::new(Ty::List(Box::new(Ty::GenericParam(0)))),
            ),
        );
        list_methods.insert(
            "get".into(),
            Ty::Fn(
                vec![Ty::Int],
                Box::new(Ty::Option(Box::new(Ty::GenericParam(0)))),
            ),
        );
        list_methods.insert("length".into(), Ty::Fn(vec![], Box::new(Ty::Int)));
        list_methods.insert("len".into(), Ty::Fn(vec![], Box::new(Ty::Int)));
        list_methods.insert("join".into(), Ty::Fn(vec![Ty::Str], Box::new(Ty::Str)));
        list_methods.insert(
            "map".into(),
            Ty::Fn(
                vec![Ty::Fn(
                    vec![Ty::GenericParam(0)],
                    Box::new(Ty::GenericParam(1)),
                )],
                Box::new(Ty::List(Box::new(Ty::GenericParam(1)))),
            ),
        );
        list_methods.insert(
            "filter".into(),
            Ty::Fn(
                vec![Ty::Fn(vec![Ty::GenericParam(0)], Box::new(Ty::Bool))],
                Box::new(Ty::List(Box::new(Ty::GenericParam(0)))),
            ),
        );
        list_methods.insert(
            "contains".into(),
            Ty::Fn(vec![Ty::GenericParam(0)], Box::new(Ty::Bool)),
        );
        methods.insert("List".into(), list_methods);

        // Fs module methods
        let mut fs_methods = std::collections::HashMap::new();
        fs_methods.insert(
            "read_file".into(),
            Ty::Fn(vec![Ty::Str], Box::new(Ty::Option(Box::new(Ty::Str)))),
        );
        fs_methods.insert(
            "write_file".into(),
            Ty::Fn(vec![Ty::Str, Ty::Str], Box::new(Ty::Bool)),
        );
        fs_methods.insert(
            "list_dir".into(),
            Ty::Fn(
                vec![Ty::Str],
                Box::new(Ty::Result(Box::new(Ty::List(Box::new(Ty::Str))), Box::new(Ty::Str))),
            ),
        );
        fs_methods.insert(
            "glob".into(),
            Ty::Fn(
                vec![Ty::Str],
                Box::new(Ty::Result(Box::new(Ty::List(Box::new(Ty::Str))), Box::new(Ty::Str))),
            ),
        );
        methods.insert("FsModule".into(), fs_methods);

        // Path module methods
        let mut path_methods = std::collections::HashMap::new();
        path_methods.insert(
            "join".into(),
            Ty::Fn(vec![Ty::Str, Ty::Str], Box::new(Ty::Str)),
        );
        methods.insert("PathModule".into(), path_methods);

        // Json module methods
        let mut json_methods = std::collections::HashMap::new();
        json_methods.insert(
            "stringify".into(),
            Ty::Fn(vec![Ty::GenericParam(0)], Box::new(Ty::Str)),
        );
        json_methods.insert(
            "parse".into(),
            Ty::Fn(vec![Ty::Str], Box::new(Ty::GenericParam(0))),
        );
        methods.insert("JsonModule".into(), json_methods);

        // Json opaque value type — produced by std.json.parse and walked via
        // typed accessors (object-shape methods + array methods + scalar reads).
        let mut json_value_methods = std::collections::HashMap::new();
        json_value_methods.insert(
            "get_str".into(),
            Ty::Fn(vec![Ty::Str], Box::new(Ty::Result(Box::new(Ty::Str), Box::new(Ty::Str)))),
        );
        json_value_methods.insert(
            "get_int".into(),
            Ty::Fn(vec![Ty::Str], Box::new(Ty::Result(Box::new(Ty::Int), Box::new(Ty::Str)))),
        );
        json_value_methods.insert(
            "get_float".into(),
            Ty::Fn(vec![Ty::Str], Box::new(Ty::Result(Box::new(Ty::Float), Box::new(Ty::Str)))),
        );
        json_value_methods.insert(
            "get_bool".into(),
            Ty::Fn(vec![Ty::Str], Box::new(Ty::Result(Box::new(Ty::Bool), Box::new(Ty::Str)))),
        );
        json_value_methods.insert(
            "get_object".into(),
            Ty::Fn(
                vec![Ty::Str],
                Box::new(Ty::Result(Box::new(Ty::Named("Json".into())), Box::new(Ty::Str))),
            ),
        );
        json_value_methods.insert(
            "get_array".into(),
            Ty::Fn(
                vec![Ty::Str],
                Box::new(Ty::Result(Box::new(Ty::Named("Json".into())), Box::new(Ty::Str))),
            ),
        );
        json_value_methods.insert("is_null".into(), Ty::Fn(vec![], Box::new(Ty::Bool)));
        json_value_methods.insert("length".into(), Ty::Fn(vec![], Box::new(Ty::Int)));
        json_value_methods.insert(
            "at".into(),
            Ty::Fn(
                vec![Ty::Int],
                Box::new(Ty::Result(Box::new(Ty::Named("Json".into())), Box::new(Ty::Str))),
            ),
        );
        json_value_methods.insert(
            "keys".into(),
            Ty::Fn(vec![], Box::new(Ty::List(Box::new(Ty::Str)))),
        );
        json_value_methods.insert("to_string".into(), Ty::Fn(vec![], Box::new(Ty::Str)));
        methods.insert("Json".into(), json_value_methods);

        // Process module methods
        let mut process_methods = std::collections::HashMap::new();
        let process_output = Ty::Record(vec![
            ("stdout".into(), Ty::Str),
            ("stderr".into(), Ty::Str),
            ("code".into(), Ty::Int),
        ]);
        process_methods.insert(
            "spawn".into(),
            Ty::Fn(
                vec![Ty::Str, Ty::List(Box::new(Ty::Str))],
                Box::new(Ty::Option(Box::new(process_output.clone()))),
            ),
        );
        process_methods.insert(
            "spawn_background".into(),
            Ty::Fn(
                vec![Ty::Str, Ty::List(Box::new(Ty::Str))],
                Box::new(Ty::Result(Box::new(Ty::Int), Box::new(Ty::Str))),
            ),
        );
        process_methods.insert(
            "run".into(),
            Ty::Fn(
                vec![Ty::Str, Ty::List(Box::new(Ty::Str))],
                Box::new(Ty::Option(Box::new(process_output.clone()))),
            ),
        );
        process_methods.insert(
            "exec".into(),
            Ty::Fn(
                vec![Ty::Str, Ty::List(Box::new(Ty::Str))],
                Box::new(Ty::Result(Box::new(Ty::Unit), Box::new(Ty::Str))),
            ),
        );
        process_methods.insert(
            "register_exit_command".into(),
            Ty::Fn(
                vec![Ty::Str, Ty::List(Box::new(Ty::Str))],
                Box::new(Ty::Result(Box::new(Ty::Unit), Box::new(Ty::Str))),
            ),
        );
        process_methods.insert("exit".into(), Ty::Fn(vec![Ty::Int], Box::new(Ty::Never)));
        methods.insert("ProcessModule".into(), process_methods);

        // Env module methods
        let mut env_methods = std::collections::HashMap::new();
        env_methods.insert(
            "get".into(),
            Ty::Fn(vec![Ty::Str], Box::new(Ty::Option(Box::new(Ty::Str)))),
        );
        env_methods.insert(
            "args".into(),
            Ty::Fn(vec![], Box::new(Ty::List(Box::new(Ty::Str)))),
        );
        env_methods.insert(
            "set".into(),
            Ty::Fn(vec![Ty::Str, Ty::Str], Box::new(Ty::Unit)),
        );
        methods.insert("EnvModule".into(), env_methods);

        // Secrets module methods
        let mut secrets_methods = std::collections::HashMap::new();
        secrets_methods.insert(
            "resolve".into(),
            Ty::Fn(vec![Ty::Str], Box::new(Ty::Option(Box::new(Ty::Str)))),
        );
        methods.insert("SecretsModule".into(), secrets_methods);

        // String methods
        let mut str_methods = std::collections::HashMap::new();
        str_methods.insert("length".into(), Ty::Fn(vec![], Box::new(Ty::Int)));
        str_methods.insert("contains".into(), Ty::Fn(vec![Ty::Str], Box::new(Ty::Bool)));
        str_methods.insert(
            "split".into(),
            Ty::Fn(vec![Ty::Str], Box::new(Ty::List(Box::new(Ty::Str)))),
        );
        str_methods.insert("trim".into(), Ty::Fn(vec![], Box::new(Ty::Str)));
        str_methods.insert("to_upper".into(), Ty::Fn(vec![], Box::new(Ty::Str)));
        str_methods.insert("to_lower".into(), Ty::Fn(vec![], Box::new(Ty::Str)));
        str_methods.insert(
            "replace".into(),
            Ty::Fn(vec![Ty::Str, Ty::Str], Box::new(Ty::Str)),
        );
        str_methods.insert(
            "ends_with".into(),
            Ty::Fn(vec![Ty::Str], Box::new(Ty::Bool)),
        );
        str_methods.insert(
            "starts_with".into(),
            Ty::Fn(vec![Ty::Str], Box::new(Ty::Bool)),
        );
        str_methods.insert(
            "slice".into(),
            Ty::Fn(vec![Ty::Int, Ty::Int], Box::new(Ty::Str)),
        );
        str_methods.insert(
            "char_at".into(),
            Ty::Fn(vec![Ty::Int], Box::new(Ty::Option(Box::new(Ty::Str)))),
        );
        str_methods.insert(
            "index_of".into(),
            Ty::Fn(vec![Ty::Str], Box::new(Ty::Option(Box::new(Ty::Int)))),
        );
        methods.insert("Str".into(), str_methods);

        // HTTP module methods
        let mut http_methods = std::collections::HashMap::new();
        http_methods.insert(
            "post".into(),
            Ty::Fn(
                vec![Ty::Str],
                Box::new(Ty::Result(Box::new(Ty::Named("Response".into())), Box::new(Ty::Str))),
            ),
        );
        http_methods.insert(
            "get".into(),
            Ty::Fn(
                vec![Ty::Str],
                Box::new(Ty::Result(Box::new(Ty::Named("Response".into())), Box::new(Ty::Str))),
            ),
        );
        http_methods.insert(
            "put".into(),
            Ty::Fn(
                vec![Ty::Str],
                Box::new(Ty::Result(Box::new(Ty::Named("Response".into())), Box::new(Ty::Str))),
            ),
        );
        http_methods.insert(
            "delete".into(),
            Ty::Fn(
                vec![Ty::Str],
                Box::new(Ty::Result(Box::new(Ty::Named("Response".into())), Box::new(Ty::Str))),
            ),
        );
        methods.insert("HTTPModule".into(), http_methods);

        // Speech module: transcribe(path: str) → Result[str] (refined display text)
        let mut speech_methods = std::collections::HashMap::new();
        speech_methods.insert(
            "transcribe".into(),
            Ty::Fn(vec![Ty::Str], Box::new(Ty::Result(Box::new(Ty::Str), Box::new(Ty::Str)))),
        );
        speech_methods.insert(
            "transcribe_microphone".into(),
            Ty::Fn(vec![], Box::new(Ty::Result(Box::new(Ty::Str), Box::new(Ty::Str)))),
        );
        methods.insert("SpeechModule".into(), speech_methods);

        // Regex / Match (std.regex compile output and find result).
        let mut regex_methods = std::collections::HashMap::new();
        regex_methods.insert("matches".into(), Ty::Fn(vec![Ty::Str], Box::new(Ty::Bool)));
        regex_methods.insert(
            "find".into(),
            Ty::Fn(
                vec![Ty::Str],
                Box::new(Ty::Option(Box::new(Ty::Named("Match".into())))),
            ),
        );
        regex_methods.insert(
            "find_all".into(),
            Ty::Fn(
                vec![Ty::Str],
                Box::new(Ty::List(Box::new(Ty::Named("Match".into())))),
            ),
        );
        methods.insert("Regex".into(), regex_methods);

        let mut match_methods = std::collections::HashMap::new();
        match_methods.insert(
            "group".into(),
            Ty::Fn(vec![Ty::Int], Box::new(Ty::Option(Box::new(Ty::Str)))),
        );
        methods.insert("Match".into(), match_methods);

        // OpenClaw module methods come from shared builtin registry entries.
        let mut openclaw_methods = std::collections::HashMap::new();
        for entry in builtin_registry_entries()
            .iter()
            .copied()
            .filter(|e| e.namespace == "OpenClaw")
        {
            let Some(params) = builtin_entry_param_tys(entry) else {
                continue;
            };
            openclaw_methods.insert(
                entry.name.to_string(),
                Ty::Fn(params, Box::new(builtin_entry_result_ty(entry))),
            );
        }
        methods.insert("OpenClawModule".into(), openclaw_methods);

        let mut browser_methods = std::collections::HashMap::new();
        for entry in builtin_registry_entries()
            .iter()
            .copied()
            .filter(|e| e.namespace == "Browser")
        {
            let Some(params) = builtin_entry_param_tys(entry) else {
                continue;
            };
            browser_methods.insert(
                entry.name.to_string(),
                Ty::Fn(params, Box::new(builtin_entry_result_ty(entry))),
            );
        }
        methods.insert("BrowserModule".into(), browser_methods);

        let mut mobile_methods = std::collections::HashMap::new();
        for entry in builtin_registry_entries()
            .iter()
            .copied()
            .filter(|e| e.namespace == "std.mobile")
        {
            let Some(params) = builtin_entry_param_tys(entry) else {
                continue;
            };
            mobile_methods.insert(
                entry.name.to_string(),
                Ty::Fn(params, Box::new(builtin_entry_result_ty(entry))),
            );
        }
        methods.insert("StdMobileNs".into(), mobile_methods);

        // Request methods
        let mut req_methods = std::collections::HashMap::new();
        req_methods.insert(
            "json".into(),
            Ty::Fn(vec![], Box::new(Ty::Named("JsonBody".into()))),
        );
        req_methods.insert("text".into(), Ty::Fn(vec![], Box::new(Ty::Str)));
        methods.insert("Request".into(), req_methods);

        // Option methods
        let mut option_methods = std::collections::HashMap::new();
        option_methods.insert(
            "unwrap".into(),
            Ty::Fn(vec![], Box::new(Ty::GenericParam(0))),
        );
        option_methods.insert("is_some".into(), Ty::Fn(vec![], Box::new(Ty::Bool)));
        option_methods.insert("is_none".into(), Ty::Fn(vec![], Box::new(Ty::Bool)));
        methods.insert("Option".into(), option_methods);

        // Response methods
        let mut resp_methods = std::collections::HashMap::new();
        resp_methods.insert("text".into(), Ty::Fn(vec![], Box::new(Ty::Str)));
        resp_methods.insert(
            "json".into(),
            Ty::Fn(vec![], Box::new(Ty::Named("JsonBody".into()))),
        );
        resp_methods.insert("status".into(), Ty::Fn(vec![], Box::new(Ty::Int)));
        methods.insert("Response".into(), resp_methods);

        // Result methods
        let mut result_methods = std::collections::HashMap::new();
        result_methods.insert(
            "unwrap".into(),
            Ty::Fn(vec![], Box::new(Ty::GenericParam(0))),
        );
        result_methods.insert("is_ok".into(), Ty::Fn(vec![], Box::new(Ty::Bool)));
        result_methods.insert("is_err".into(), Ty::Fn(vec![], Box::new(Ty::Bool)));
        methods.insert("Result".into(), result_methods);

        Self { methods }
    }

    /// Look up a method on a given type.
    pub fn lookup_method(&self, obj_ty: &Ty, method: &str) -> Option<Ty> {
        if let Ty::Table(name, fields) = obj_ty {
            // Row type: use the Named alias of the @table so callers writing
            // `fn list_todos() to List[Todo]` unify with what db.Todo.all()
            // returns. The Record-shape form is kept as a fallback for the
            // insert input (so { ... } object literals still type-check
            // against the column shape).
            let row_ty = Ty::Named(name.clone());
            let record_ty = Ty::Record(fields.clone());
            return match method {
                "insert" => {
                    // insert(item: Record) -> Result[Id[T]] / Result[int]
                    // The input is still Record-shaped so { title: ... }
                    // literals match. The output is the row's Named type
                    // wrapped — callers expecting Result[Id[T]] need a
                    // follow-on for the Id[T] codepath; today the int
                    // return matches the existing examples (blog_fullstack
                    // returns `Result[Id[Post]]` against Result[int] —
                    // that's a separate gap).
                    Some(Ty::Fn(
                        vec![record_ty.clone()],
                        Box::new(Ty::Result(Box::new(Ty::Int), Box::new(Ty::Str))),
                    ))
                }
                "get" => {
                    // get(id: int) -> Result[Option[Row]]
                    Some(Ty::Fn(
                        vec![Ty::Int],
                        Box::new(Ty::Result(Box::new(Ty::Option(Box::new(row_ty.clone()))), Box::new(Ty::Str))),
                    ))
                }
                "delete" => {
                    // delete(id: int) -> Result[Unit]
                    Some(Ty::Fn(
                        vec![Ty::Int],
                        Box::new(Ty::Result(Box::new(Ty::Unit), Box::new(Ty::Str))),
                    ))
                }
                "query" => {
                    // query(sql: str) -> Result[List[Row]]
                    Some(Ty::Fn(
                        vec![Ty::Str],
                        Box::new(Ty::Result(Box::new(Ty::List(Box::new(row_ty.clone()))), Box::new(Ty::Str))),
                    ))
                }
                "all" => {
                    // all() -> Result[List[Row]]
                    Some(Ty::Fn(
                        vec![],
                        Box::new(Ty::Result(Box::new(Ty::List(Box::new(row_ty.clone()))), Box::new(Ty::Str))),
                    ))
                }
                "count" => Some(Ty::Fn(vec![], Box::new(Ty::Result(Box::new(Ty::Int), Box::new(Ty::Str))))),
                "find" => {
                    // find(id: int) -> Result[Option[Row]]
                    Some(Ty::Fn(
                        vec![Ty::Int],
                        Box::new(Ty::Result(Box::new(Ty::Option(Box::new(row_ty.clone()))), Box::new(Ty::Str))),
                    ))
                }
                "filter" => {
                    // filter({ field: value, ... }) -> Result[List[Row]]
                    // The predicate input mirrors the row column shape so
                    // partial-match object literals (`{ name: "x" }`) type-
                    // check against the table's columns.
                    Some(Ty::Fn(
                        vec![record_ty.clone()],
                        Box::new(Ty::Result(Box::new(Ty::List(Box::new(row_ty.clone()))), Box::new(Ty::Str))),
                    ))
                }
                "update" => {
                    // update(id: int, { field: value, ... }) -> Result[Unit]
                    Some(Ty::Fn(
                        vec![Ty::Int, record_ty.clone()],
                        Box::new(Ty::Result(Box::new(Ty::Unit), Box::new(Ty::Str))),
                    ))
                }
                "first" => {
                    // first() -> Result[Option[Row]]
                    Some(Ty::Fn(
                        vec![],
                        Box::new(Ty::Result(Box::new(Ty::Option(Box::new(row_ty))), Box::new(Ty::Str))),
                    ))
                }
                _ => None,
            };
        }

        if let Ty::Record(_) = obj_ty {
            return match method {
                "get" => {
                    // get(key: str) -> Option[any]
                    Some(Ty::Fn(
                        vec![Ty::Str],
                        Box::new(Ty::Option(Box::new(Ty::GenericParam(0)))),
                    ))
                }
                "keys" => {
                    // keys() -> List[str]
                    Some(Ty::Fn(vec![], Box::new(Ty::List(Box::new(Ty::Str)))))
                }
                _ => None,
            };
        }

        let type_key = match obj_ty {
            Ty::Named(n) => n.as_str(),
            Ty::List(_) => "List",
            Ty::Str => "Str",
            Ty::Result(_, _) => "Result",
            Ty::Option(_) => "Option",
            _ => return None,
        };
        self.methods.get(type_key)?.get(method).cloned()
    }

    /// Look up a variable in builtins (legacy interface, used by old infer code).
    pub fn lookup_var(&self, name: &str) -> Option<Ty> {
        // This is now handled by TypeEnv, so this method is a no-op.
        // Kept for backward compatibility during migration.
        let _ = std::hint::black_box(name.as_ptr() as usize);
        None
    }
}

impl Default for BuiltinTypes {
    fn default() -> Self {
        let mut env = TypeEnv::new();
        Self::register_all(&mut env)
    }
}
