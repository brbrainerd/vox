/// Task 8B: VUV control-flow lowering.
///
/// `for`/`if`/`match` in render position must produce structured DomNode variants
/// (DomNode::Loop / DomNode::Conditional) instead of falling through to DomNode::Expr.
///
/// Also covers Task 2.1 (PreludeStmt was silently dropped) and Task 2.2 (Derived
/// names were missing from the reactive name-set).
///
/// We construct HirModule directly to avoid parser-level JSX constraints.
use vox_codegen::web_ir::{BehaviorNode, DomNode, lower::lower_hir_to_web_ir};
use vox_compiler::ast::span::Span;
use vox_compiler::hir::{
    DefId, HirDerived, HirExpr, HirModule, HirReactiveComponent, HirReactiveMember, HirStmt,
};

fn s() -> Span {
    Span::new(0, 0)
}
fn id() -> DefId {
    DefId(0)
}

fn component_with_view(name: &str, view: HirExpr) -> HirModule {
    let mut m = HirModule::default();
    m.components.push(HirReactiveComponent {
        id: id(),
        name: name.to_string(),
        params: vec![],
        members: vec![],
        view: Some(view),
        styles: vec![],
        layer: None,
        span: s(),
    });
    m
}

#[test]
fn for_expr_in_view_produces_loop_node() {
    // for item in items { item } in a component view must lower to DomNode::Loop.
    let view = HirExpr::For(
        "item".to_string(),
        None,
        Box::new(HirExpr::Ident("items".to_string(), s())),
        Box::new(HirExpr::Ident("item".to_string(), s())),
        None,
        s(),
    );
    let hir = component_with_view("TodoList", view);
    let web = lower_hir_to_web_ir(&hir);

    let has_loop = web
        .dom_nodes
        .iter()
        .any(|n| matches!(n, DomNode::Loop { .. }));
    assert!(
        has_loop,
        "HirExpr::For in view must lower to DomNode::Loop, not DomNode::Expr\nnodes: {:?}",
        web.dom_nodes
    );
}

#[test]
fn if_expr_in_view_is_not_a_loop() {
    // HirExpr::If must NOT produce DomNode::Loop — it should be Conditional or Expr.
    let view = HirExpr::If(
        Box::new(HirExpr::BoolLit(true, s())),
        vec![HirStmt::Expr {
            expr: HirExpr::StringLit("yes".to_string(), s()),
            span: s(),
        }],
        Some(vec![HirStmt::Expr {
            expr: HirExpr::StringLit("no".to_string(), s()),
            span: s(),
        }]),
        s(),
    );
    let hir = component_with_view("Guard", view);
    let web = lower_hir_to_web_ir(&hir);

    let has_spurious_loop = web
        .dom_nodes
        .iter()
        .any(|n| matches!(n, DomNode::Loop { .. }));
    assert!(
        !has_spurious_loop,
        "HirExpr::If in view must not produce DomNode::Loop\nnodes: {:?}",
        web.dom_nodes
    );
    assert!(
        !web.dom_nodes.is_empty(),
        "view lowering must produce at least one node"
    );
}

// ---------------------------------------------------------------------------
// Task 2.1 — HirReactiveMember::Stmt must produce BehaviorNode::PreludeStmt
// ---------------------------------------------------------------------------

fn component_with_members(name: &str, members: Vec<HirReactiveMember>) -> HirModule {
    let mut m = HirModule::default();
    m.components.push(HirReactiveComponent {
        id: id(),
        name: name.to_string(),
        params: vec![],
        members,
        view: None,
        styles: vec![],
        layer: None,
        span: s(),
    });
    m
}

#[test]
fn reactive_prelude_stmt_produces_behavior_node() {
    // A HirReactiveMember::Stmt must lower to BehaviorNode::PreludeStmt (was silently dropped).
    let stmt = HirStmt::Expr {
        expr: HirExpr::IntLit(42, s()),
        span: s(),
    };
    let hir = component_with_members("MyComp", vec![HirReactiveMember::Stmt(stmt)]);
    let web = lower_hir_to_web_ir(&hir);

    let has_prelude = web
        .behavior_nodes
        .iter()
        .any(|n| matches!(n, BehaviorNode::PreludeStmt { .. }));
    assert!(
        has_prelude,
        "HirReactiveMember::Stmt must lower to BehaviorNode::PreludeStmt, not be silently dropped\nnodes: {:?}",
        web.behavior_nodes
    );
}

// ---------------------------------------------------------------------------
// Task 2.2 — HirReactiveMember::Derived name must be in the reactive name-set
// ---------------------------------------------------------------------------

// The name-set is computed by the private reactive_component_name_set_for_web_ir fn.
// We test it indirectly: lower a component with a Derived member whose name appears
// in a State expression, and assert the DerivedDecl BehaviorNode carries that name
// (which confirms the lower path ran the Derived arm).  The name-set itself is used
// at lower.rs:885 to qualify idents in the view — absence of a crash is the observable
// outcome when there is no view, but we additionally verify the DerivedDecl is emitted.
#[test]
fn reactive_derived_name_collected_into_name_set() {
    // A HirReactiveMember::Derived must (a) emit a DerivedDecl behavior node and
    // (b) not panic — the name must have been inserted into the name set used by the
    // lower pass (regression guard: previously the Derived arm was in _ => {} and the
    // name was invisible to the lowering context).
    let derived = HirDerived {
        id: id(),
        name: "doubled".to_string(),
        ty: None,
        expr: HirExpr::IntLit(0, s()),
        span: s(),
    };
    let hir = component_with_members("Counter", vec![HirReactiveMember::Derived(derived)]);
    let web = lower_hir_to_web_ir(&hir);

    // The lower pass qualifies the name as "<Component>::<field>" (e.g. "Counter::doubled").
    let has_derived_decl = web
        .behavior_nodes
        .iter()
        .any(|n| matches!(n, BehaviorNode::DerivedDecl { name, .. } if name.ends_with("doubled")));
    assert!(
        has_derived_decl,
        "HirReactiveMember::Derived must produce BehaviorNode::DerivedDecl with the correct name\nnodes: {:?}",
        web.behavior_nodes
    );
}
