//! Behavioral tests for in-memory `db.*` execution under `--mode interp`.
//!
//! Before this, `db` was an undefined variable in the interpreter — any
//! data-layer program failed at runtime with `UndefinedVariable("db")`. These
//! tests pin the input→output behavior of insert/all/get/find/delete/count and
//! single-call `where`/`filter` (incl. the comparison-operator and boolean
//! algebra), and the loud-`Err` fallback for fused predicate chains.
//! See `docs/superpowers/specs/2026-06-03-interpreter-db-execution-design.md`.

use vox_compiler::eval::value::VoxValue;

/// Lex → parse → lower → run, returning `main()`'s value.
fn run_main(source: &str) -> VoxValue {
    let tokens = vox_compiler::lexer::lex(source);
    let module = vox_compiler::parser::descent::parse(tokens).expect("parse");
    let lowered = vox_compiler::hir::lower::lower_module(&module);
    let mut interp = vox_compiler::eval::Interpreter::new(1_000_000);
    interp.run_module(&lowered).expect("run_module");
    interp.call("main", vec![]).expect("call main")
}

#[test]
fn insert_then_all_and_count() {
    let res = run_main(
        "
        @table type User { name: str, age: int }
        fn main() to int {
            db.User.insert({ name: \"alice\", age: 30 })
            db.User.insert({ name: \"bob\", age: 17 })
            return len(db.User.all().unwrap()) * 10 + db.User.count().unwrap()
        }
        ",
    );
    // 2 rows → len 2, count 2 → 22.
    assert_eq!(res, VoxValue::Int(22), "insert x2 → all len 2, count 2");
}

#[test]
fn insert_returns_monotonic_ids() {
    let res = run_main(
        "
        @table type Item { name: str }
        fn main() to int {
            let a = db.Item.insert({ name: \"x\" })
            let b = db.Item.insert({ name: \"y\" })
            return a.unwrap() * 10 + b.unwrap()
        }
        ",
    );
    // ids 0 then 1 → 1.
    assert_eq!(res, VoxValue::Int(1), "first insert _id=0, second _id=1");
}

#[test]
fn where_gte_filters_rows() {
    let res = run_main(
        "
        @table type User { name: str, age: int }
        fn main() to int {
            db.User.insert({ name: \"a\", age: 30 })
            db.User.insert({ name: \"b\", age: 17 })
            db.User.insert({ name: \"c\", age: 25 })
            return len(db.User.where({ age: { gte: 18 } }).unwrap())
        }
        ",
    );
    assert_eq!(res, VoxValue::Int(2), "age >= 18 keeps 30 and 25");
}

#[test]
fn filter_equality_on_bool() {
    let res = run_main(
        "
        @table type User { name: str, active: bool }
        fn main() to int {
            db.User.insert({ name: \"a\", active: true })
            db.User.insert({ name: \"b\", active: false })
            db.User.insert({ name: \"c\", active: true })
            return len(db.User.filter({ active: true }).unwrap())
        }
        ",
    );
    assert_eq!(res, VoxValue::Int(2), "active == true keeps 2 rows");
}

#[test]
fn get_by_id_returns_some_then_delete() {
    let res = run_main(
        "
        @table type Item { name: str }
        fn main() to int {
            db.Item.insert({ name: \"a\" })
            db.Item.insert({ name: \"b\" })
            let hit = match db.Item.get(1).unwrap() {
                Some(row) => 1
                None => 0
            }
            db.Item.delete(0)
            return hit * 10 + db.Item.count().unwrap()
        }
        ",
    );
    // get(1) is Some → 1; after delete(0), count 1 → 11.
    assert_eq!(res, VoxValue::Int(11), "get(1)=Some, delete(0) leaves 1 row");
}

#[test]
fn all_order_by_then_limit() {
    let res = run_main(
        "
        @table type User { name: str, age: int }
        fn main() to int {
            db.User.insert({ name: \"old\", age: 90 })
            db.User.insert({ name: \"young\", age: 5 })
            db.User.insert({ name: \"mid\", age: 40 })
            return len(db.User.all().order_by(\"age\", true).limit(2).unwrap())
        }
        ",
    );
    assert_eq!(res, VoxValue::Int(2), "limit(2) over ordered scan keeps 2");
}

#[test]
fn fused_predicate_chain_filters_then_projects() {
    // `.where({..}).select(..)` is a fused chain: the predicate value lives on
    // an inner node, but the plan now carries it, so the interpreter filters
    // correctly before projecting (was a loud Err before the plan-carried
    // predicate values landed).
    let res = run_main(
        "
        @table type User { name: str, age: int }
        fn main() to int {
            db.User.insert({ name: \"a\", age: 5 })
            db.User.insert({ name: \"b\", age: 90 })
            db.User.insert({ name: \"c\", age: 40 })
            return len(db.User.where({ age: { gte: 18 } }).select(\"name\"))
        }
        ",
    );
    // age >= 18 keeps b(90) and c(40); the projection does not change the count.
    assert_eq!(
        res,
        VoxValue::Int(2),
        "fused where(gte 18).select() must filter to 2 rows, then project"
    );
}

#[test]
fn fused_where_order_by_limit_compose() {
    // where + order_by + limit fused into one chain, all carried on the plan.
    let res = run_main(
        "
        @table type User { name: str, age: int }
        fn main() to str {
            db.User.insert({ name: \"young\", age: 5 })
            db.User.insert({ name: \"old\", age: 90 })
            db.User.insert({ name: \"mid\", age: 40 })
            let top = db.User.where({ age: { gte: 18 } }).order_by(\"age\", true).limit(1).unwrap()
            return top.first().unwrap().name
        }
        ",
    );
    // age >= 18 → {old:90, mid:40}; asc → mid first; limit 1 → [mid].
    assert_eq!(
        res,
        VoxValue::Str("mid".to_string()),
        "fused where+order_by(asc)+limit(1) must yield the youngest adult"
    );
}
