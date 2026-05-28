//! Single source of truth for how Vox method/function/namespace identifiers
//! lower to TypeScript. Adding a new builtin: add a row here, write a test.

use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuiltinLowering {
    /// Drop the call parens, emit as a property access. e.g. `s.length()` → `s.length`.
    Property(&'static str),
    /// Replace the entire call expression with this literal TS. e.g. `std.time.now_ms()` → `Date.now()`.
    Inline(&'static str),
    /// Rewrite the method name. e.g. `arr.append(x)` → `arr.push(x)`.
    MethodRename(&'static str),
    /// Rewrite a free function name. e.g. `str(x)` → `String(x)`.
    FunctionRename(&'static str),
}

pub struct BuiltinRegistry {
    methods: HashMap<(&'static str, &'static str, usize), BuiltinLowering>,
    functions: HashMap<(&'static str, usize), BuiltinLowering>,
    namespaces: HashMap<&'static str, &'static str>,
}

impl BuiltinRegistry {
    pub fn standard() -> Self {
        let mut methods = HashMap::new();
        methods.insert(("str", "length", 0), BuiltinLowering::Property("length"));
        methods.insert(("list", "length", 0), BuiltinLowering::Property("length"));
        methods.insert(("list", "push", 1), BuiltinLowering::MethodRename("push"));
        methods.insert(("list", "pop", 0), BuiltinLowering::MethodRename("pop"));
        methods.insert(("str", "trim", 0), BuiltinLowering::MethodRename("trim"));
        methods.insert(
            ("str", "to_lower", 0),
            BuiltinLowering::MethodRename("toLowerCase"),
        );
        methods.insert(
            ("str", "to_upper", 0),
            BuiltinLowering::MethodRename("toUpperCase"),
        );
        methods.insert(("str", "split", 1), BuiltinLowering::MethodRename("split"));
        methods.insert(
            ("str", "starts_with", 1),
            BuiltinLowering::MethodRename("startsWith"),
        );
        methods.insert(
            ("str", "ends_with", 1),
            BuiltinLowering::MethodRename("endsWith"),
        );

        let mut functions = HashMap::new();
        functions.insert(
            ("std.time.now_ms", 0),
            BuiltinLowering::Inline("Date.now()"),
        );
        functions.insert(
            ("std.time.iso_now", 0),
            BuiltinLowering::Inline("new Date().toISOString()"),
        );
        functions.insert(("len", 1), BuiltinLowering::FunctionRename("__vox_len"));
        functions.insert(("str", 1), BuiltinLowering::FunctionRename("String"));
        functions.insert(("print", 1), BuiltinLowering::FunctionRename("console.log"));

        let mut namespaces = HashMap::new();
        namespaces.insert("Speech", "Speech");
        namespaces.insert("std.mobile", "Speech");

        Self {
            methods,
            functions,
            namespaces,
        }
    }

    pub fn lookup_method(&self, ty: &str, method: &str, arity: usize) -> Option<BuiltinLowering> {
        self.methods.get(&(ty, method, arity)).cloned().or_else(|| {
            self.methods
                .iter()
                .find(|((t, m, _), _)| *t == ty && *m == method)
                .map(|(_, l)| l.clone())
        })
    }

    pub fn lookup_function(&self, name: &str, arity: usize) -> Option<BuiltinLowering> {
        self.functions.get(&(name, arity)).cloned().or_else(|| {
            self.functions
                .iter()
                .find(|((n, _), _)| *n == name)
                .map(|(_, l)| l.clone())
        })
    }

    pub fn lookup_namespace(&self, ns: &str) -> Option<&'static str> {
        self.namespaces.get(ns).copied()
    }
}
