//! Projection parity gate: WebIR + AppContract + RuntimeProjection + bundle SSOT.

use std::collections::BTreeSet;

use vox_codegen::projection_bundle::project_bundle_from_hir;
use vox_codegen::syntax_k::{canonical_web_ir_bytes, sha3_hex};
use vox_compiler::app_contract::canonical_app_contract_bytes;
use vox_compiler::hir::lower_module;
use vox_compiler::parser::parse;
use vox_compiler::required_capabilities::canonical_required_capabilities_bytes;
use vox_compiler::runtime_projection::canonical_runtime_projection_bytes;
use vox_compiler::shell_projection::canonical_shell_projection_bytes;

fn lower_src(src: &str) -> vox_compiler::hir::TypedCoreIR_v2 {
    let tokens = vox_compiler::lexer::lex(src);
    let module = parse(tokens).expect("parse");
    lower_module(&module)
}

#[test]
fn projection_triplet_is_deterministic_and_schema_versioned() {
    let src = r#"
@table type Task { title: str done: bool }

fn Home_render() to str {
    let rows = db.Task.filter({ done: false }).select("title", "done")
    return "div"
}

routes {
    "/" to Home_render
}

@query fn ping() to int { return 1 }
@server fn sf_ping() to int { return 1 }
@query fn list_tasks() to int { return 0 }
@mutation fn save_task(title: str) to int {
    db.Task.insert({ title: title, done: false })
    return 1
}
"#;
    let hir = lower_src(src);
    let bundle = project_bundle_from_hir(&hir);

    let web_bytes_a = canonical_web_ir_bytes(&bundle.web).expect("web bytes");
    let app_bytes_a = canonical_app_contract_bytes(&bundle.app).expect("app bytes");
    let rt_bytes_a = canonical_runtime_projection_bytes(&bundle.runtime).expect("runtime bytes");

    let web_bytes_b = canonical_web_ir_bytes(&bundle.web).expect("web bytes");
    let app_bytes_b = canonical_app_contract_bytes(&bundle.app).expect("app bytes");
    let rt_bytes_b = canonical_runtime_projection_bytes(&bundle.runtime).expect("runtime bytes");

    assert_eq!(
        web_bytes_a, web_bytes_b,
        "webir canonical bytes must be stable"
    );
    assert_eq!(
        app_bytes_a, app_bytes_b,
        "app contract canonical bytes must be stable"
    );
    assert_eq!(
        rt_bytes_a, rt_bytes_b,
        "runtime projection canonical bytes must be stable"
    );

    assert_eq!(bundle.app.schema_version, 2);
    assert_eq!(bundle.runtime.schema_version, 1);
    assert!(
        !bundle.app.server_fns.is_empty(),
        "expected @server contract"
    );
    assert!(!bundle.app.query_fns.is_empty(), "expected @query contract");
    assert!(
        !bundle.app.mutation_fns.is_empty(),
        "expected @mutation contract"
    );

    // Ensure we can monitor drift of each projection independently in CI logs.
    let web_hash = sha3_hex(&web_bytes_a);
    let app_hash = sha3_hex(&app_bytes_a);
    let runtime_hash = sha3_hex(&rt_bytes_a);
    assert_ne!(web_hash, app_hash);
    assert_ne!(app_hash, runtime_hash);
}

/// `@back_button` + endpoint: triplet remains deterministic (mobile primitive on HIR).
#[test]
fn projection_triplet_with_back_button_is_deterministic() {
    let src = r#"
@query fn on_back() to bool { return true }
@back_button {
    on_press: on_back
}
"#;
    let hir = lower_src(src);
    let bundle = project_bundle_from_hir(&hir);

    let web_bytes_a = canonical_web_ir_bytes(&bundle.web).expect("web bytes");
    let app_bytes_a = canonical_app_contract_bytes(&bundle.app).expect("app bytes");
    let rt_bytes_a = canonical_runtime_projection_bytes(&bundle.runtime).expect("runtime bytes");

    let web_bytes_b = canonical_web_ir_bytes(&bundle.web).expect("web bytes");
    let app_bytes_b = canonical_app_contract_bytes(&bundle.app).expect("app bytes");
    let rt_bytes_b = canonical_runtime_projection_bytes(&bundle.runtime).expect("runtime bytes");

    assert_eq!(web_bytes_a, web_bytes_b);
    assert_eq!(app_bytes_a, app_bytes_b);
    assert_eq!(rt_bytes_a, rt_bytes_b);
}

/// Bundle: determinism, required capabilities, and pairwise-distinct canonical hashes.
#[test]
fn projection_bundle_fixture_is_deterministic_and_distinct() {
    let src = r#"
@query @uses(net) fn api_ping() to int { return 1 }

@query fn handle_link(url: str) to str { return "/" }
@mutation fn store_token(token: str) to str { return token }

@deep_link { scheme: "vox" on_link: handle_link }
@push { on_register: store_token }

routes {
    "/" to Dash
}

component Dash() {
    state n: int = 0
    view: column(raw_class="dash") { text() { "d" } }
}
"#;
    let hir = lower_src(src);
    let b1 = project_bundle_from_hir(&hir);
    let b2 = project_bundle_from_hir(&hir);

    let w1 = canonical_web_ir_bytes(&b1.web).unwrap();
    let w2 = canonical_web_ir_bytes(&b2.web).unwrap();
    assert_eq!(w1, w2, "web canonical bytes");
    assert_eq!(
        canonical_app_contract_bytes(&b1.app).unwrap(),
        canonical_app_contract_bytes(&b2.app).unwrap()
    );
    assert_eq!(
        canonical_runtime_projection_bytes(&b1.runtime).unwrap(),
        canonical_runtime_projection_bytes(&b2.runtime).unwrap()
    );
    assert_eq!(
        canonical_shell_projection_bytes(&b1.shell).unwrap(),
        canonical_shell_projection_bytes(&b2.shell).unwrap()
    );
    assert_eq!(
        canonical_required_capabilities_bytes(&b1.capabilities).unwrap(),
        canonical_required_capabilities_bytes(&b2.capabilities).unwrap()
    );

    let expected_caps: BTreeSet<_> = ["deep_link", "net.http", "notifications"]
        .into_iter()
        .map(String::from)
        .collect();
    let got: BTreeSet<_> = b1.capabilities.capability_ids.iter().cloned().collect();
    assert_eq!(got, expected_caps, "capability_ids = {got:?}");

    let h_web = sha3_hex(&canonical_web_ir_bytes(&b1.web).unwrap());
    let h_app = sha3_hex(&canonical_app_contract_bytes(&b1.app).unwrap());
    let h_rt = sha3_hex(&canonical_runtime_projection_bytes(&b1.runtime).unwrap());
    let h_shell = sha3_hex(&canonical_shell_projection_bytes(&b1.shell).unwrap());
    let h_caps = sha3_hex(&canonical_required_capabilities_bytes(&b1.capabilities).unwrap());
    let set: BTreeSet<_> = [h_web, h_app, h_rt, h_shell, h_caps].into_iter().collect();
    assert_eq!(
        set.len(),
        5,
        "pairwise-distinct canonical projection hashes"
    );
}
