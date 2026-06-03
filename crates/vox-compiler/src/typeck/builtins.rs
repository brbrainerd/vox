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

        // Ok(value: T) → Result[T]
        env.define(
            "Ok".into(),
            Binding {
                ty: Ty::Fn(
                    vec![Ty::GenericParam(0)],
                    Box::new(Ty::Result(Box::new(Ty::GenericParam(0)))),
                ),
                mutable: false,
                kind: BindingKind::Constructor,
                is_deprecated: false,
            },
        );
        // Error(message: str) → Result[T]
        env.define(
            "Error".into(),
            Binding {
                ty: Ty::Fn(
                    vec![Ty::Str],
                    // Error returns Result[T]
                    Box::new(Ty::Result(Box::new(Ty::GenericParam(0)))),
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
        // Unit — the sole inhabitant of the unit type. Used in `return Unit`,
        // `Ok(Unit)`, and `Result[Unit]` patterns to signal "no meaningful
        // payload." Registered alongside `true`/`false` as a zero-argument
        // constructor constant.
        env.define(
            "Unit".into(),
            Binding {
                ty: Ty::Unit,
                mutable: false,
                kind: BindingKind::Constructor,
                is_deprecated: false,
            },
        );

        // ── Standard library functions ────────────────────────

        // print(value: str) → Unit
        // NOTE: The call checker special-cases `print` to accept any single
        // argument type, so `print(42)`, `print(true)`, etc. all type-check.
        // The Str param here is a nominal fallback for signature display only.
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

        // panic(message: str) → Unit
        // Terminates the current execution with an error message. Return type is
        // Unit rather than Never so that callers in void-returning functions don't
        // trigger return-type mismatches.
        env.define(
            "panic".into(),
            Binding {
                ty: Ty::Fn(vec![Ty::Str], Box::new(Ty::Unit)),
                mutable: false,
                kind: BindingKind::Function,
                is_deprecated: false,
            },
        );

        // chr(code: int) → str
        // Returns the single-character string for a Unicode code point.
        // Complement of s.ord().
        env.define(
            "chr".into(),
            Binding {
                ty: Ty::Fn(vec![Ty::Int], Box::new(Ty::Str)),
                mutable: false,
                kind: BindingKind::Function,
                is_deprecated: false,
            },
        );

        // abs(n: int) → int   (global convenience — also available as n.abs())
        env.define(
            "abs".into(),
            Binding {
                ty: Ty::Fn(vec![Ty::Int], Box::new(Ty::Int)),
                mutable: false,
                kind: BindingKind::Function,
                is_deprecated: false,
            },
        );

        // max(a: int, b: int) → int   (two-arg global)
        env.define(
            "max".into(),
            Binding {
                ty: Ty::Fn(vec![Ty::Int, Ty::Int], Box::new(Ty::Int)),
                mutable: false,
                kind: BindingKind::Function,
                is_deprecated: false,
            },
        );

        // min(a: int, b: int) → int   (two-arg global)
        env.define(
            "min".into(),
            Binding {
                ty: Ty::Fn(vec![Ty::Int, Ty::Int], Box::new(Ty::Int)),
                mutable: false,
                kind: BindingKind::Function,
                is_deprecated: false,
            },
        );

        // sorted(list: list[T]) → list[T]  (free-function form of list.sorted())
        env.define(
            "sorted".into(),
            Binding {
                ty: Ty::Fn(
                    vec![Ty::List(Box::new(Ty::GenericParam(0)))],
                    Box::new(Ty::List(Box::new(Ty::GenericParam(0)))),
                ),
                mutable: false,
                kind: BindingKind::Function,
                is_deprecated: false,
            },
        );

        // sum(list: list[T]) → T  (free-function form of list.sum())
        env.define(
            "sum".into(),
            Binding {
                ty: Ty::Fn(
                    vec![Ty::List(Box::new(Ty::GenericParam(0)))],
                    Box::new(Ty::GenericParam(0)),
                ),
                mutable: false,
                kind: BindingKind::Function,
                is_deprecated: false,
            },
        );

        // has_capability(token: cap) → bool
        // Runtime predicate that checks whether the supplied capability token is
        // valid and has not been revoked. Used in the platform capability model.
        env.define(
            "has_capability".into(),
            Binding {
                ty: Ty::Fn(vec![Ty::Named("cap".into())], Box::new(Ty::Bool)),
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

        // regex module (eval-side registered 2026-05-23). The compiled-Regex
        // value is a separate Ty::Named("Regex") that already has methods —
        // this binding is the namespace for the free-function calls
        // `regex.replace` / `regex.is_match` / `regex.captures`.
        env.define(
            "regex".into(),
            Binding {
                ty: Ty::Named("RegexModule".into()),
                mutable: false,
                kind: BindingKind::Import,
                is_deprecated: false,
            },
        );

        // log module (level-tagged stderr-channel diagnostics).
        env.define(
            "log".into(),
            Binding {
                ty: Ty::Named("LogModule".into()),
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

        // Static scraping module (fetch + CSS-select; no browser).
        env.define(
            "Scrape".into(),
            Binding {
                ty: Ty::Named("ScrapeModule".into()),
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
        // Closure-taking iterator methods (eval impls in apply_closure_method).
        // Per closures RFC §9.6 — typeck registers Ty::Fn closure params so
        // `xs.any(fn(x) { ... })` etc. typechecks.
        list_methods.insert(
            "any".into(),
            Ty::Fn(
                vec![Ty::Fn(vec![Ty::GenericParam(0)], Box::new(Ty::Bool))],
                Box::new(Ty::Bool),
            ),
        );
        list_methods.insert(
            "all".into(),
            Ty::Fn(
                vec![Ty::Fn(vec![Ty::GenericParam(0)], Box::new(Ty::Bool))],
                Box::new(Ty::Bool),
            ),
        );
        list_methods.insert(
            "for_each".into(),
            Ty::Fn(
                vec![Ty::Fn(vec![Ty::GenericParam(0)], Box::new(Ty::Unit))],
                Box::new(Ty::Unit),
            ),
        );
        // `fold(init, fn(acc, item) { ... })` — accumulator + closure.
        list_methods.insert(
            "fold".into(),
            Ty::Fn(
                vec![
                    Ty::GenericParam(1),
                    Ty::Fn(
                        vec![Ty::GenericParam(1), Ty::GenericParam(0)],
                        Box::new(Ty::GenericParam(1)),
                    ),
                ],
                Box::new(Ty::GenericParam(1)),
            ),
        );
        // sorted() — returns a new sorted copy of the list.
        list_methods.insert(
            "sorted".into(),
            Ty::Fn(vec![], Box::new(Ty::List(Box::new(Ty::GenericParam(0))))),
        );
        // reversed() — returns a new reversed copy of the list.
        list_methods.insert(
            "reversed".into(),
            Ty::Fn(vec![], Box::new(Ty::List(Box::new(Ty::GenericParam(0))))),
        );
        // reverse() — in-place reverse, returns unit (mutation variant).
        list_methods.insert("reverse".into(), Ty::Fn(vec![], Box::new(Ty::Unit)));
        // sum() — sums numeric elements; registered as T→T for both int and float lists.
        list_methods.insert("sum".into(), Ty::Fn(vec![], Box::new(Ty::GenericParam(0))));
        // max() → Option[T] — largest element.
        list_methods.insert(
            "max".into(),
            Ty::Fn(vec![], Box::new(Ty::Option(Box::new(Ty::GenericParam(0))))),
        );
        // min() → Option[T] — smallest element.
        list_methods.insert(
            "min".into(),
            Ty::Fn(vec![], Box::new(Ty::Option(Box::new(Ty::GenericParam(0))))),
        );
        // flatten() → List[T] — flattens one level (list[list[T]] → list[T]).
        list_methods.insert(
            "flatten".into(),
            Ty::Fn(vec![], Box::new(Ty::List(Box::new(Ty::GenericParam(0))))),
        );
        // first() → Option[T] and last() → Option[T] — safe head/tail.
        list_methods.insert(
            "first".into(),
            Ty::Fn(vec![], Box::new(Ty::Option(Box::new(Ty::GenericParam(0))))),
        );
        list_methods.insert(
            "last".into(),
            Ty::Fn(vec![], Box::new(Ty::Option(Box::new(Ty::GenericParam(0))))),
        );
        // is_empty() → bool
        list_methods.insert("is_empty".into(), Ty::Fn(vec![], Box::new(Ty::Bool)));
        // pop() → Option[T] — removes and returns last element.
        list_methods.insert(
            "pop".into(),
            Ty::Fn(vec![], Box::new(Ty::Option(Box::new(Ty::GenericParam(0))))),
        );
        // index(val) / find_index(val) → int (-1 if absent)
        list_methods.insert(
            "index".into(),
            Ty::Fn(vec![Ty::GenericParam(0)], Box::new(Ty::Int)),
        );
        list_methods.insert(
            "find_index".into(),
            Ty::Fn(vec![Ty::GenericParam(0)], Box::new(Ty::Int)),
        );
        // count(val) → int — occurrences of val
        list_methods.insert(
            "count".into(),
            Ty::Fn(vec![Ty::GenericParam(0)], Box::new(Ty::Int)),
        );
        // extend(other) → List[T] — append all elements from other
        list_methods.insert(
            "extend".into(),
            Ty::Fn(
                vec![Ty::List(Box::new(Ty::GenericParam(0)))],
                Box::new(Ty::List(Box::new(Ty::GenericParam(0)))),
            ),
        );
        // remove(val) → List[T] — new list with first occurrence of val removed
        list_methods.insert(
            "remove".into(),
            Ty::Fn(
                vec![Ty::GenericParam(0)],
                Box::new(Ty::List(Box::new(Ty::GenericParam(0)))),
            ),
        );
        // remove_at(i) → List[T] — new list without element at index i
        list_methods.insert(
            "remove_at".into(),
            Ty::Fn(
                vec![Ty::Int],
                Box::new(Ty::List(Box::new(Ty::GenericParam(0)))),
            ),
        );
        // zip(other) → List[List[T]] — list of [a,b] pairs
        list_methods.insert(
            "zip".into(),
            Ty::Fn(
                vec![Ty::List(Box::new(Ty::GenericParam(0)))],
                Box::new(Ty::List(Box::new(Ty::List(Box::new(Ty::GenericParam(0)))))),
            ),
        );
        // enumerate() → List[List[T]] — [[0, a], [1, b], ...]
        list_methods.insert(
            "enumerate".into(),
            Ty::Fn(
                vec![],
                Box::new(Ty::List(Box::new(Ty::List(Box::new(Ty::GenericParam(0)))))),
            ),
        );
        // slice_list(start, end?) → List[T]
        list_methods.insert(
            "slice_list".into(),
            Ty::Fn(
                vec![Ty::Int],
                Box::new(Ty::List(Box::new(Ty::GenericParam(0)))),
            ),
        );
        // sorted_by_key(fn) → List[T] — closure key sort
        list_methods.insert(
            "sorted_by_key".into(),
            Ty::Fn(
                vec![Ty::Fn(
                    vec![Ty::GenericParam(0)],
                    Box::new(Ty::GenericParam(1)),
                )],
                Box::new(Ty::List(Box::new(Ty::GenericParam(0)))),
            ),
        );
        list_methods.insert(
            "sort_by_key".into(),
            Ty::Fn(
                vec![Ty::Fn(
                    vec![Ty::GenericParam(0)],
                    Box::new(Ty::GenericParam(1)),
                )],
                Box::new(Ty::List(Box::new(Ty::GenericParam(0)))),
            ),
        );
        // sorted_by / sort_by — comparator closure
        list_methods.insert(
            "sorted_by".into(),
            Ty::Fn(
                vec![Ty::Fn(
                    vec![Ty::GenericParam(0), Ty::GenericParam(0)],
                    Box::new(Ty::Int),
                )],
                Box::new(Ty::List(Box::new(Ty::GenericParam(0)))),
            ),
        );
        list_methods.insert(
            "sort_by".into(),
            Ty::Fn(
                vec![Ty::Fn(
                    vec![Ty::GenericParam(0), Ty::GenericParam(0)],
                    Box::new(Ty::Int),
                )],
                Box::new(Ty::List(Box::new(Ty::GenericParam(0)))),
            ),
        );
        methods.insert("List".into(), list_methods);

        // Fs module methods. Every entry mirrors a registered arm in
        // `crates/vox-compiler/src/eval/builtins.rs`. Adding methods here without
        // a matching eval impl will cause typecheck to pass and runtime to fail
        // — the stdlib-coverage gate at `vox audit stdlib-coverage` catches this
        // direction of drift (registered_but_undocumented warns + corpus runs).
        let mut fs_methods = std::collections::HashMap::new();
        // `fs.read` / `fs.read_file` / `fs.read_to_string` share an or-pattern.
        for name in ["read", "read_file", "read_to_string"] {
            fs_methods.insert(
                name.into(),
                Ty::Fn(vec![Ty::Str], Box::new(Ty::Result(Box::new(Ty::Str)))),
            );
        }
        // `fs.write` / `fs.write_file` / `fs.write_to_file` are or-pattern aliases.
        for name in ["write", "write_file", "write_to_file"] {
            fs_methods.insert(
                name.into(),
                Ty::Fn(
                    vec![Ty::Str, Ty::Str],
                    Box::new(Ty::Result(Box::new(Ty::Bool))),
                ),
            );
        }
        // `fs.cwd` — current working directory.
        fs_methods.insert(
            "cwd".into(),
            Ty::Fn(vec![], Box::new(Ty::Result(Box::new(Ty::Str)))),
        );
        // `fs.copy(src, dst)` — copy a file. Eval impl uses `std::fs::copy`.
        fs_methods.insert(
            "copy".into(),
            Ty::Fn(
                vec![Ty::Str, Ty::Str],
                Box::new(Ty::Result(Box::new(Ty::Bool))),
            ),
        );
        // `fs.remove(path)` — remove a file. For directories use remove_dir_all.
        fs_methods.insert(
            "remove".into(),
            Ty::Fn(vec![Ty::Str], Box::new(Ty::Result(Box::new(Ty::Bool)))),
        );
        // `fs.walk(dir)` / `fs.list_recursive(dir)` — recursive lister; eval
        // aliases both to a `**/*` glob expansion.
        for name in ["walk", "list_recursive"] {
            fs_methods.insert(
                name.into(),
                Ty::Fn(
                    vec![Ty::Str],
                    Box::new(Ty::Result(Box::new(Ty::List(Box::new(Ty::Str))))),
                ),
            );
        }
        fs_methods.insert("exists".into(), Ty::Fn(vec![Ty::Str], Box::new(Ty::Bool)));
        fs_methods.insert("is_file".into(), Ty::Fn(vec![Ty::Str], Box::new(Ty::Bool)));
        fs_methods.insert("is_dir".into(), Ty::Fn(vec![Ty::Str], Box::new(Ty::Bool)));
        fs_methods.insert(
            "list_dir".into(),
            Ty::Fn(
                vec![Ty::Str],
                Box::new(Ty::Result(Box::new(Ty::List(Box::new(Ty::Str))))),
            ),
        );
        fs_methods.insert(
            "list_dir_detailed".into(),
            Ty::Fn(
                vec![Ty::Str],
                Box::new(Ty::Result(Box::new(Ty::List(Box::new(Ty::Record(vec![
                    ("name".into(), Ty::Str),
                    ("path".into(), Ty::Str),
                    ("is_dir".into(), Ty::Bool),
                ])))))),
            ),
        );
        fs_methods.insert(
            "glob".into(),
            Ty::Fn(
                vec![Ty::Str],
                Box::new(Ty::Result(Box::new(Ty::List(Box::new(Ty::Str))))),
            ),
        );
        fs_methods.insert(
            "mkdir".into(),
            Ty::Fn(vec![Ty::Str], Box::new(Ty::Result(Box::new(Ty::Unit)))),
        );
        fs_methods.insert(
            "remove_dir_all".into(),
            Ty::Fn(vec![Ty::Str], Box::new(Ty::Result(Box::new(Ty::Unit)))),
        );
        fs_methods.insert(
            "stat".into(),
            Ty::Fn(
                vec![Ty::Str],
                Box::new(Ty::Result(Box::new(Ty::Record(vec![
                    ("is_dir".into(), Ty::Bool),
                    ("is_file".into(), Ty::Bool),
                    ("size".into(), Ty::Int),
                ])))),
            ),
        );
        methods.insert("FsModule".into(), fs_methods);

        // Path module methods. `extension` / `parent` / `file_name` / `stem` /
        // `is_absolute` registered alongside `path.join` 2026-05-23 (audit doc §10).
        let mut path_methods = std::collections::HashMap::new();
        path_methods.insert(
            "join".into(),
            Ty::Fn(vec![Ty::Str, Ty::Str], Box::new(Ty::Str)),
        );
        for name in ["extension", "parent", "file_name", "stem"] {
            path_methods.insert(name.into(), Ty::Fn(vec![Ty::Str], Box::new(Ty::Str)));
        }
        path_methods.insert(
            "is_absolute".into(),
            Ty::Fn(vec![Ty::Str], Box::new(Ty::Bool)),
        );
        methods.insert("PathModule".into(), path_methods);

        // Regex module methods (the free-function namespace, distinct from
        // `Ty::Named("Regex")` which is the compiled-Regex value type with its
        // own method set below). Eval-side registered 2026-05-23.
        let mut regex_module_methods = std::collections::HashMap::new();
        regex_module_methods.insert(
            "replace".into(),
            Ty::Fn(vec![Ty::Str, Ty::Str, Ty::Str], Box::new(Ty::Str)),
        );
        // regex.find(haystack, pattern) → Option[str]  — first match substring.
        regex_module_methods.insert(
            "find".into(),
            Ty::Fn(
                vec![Ty::Str, Ty::Str],
                Box::new(Ty::Option(Box::new(Ty::Str))),
            ),
        );
        regex_module_methods.insert(
            "is_match".into(),
            Ty::Fn(vec![Ty::Str, Ty::Str], Box::new(Ty::Bool)),
        );
        regex_module_methods.insert(
            "captures".into(),
            Ty::Fn(
                vec![Ty::Str, Ty::Str],
                Box::new(Ty::Option(Box::new(Ty::List(Box::new(Ty::Str))))),
            ),
        );
        // `regex.compile(pattern) -> Result[Regex]` — pre-validate pattern.
        // Returns the pattern string back on success (no dedicated compiled-
        // Regex value type in interp yet; see audit doc §10 for rationale).
        regex_module_methods.insert(
            "compile".into(),
            Ty::Fn(vec![Ty::Str], Box::new(Ty::Result(Box::new(Ty::Str)))),
        );
        methods.insert("RegexModule".into(), regex_module_methods);

        // Log module methods (level-tagged stderr-channel output).
        let mut log_methods = std::collections::HashMap::new();
        for name in ["debug", "info", "warn", "error"] {
            log_methods.insert(name.into(), Ty::Fn(vec![Ty::Str], Box::new(Ty::Unit)));
        }
        methods.insert("LogModule".into(), log_methods);

        // Json module methods
        let mut json_methods = std::collections::HashMap::new();
        json_methods.insert(
            "stringify".into(),
            Ty::Fn(vec![Ty::GenericParam(0)], Box::new(Ty::Str)),
        );
        // RFC json-ergonomics-rfc-2026-05-23 §4.1: parse returns a typed
        // `Result[Json]` so the chainable Json method surface (`get`, `at`,
        // `pointer`, `as_str`, ...) actually dispatches at typecheck.
        json_methods.insert(
            "parse".into(),
            Ty::Fn(
                vec![Ty::Str],
                Box::new(Ty::Result(Box::new(Ty::Named("Json".into())))),
            ),
        );
        methods.insert("JsonModule".into(), json_methods);

        // Json opaque value type — produced by std.json.parse and walked via
        // typed accessors. Strict-Option API per
        // json-ergonomics-rfc-2026-05-23. Every fallible access returns
        // Option[T]; coercion at leaves and navigation at intermediates
        // share one discipline.
        let mut json_value_methods = std::collections::HashMap::new();
        let json_ty = Ty::Named("Json".into());

        // Navigation (single hop) — all Option[Json].
        json_value_methods.insert(
            "get".into(),
            Ty::Fn(
                vec![Ty::Str],
                Box::new(Ty::Option(Box::new(json_ty.clone()))),
            ),
        );
        json_value_methods.insert(
            "at".into(),
            Ty::Fn(
                vec![Ty::Int],
                Box::new(Ty::Option(Box::new(json_ty.clone()))),
            ),
        );
        json_value_methods.insert(
            "pointer".into(),
            Ty::Fn(
                vec![Ty::Str],
                Box::new(Ty::Option(Box::new(json_ty.clone()))),
            ),
        );

        // Leaf coercion — Option[T]; None on wrong type OR is-null.
        json_value_methods.insert(
            "as_str".into(),
            Ty::Fn(vec![], Box::new(Ty::Option(Box::new(Ty::Str)))),
        );
        json_value_methods.insert(
            "as_int".into(),
            Ty::Fn(vec![], Box::new(Ty::Option(Box::new(Ty::Int)))),
        );
        json_value_methods.insert(
            "as_float".into(),
            Ty::Fn(vec![], Box::new(Ty::Option(Box::new(Ty::Float)))),
        );
        json_value_methods.insert(
            "as_bool".into(),
            Ty::Fn(vec![], Box::new(Ty::Option(Box::new(Ty::Bool)))),
        );
        json_value_methods.insert(
            "as_array".into(),
            Ty::Fn(
                vec![],
                Box::new(Ty::Option(Box::new(Ty::List(Box::new(json_ty.clone()))))),
            ),
        );
        json_value_methods.insert(
            "as_object".into(),
            Ty::Fn(vec![], Box::new(Ty::Option(Box::new(json_ty.clone())))),
        );

        // Inspection (no Option).
        json_value_methods.insert("is_null".into(), Ty::Fn(vec![], Box::new(Ty::Bool)));
        json_value_methods.insert("has".into(), Ty::Fn(vec![Ty::Str], Box::new(Ty::Bool)));
        json_value_methods.insert(
            "length".into(),
            Ty::Fn(vec![], Box::new(Ty::Option(Box::new(Ty::Int)))),
        );
        json_value_methods.insert(
            "keys".into(),
            Ty::Fn(
                vec![],
                Box::new(Ty::Option(Box::new(Ty::List(Box::new(Ty::Str))))),
            ),
        );
        json_value_methods.insert("to_string".into(), Ty::Fn(vec![], Box::new(Ty::Str)));
        methods.insert("Json".into(), json_value_methods);

        // Process module methods. `run` / `spawn` return `Option[Record]`
        // matching the eval impl (Some(Object) on success, None on spawn
        // failure). Corpus scripts that access `.code` directly without
        // unwrapping the Option are bugs and surface as type errors here —
        // that's the diagnostic doing its job.
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
                Box::new(Ty::Result(Box::new(Ty::Int))),
            ),
        );
        process_methods.insert(
            "run".into(),
            Ty::Fn(
                vec![Ty::Str, Ty::List(Box::new(Ty::Str))],
                Box::new(Ty::Option(Box::new(process_output.clone()))),
            ),
        );
        // `run_ex` — variant of `run` that also takes a cwd + env map. Same
        // return shape. The 3-arg form matches the corpus call sites.
        process_methods.insert(
            "run_ex".into(),
            Ty::Fn(
                vec![Ty::Str, Ty::List(Box::new(Ty::Str)), Ty::Str],
                Box::new(Ty::Result(Box::new(process_output.clone()))),
            ),
        );
        // `run_capture_lines` — stdout split on newlines.
        process_methods.insert(
            "run_capture_lines".into(),
            Ty::Fn(
                vec![Ty::Str, Ty::List(Box::new(Ty::Str))],
                Box::new(Ty::Result(Box::new(Ty::List(Box::new(Ty::Str))))),
            ),
        );
        // `process.cwd` — same as `fs.cwd`, aliased under process namespace.
        process_methods.insert(
            "cwd".into(),
            Ty::Fn(vec![], Box::new(Ty::Result(Box::new(Ty::Str)))),
        );
        // `process.which(cmd)` — locate binary on PATH; cross-platform.
        process_methods.insert(
            "which".into(),
            Ty::Fn(vec![Ty::Str], Box::new(Ty::Option(Box::new(Ty::Str)))),
        );
        process_methods.insert(
            "exec".into(),
            Ty::Fn(
                vec![Ty::Str, Ty::List(Box::new(Ty::Str))],
                Box::new(Ty::Result(Box::new(Ty::Unit))),
            ),
        );
        process_methods.insert(
            "register_exit_command".into(),
            Ty::Fn(
                vec![Ty::Str, Ty::List(Box::new(Ty::Str))],
                Box::new(Ty::Result(Box::new(Ty::Unit))),
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

        // String methods. Mirror the eval-side method dispatch on
        // `VoxValue::Str`. The `to_lowercase` / `to_uppercase` aliases match
        // eval's or-pattern arms.
        let mut str_methods = std::collections::HashMap::new();
        str_methods.insert("length".into(), Ty::Fn(vec![], Box::new(Ty::Int)));
        str_methods.insert("len".into(), Ty::Fn(vec![], Box::new(Ty::Int)));
        str_methods.insert("chars_count".into(), Ty::Fn(vec![], Box::new(Ty::Int)));
        str_methods.insert("is_empty".into(), Ty::Fn(vec![], Box::new(Ty::Bool)));
        str_methods.insert("contains".into(), Ty::Fn(vec![Ty::Str], Box::new(Ty::Bool)));
        str_methods.insert(
            "split".into(),
            Ty::Fn(vec![Ty::Str], Box::new(Ty::List(Box::new(Ty::Str)))),
        );
        for name in ["trim", "trim_start", "trim_end"] {
            str_methods.insert(name.into(), Ty::Fn(vec![], Box::new(Ty::Str)));
        }
        for name in ["to_upper", "to_uppercase", "to_lower", "to_lowercase"] {
            str_methods.insert(name.into(), Ty::Fn(vec![], Box::new(Ty::Str)));
        }
        for name in ["to_str", "to_string"] {
            str_methods.insert(name.into(), Ty::Fn(vec![], Box::new(Ty::Str)));
        }
        str_methods.insert(
            "replace".into(),
            Ty::Fn(vec![Ty::Str, Ty::Str], Box::new(Ty::Str)),
        );
        str_methods.insert("repeat".into(), Ty::Fn(vec![Ty::Int], Box::new(Ty::Str)));
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
        // chars() — iterate over the string as a list of single-character strings.
        // This is the primary Vox idiom for character-level string processing.
        str_methods.insert(
            "chars".into(),
            Ty::Fn(vec![], Box::new(Ty::List(Box::new(Ty::Str)))),
        );
        // ord() — Unicode code point of a single character (str).
        str_methods.insert("ord".into(), Ty::Fn(vec![], Box::new(Ty::Int)));
        // bytes() — UTF-8 byte list.
        str_methods.insert(
            "bytes".into(),
            Ty::Fn(vec![], Box::new(Ty::List(Box::new(Ty::Int)))),
        );
        // to_int() → Option[int] — parse string as integer (None on failure).
        str_methods.insert(
            "to_int".into(),
            Ty::Fn(vec![], Box::new(Ty::Option(Box::new(Ty::Int)))),
        );
        // to_float() → Option[float] — parse string as float (None on failure).
        str_methods.insert(
            "to_float".into(),
            Ty::Fn(vec![], Box::new(Ty::Option(Box::new(Ty::Float)))),
        );
        // count(sub) → int — non-overlapping occurrences of substring
        str_methods.insert("count".into(), Ty::Fn(vec![Ty::Str], Box::new(Ty::Int)));
        // is_alpha / is_digit / is_alnum / is_upper / is_lower
        for name in ["is_alpha", "is_digit", "is_alnum", "is_upper", "is_lower"] {
            str_methods.insert(name.into(), Ty::Fn(vec![], Box::new(Ty::Bool)));
        }
        methods.insert("Str".into(), str_methods);

        // HTTP module methods
        let mut http_methods = std::collections::HashMap::new();
        http_methods.insert(
            "post".into(),
            Ty::Fn(
                vec![Ty::Str],
                Box::new(Ty::Result(Box::new(Ty::Named("Response".into())))),
            ),
        );
        http_methods.insert(
            "get".into(),
            Ty::Fn(
                vec![Ty::Str],
                Box::new(Ty::Result(Box::new(Ty::Named("Response".into())))),
            ),
        );
        http_methods.insert(
            "put".into(),
            Ty::Fn(
                vec![Ty::Str],
                Box::new(Ty::Result(Box::new(Ty::Named("Response".into())))),
            ),
        );
        http_methods.insert(
            "delete".into(),
            Ty::Fn(
                vec![Ty::Str],
                Box::new(Ty::Result(Box::new(Ty::Named("Response".into())))),
            ),
        );
        methods.insert("HTTPModule".into(), http_methods);

        // Speech module: transcribe(path: str) → Result[str] (refined display text)
        let mut speech_methods = std::collections::HashMap::new();
        speech_methods.insert(
            "transcribe".into(),
            Ty::Fn(vec![Ty::Str], Box::new(Ty::Result(Box::new(Ty::Str)))),
        );
        speech_methods.insert(
            "transcribe_microphone".into(),
            Ty::Fn(vec![], Box::new(Ty::Result(Box::new(Ty::Str)))),
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

        let mut scrape_methods = std::collections::HashMap::new();
        for entry in builtin_registry_entries()
            .iter()
            .copied()
            .filter(|e| e.namespace == "Scrape")
        {
            let Some(params) = builtin_entry_param_tys(entry) else {
                continue;
            };
            scrape_methods.insert(
                entry.name.to_string(),
                Ty::Fn(params, Box::new(builtin_entry_result_ty(entry))),
            );
        }
        methods.insert("ScrapeModule".into(), scrape_methods);

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

        // Option methods. `unwrap_or` / `unwrap_or_default` / `expect` mirror
        // the eval-side dispatch arms on `VoxValue::Option`. The closure-taking
        // methods (`map`, `and_then`) are deferred until closures land per
        // audit doc §11/§12 Phase G.
        let mut option_methods = std::collections::HashMap::new();
        option_methods.insert(
            "unwrap".into(),
            Ty::Fn(vec![], Box::new(Ty::GenericParam(0))),
        );
        option_methods.insert(
            "unwrap_or".into(),
            Ty::Fn(vec![Ty::GenericParam(0)], Box::new(Ty::GenericParam(0))),
        );
        option_methods.insert(
            "unwrap_or_default".into(),
            Ty::Fn(vec![], Box::new(Ty::GenericParam(0))),
        );
        option_methods.insert(
            "expect".into(),
            Ty::Fn(vec![Ty::Str], Box::new(Ty::GenericParam(0))),
        );
        option_methods.insert("is_some".into(), Ty::Fn(vec![], Box::new(Ty::Bool)));
        option_methods.insert("is_none".into(), Ty::Fn(vec![], Box::new(Ty::Bool)));
        // Closure-taking Option methods (eval in apply_closure_method).
        option_methods.insert(
            "map".into(),
            Ty::Fn(
                vec![Ty::Fn(
                    vec![Ty::GenericParam(0)],
                    Box::new(Ty::GenericParam(1)),
                )],
                Box::new(Ty::Option(Box::new(Ty::GenericParam(1)))),
            ),
        );
        option_methods.insert(
            "and_then".into(),
            Ty::Fn(
                vec![Ty::Fn(
                    vec![Ty::GenericParam(0)],
                    Box::new(Ty::Option(Box::new(Ty::GenericParam(1)))),
                )],
                Box::new(Ty::Option(Box::new(Ty::GenericParam(1)))),
            ),
        );
        option_methods.insert(
            "filter".into(),
            Ty::Fn(
                vec![Ty::Fn(vec![Ty::GenericParam(0)], Box::new(Ty::Bool))],
                Box::new(Ty::Option(Box::new(Ty::GenericParam(0)))),
            ),
        );
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

        // Result methods. Mirrors the eval-side dispatch on
        // `VoxValue::Result(Ok(_) | Err(_))`. Closure-taking methods (`map`,
        // `map_err`, `and_then`) are deferred until closures land per
        // audit doc §11/§12 Phase G.
        let mut result_methods = std::collections::HashMap::new();
        result_methods.insert(
            "unwrap".into(),
            Ty::Fn(vec![], Box::new(Ty::GenericParam(0))),
        );
        result_methods.insert(
            "unwrap_or".into(),
            Ty::Fn(vec![Ty::GenericParam(0)], Box::new(Ty::GenericParam(0))),
        );
        result_methods.insert(
            "unwrap_or_default".into(),
            Ty::Fn(vec![], Box::new(Ty::GenericParam(0))),
        );
        result_methods.insert(
            "expect".into(),
            Ty::Fn(vec![Ty::Str], Box::new(Ty::GenericParam(0))),
        );
        result_methods.insert("is_ok".into(), Ty::Fn(vec![], Box::new(Ty::Bool)));
        result_methods.insert("is_err".into(), Ty::Fn(vec![], Box::new(Ty::Bool)));
        result_methods.insert("unwrap_err".into(), Ty::Fn(vec![], Box::new(Ty::Str)));
        result_methods.insert(
            "ok".into(),
            Ty::Fn(vec![], Box::new(Ty::Option(Box::new(Ty::GenericParam(0))))),
        );
        result_methods.insert(
            "err".into(),
            Ty::Fn(vec![], Box::new(Ty::Option(Box::new(Ty::Str)))),
        );
        // Closure-taking Result methods (eval in apply_closure_method).
        result_methods.insert(
            "map".into(),
            Ty::Fn(
                vec![Ty::Fn(
                    vec![Ty::GenericParam(0)],
                    Box::new(Ty::GenericParam(1)),
                )],
                Box::new(Ty::Result(Box::new(Ty::GenericParam(1)))),
            ),
        );
        result_methods.insert(
            "map_err".into(),
            Ty::Fn(
                vec![Ty::Fn(vec![Ty::Str], Box::new(Ty::Str))],
                Box::new(Ty::Result(Box::new(Ty::GenericParam(0)))),
            ),
        );
        result_methods.insert(
            "and_then".into(),
            Ty::Fn(
                vec![Ty::Fn(
                    vec![Ty::GenericParam(0)],
                    Box::new(Ty::Result(Box::new(Ty::GenericParam(1)))),
                )],
                Box::new(Ty::Result(Box::new(Ty::GenericParam(1)))),
            ),
        );
        methods.insert("Result".into(), result_methods);

        // Int methods (scalar, no namespace marker — looked up via
        // `Ty::Int` → `"Int"` key per `lookup_method` fallback chain).
        let mut int_methods = std::collections::HashMap::new();
        for name in ["to_str", "to_string"] {
            int_methods.insert(name.into(), Ty::Fn(vec![], Box::new(Ty::Str)));
        }
        int_methods.insert("abs".into(), Ty::Fn(vec![], Box::new(Ty::Int)));
        int_methods.insert("min".into(), Ty::Fn(vec![Ty::Int], Box::new(Ty::Int)));
        int_methods.insert("max".into(), Ty::Fn(vec![Ty::Int], Box::new(Ty::Int)));
        methods.insert("Int".into(), int_methods);

        // Float methods.
        let mut float_methods = std::collections::HashMap::new();
        for name in ["to_str", "to_string"] {
            float_methods.insert(name.into(), Ty::Fn(vec![], Box::new(Ty::Str)));
        }
        for name in ["abs", "floor", "ceil", "round", "sqrt"] {
            float_methods.insert(name.into(), Ty::Fn(vec![], Box::new(Ty::Float)));
        }
        methods.insert("Float".into(), float_methods);

        // Bool methods (used as `b.to_string()` in scripts). Eval-side has
        // both `to_str` and `to_string`; `to_s` (Ruby-style) is intentionally
        // absent — see audit doc §2 issue-set.
        let mut bool_methods = std::collections::HashMap::new();
        for name in ["to_str", "to_string"] {
            bool_methods.insert(name.into(), Ty::Fn(vec![], Box::new(Ty::Str)));
        }
        methods.insert("Bool".into(), bool_methods);

        Self { methods }
    }

    /// Look up a method on a given type.
    pub fn lookup_method(&self, obj_ty: &Ty, method: &str) -> Option<Ty> {
        if let Ty::Table(_, fields) = obj_ty {
            return match method {
                "insert" => {
                    // insert(item: Record) -> Result[i64]
                    let item_ty = Ty::Record(fields.clone());
                    Some(Ty::Fn(
                        vec![item_ty],
                        Box::new(Ty::Result(Box::new(Ty::Int))),
                    ))
                }
                "get" => {
                    // get(id: int) -> Result[Option[Record]]
                    let record_ty = Ty::Record(fields.clone());
                    Some(Ty::Fn(
                        vec![Ty::Int],
                        Box::new(Ty::Result(Box::new(Ty::Option(Box::new(record_ty))))),
                    ))
                }
                "delete" => {
                    // delete(id: int) -> Result[Unit]
                    Some(Ty::Fn(
                        vec![Ty::Int],
                        Box::new(Ty::Result(Box::new(Ty::Unit))),
                    ))
                }
                "query" => {
                    // query(sql: str) -> Result[List[Record]] (Simplified params)
                    let record_ty = Ty::Record(fields.clone());
                    Some(Ty::Fn(
                        vec![Ty::Str],
                        Box::new(Ty::Result(Box::new(Ty::List(Box::new(record_ty))))),
                    ))
                }
                "all" => {
                    let record_ty = Ty::Record(fields.clone());
                    Some(Ty::Fn(
                        vec![],
                        Box::new(Ty::Result(Box::new(Ty::List(Box::new(record_ty))))),
                    ))
                }
                "count" => Some(Ty::Fn(vec![], Box::new(Ty::Result(Box::new(Ty::Int))))),
                "find" => {
                    let record_ty = Ty::Record(fields.clone());
                    Some(Ty::Fn(
                        vec![Ty::Int],
                        Box::new(Ty::Result(Box::new(Ty::Option(Box::new(record_ty))))),
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

        // ── Map[K, V] methods ─────────────────────────────────────────────
        // GenericParam(0) = K (key type), GenericParam(1) = V (value type).
        // Bindings are resolved in the MethodCall arm of check_expr via the
        // `Ty::Map(k, v) => vec![k, v]` branch.
        if let Ty::Map(_, _) = obj_ty {
            let k = Ty::GenericParam(0);
            let v = Ty::GenericParam(1);
            let map_ty = Ty::Map(Box::new(k.clone()), Box::new(v.clone()));
            return match method {
                "len" => Some(Ty::Fn(vec![], Box::new(Ty::Int))),
                "is_empty" => Some(Ty::Fn(vec![], Box::new(Ty::Bool))),
                "keys" => Some(Ty::Fn(vec![], Box::new(Ty::List(Box::new(k.clone()))))),
                "values" => Some(Ty::Fn(vec![], Box::new(Ty::List(Box::new(v.clone()))))),
                "items" | "entries" => Some(Ty::Fn(
                    vec![],
                    Box::new(Ty::List(Box::new(Ty::List(Box::new(k.clone()))))),
                )),
                "get" => Some(Ty::Fn(
                    vec![k.clone()],
                    Box::new(Ty::Option(Box::new(v.clone()))),
                )),
                "get_or" => Some(Ty::Fn(vec![k.clone(), v.clone()], Box::new(v.clone()))),
                "contains_key" | "has_key" | "has" => {
                    Some(Ty::Fn(vec![k.clone()], Box::new(Ty::Bool)))
                }
                "insert" | "set" => {
                    Some(Ty::Fn(vec![k.clone(), v.clone()], Box::new(map_ty.clone())))
                }
                "remove" | "delete" => Some(Ty::Fn(vec![k.clone()], Box::new(map_ty.clone()))),
                "update" => Some(Ty::Fn(vec![map_ty.clone()], Box::new(map_ty))),
                _ => None,
            };
        }

        let type_key = match obj_ty {
            Ty::Named(n) => n.as_str(),
            Ty::List(_) => "List",
            Ty::Str => "Str",
            Ty::Int => "Int",
            Ty::Float => "Float",
            Ty::Bool => "Bool",
            Ty::Result(_) => "Result",
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
