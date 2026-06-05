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

/// Emit and return the content of a single generated file by path.
fn emit_file(src: &str, path: &str) -> String {
    let m = parse(lex(src)).expect("parse");
    let hir = lower_module(&m);
    generate(&hir)
        .expect("emit")
        .files
        .into_iter()
        .find(|(p, _)| p == path)
        .unwrap_or_else(|| panic!("no emitted file `{path}`"))
        .1
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
fn multi_arg_endpoint_call_is_rewritten_to_named_object() {
    // `vox-client.ts` exposes endpoint fns as taking a single named-args object.
    // A positional call in a handler must be rewritten to that object form so
    // the call site matches the client signature (otherwise `args.field` is
    // undefined at runtime).
    let src = r#"
@mutation fn record_event(kind: str, payload: str) to Result[str] { return Ok("x") }

component Logger() {
    view: column {
        button(on_click={fn() {
            let _ = record_event("mood", "{}")
        }}) {
            "Log"
        }
    }
}
"#;
    let ts = emit(src);
    assert!(
        ts.contains("record_event({ kind: \"mood\", payload: \"{}\" })"),
        "positional endpoint call must become a named-args object; got:\n{ts}"
    );
    assert!(
        !ts.contains("record_event(\"mood\", \"{}\")"),
        "positional form must NOT survive; got:\n{ts}"
    );
}

#[test]
fn zero_arg_endpoint_call_stays_bare() {
    // Zero-param endpoint fns take no args object — a bare call is correct and
    // must not be wrapped into `f({})`.
    let src = r#"
@query fn refresh() to int { return 0 }

component Panel() {
    state n: int = 0
    on mount: {
        n = refresh()
    }
    view: column { text { "n" } }
}
"#;
    let ts = emit(src);
    assert!(
        ts.contains("refresh()"),
        "zero-arg call must stay bare; got:\n{ts}"
    );
    assert!(
        !ts.contains("refresh({"),
        "zero-arg endpoint must not gain an args object; got:\n{ts}"
    );
}

#[test]
fn no_spurious_self_import() {
    // A component's own file must not import itself. (Scoped to `Solo.tsx`: the
    // web bootstrap `vox-app.tsx` legitimately imports the root component to
    // render `<Solo />`, so a combined-files scan would false-positive on that.)
    let src = r#"
component Solo() {
    view: column { text { "alone" } }
}
"#;
    let solo = emit_file(src, "Solo.tsx");
    assert!(
        !solo.contains("import { Solo } from \"./Solo\""),
        "component's own file must not self-import; got:\n{solo}"
    );
}
