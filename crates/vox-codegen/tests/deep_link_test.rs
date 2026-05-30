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

// Deep-link routing is emitted through the `@vox/runtime` adapter contract
// (commit f65fc27d5d), NOT direct Tauri `listen('vox-deep-link')`. The adapter
// wraps the platform deep-link listener; `useDeepLinkRouting(navigate)` takes
// the router's navigate fn as a parameter rather than importing `useNavigate`.

#[test]
fn deep_link_emits_runtime_adapter_hook() {
    let src = r#"
@query fn handle_link(url: str) to str { return "/" }
@deep_link {
    scheme: "voxmental"
    on_link: handle_link
}
"#;
    let ts = emit(src);
    assert!(ts.contains("voxRuntime.onDeepLink("), "got:\n{ts}");
    assert!(ts.contains("handle_link("), "got:\n{ts}");
    assert!(
        ts.contains("useDeepLinkRouting"),
        "must emit the useDeepLinkRouting hook, got:\n{ts}"
    );
    assert!(
        ts.contains("useEffect"),
        "must import useEffect, got:\n{ts}"
    );
    assert!(
        ts.contains("@vox/runtime"),
        "must import the @vox/runtime adapter, got:\n{ts}"
    );
}

#[test]
fn back_button_and_deep_link_deduplicates_runtime_import() {
    let src = r#"
@query fn handle_back() to bool { return true }
@query fn handle_link(url: str) to str { return "/" }
@back_button { on_press: handle_back }
@deep_link { scheme: "vox" on_link: handle_link }
"#;
    let ts = emit(src);
    // Both primitives route through the single `@vox/runtime` adapter; its
    // import must be deduplicated to one line in the shared mobile.ts file.
    let count = ts.matches("from '@vox/runtime'").count();
    assert_eq!(
        count, 1,
        "@vox/runtime import should appear once, got {count} times in:\n{ts}"
    );
}
