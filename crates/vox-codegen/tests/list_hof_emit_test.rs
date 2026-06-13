//! List higher-order method lowering: map, filter, fold, sorted_by_key.

use vox_codegen::codegen_rust::emit::emit_fn;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse_script;
use vox_compiler::typeck::typecheck_hir_module;

fn emit_fn_body(src: &str, name: &str) -> String {
    let module = parse_script(lex(src)).expect("parse");
    let mut hir = lower_module(&module);
    let _ = typecheck_hir_module(src, &mut hir);
    let f = hir
        .functions
        .iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("missing fn {name}"));
    emit_fn(f, Some(&hir.inferred_types), &[]).replace("\r\n", "\n")
}

#[test]
fn list_map_filter_fold_and_sorted_by_key_emit() {
    let src = r#"
        fn scale_all(nums: list[int], factor: int) to list[int] {
            return nums.map(fn(x: int) to int { x * factor })
        }
        fn sum_of_evens(nums: list[int]) to int {
            let evens = nums.filter(fn(x: int) to bool { x % 2 is 0 })
            return evens.fold(0, fn(acc: int, x: int) to int { acc + x })
        }
        fn sort_by_length(words: list[str]) to list[str] {
            return words.sorted_by_key(fn(w: str) to int { len(w) })
        }
    "#;
    let scale = emit_fn_body(src, "scale_all");
    assert!(
        scale.contains(".into_iter().map(") && scale.contains(".collect::<Vec<_>>()"),
        "map must lower to into_iter().map().collect(); got:\n{scale}"
    );
    let sum = emit_fn_body(src, "sum_of_evens");
    assert!(
        sum.contains(".into_iter().filter(") && sum.contains(".into_iter().fold("),
        "filter/fold must lower to iterator adapters; got:\n{sum}"
    );
    let sort = emit_fn_body(src, "sort_by_length");
    assert!(
        sort.contains("sort_by_key("),
        "sorted_by_key must lower to sort_by_key; got:\n{sort}"
    );
}

#[test]
fn returning_closure_boxes_for_fn_return_type() {
    let src = r#"
        fn make_adder(n: int) to fn(int) to int {
            return fn(x: int) to int { x + n }
        }
    "#;
    let emitted = emit_fn_body(src, "make_adder");
    assert!(
        emitted.contains("-> std::rc::Rc<dyn Fn(i64) -> i64 + 'static>"),
        "fn return type must be Rc<dyn Fn> trait; got:\n{emitted}"
    );
    assert!(
        emitted.contains("return std::rc::Rc::new(move |"),
        "returned closure must be Rc::new(move |…|); got:\n{emitted}"
    );
}
