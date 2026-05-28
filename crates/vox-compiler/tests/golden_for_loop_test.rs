//! Golden tests for `for x, i in arr { … }` loops in view blocks.
//!
//! Validates the 5-field `HirExpr::For` variant (binding, optional index,
//! iterable, body, span) and its TSX lowering to `arr.map((x, i) => …)`.
//!
//! Fixtures use post-VUV-9 view-call syntax (`panel(class="x") { … }` rather
//! than `<div class="x">…</div>`).

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
    let path = format!("{}/tests/fixtures/for/{}", env!("CARGO_MANIFEST_DIR"), name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

// ── for r in rows { … } — synthetic _i ────────────────────────────────────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn for_loop_minimal_body_compiles() {
    let src = read_fixture("for_minimal_body.vox");
    let files = compile_components(&src);
    let ts = get_component(&files, "Empty");

    assert!(
        ts.contains("rows.map((r, _i) =>"),
        "Empty.tsx must lower `for r in rows` to `.map((r, _i) =>`. got:\n{ts}"
    );
    assert!(
        ts.contains("<span"),
        "Empty.tsx body must contain a <span> render. got:\n{ts}"
    );
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn for_loop_no_index_emits_underscore_i() {
    let src = read_fixture("for_no_index.vox");
    let files = compile_components(&src);
    let ts = get_component(&files, "NoIndex");

    assert!(
        ts.contains("rows.map((r, _i) =>"),
        "NoIndex.tsx must use synthetic _i for index-free for-loop. got:\n{ts}"
    );
    assert!(
        ts.contains("{r.name}"),
        "NoIndex.tsx must render `r.name` as JSX expression. got:\n{ts}"
    );
}

// ── for x, i in arr { … } — explicit index ────────────────────────────────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn for_loop_emits_array_map_with_explicit_index() {
    let src = read_fixture("runs_table.vox");
    let files = compile_components(&src);
    let ts = get_component(&files, "RunsTable");

    assert!(
        ts.contains("rows.map((r, i) =>"),
        "RunsTable.tsx must preserve user-named `i` index. got:\n{ts}"
    );
    assert!(
        ts.contains("key={r.id}"),
        "RunsTable.tsx must forward `key={{r.id}}` prop. got:\n{ts}"
    );
    assert!(
        ts.contains("{r.id}") && ts.contains("{r.duration}"),
        "RunsTable.tsx must render both r.id and r.duration. got:\n{ts}"
    );
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn for_loop_nested_emits_nested_maps() {
    let src = read_fixture("for_nested.vox");
    let files = compile_components(&src);
    let ts = get_component(&files, "Matrix");

    // Outer for row, i in matrix
    assert!(
        ts.contains("matrix.map((row, i) =>"),
        "Matrix.tsx outer loop must be matrix.map((row, i) => …). got:\n{ts}"
    );
    // Inner for cell, j in row
    assert!(
        ts.contains("row.map((cell, j) =>"),
        "Matrix.tsx inner loop must be row.map((cell, j) => …). got:\n{ts}"
    );
    assert!(
        ts.contains("{cell}"),
        "Matrix.tsx must render the cell expression. got:\n{ts}"
    );
}
