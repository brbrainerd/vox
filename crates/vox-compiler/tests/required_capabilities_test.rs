//! Fixture tests for [`vox_compiler::required_capabilities`].

use vox_compiler::hir::lower_module;
use vox_compiler::lexer::lex;
use vox_compiler::parser::parse;
use vox_compiler::required_capabilities::project_required_capabilities;

fn lower_src(src: &str) -> vox_compiler::hir::TypedCoreIR_v2 {
    let tokens = lex(src);
    let module = parse(tokens).expect("parse");
    lower_module(&module)
}

#[test]
fn empty_module_capabilities_empty() {
    let hir = lower_src("");
    let r = project_required_capabilities(&hir);
    assert!(r.capability_ids.is_empty());
}

#[test]
fn endpoint_uses_net_maps_to_net_http() {
    let src = r#"
@query fn ping() uses net to int { return 1 }
"#;
    let hir = lower_src(src);
    let r = project_required_capabilities(&hir);
    assert_eq!(r.capability_ids, vec!["net.http".to_string()]);
}

#[test]
fn push_decl_adds_notifications() {
    let src = r#"
@mutation fn store_token(token: str) to str { return token }
@push {
    on_register: store_token
}
"#;
    let hir = lower_src(src);
    let r = project_required_capabilities(&hir);
    assert_eq!(r.capability_ids, vec!["notifications".to_string()]);
}

#[test]
fn deep_link_and_push_sorted() {
    let src = r#"
@query fn handle_link(url: str) to str { return "/" }
@mutation fn store_token(token: str) to str { return token }
@deep_link {
    scheme: "vox"
    on_link: handle_link
}
@push {
    on_register: store_token
}
"#;
    let hir = lower_src(src);
    let r = project_required_capabilities(&hir);
    assert_eq!(
        r.capability_ids,
        vec!["deep_link".to_string(), "notifications".to_string()]
    );
}

#[test]
fn transcribe_microphone_derives_speech_and_microphone() {
    let hir = lower_src("fn note() -> Result[str] { Speech.transcribe_microphone() }");
    let r = project_required_capabilities(&hir);
    assert!(r.capability_ids.iter().any(|c| c == "speech"), "{r:?}");
    assert!(r.capability_ids.iter().any(|c| c == "microphone"), "{r:?}");
}

#[test]
fn file_transcribe_derives_speech_only() {
    // vox-arch-check: allow abs-path
    let hir = lower_src("fn note() -> Result[str] { Speech.transcribe(\"/tmp/a.wav\") }");
    let r = project_required_capabilities(&hir);
    assert!(r.capability_ids.iter().any(|c| c == "speech"), "{r:?}");
    assert!(!r.capability_ids.iter().any(|c| c == "microphone"), "{r:?}");
}

#[test]
fn fs_read_in_body_maps_fs_read() {
    let src = r#"
// vox-arch-check: allow abs-path
fn read_hosts() uses fs to str {
    return fs.read("/etc/hosts")
}
"#;
    let hir = lower_src(src);
    let r = project_required_capabilities(&hir);
    assert_eq!(r.capability_ids, vec!["fs.read".to_string()]);
}
