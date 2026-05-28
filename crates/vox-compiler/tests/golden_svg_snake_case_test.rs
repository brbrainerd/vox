//! Golden tests for SVG snake_case → camelCase normalization.
//!
//! Validates that `is_known_html_view_tag` recognises SVG element forms
//! (`radial_gradient`, `linear_gradient`, `fe_gaussian_blur`, `foreign_object`,
//! `clip_path`, `filter`) AND that `map_jsx_tag` / `map_jsx_attr_name` rewrite
//! them to React-canonical camelCase at emit time.
//!
//! Fixtures use post-VUV-9 view-call syntax (`svg(view_box="…") { … }`).

fn compile_components(src: &str) -> Vec<(String, String)> {
    let tokens = vox_compiler::lexer::lex(src);
    let module =
        vox_compiler::parser::parse(tokens).unwrap_or_else(|e| panic!("parse failed: {e:?}"));
    let hir = vox_compiler::hir::lower_module(&module);
    let out =
        vox_codegen::codegen_ts::generate(&hir).unwrap_or_else(|e| panic!("codegen failed: {e:?}"));
    out.files.into_iter().collect()
}

fn get_component<'a>(files: &'a [(String, String)], name: &str) -> &'a str {
    let filename = format!("{name}.tsx");
    files
        .iter()
        .find(|(n, _)| n == &filename)
        .map(|(_, c)| c.as_str())
        .unwrap_or_else(|| panic!("{filename} not found in codegen output"))
}

fn read_fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/svg/{}", env!("CARGO_MANIFEST_DIR"), name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

// ── snake_case attribute aliases on <svg> + children ─────────────────────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn svg_attribute_snake_case_normalises_to_camel() {
    let src = read_fixture("snake_case.vox");
    let files = compile_components(&src);
    let ts = get_component(&files, "PlayIcon");

    for (snake, camel) in &[
        ("view_box", "viewBox"),
        ("stroke_width", "strokeWidth"),
        ("stroke_linecap", "strokeLinecap"),
        ("stroke_linejoin", "strokeLinejoin"),
    ] {
        assert!(
            !ts.contains(&format!("{snake}=")),
            "PlayIcon.tsx must not emit raw snake `{snake}=`. got:\n{ts}"
        );
        assert!(
            ts.contains(&format!("{camel}=")),
            "PlayIcon.tsx must emit camel `{camel}=`. got:\n{ts}"
        );
    }
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn svg_tag_snake_case_normalises_to_camel() {
    let src = read_fixture("snake_case.vox");
    let files = compile_components(&src);
    let ts = get_component(&files, "Halo");

    // Tag rewrites
    assert!(
        ts.contains("<radialGradient"),
        "Halo.tsx must emit <radialGradient> (not <radial_gradient>). got:\n{ts}"
    );
    assert!(
        !ts.contains("<radial_gradient"),
        "Halo.tsx must not emit raw <radial_gradient>. got:\n{ts}"
    );
    // Attribute rewrites on children
    for (snake, camel) in &[
        ("pattern_units", "patternUnits"),
        ("preserve_aspect_ratio", "preserveAspectRatio"),
        ("stop_color", "stopColor"),
        ("stop_opacity", "stopOpacity"),
    ] {
        assert!(
            ts.contains(&format!("{camel}=")),
            "Halo.tsx must emit camel `{camel}=`. got:\n{ts}"
        );
        assert!(
            !ts.contains(&format!("{snake}=")),
            "Halo.tsx must not emit raw `{snake}=`. got:\n{ts}"
        );
    }
}

// ── linear_gradient / fe_gaussian_blur / foreign_object / filter ─────────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn additional_svg_snake_case_tags_normalise() {
    let src = read_fixture("uncovered_tags.vox");
    let files = compile_components(&src);
    let ts = get_component(&files, "Filtered");

    for (snake, camel) in &[
        ("linear_gradient", "linearGradient"),
        ("fe_gaussian_blur", "feGaussianBlur"),
        ("foreign_object", "foreignObject"),
    ] {
        assert!(
            ts.contains(&format!("<{camel}")),
            "Filtered.tsx must emit <{camel}>. got:\n{ts}"
        );
        assert!(
            !ts.contains(&format!("<{snake}")),
            "Filtered.tsx must not emit raw <{snake}>. got:\n{ts}"
        );
    }
    // <filter> stays as filter (already canonical)
    assert!(
        ts.contains("<filter "),
        "Filtered.tsx must emit <filter> tag. got:\n{ts}"
    );
    // std_deviation → stdDeviation attribute
    assert!(
        ts.contains("stdDeviation="),
        "Filtered.tsx must emit stdDeviation= attr. got:\n{ts}"
    );
}
