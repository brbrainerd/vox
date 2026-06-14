/// Task 8B: VUV control-flow lowering.
///
/// `for`/`if`/`match` in render position must produce structured DomNode variants
/// (DomNode::Loop / DomNode::Conditional) instead of falling through to DomNode::Expr.
///
/// We construct HirModule directly to avoid parser-level JSX constraints.
use vox_codegen::web_ir::{DomNode, lower::lower_hir_to_web_ir};
use vox_compiler::hir::{
    DefId, HirExpr, HirModule, HirReactiveComponent,
};
use vox_compiler::ast::span::Span;

fn s() -> Span { Span::new(0, 0) }
fn id() -> DefId { DefId(0) }

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

    let has_loop = web.dom_nodes.iter().any(|n| matches!(n, DomNode::Loop { .. }));
    assert!(
        has_loop,
        "HirExpr::For in view must lower to DomNode::Loop, not DomNode::Expr\nnodes: {:?}",
        web.dom_nodes
    );
}

#[test]
fn if_expr_in_view_is_not_a_loop() {
    // HirExpr::If must NOT produce DomNode::Loop — it should be Conditional or Expr.
    use vox_compiler::hir::HirStmt;
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

    let has_spurious_loop = web.dom_nodes.iter().any(|n| matches!(n, DomNode::Loop { .. }));
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
