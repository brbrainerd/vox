//! Characterization tests pinning Vox's **value semantics** for collections —
//! the behavioral contract that the copy-on-write (`Rc` + `make_mut`) Phase 1
//! refactor MUST preserve bit-for-bit.
//!
//! These pass under the current clone-everything interpreter. They are the
//! safety net for the representation change `List/Object/Tuple` →
//! `Rc<...>`: after CoW lands, every assertion here must still hold, proving
//! that cheap O(1) clones did not accidentally introduce shared mutable state
//! (aliasing). See
//! `docs/src/architecture/vox-memory-model-audit-and-value-optimization-2026-06-05.md`.

use vox_compiler::eval::value::VoxValue;

// ── Phase 5: Rc-soundness invariant guard ─────────────────────────────────────
// `VoxValue` holds `Rc` payloads (CoW lists/objects/tuples, `Rc`-shared scope
// frames and closure bodies), which makes O(1) clones possible WITHOUT a garbage
// collector. `Rc` is `!Send`, so this also makes the "interpreter values never
// cross threads" invariant *self-enforcing* — any attempt to send a `VoxValue`
// to another thread fails to compile. This static assertion is a deliberate
// tripwire: if someone swaps `Rc`→`Arc` ("to make it Send") or otherwise makes
// `VoxValue` `Send`/`Sync`, this STOPS COMPILING, forcing a conscious review of
// the single-threaded-interpreter assumption before the change can land.
// See docs/src/architecture/vox-memory-model-audit-and-value-optimization-2026-06-05.md §Part 2.
static_assertions::assert_not_impl_any!(VoxValue: Send, Sync);

/// Run a Vox program's `main` through the tree-walking interpreter and return
/// its value, mirroring the real `--mode interp` path (`parse_script`).
fn run_main(src: &str) -> VoxValue {
    let tokens = vox_compiler::lexer::lex(src);
    let module = vox_compiler::parser::parse_script(tokens).expect("parse_script");
    let lowered = vox_compiler::hir::lower::lower_module(&module);
    let mut interp = vox_compiler::eval::Interpreter::new(1_000_000);
    interp.run_module(&lowered).expect("run_module");
    interp.call("main", vec![]).expect("call main")
}

/// `let b = a; a.push(4)` must NOT change `b` — assignment copies.
/// This is the canonical aliasing-independence test CoW must keep green.
#[test]
fn list_assignment_is_a_copy_not_an_alias() {
    let src = r#"
fn main() to int {
    let mut a: list[int] = [1, 2, 3]
    let b = a
    a.push(4)
    return len(b)
}
"#;
    assert_eq!(
        run_main(src),
        VoxValue::Int(3),
        "mutating `a` after `let b = a` must leave `b` at length 3 (value semantics)"
    );
}

/// Index assignment through one binding must not leak into a prior copy.
#[test]
fn list_index_assign_does_not_alias_prior_copy() {
    let src = r#"
fn main() to int {
    let mut a: list[int] = [1, 2, 3]
    let b = a
    a[0] = 99
    match b[0] {
        Some(v) => { return v }
        None => { return -1 }
    }
}
"#;
    assert_eq!(
        run_main(src),
        VoxValue::Int(1),
        "`a[0] = 99` must not change `b[0]`, which stays 1"
    );
}

/// A list passed by value into a function and mutated there must not affect the
/// caller's list. (`big_list_pass` cliff — must stay correct after O(1) clones.)
#[test]
fn list_passed_to_fn_is_independent_of_caller() {
    let src = r#"
fn grow(xs: list[int]) to int {
    let mut local = xs
    local.push(0)
    return len(local)
}
fn main() to int {
    let original: list[int] = [1, 2, 3]
    let inner_len = grow(original)
    let outer_len = len(original)
    return inner_len * 100 + outer_len
}
"#;
    // inner sees 4, caller still sees 3 → 4*100 + 3 = 403
    assert_eq!(
        run_main(src),
        VoxValue::Int(403),
        "callee mutation must not propagate back to the caller's list"
    );
}

/// In-place append is observable within the same binding (push actually grows
/// the owner). Guards against CoW make_mut breaking single-owner mutation.
#[test]
fn list_push_mutates_the_owner() {
    let src = r#"
fn main() to int {
    let mut a: list[int] = []
    a.push(10)
    a.push(20)
    a.push(30)
    return len(a)
}
"#;
    assert_eq!(run_main(src), VoxValue::Int(3));
}

/// Object/record passed by value is independent of its source binding.
#[test]
fn object_pass_by_value_reads_correct_fields() {
    let src = r#"
type Point { x: int, y: int }
fn sum(p: Point) to int {
    return p.x + p.y
}
fn main() to int {
    let a: Point = { x: 3, y: 4 }
    let b = a
    return sum(a) + sum(b)
}
"#;
    assert_eq!(run_main(src), VoxValue::Int(14), "3+4 read twice = 14");
}

/// A closure captures its environment by value (snapshot) and reads it back
/// correctly. Guards the Phase 2 cactus/`Rc`-frame scope change against breaking
/// closure capture semantics.
#[test]
fn closure_captures_environment_by_value() {
    let src = r#"
fn main() to int {
    let n = 10
    let add_n = fn(x: int) to int { x + n }
    let a = add_n(5)
    let b = add_n(100)
    return a + b
}
"#;
    // (5+10) + (100+10) = 15 + 110 = 125
    assert_eq!(run_main(src), VoxValue::Int(125));
}

/// Nested/recursive calls keep independent frames — a recursive function must
/// compute correctly after the scope-frame representation changes.
#[test]
fn recursion_keeps_independent_frames() {
    let src = r#"
fn fib(n: int) to int {
    if n < 2 { return n }
    return fib(n - 1) + fib(n - 2)
}
fn main() to int {
    return fib(10)
}
"#;
    assert_eq!(run_main(src), VoxValue::Int(55));
}

/// Non-mutating higher-order ops (`map`) return a new list and leave the source
/// unchanged — the source must keep its original length.
#[test]
fn map_does_not_mutate_source_list() {
    let src = r#"
fn main() to int {
    let src: list[int] = [1, 2, 3, 4]
    let doubled = src.map(fn(x: int) to int { x * 2 })
    return len(src) * 100 + len(doubled)
}
"#;
    // both length 4 → 4*100 + 4 = 404
    assert_eq!(run_main(src), VoxValue::Int(404));
}
