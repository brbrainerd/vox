use crate::hir::nodes::HirStmt;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub enum VoxValue {
    Int(i64),
    Float(f64),
    /// Exact fixed-point decimal (`dec` literals), backed by `rust_decimal` so
    /// interpreter arithmetic matches the Rust-codegen (`--mode script`) path:
    /// `0.1dec + 0.2dec` is exactly `0.3dec`, not an IEEE-754 approximation.
    Decimal(rust_decimal::Decimal),
    /// A compiled regular expression (`std.regex.compile`). Holds the parsed
    /// `regex::Regex` so `re.find/matches/find_all` run without recompiling,
    /// matching the Rust-codegen `VoxRegex` path.
    Regex(regex::Regex),
    /// A regex match: capture groups by index (group 0 = whole match). A `None`
    /// slot means that group did not participate. `m.group(i)` → `Option[str]`.
    Match(Vec<core::option::Option<String>>),
    Str(String),
    Bool(bool),
    /// Copy-on-write list payload. `Rc` makes `VoxValue::clone()` O(1) (a refcount
    /// bump) so pass-by-value is cheap; in-place mutation uses [`Rc::make_mut`],
    /// which clones once iff the payload is aliased — preserving Vox's value
    /// semantics. Construct via [`VoxValue::list`].
    List(Rc<Vec<VoxValue>>),
    /// Copy-on-write object payload. See [`VoxValue::List`]. Construct via
    /// [`VoxValue::object`].
    Object(Rc<Vec<(String, VoxValue)>>),
    /// Copy-on-write tuple payload. See [`VoxValue::List`]. Construct via
    /// [`VoxValue::tuple`].
    Tuple(Rc<Vec<VoxValue>>),
    Null,
    Fn {
        params: Vec<String>,
        /// `Rc`-shared closure body so cloning a function value (every closure
        /// capture and every recursive call) is an O(1) refcount bump rather
        /// than a deep clone of the HIR statement list.
        body: Rc<Vec<HirStmt>>,
        env: crate::eval::env::Scope,
        /// Function name, used to build the auto-snapshot label for
        /// `@versioned` functions; empty (`""`) for anonymous lambdas.
        name: String,
        /// `@versioned`/`@tracked` — when true, the interpreter records one
        /// `repo.snapshot()` checkpoint on this function's successful return.
        is_versioned: bool,
    },
    Option(core::option::Option<Box<VoxValue>>),
    /// `Result[T, E]`. The Err side carries a real `VoxValue` (was `String`) so
    /// typed errors — `Error(MyAdt)` as well as `Error("string")` — survive at
    /// runtime, matching the two-parameter `Ty::Result`. A string error boxes to
    /// `VoxValue::Str` (see [`err_str`]), preserving the historical behavior.
    Result(core::result::Result<Box<VoxValue>, Box<VoxValue>>),
    /// An ADT variant constructor callable (not yet applied). Created by `run_module`.
    Constructor(String),
    /// An applied ADT variant value, e.g. `Applied(10, 0)`.
    Tagged {
        name: String,
        fields: Vec<VoxValue>,
    },
    // Sentinel for control flow
    _Return(Box<VoxValue>),
    _Break,
    _Continue,
    /// Builtin-method panic sentinel — produced by `unwrap()` on `None` /
    /// `Err`, `expect(msg)` on `None` / `Err`, `unwrap_err()` on `Ok`, and
    /// the rare other "this is a programmer error, halt loudly" cases.
    /// Caught by the method-call handler in
    /// [`crate::eval::expr`] and converted to
    /// [`crate::eval::EvalError::AssertionFailed`].
    ///
    /// This sentinel exists because `call_builtin_method` returns
    /// `Option<VoxValue>`, where `None` means "method not found". To
    /// signal "method ran but the program should halt", we need an
    /// in-band marker. Without it, `unwrap` on `None` would either
    /// silently return `Null` (the prior behavior — a footgun) or
    /// surface as the misleading "Method unwrap not found" diagnostic.
    /// See `docs/src/architecture/vox-stdlib-gap-audit-2026-05-23.md` §10.4
    /// for the design discussion.
    _Panic(String),
}

impl VoxValue {
    /// Build a CoW [`VoxValue::List`] from an owned `Vec`.
    #[inline]
    pub fn list(items: Vec<VoxValue>) -> Self {
        VoxValue::List(Rc::new(items))
    }
    /// Build a CoW [`VoxValue::Object`] from owned fields.
    #[inline]
    pub fn object(fields: Vec<(String, VoxValue)>) -> Self {
        VoxValue::Object(Rc::new(fields))
    }
    /// Build a CoW [`VoxValue::Tuple`] from an owned `Vec`.
    #[inline]
    pub fn tuple(items: Vec<VoxValue>) -> Self {
        VoxValue::Tuple(Rc::new(items))
    }
}

impl PartialEq for VoxValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => a == b,
            // Cross-numeric value equality, consistent with mixed Int/Float
            // arithmetic promotion — `1 is 1.0` is true.
            (Self::Int(a), Self::Float(b)) | (Self::Float(b), Self::Int(a)) => (*a as f64) == *b,
            // rust_decimal's Eq compares numeric value regardless of trailing
            // zeros, so `8.2500dec == 8.25dec` is true.
            (Self::Decimal(a), Self::Decimal(b)) => a == b,
            (Self::Regex(a), Self::Regex(b)) => a.as_str() == b.as_str(),
            (Self::Match(a), Self::Match(b)) => a == b,
            (Self::Str(a), Self::Str(b)) => a == b,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::List(a), Self::List(b)) => a == b,
            (Self::Object(a), Self::Object(b)) => a == b,
            (Self::Tuple(a), Self::Tuple(b)) => a == b,
            (Self::Null, Self::Null) => true,
            (Self::Option(a), Self::Option(b)) => a == b,
            (Self::Result(a), Self::Result(b)) => a == b,
            (Self::Constructor(a), Self::Constructor(b)) => a == b,
            (
                Self::Tagged {
                    name: na,
                    fields: fa,
                },
                Self::Tagged {
                    name: nb,
                    fields: fb,
                },
            ) => na == nb && fa == fb,
            _ => false,
        }
    }
}

/// Box a string error message as the `VoxValue::Str` carried in a `Result` Err
/// slot. Used by stdlib builtins that produce string-valued errors, so they keep
/// their historical behavior under the widened `Result(_, Box<VoxValue>)` shape.
pub(crate) fn err_str(s: String) -> Box<VoxValue> {
    Box::new(VoxValue::Str(s))
}

#[cfg(test)]
mod tests {
    use super::VoxValue;

    /// The CoW constructors wrap their owned input in the expected variant.
    #[test]
    fn constructors_wrap_the_expected_variants() {
        assert!(matches!(VoxValue::list(vec![]), VoxValue::List(_)));
        assert!(matches!(VoxValue::object(vec![]), VoxValue::Object(_)));
        assert!(matches!(VoxValue::tuple(vec![]), VoxValue::Tuple(_)));
    }

    /// Cloning a constructed value shares the payload (O(1)) yet compares equal.
    #[test]
    fn list_clone_is_value_equal() {
        let a = VoxValue::list(vec![VoxValue::Int(1), VoxValue::Int(2)]);
        let b = a.clone();
        assert_eq!(a, b);
    }
}
