//! Tests that Config/Theme/Message/Skill/AgentDef declarations are lowered into
//! their respective `HirModule` vectors and do NOT fall into `legacy_ast_nodes`.
//!
//! Note: The descent parser does not currently produce these Decl variants from
//! plain source text — they are constructed programmatically here to verify the
//! HIR lowering arms. The tests mirror the pattern used in
//! `web_ir_lower_emit_test.rs` for `ThemeDecl`.

use vox_compiler::ast::decl::db::TableField;
use vox_compiler::ast::decl::typedef::VariantField;
use vox_compiler::ast::decl::{
    AgentDefDecl, ConfigDecl, Decl, FnDecl, MessageDecl, SkillDecl, ThemeDecl,
};
use vox_compiler::ast::span::Span;
use vox_compiler::ast::types::TypeExpr;

fn zero_span() -> Span {
    Span::new(0, 0)
}

fn fn_decl(name: &str) -> FnDecl {
    FnDecl {
        name: name.to_string(),
        generics: vec![],
        params: vec![],
        return_type: None,
        body: vec![],
        is_async: false,
        is_deprecated: false,
        is_pure: false,
        is_reactive: false,
        is_versioned: false,
        effects: vec![],
        is_traced: false,
        is_llm: false,
        llm_model: None,
        ai_structured_output_type: None,
        ai_max_iterations: 0,
        ai_task_category: None,
        ai_strengths: vec![],
        ai_tier_max: None,
        ai_cost_ceiling_usd_per_call: None,
        prompt_stage: None,
        prompt_schema: None,
        prompt_redact: vec![],
        subagent_policy: None,
        subagent_max_depth: None,
        subagent_budget_usd: None,
        subagent_description: None,
        subagent_parallel: false,
        subagent_complexity: None,
        search_corpus: None,
        search_query: None,
        search_into: None,
        search_top_k: None,
        search_policy: None,
        hole_spec: None,
        hole_reviewer: None,
        hole_cache_key: None,
        hole_constraints: vec![],
        embed: None,
        is_pub: false,
        auth_provider: None,
        roles: vec![],
        cors: None,
        webhook: None,
        cors_spec: None,
        rate_limit: None,
        pii: None,
        layer: None,
        preconditions: vec![],
        postconditions: vec![],
        invariants: vec![],
        verify_mode: vox_compiler::ast::decl::VerifyMode::Off,
        test_strategy: None,
        is_mobile_native: false,
        ts_extern_module: None,
        is_remote: false,
        inference_model: None,
        training_step: false,
        span: zero_span(),
    }
}

fn make_module(decls: Vec<Decl>) -> vox_compiler::ast::decl::Module {
    vox_compiler::ast::decl::Module {
        declarations: decls,
        span: zero_span(),
    }
}

// ── Config ────────────────────────────────────────────────────────────────────

#[test]
fn config_decl_lowers_out_of_legacy_nodes() {
    let module = make_module(vec![Decl::Config(ConfigDecl {
        name: "Settings".to_string(),
        fields: vec![TableField {
            name: "api_key".to_string(),
            type_ann: TypeExpr::Named {
                name: "String".to_string(),
                span: zero_span(),
            },
            description: None,
            span: zero_span(),
        }],
        is_deprecated: false,
        span: zero_span(),
    })]);
    let hir = vox_compiler::hir::lower::lower_module(&module);
    assert_eq!(hir.configs.len(), 1, "Config must produce a HirConfig");
    assert_eq!(hir.configs[0].name, "Settings");
    assert_eq!(hir.configs[0].fields.len(), 1);
    assert!(
        hir.legacy_ast_nodes.is_empty(),
        "Config must not fall into legacy_ast_nodes, got: {:?}",
        hir.legacy_ast_nodes
    );
}

// ── Theme ─────────────────────────────────────────────────────────────────────

#[test]
fn theme_decl_lowers_out_of_legacy_nodes() {
    let module = make_module(vec![Decl::Theme(ThemeDecl {
        name: "App".to_string(),
        light: vec![("primary".to_string(), "#fff".to_string())],
        dark: vec![("primary".to_string(), "#000".to_string())],
        span: zero_span(),
    })]);
    let hir = vox_compiler::hir::lower::lower_module(&module);
    assert_eq!(hir.themes.len(), 1, "Theme must produce a HirTheme");
    assert_eq!(hir.themes[0].name, "App");
    assert_eq!(hir.themes[0].light.len(), 1);
    assert!(
        hir.legacy_ast_nodes.is_empty(),
        "Theme must not fall into legacy_ast_nodes, got: {:?}",
        hir.legacy_ast_nodes
    );
}

// ── Message ───────────────────────────────────────────────────────────────────

#[test]
fn message_decl_lowers_out_of_legacy_nodes() {
    let module = make_module(vec![Decl::Message(MessageDecl {
        name: "UserCreated".to_string(),
        fields: vec![VariantField {
            name: "user_id".to_string(),
            type_ann: TypeExpr::Named {
                name: "String".to_string(),
                span: zero_span(),
            },
            json_as_attr: Default::default(),
            span: zero_span(),
        }],
        is_deprecated: false,
        span: zero_span(),
    })]);
    let hir = vox_compiler::hir::lower::lower_module(&module);
    assert_eq!(hir.messages.len(), 1, "Message must produce a HirMessage");
    assert_eq!(hir.messages[0].name, "UserCreated");
    assert_eq!(hir.messages[0].fields.len(), 1);
    assert!(
        hir.legacy_ast_nodes.is_empty(),
        "Message must not fall into legacy_ast_nodes, got: {:?}",
        hir.legacy_ast_nodes
    );
}

// ── Skill ─────────────────────────────────────────────────────────────────────

#[test]
fn skill_decl_lowers_out_of_legacy_nodes() {
    let module = make_module(vec![Decl::Skill(SkillDecl {
        func: fn_decl("summarize"),
    })]);
    let hir = vox_compiler::hir::lower::lower_module(&module);
    assert_eq!(hir.skills.len(), 1, "Skill must produce a HirSkill");
    assert_eq!(hir.skills[0].fn_name, "summarize");
    // The underlying function must also be in hir.functions.
    assert!(
        hir.functions.iter().any(|f| f.name == "summarize"),
        "Skill fn must also be in hir.functions"
    );
    assert!(
        hir.legacy_ast_nodes.is_empty(),
        "Skill must not fall into legacy_ast_nodes, got: {:?}",
        hir.legacy_ast_nodes
    );
}

// ── AgentDef ──────────────────────────────────────────────────────────────────

#[test]
fn agent_def_decl_lowers_out_of_legacy_nodes() {
    let module = make_module(vec![Decl::AgentDef(AgentDefDecl {
        func: fn_decl("my_agent"),
    })]);
    let hir = vox_compiler::hir::lower::lower_module(&module);
    assert_eq!(
        hir.agent_defs.len(),
        1,
        "AgentDef must produce a HirAgentDef"
    );
    assert_eq!(hir.agent_defs[0].fn_name, "my_agent");
    // The underlying function must also be in hir.functions.
    assert!(
        hir.functions.iter().any(|f| f.name == "my_agent"),
        "AgentDef fn must also be in hir.functions"
    );
    assert!(
        hir.legacy_ast_nodes.is_empty(),
        "AgentDef must not fall into legacy_ast_nodes, got: {:?}",
        hir.legacy_ast_nodes
    );
}
