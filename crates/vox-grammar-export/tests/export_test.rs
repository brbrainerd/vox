use vox_grammar_export::ebnf::emit_ebnf;
use vox_grammar_export::gbnf::emit_gbnf;
use vox_grammar_export::json_schema::emit_json_schema;
use vox_grammar_export::lark::emit_lark;
use vox_grammar_export::versioning::{get_compiler_version, get_version, verify_grammar_alignment};
use vox_grammar_export::{GrammarExportConfig, GrammarFormat, export};

// ── EBNF tests (Tasks 42-44) ─────────────────────────────────────────────────

#[test]
fn test_ebnf_export() {
    let ebnf = emit_ebnf();
    // The full grammar includes all declaration types
    assert!(ebnf.contains("fn_decl"), "EBNF must include fn_decl");
    assert!(ebnf.contains("actor"), "EBNF must include actor");
    assert!(ebnf.contains("workflow"), "EBNF must include workflow");
    assert!(ebnf.contains("match_expr"), "EBNF must include match_expr");
    assert!(ebnf.contains("@table"), "EBNF must include @table");
    assert!(ebnf.contains("@mcp.tool"), "EBNF must include @mcp.tool");
}

#[test]
fn test_ebnf_non_empty() {
    let ebnf = emit_ebnf();
    assert!(!ebnf.is_empty(), "EBNF must not be empty");
    assert!(
        ebnf.lines().count() >= 10,
        "EBNF should have at least 10 lines"
    );
}

#[test]
fn test_ebnf_known_valid_constructs() {
    let ebnf = emit_ebnf();
    // All 57 production rules must be represented
    for rule in &[
        "module",
        "decl",
        "fn_decl",
        "type_def",
        "import",
        "actor",
        "workflow",
        "activity",
        "http_route",
        "table",
        "index",
        "test",
        "forall",
        "server_fn",
        "query_fn",
        "mutation_fn",
        "mcp_tool",
        "mcp_resource",
        "component",
        "reactive_component",
        "routes",
        "agent",
        "environment",
        "stmt",
        "let_stmt",
        "while_stmt",
        "block",
        "expr",
        "match_expr",
        "if_expr",
        "for_expr",
        "call_expr",
        "field_access",
        "method_call",
        "literal",
        "int_lit",
        "string_lit",
        "bool_lit",
        "object_lit",
        "list_lit",
        "tuple_lit",
        "jsx_expr",
        "type_expr",
        "ident",
        "bin_op",
        "pattern",
        "lambda",
        "spawn_expr",
    ] {
        assert!(ebnf.contains(rule), "EBNF missing rule: {rule}");
    }
}

// ── GBNF tests ───────────────────────────────────────────────────────────────

#[test]
fn test_gbnf_export() {
    let gbnf = emit_gbnf();
    assert!(gbnf.contains("root ::= expr"));
    assert!(
        gbnf.contains("expr ::= literal | record_lit | tuple_lit | ident | call_expr | math_expr")
    );
}

#[test]
fn test_gbnf_no_left_recursion() {
    let gbnf = emit_gbnf();
    // In GBNF, `expr` must not directly recurse as the first alternative.
    // If "expr ::= expr ..." appears that's left-recursion.
    let has_left_rec = gbnf.lines().any(|l| {
        l.starts_with("expr") && l.contains("::=") && {
            let rhs = l.split_once("::=").map(|x| x.1.trim()).unwrap_or("");
            rhs.starts_with("expr")
        }
    });
    assert!(
        !has_left_rec,
        "GBNF should not have direct left-recursion in 'expr'"
    );
}

// ── Lark tests ───────────────────────────────────────────────────────────────

#[test]
fn test_lark_export() {
    let lark = emit_lark();
    assert!(!lark.is_empty(), "Lark grammar must not be empty");
    assert!(
        lark.contains("start: module"),
        "Lark grammar must define 'start' rule"
    );
    assert!(
        lark.contains("fn_decl"),
        "Lark grammar must include fn_decl"
    );
    assert!(
        lark.contains("IDENT:"),
        "Lark grammar must define IDENT terminal"
    );
}

#[test]
fn test_lark_known_constructs() {
    let lark = emit_lark();
    for construct in &[
        "module",
        "decl",
        "fn_decl",
        "let_stmt",
        "block",
        "expr",
        "literal",
        "call_expr",
        "if_expr",
        "STRING_LIT",
        "INT_LIT",
        "BOOL_LIT",
        "BIN_OP",
        "%ignore",
    ] {
        assert!(
            lark.contains(construct),
            "Lark grammar missing: {construct}"
        );
    }
}

#[test]
fn test_grammar_exports_track_view_call_surface_not_retired_pratt_jsx() {
    let ebnf = emit_ebnf();
    let lark = emit_lark();

    assert!(
        ebnf.contains("view_call_expr"),
        "EBNF must expose view_call_expr as parser-facing surface"
    );
    assert!(
        lark.contains("view_call_expr"),
        "Lark must expose view_call_expr as parser-facing surface"
    );
    assert!(
        !ebnf.contains("pratt_jsx.rs"),
        "EBNF must not reference retired parser file names"
    );
    assert!(
        !lark.contains("pratt_jsx.rs"),
        "Lark must not reference retired parser file names"
    );
}

// ── JSON Schema tests ─────────────────────────────────────────────────────────

#[test]
fn test_json_schema_export() {
    let schema = emit_json_schema();
    assert!(!schema.is_empty(), "JSON Schema must not be empty");
    let parsed: serde_json::Value =
        serde_json::from_str(&schema).expect("JSON Schema must be valid JSON");
    assert_eq!(
        parsed["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
    assert!(
        parsed["$defs"].is_object(),
        "JSON Schema must contain $defs"
    );
}

#[test]
fn test_json_schema_core_nodes() {
    let schema = emit_json_schema();
    let parsed: serde_json::Value = serde_json::from_str(&schema).unwrap();
    let defs = parsed["$defs"].as_object().unwrap();

    for node in &[
        "Module",
        "FnDecl",
        "LetDecl",
        "TypeDecl",
        "Block",
        "Expr",
        "Literal",
        "BinaryExpr",
        "CallExpr",
        "IfExpr",
        "RecordLit",
        "TupleLit",
        "ArrayLit",
        "FieldAccess",
    ] {
        assert!(defs.contains_key(*node), "JSON Schema missing $defs.{node}");
    }
}

#[test]
fn test_json_schema_binary_expr_ops() {
    let schema = emit_json_schema();
    let parsed: serde_json::Value = serde_json::from_str(&schema).unwrap();
    let ops = &parsed["$defs"]["BinaryExpr"]["properties"]["op"]["enum"];
    assert!(ops.is_array(), "BinaryExpr.op must be an enum array");
    let op_list: Vec<&str> = ops
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(op_list.contains(&"+"), "operator + must be present");
    assert!(op_list.contains(&"=="), "operator == must be present");
}

// ── Compact LLM Prompt tests ──────────────────────────────────────────────

#[test]
fn test_compact_prompt_non_empty() {
    let prompt = vox_grammar_export::compact_prompt::emit_compact_llm_prompt();
    assert!(!prompt.is_empty(), "Compact prompt must not be empty");
    assert!(
        prompt.contains("Vox 0.4 Grammar Cheatsheet"),
        "Compact prompt must contain version header"
    );
}

#[test]
fn test_compact_prompt_covers_all_declaration_types() {
    let prompt = vox_grammar_export::compact_prompt::emit_compact_llm_prompt();
    // Every top-level declaration type from the EBNF must appear
    for decl in &[
        "fn",
        "type",
        "import",
        "actor",
        "workflow",
        "activity",
        "http",
        "@table",
        "@index",
        "@test",
        "@forall",
        "@server",
        "@query",
        "@mutation",
        "@mcp.tool",
        "@mcp.resource",
        "component",
        "@v0",
        "routes",
        "@loading",
        "agent",
        "environment",
    ] {
        assert!(
            prompt.contains(decl),
            "Compact prompt missing declaration: {decl}"
        );
    }
}

#[test]
fn test_compact_prompt_shorter_than_ebnf() {
    let ebnf = emit_ebnf();
    let prompt = vox_grammar_export::compact_prompt::emit_compact_llm_prompt();
    assert!(
        prompt.len() < ebnf.len(),
        "Compact prompt ({}) should be shorter than raw EBNF ({})",
        prompt.len(),
        ebnf.len()
    );
}

// ── Dispatch tests ────────────────────────────────────────────────────────

#[test]
fn test_export_dispatch_all_formats() {
    for format in &[
        GrammarFormat::Ebnf,
        GrammarFormat::Gbnf,
        GrammarFormat::Lark,
        GrammarFormat::JsonSchema,
        GrammarFormat::TreeSitterGrammar,
    ] {
        let config = GrammarExportConfig {
            format: format.clone(),
            ..GrammarExportConfig::default()
        };
        let result = export(&config);
        if matches!(
            format,
            GrammarFormat::Gbnf | GrammarFormat::TreeSitterGrammar
        ) {
            assert!(
                result.is_err(),
                "export({}) should be an error",
                format.as_str()
            );
            let err = result.err().unwrap().to_string();
            if matches!(format, GrammarFormat::Gbnf) {
                assert!(
                    err.contains("CVE-2026-2069"),
                    "GBNF error missing CVE reference"
                );
            } else {
                assert!(
                    err.contains("not yet implemented"),
                    "Tree-sitter error missing 'not yet implemented'"
                );
            }
        } else {
            assert!(
                result.is_ok(),
                "export({}) returned error: {:?}",
                format.as_str(),
                result.err()
            );
            assert!(
                !result.unwrap().grammar_text.is_empty(),
                "export({}) returned empty text",
                format.as_str()
            );
        }
    }
}

// ── Versioning ────────────────────────────────────────────────────────────────

#[test]
fn test_versioning_alignment() {
    assert!(verify_grammar_alignment().is_ok());
    assert_eq!(get_version(), get_compiler_version());
}
