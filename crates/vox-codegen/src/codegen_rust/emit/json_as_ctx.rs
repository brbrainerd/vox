//! Thread-local context for `@json_as`-generated function bodies.
//!
//! In the **script** codegen lane the module is lowered without typechecking,
//! so `HirModule::inferred_types` is empty. The Rust tail-emitter therefore has
//! no per-expression type to decide whether an `ObjectLit` is a JSON value
//! (`serde_json::json!({..})`) or a Rust struct literal (`Type { .. }`).
//!
//! For `<Type>_from_json` the answer is unambiguous from the *enclosing
//! function*: its body returns `Ok(Type { .. })`, and its return type is
//! `Result[Type]`. `emit_fn` records that `Type` name here for the duration of
//! the body emit; the `ObjectLit` arm of the tail-emitter reads it as a
//! fallback when `inferred_types` yields nothing. `to_json` bodies set no hint,
//! so their `ObjectLit` keeps the JSON form.

use std::cell::Cell;

thread_local! {
    /// The struct name to ascribe to a bare `ObjectLit` inside the
    /// currently-emitting `<Type>_from_json` body, or `None` outside one.
    static CURRENT_FROM_JSON_STRUCT: Cell<Option<&'static str>> = const { Cell::new(None) };
    /// Leaked, interned struct-name strings (codegen is short-lived; the leak is
    /// bounded by the number of distinct `@json_as` types in a compilation).
    static INTERNED: std::cell::RefCell<Vec<&'static str>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// RAII guard that sets the `from_json` struct hint for the lifetime of a body
/// emit and clears it (restoring the prior value) on drop.
pub(super) struct FromJsonGuard(Option<&'static str>);

impl Drop for FromJsonGuard {
    fn drop(&mut self) {
        CURRENT_FROM_JSON_STRUCT.with(|c| c.set(self.0));
    }
}

/// Enter a `<Type>_from_json` body, recording `struct_name` as the hint. The
/// returned guard restores the previous hint when dropped.
pub(super) fn enter_from_json(struct_name: &str) -> FromJsonGuard {
    let interned: &'static str = INTERNED.with(|v| {
        let mut v = v.borrow_mut();
        if let Some(s) = v.iter().find(|s| **s == struct_name) {
            return *s;
        }
        let leaked: &'static str = Box::leak(struct_name.to_string().into_boxed_str());
        v.push(leaked);
        leaked
    });
    let prev = CURRENT_FROM_JSON_STRUCT.with(|c| c.replace(Some(interned)));
    FromJsonGuard(prev)
}

/// The current `from_json` struct hint, if emission is inside such a body.
pub(super) fn current_from_json_struct() -> Option<String> {
    CURRENT_FROM_JSON_STRUCT.with(|c| c.get().map(|s| s.to_string()))
}
