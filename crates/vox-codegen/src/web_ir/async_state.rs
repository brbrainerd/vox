//! TypeScript / TSX emit for `Async[T]` view-arm expressions (GA-01).
//!
//! Wire-format-v1 encodes `Async[T]` as a `_tag`-discriminated union:
//! ```ts
//! type Async<T> =
//!   | { _tag: "fetching" }
//!   | { _tag: "empty" }
//!   | { _tag: "error"; error: unknown }
//!   | { _tag: "ok"; value: T };
//! ```
//!
//! The emitter narrows on `_tag` and renders the matching TSX subtree.
//! Generated output is a self-contained `(() => { … })()` IIFE so the
//! expression can be embedded anywhere JSX accepts an expression.

use vox_compiler::hir::nodes::async_view::HirAsyncView;

/// TypeScript type alias for `Async<T>` — emitted once per file that uses `Async[T]`.
pub const ASYNC_TYPE_ALIAS: &str = r#"/** Vox Async[T] — wire-format-v1 discriminated union. */
export type Async<T> =
  | { readonly _tag: "fetching" }
  | { readonly _tag: "empty" }
  | { readonly _tag: "error"; readonly error: unknown }
  | { readonly _tag: "ok"; readonly value: T };
"#;

/// Emit a TSX expression that narrows an `Async[T]` value and renders the matching arm.
///
/// `source_tsx` — already-emitted TSX expression for the `Async[T]` value.
/// `fetching_tsx` — JSX subtree for the `fetching` arm.
/// `empty_tsx` — JSX subtree for the `empty` arm.
/// `error_binding` — identifier to bind the error value in the error arm.
/// `error_tsx` — JSX subtree for the `error` arm.
/// `ok_binding` — identifier to bind the resolved value in the ok arm.
/// `ok_tsx` — JSX subtree for the `ok` arm.
#[must_use]
pub fn emit_async_view_tsx(
    source_tsx: &str,
    fetching_tsx: &str,
    empty_tsx: &str,
    error_binding: &str,
    error_tsx: &str,
    ok_binding: &str,
    ok_tsx: &str,
) -> String {
    format!(
        r#"(() => {{
  const _async = {source_tsx};
  if (_async._tag === "fetching") return ({fetching_tsx});
  if (_async._tag === "empty") return ({empty_tsx});
  if (_async._tag === "error") {{ const {error_binding} = _async.error; return ({error_tsx}); }}
  const {ok_binding} = _async.value; return ({ok_tsx});
}})()"#
    )
}

/// Validate that an `HirAsyncView` is exhaustive before emitting.
///
/// Returns `Err` with missing arm names if any arm is absent.
pub fn validate_async_view(view: &HirAsyncView) -> Result<(), Vec<&'static str>> {
    let missing = view.missing_arms();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(missing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_iife_with_all_arms() {
        let tsx = emit_async_view_tsx(
            "userData",
            "<Spinner />",
            "<EmptyState />",
            "e",
            "<ErrorBanner error={e} />",
            "user",
            "<UserCard user={user} />",
        );
        assert!(tsx.contains("userData"));
        assert!(tsx.contains("_tag === \"fetching\""));
        assert!(tsx.contains("_tag === \"empty\""));
        assert!(tsx.contains("_tag === \"error\""));
        assert!(tsx.contains("const e = _async.error"));
        assert!(tsx.contains("const user = _async.value"));
        assert!(tsx.contains("<Spinner />"));
        assert!(tsx.contains("<UserCard user={user} />"));
    }

    #[test]
    fn async_type_alias_has_all_tags() {
        assert!(ASYNC_TYPE_ALIAS.contains("\"fetching\""));
        assert!(ASYNC_TYPE_ALIAS.contains("\"empty\""));
        assert!(ASYNC_TYPE_ALIAS.contains("\"error\""));
        assert!(ASYNC_TYPE_ALIAS.contains("\"ok\""));
    }

    #[test]
    fn wire_format_v1_tag_field_is_underscore_tag() {
        // Per wire-format-v1 SSOT: discriminant key is `_tag`, not `type` or `kind`.
        assert!(ASYNC_TYPE_ALIAS.contains("_tag"));
        assert!(!ASYNC_TYPE_ALIAS.contains("_kind"));
        assert!(!ASYNC_TYPE_ALIAS.contains(": \"type\""));
    }
}
