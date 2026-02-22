use vox_lexer::cursor::lex;
use vox_parser::parser::parse;
use vox_typeck::diagnostics::Severity;
use vox_typeck::typecheck_module;

fn check_src(src: &str) -> Vec<vox_typeck::Diagnostic> {
    let tokens = lex(src);
    let module = parse(tokens).expect("Source should parse without generic errors");
    typecheck_module(&module)
}

fn errors(src: &str) -> Vec<vox_typeck::Diagnostic> {
    check_src(src)
        .into_iter()
        .filter(|d| d.severity == Severity::Error)
        .collect()
}

#[test]
fn test_activity_valid_definition() {
    let src = r#"


activity send_email(recipient: str, subject: str) to Result[str]:
    let msg = "Sending to " + recipient
    Ok(msg)
"#;
    let errs = errors(src);
    assert!(
        errs.is_empty(),
        "Valid activity should pass check, got: {:?}",
        errs
    );
}

#[test]
fn test_activity_invalid_return_type() {
    let src = r#"
activity bad_return() to str:
    "oops"
"#;
    let errs = errors(src);
    assert!(!errs.is_empty());
    assert!(errs[0].message.contains("must return a Result"));
}

#[test]
fn test_activity_with_syntax() {
    let src = r#"


activity fetch_data() to Result[str]:
    Ok("data")

workflow main_flow() to Result[str]:
    # Call activity directly and apply 'with' options
    let res = fetch_data() with { timeout: "10s", retries: 3 }
    res
"#;
    let errs = errors(src);
    assert!(
        errs.is_empty(),
        "Activity call with options should pass type check, got: {:?}",
        errs
    );
}

#[test]
fn test_with_operator_associativity() {
    // Ensures 'with' works on expressions
    let src = r#"
fn f() to int:
    let x = 1 with { meta: "data" }
    x
"#;
    let errs = errors(src);
    // Currently 'with' works on ANY expression in type checker (just propagates type)
    assert!(
        errs.is_empty(),
        "With syntax on int literal should be valid structurally, got: {:?}",
        errs
    );
}

fn warnings(src: &str) -> Vec<vox_typeck::Diagnostic> {
    check_src(src)
        .into_iter()
        .filter(|d| d.severity == Severity::Warning)
        .collect()
}

#[test]
fn test_activity_missing_return_type_warning() {
    let src = r#"
activity fire_and_forget(msg: str):
    let x = msg
"#;
    let warns = warnings(src);
    assert!(
        !warns.is_empty(),
        "Activity without return type should produce warning"
    );
    assert!(warns[0]
        .message
        .contains("should have an explicit return type"));
}

#[test]
fn test_with_non_record_options_error() {
    let src = r#"
fn f() to int:
    let x = 1 with "invalid"
    x
"#;
    let errs = errors(src);
    assert!(
        !errs.is_empty(),
        "Using 'with' with a non-record should produce error"
    );
    assert!(errs[0].message.contains("'with' options must be a record"));
}

#[test]
fn test_activity_callable_from_function() {
    // Activities are registered as functions in the type env and can be called
    let src = r#"

activity do_work(input: str) to Result[str]:
    Ok(input)

fn main() to Result[str]:
    let result = do_work("test")
    result
"#;
    let errs = errors(src);
    assert!(
        errs.is_empty(),
        "Activity should be callable like a function, got: {:?}",
        errs
    );
}

#[test]
fn test_hir_lowering_activity_and_with() {
    // Verify the full pipeline through HIR lowering doesn't panic
    use vox_hir::lower::lower_module;

    let src = r#"

activity process_data(data: str) to Result[str]:
    Ok(data)

workflow pipeline() to Result[str]:
    let result = process_data("test") with { retries: 3 }
    result
"#;
    let tokens = vox_lexer::cursor::lex(src);
    let module = vox_parser::parser::parse(tokens).expect("Should parse");
    let hir = lower_module(&module);
    assert_eq!(hir.activities.len(), 1, "Should have 1 activity");
    assert_eq!(hir.activities[0].name, "process_data");
    assert_eq!(hir.workflows.len(), 1, "Should have 1 workflow");
}

#[test]
fn test_with_unknown_option_key_warning() {
    let src = r#"
fn f() to int:
    let x = 1 with { unknown_key: 42 }
    x
"#;
    let warns = warnings(src);
    assert!(
        !warns.is_empty(),
        "Unknown 'with' option key should produce warning"
    );
    assert!(
        warns[0].message.contains("Unknown 'with' option"),
        "Got: {}",
        warns[0].message
    );
}

#[test]
fn test_with_wrong_option_type_warning() {
    let src = r#"
fn f() to int:
    let x = 1 with { retries: "not_a_number" }
    x
"#;
    let warns = warnings(src);
    assert!(
        !warns.is_empty(),
        "Wrong type for 'retries' should produce warning"
    );
    assert!(
        warns[0].message.contains("retries"),
        "Got: {}",
        warns[0].message
    );
    assert!(
        warns[0].message.contains("Int"),
        "Should mention expected type Int"
    );
}

#[test]
fn test_with_valid_known_options_pass() {
    let src = r#"

activity do_work(input: str) to Result[str]:
    Ok(input)

workflow run() to Result[str]:
    let result = do_work("test") with { retries: 3, timeout: "10s", activity_id: "unique-1" }
    result
"#;
    let errs = errors(src);
    // Only known options with correct types — should produce 0 errors
    assert!(
        errs.is_empty(),
        "Valid 'with' options should have no errors, got: {:?}",
        errs
    );
    // And no type-related warnings either
    let warns = warnings(src);
    let type_warns: Vec<_> = warns
        .iter()
        .filter(|w| w.message.contains("'with' option"))
        .collect();
    assert!(
        type_warns.is_empty(),
        "Valid options should have no type warnings, got: {:?}",
        type_warns
    );
}

#[test]
fn test_durable_execution_example_e2e() {
    // Full end-to-end test using the example file content
    let src = r#"


activity validate_order(order_data: str) to Result[str]:
    let validated = "validated-" + order_data
    Ok(validated)

activity charge_payment(amount: int, card_token: str) to Result[str]:
    let tx = "tx-" + card_token
    Ok(tx)

activity send_confirmation(recipient: str, order_id: str) to Result[str]:
    let msg = "Order " + order_id + " confirmed for " + recipient
    Ok(msg)

workflow process_order(customer: str, order_data: str, amount: int) to Result[str]:
    let validated = validate_order(order_data) with { timeout: "5s" }
    let payment = charge_payment(amount, "card-123") with { retries: 3, timeout: "30s", initial_backoff: "500ms" }
    let confirmation = send_confirmation(customer, "order-001") with { retries: 2, activity_id: "confirm-order-001" }
    confirmation
"#;
    // Parse
    let tokens = vox_lexer::cursor::lex(src);
    let module = vox_parser::parser::parse(tokens).expect("Example should parse");

    // Type check
    let diags = vox_typeck::typecheck_module(&module);
    let type_errors: Vec<_> = diags
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    assert!(
        type_errors.is_empty(),
        "Example should have no type errors: {:?}",
        type_errors
    );

    // HIR lowering
    let hir = vox_hir::lower::lower_module(&module);
    assert_eq!(hir.activities.len(), 3, "Should have 3 activities");
    assert_eq!(hir.workflows.len(), 1, "Should have 1 workflow");

    // Rust codegen
    let rust_output = vox_codegen_rust::emit::emit_lib(&hir);
    assert!(
        rust_output.contains("pub async fn validate_order("),
        "Rust: validate_order"
    );
    assert!(
        rust_output.contains("pub async fn charge_payment("),
        "Rust: charge_payment"
    );
    assert!(
        rust_output.contains("pub async fn send_confirmation("),
        "Rust: send_confirmation"
    );
    assert!(
        rust_output.contains("pub async fn process_order("),
        "Rust: process_order workflow"
    );
    assert!(
        rust_output.contains("execute_activity"),
        "Rust: should use execute_activity"
    );

    // TS codegen
    let ts_output = vox_codegen_ts::generate(&module).expect("TS codegen should succeed");
    let ts_filenames: Vec<&str> = ts_output.files.iter().map(|(n, _)| n.as_str()).collect();
    assert!(
        ts_filenames.contains(&"activities.ts"),
        "TS: should produce activities.ts"
    );
    let activities_ts = ts_output
        .files
        .iter()
        .find(|(n, _)| n == "activities.ts")
        .unwrap();
    assert!(
        activities_ts
            .1
            .contains("export async function validate_order("),
        "TS: validate_order"
    );
    assert!(
        activities_ts.1.contains("executeActivity"),
        "TS: executeActivity helper"
    );
}

// --- Table / Index type checking ---

#[test]
fn test_table_registration_no_errors() {
    let src = r#"
@table type Task:
    title: str
    done: bool
    priority: int
"#;
    let errs = errors(src);
    assert!(
        errs.is_empty(),
        "Valid table should have no errors, got: {:?}",
        errs
    );
}

#[test]
fn test_index_on_known_table_no_errors() {
    let src = r#"
@table type Task:
    title: str
    done: bool

@index Task.by_done on (done)
"#;
    let errs = errors(src);
    assert!(
        errs.is_empty(),
        "Index on known table should have no errors, got: {:?}",
        errs
    );
}

#[test]
fn test_index_on_unknown_table_error() {
    let src = r#"
@index Missing.by_name on (name)
"#;
    let errs = errors(src);
    assert!(
        !errs.is_empty(),
        "Index on unknown table should produce an error"
    );
    assert!(
        errs[0].message.contains("unknown table 'Missing'"),
        "Error message: {}",
        errs[0].message
    );
}
#[test]
fn test_arg_type_mismatch_error() {
    let src = r#"
fn add(a: int, b: int) to int:
    ret a

fn main() to int:
    ret add(1, "str")
"#;
    let errs = errors(src);
    assert!(
        !errs.is_empty(),
        "Type mismatch in args should produce error"
    );
    assert!(
        errs[0].message.contains("Argument type mismatch"),
        "Got: {}",
        errs[0].message
    );
}

#[test]
fn test_arg_count_mismatch_error() {
    let src = r#"
fn add(a: int, b: int) to int:
    ret a

fn main() to int:
    ret add(1)
"#;
    let errs = errors(src);
    assert!(!errs.is_empty(), "Arg count mismatch should produce error");
    assert!(
        errs[0].message.contains("Argument count mismatch"),
        "Got: {}",
        errs[0].message
    );
}

#[test]
fn test_generic_type_mismatch() {
    let src = r#"
fn id<T>(x: T) to T:
    ret x

fn main() to int:
    let s: str = id(1)
    ret 0
"#;
    let errs = errors(src);
    assert!(
        !errs.is_empty(),
        "Generic type mismatch should produce error"
    );
    // Ensure the error relates to mistmatch between Int (inferred T) and Str (expected)
    // The exact message depends on unification order but mismatch term should appear.
    let msg = &errs[0].message;
    assert!(
        msg.contains("mismatch") || msg.contains("Incompatible"),
        "Got: {}",
        msg
    );
}

#[test]
fn test_generic_identity_works() {
    let src = r#"
fn id<T>(x: T) to T:
    ret x

fn main() to int:
    let i: int = id(1)
    ret i
"#;
    let errs = errors(src);
    assert!(
        errs.is_empty(),
        "Valid generic identity call should pass type check, got: {:?}",
        errs
    );
}
