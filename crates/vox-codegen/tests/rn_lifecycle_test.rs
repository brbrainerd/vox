//! The RN target must emit lifecycle hooks (`on mount`, effects) — previously
//! it dropped them, so a component that loaded data in `on mount:` rendered its
//! initial state forever. Async `@query`/`@mutation` calls in those bodies must
//! be awaited inside a fire-and-forget async IIFE (a `useEffect` callback can't
//! be async).

use vox_codegen::codegen_ts::CodegenOptions;
use vox_codegen::codegen_ts::rn::generate_rn;
use vox_compiler::{hir::lower_module, lexer::cursor::lex, parser::parse};

fn rn_component(src: &str, component: &str) -> String {
    let m = parse(lex(src)).expect("parse");
    let hir = lower_module(&m);
    let out = generate_rn(&hir, &CodegenOptions::default()).expect("rn emit");
    let target = format!("{component}.tsx");
    out.files
        .into_iter()
        .find(|(name, _)| name == &target)
        .unwrap_or_else(|| panic!("no {target} emitted; files: see emit"))
        .1
}

const SRC: &str = r#"
@query fn load_count() to int { return 0 }

component Dashboard() {
    state total: int = 0
    on mount: {
        total = load_count()
    }
    view: column {
        text { "Total: " + str(total) }
    }
}
"#;

#[test]
fn rn_emits_on_mount_useeffect() {
    let tsx = rn_component(SRC, "Dashboard");
    assert!(
        tsx.contains("useEffect(() => {"),
        "RN must emit a useEffect for `on mount`; got:\n{tsx}"
    );
    assert!(
        tsx.lines()
            .any(|l| l.contains("from \"react\"") && l.contains("useEffect")),
        "RN must import useEffect from react; got:\n{tsx}"
    );
}

#[test]
fn rn_on_mount_awaits_async_endpoint_in_iife() {
    let tsx = rn_component(SRC, "Dashboard");
    assert!(
        tsx.contains("set_total(await load_count())"),
        "async endpoint call in on-mount must be awaited; got:\n{tsx}"
    );
    assert!(
        tsx.contains("async () =>"),
        "awaited body must run in an async IIFE; got:\n{tsx}"
    );
}
