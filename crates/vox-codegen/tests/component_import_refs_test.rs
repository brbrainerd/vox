//! Regression: a component that calls a `@query`/`@mutation` fn inside an
//! `on mount:` block (the common data-loading pattern) must emit an import for
//! that fn from `./vox-client`. Before the shared `collect_component_import_refs`
//! helper, only the view + prelude statements were walked, so on-mount /
//! effect / derived calls produced a reference with no matching import — a
//! runtime ReferenceError in the generated app.
//!
//! Both the web (reactive) and RN emitters draw their import sets from the same
//! helper, so this test pins the web path; the RN path shares the code.

use vox_codegen::codegen_ts::emitter::generate;
use vox_compiler::{hir::lower_module, lexer::cursor::lex, parser::parse};

fn emit(src: &str) -> String {
    let m = parse(lex(src)).expect("parse");
    let hir = lower_module(&m);
    generate(&hir)
        .expect("emit")
        .files
        .iter()
        .map(|(_, c)| c.clone())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn on_mount_endpoint_call_emits_vox_client_import() {
    let src = r#"
@query fn load_count() to int { return 0 }

component Dashboard() {
    state count: int = 0
    on mount: {
        count = load_count()
    }
    view: column {
        text { "Count: " + str(count) }
    }
}
"#;
    let ts = emit(src);
    assert!(
        ts.contains("import { load_count } from \"./vox-client\""),
        "on-mount @query call must import its fn from ./vox-client; got:\n{ts}"
    );
}

#[test]
fn view_component_ref_emits_sibling_import() {
    let src = r#"
component NavBar() {
    view: row { text { "nav" } }
}

component Page() {
    view: column {
        NavBar()
        text { "body" }
    }
}
"#;
    let ts = emit(src);
    assert!(
        ts.contains("import { NavBar } from \"./NavBar\""),
        "view component reference must import the sibling; got:\n{ts}"
    );
}

#[test]
fn no_spurious_self_import() {
    // A component that references only itself-shaped tags must not import itself.
    let src = r#"
component Solo() {
    view: column { text { "alone" } }
}
"#;
    let ts = emit(src);
    assert!(
        !ts.contains("import { Solo } from \"./Solo\""),
        "component must not self-import; got:\n{ts}"
    );
}
