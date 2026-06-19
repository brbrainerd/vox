//! Effect-level proof that validate_vuv_source catches real GUI-guarantee
//! violations — fed the forbidden-corpus fixtures, not substrings. Closes the
//! AGH-0007 follow-up (a) and enforces the §B-9 "prove the effect" rule.
use vox_orchestrator_mcp::gui_registry_tools::validate_vuv_source;

fn errors(v: &serde_json::Value) -> u64 {
    v["error_count"].as_u64().unwrap_or(0)
}

fn codes(v: &serde_json::Value) -> Vec<String> {
    v["diagnostics"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|d| d["code"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn clean_source_validates_ok() {
    let src = include_str!("../../../examples/golden-ts/form_basic.vox");
    let report = validate_vuv_source(src);
    assert_eq!(report["ok"], serde_json::Value::Bool(true), "report: {report}");
    assert_eq!(errors(&report), 0, "report: {report}");
}

#[test]
fn contrast_source_is_rejected() {
    // contrast_gray_on_white.vox uses Tailwind palette colors inline (no token
    // registry needed) — triggers web_ir_validate.a11y.insufficient_contrast.
    let src = include_str!("../../../examples/forbidden/contrast_gray_on_white.vox");
    let report = validate_vuv_source(src);
    assert!(
        codes(&report).iter().any(|c| c.contains("contrast")),
        "expected a contrast diagnostic, got: {report}"
    );
}

#[test]
fn occlusion_source_is_rejected() {
    let src = include_str!("../../../examples/forbidden/raw_class_occlusion.vox");
    let report = validate_vuv_source(src);
    assert!(
        errors(&report) > 0 || !codes(&report).is_empty(),
        "expected an occlusion/style diagnostic, got: {report}"
    );
}
