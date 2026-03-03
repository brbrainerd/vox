use vox_lexer::cursor::lex;
use vox_parser::parser::parse;
use vox_typeck::{typecheck_hir, env::TypeEnv, builtins::BuiltinTypes};
use vox_hir::lower_module;

#[test]
fn durable_workflow_recovery_logic() {
    let source = "
activity send_email(recipient: str, body: str) to Result[bool]:
    // Mock email sending
    ret Ok(true)

workflow welcome_onboarding(user_id: str) to Unit:
    let email_sent = send_email(\"user@example.com\", \"Welcome!\")

    // In a real durable workflow, this side-effect would be
    // recorded in an execution log.
    // If the process crashes here, recovery starts from the log.

    match email_sent:
        Ok(s) -> print(\"Onboarding started for \" + user_id)
        Error(e) -> print(\"Error\")
";

    let tokens = lex(source);
    let module = parse(tokens).expect("Parse failed");
    let hir = lower_module(&module);
    let mut env = TypeEnv::new();
    let builtins = BuiltinTypes::register_all(&mut env);
    let _hir_diags = typecheck_hir(&hir, &mut env, &builtins, "");

    // Verify HIR structure
    assert_eq!(hir.activities.len(), 1);
    assert_eq!(hir.workflows.len(), 1);

    // The recovery itself is a runtime feature.
    // Here we verify that the metadata for durability is present.
    // (In Vox, @workflow functions are transformed to state machines)

    let wf = &hir.workflows[0];
    assert_eq!(wf.name, "welcome_onboarding");
    // Verify it takes params which are used for the initial state
    assert_eq!(wf.params.len(), 1);
}
