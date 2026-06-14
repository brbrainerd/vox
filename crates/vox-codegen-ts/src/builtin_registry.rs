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
        if let Some(l) = self.methods.get(&(ty, method, arity)) {
            return Some(l.clone());
        }
        if !ty.is_empty() {
            return self
                .methods
                .iter()
                .find(|((t, m, _), _)| *t == ty && *m == method)
                .map(|(_, l)| l.clone());
        }
        // No type hint (the common `hir_emit` path, where the receiver's type
        // isn't tracked). Resolve only an UNAMBIGUOUS `Property` lowering, e.g.
        // `html.length()` → `html.length`. A property accessed as a call is
        // always a bug, so applying it can never change correct code; method
        // renames are left to emit literally to avoid altering call semantics.
        let mut prop: Option<&BuiltinLowering> = None;
        for ((_, m, a), l) in &self.methods {
            if *m == method && *a == arity && matches!(l, BuiltinLowering::Property(_)) {
                match prop {
                    None => prop = Some(l),
                    Some(p) if p == l => {}
                    Some(_) => return None, // ambiguous across types — bail
                }
            }
        }
        prop.cloned()
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

#[cfg(test)]
mod semcov_wave2_tests {
    #![allow(unused_imports)]
    use super::*;

    #[test]
    fn lookup_method_exact_arity_returns_known_lowering() {
        let reg = BuiltinRegistry::standard();
        // str.length() with arity 0 → Property("length")
        let result = reg.lookup_method("str", "length", 0);
        assert_eq!(result, Some(BuiltinLowering::Property("length")));
    }

    #[test]
    fn lookup_method_arity_mismatch_falls_back_to_name_scan() {
        let reg = BuiltinRegistry::standard();
        // str.split has arity=1 registered; ask with arity=99 → falls back via name scan
        let result = reg.lookup_method("str", "split", 99);
        // Should still find the MethodRename via the fuzzy scan
        assert!(result.is_some(), "expected fallback hit for str.split/99");
        assert!(matches!(
            result,
            Some(BuiltinLowering::MethodRename("split"))
        ));
    }

    #[test]
    fn lookup_method_no_type_hint_resolves_unambiguous_property() {
        let reg = BuiltinRegistry::standard();
        // Both str.length and list.length map to Property("length") with arity 0.
        // The no-type-hint path should resolve this when both agree.
        let result = reg.lookup_method("", "length", 0);
        assert_eq!(result, Some(BuiltinLowering::Property("length")));
    }

    #[test]
    fn lookup_method_returns_none_for_unknown_method() {
        let reg = BuiltinRegistry::standard();
        let result = reg.lookup_method("str", "nonexistent_method_xyz", 0);
        assert!(result.is_none());
    }

    #[test]
    fn lookup_function_exact_hit_returns_inline() {
        let reg = BuiltinRegistry::standard();
        let result = reg.lookup_function("std.time.now_ms", 0);
        assert_eq!(result, Some(BuiltinLowering::Inline("Date.now()")));
    }

    #[test]
    fn lookup_function_arity_fallback_still_finds_by_name() {
        let reg = BuiltinRegistry::standard();
        // print is registered with arity=1; ask with arity=99
        let result = reg.lookup_function("print", 99);
        assert!(result.is_some(), "fallback by name should find print");
        assert!(matches!(
            result,
            Some(BuiltinLowering::FunctionRename("console.log"))
        ));
    }

    #[test]
    fn lookup_function_unknown_returns_none() {
        let reg = BuiltinRegistry::standard();
        assert!(reg.lookup_function("totally_unknown_fn", 0).is_none());
    }
}
