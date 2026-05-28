/// Integration tests for GA-01, GA-05, GA-19, GA-20, GA-26 boilerplate-reduction grafts.
use vox_compiler::typeck::typecheck_ast_module;
use vox_compiler::{lexer::cursor::lex, parser::parse};

// ── GA-20 / CC-23 — @tokens contrast check ────────────────────────────────

#[test]
fn tokens_block_parses_without_error() {
    let src = r##"
@tokens {
    color primary light: "#1a73e8" dark: "#8ab4f8"
    spacing sm: "4px"
    font sans family: "Inter, sans-serif"
}
fn dummy() to int { return 1 }
"##;
    let m = parse(lex(src)).expect("parse should succeed for @tokens block");
    // No parse errors; typecheck should also not crash.
    let _ds = typecheck_ast_module(src, &m);
}

#[test]
fn tokens_contrast_violation_produces_diagnostic() {
    // Both hex values here have very similar luminance → WCAG AA failure.
    let src = r##"
@tokens {
    color bad_pair light: "#777777" dark: "#888888"
}
fn dummy() to int { return 1 }
"##;
    let m = parse(lex(src)).expect("parse");
    let ds = typecheck_ast_module(src, &m);
    let hit = ds
        .iter()
        .find(|d| d.code.as_deref() == Some("vox/tokens/contrast-violation"));
    assert!(
        hit.is_some(),
        "expected vox/tokens/contrast-violation diagnostic for low-contrast pair; got {:?}",
        ds.iter().map(|d| d.code.as_deref()).collect::<Vec<_>>()
    );
}

// ── GA-05 — @uses decorator ───────────────────────────────────────────────

#[test]
fn uses_decorator_parses_without_error() {
    let src = r#"
@uses(net)
fn fetch_data() to int { return 1 }
"#;
    let m = parse(lex(src)).expect("parse should succeed for @uses decorator");
    // Verify effect was stored
    let func = m.declarations.iter().find_map(|d| {
        if let vox_compiler::ast::decl::Decl::Function(f) = d {
            Some(f)
        } else {
            None
        }
    });
    assert!(func.is_some(), "should have parsed function");
    let effects = &func.unwrap().effects;
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, vox_compiler::ast::decl::effect::EffectAnnotation::Net)),
        "@uses(net) should populate effects with Net; got {:?}",
        effects
    );
}

#[test]
fn uses_decorator_multi_effects_parses() {
    let src = r#"
@uses(net, fs)
fn upload_file() to int { return 1 }
"#;
    let m = parse(lex(src)).expect("parse");
    let func = m.declarations.iter().find_map(|d| {
        if let vox_compiler::ast::decl::Decl::Function(f) = d {
            Some(f)
        } else {
            None
        }
    });
    let effects = &func.unwrap().effects;
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, vox_compiler::ast::decl::effect::EffectAnnotation::Net)),
        "expected Net effect"
    );
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, vox_compiler::ast::decl::effect::EffectAnnotation::Fs)),
        "expected Fs effect"
    );
}

// ── GA-19 — semantic UI a11y ──────────────────────────────────────────────

#[test]
fn semantic_ui_missing_label_produces_diagnostic() {
    // Component view with Dialog but no label prop → a11y error.
    let src = r#"
component Nav() {
    view: Dialog() { "content" }
}
"#;
    let m = parse(lex(src)).expect("parse");
    let ds = typecheck_ast_module(src, &m);
    let hit = ds
        .iter()
        .find(|d| d.code.as_deref() == Some("vox/a11y/dialog-missing-label"));
    assert!(
        hit.is_some(),
        "expected vox/a11y/dialog-missing-label for Dialog without label; got {:?}",
        ds.iter().map(|d| d.code.as_deref()).collect::<Vec<_>>()
    );
}

#[test]
fn semantic_ui_with_label_passes() {
    // Component view with Dialog + label prop → no a11y error.
    let src = r#"
component Nav() {
    view: Dialog(label="Confirm action") { "content" }
}
"#;
    let m = parse(lex(src)).expect("parse");
    let ds = typecheck_ast_module(src, &m);
    let hit = ds
        .iter()
        .find(|d| d.code.as_deref() == Some("vox/a11y/dialog-missing-label"));
    assert!(
        hit.is_none(),
        "Dialog with label= should not trigger a11y diagnostic; got {:?}",
        ds.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
}

// ── GA-01 — Async[T] view exhaustiveness ──────────────────────────────────

#[test]
fn other_decorators_parse_without_error() {
    // Smoke-test that @auth, @cors, @rate_limit, @webhook, @layer, @pii,
    // @embed, @offline_capable, @collaborative all tokenize and parse
    // without crashing the parser.
    let src = r#"
@auth(provider: bearer)
@cors(origins: ["*"])
@rate_limit(by: ip, per: "1m", max: 100)
fn my_endpoint() to int { return 1 }

@webhook(provider: stripe)
fn stripe_hook() to int { return 2 }

@layer(tier: modal)
fn layer_fn() to int { return 3 }
"#;
    let result = parse(lex(src));
    assert!(
        result.is_ok(),
        "parse should succeed for multi-decorator functions; errors: {:?}",
        result.err()
    );
}

// ── GA-16 — @webhook validation ───────────────────────────────────────────

#[test]
fn webhook_custom_without_secret_emits_diagnostic() {
    // @webhook(provider: custom) on an endpoint must declare a secret env-var.
    let src = r#"
@server
@webhook(provider: custom)
fn custom_hook() to int { return 1 }
"#;
    let m = parse(lex(src)).expect("parse should succeed");
    let ds = typecheck_ast_module(src, &m);
    let hit = ds
        .iter()
        .find(|d| d.code.as_deref() == Some("vox/webhook/missing-secret-var"));
    assert!(
        hit.is_some(),
        "expected vox/webhook/missing-secret-var; got {:?}",
        ds.iter().map(|d| d.code.as_deref()).collect::<Vec<_>>()
    );
}

#[test]
fn webhook_custom_with_secret_is_clean() {
    let src = r#"
@server
@webhook(provider: custom, secret: "WEBHOOK_SECRET")
fn custom_hook() to int { return 1 }
"#;
    let m = parse(lex(src)).expect("parse should succeed");
    let ds = typecheck_ast_module(src, &m);
    let hit = ds
        .iter()
        .find(|d| d.code.as_deref() == Some("vox/webhook/missing-secret-var"));
    assert!(
        hit.is_none(),
        "@webhook with explicit secret should not trigger missing-secret-var; got {:?}",
        ds.iter()
            .map(|d| (d.code.as_deref(), &d.message))
            .collect::<Vec<_>>()
    );
}

#[test]
fn webhook_replay_window_out_of_range_warns() {
    // 4 seconds is below the recommended 5..=3600 range.
    let src = r#"
@server
@webhook(provider: stripe, replay_window_secs: 4)
fn tight_hook() to int { return 1 }
"#;
    let m = parse(lex(src)).expect("parse should succeed");
    let ds = typecheck_ast_module(src, &m);
    let hit = ds
        .iter()
        .find(|d| d.code.as_deref() == Some("vox/webhook/replay-window-out-of-range"));
    assert!(
        hit.is_some(),
        "expected vox/webhook/replay-window-out-of-range for 4s window; got {:?}",
        ds.iter().map(|d| d.code.as_deref()).collect::<Vec<_>>()
    );
}

#[test]
fn webhook_stripe_default_window_is_clean() {
    let src = r#"
@server
@webhook(provider: stripe)
fn stripe_hook() to int { return 1 }
"#;
    let m = parse(lex(src)).expect("parse should succeed");
    let ds = typecheck_ast_module(src, &m);
    for code in [
        "vox/webhook/missing-secret-var",
        "vox/webhook/replay-window-out-of-range",
    ] {
        assert!(
            !ds.iter().any(|d| d.code.as_deref() == Some(code)),
            "did not expect {code}; got {:?}",
            ds.iter().map(|d| d.code.as_deref()).collect::<Vec<_>>()
        );
    }
}

// ── GA-06 — @cors / @rate_limit ───────────────────────────────────────────

#[test]
fn cors_credentials_with_wildcard_warns() {
    let src = r#"
@server
@cors(origins: ["*"], allow_credentials: true)
fn my_api() to int { return 1 }
"#;
    let m = parse(lex(src)).expect("parse should succeed");
    let ds = typecheck_ast_module(src, &m);
    let hit = ds
        .iter()
        .find(|d| d.code.as_deref() == Some("vox/cors/credentials-with-wildcard"));
    assert!(
        hit.is_some(),
        "expected vox/cors/credentials-with-wildcard; got {:?}",
        ds.iter().map(|d| d.code.as_deref()).collect::<Vec<_>>()
    );
}

#[test]
fn cors_specific_origin_with_credentials_is_clean() {
    let src = r#"
@server
@cors(origins: ["https://app.example.com"], allow_credentials: true)
fn my_api() to int { return 1 }
"#;
    let m = parse(lex(src)).expect("parse should succeed");
    let ds = typecheck_ast_module(src, &m);
    assert!(
        !ds.iter()
            .any(|d| d.code.as_deref() == Some("vox/cors/credentials-with-wildcard")),
        "explicit origin with credentials should be clean"
    );
}

// ── GA-23 — @pii unannotated net effect ───────────────────────────────────

#[test]
fn pii_without_uses_net_warns_unannotated() {
    let src = r#"
@server
@pii(class: email)
fn send_email_fn() to int { return 1 }
"#;
    let m = parse(lex(src)).expect("parse should succeed");
    let ds = typecheck_ast_module(src, &m);
    let hit = ds
        .iter()
        .find(|d| d.code.as_deref() == Some("vox/pii/unannotated-net-effect"));
    assert!(
        hit.is_some(),
        "expected vox/pii/unannotated-net-effect for PII endpoint without @uses(net); got {:?}",
        ds.iter().map(|d| d.code.as_deref()).collect::<Vec<_>>()
    );
}

#[test]
fn pii_with_uses_net_is_clean() {
    let src = r#"
@server
@pii(class: email)
@uses(net)
fn send_email_fn() to int { return 1 }
"#;
    let m = parse(lex(src)).expect("parse should succeed");
    let ds = typecheck_ast_module(src, &m);
    assert!(
        !ds.iter()
            .any(|d| d.code.as_deref() == Some("vox/pii/unannotated-net-effect")),
        "@pii + @uses(net) should not warn; got {:?}",
        ds.iter().map(|d| d.code.as_deref()).collect::<Vec<_>>()
    );
}

// ── GA-26 — @layer tier validation ────────────────────────────────────────

#[test]
fn layer_system_overlay_is_reserved() {
    let src = r#"
@server
@layer(tier: system_overlay)
fn debug_fn() to int { return 1 }
"#;
    let m = parse(lex(src)).expect("parse should succeed");
    let ds = typecheck_ast_module(src, &m);
    let hit = ds
        .iter()
        .find(|d| d.code.as_deref() == Some("vox/layer/reserved-tier"));
    assert!(
        hit.is_some(),
        "expected vox/layer/reserved-tier for system-overlay; got {:?}",
        ds.iter().map(|d| d.code.as_deref()).collect::<Vec<_>>()
    );
}

#[test]
fn layer_modal_tier_is_allowed() {
    let src = r#"
@server
@layer(tier: modal)
fn confirm_fn() to int { return 1 }
"#;
    let m = parse(lex(src)).expect("parse should succeed");
    let ds = typecheck_ast_module(src, &m);
    assert!(
        !ds.iter()
            .any(|d| d.code.as_deref() == Some("vox/layer/reserved-tier")),
        "modal tier should be allowed; got {:?}",
        ds.iter().map(|d| d.code.as_deref()).collect::<Vec<_>>()
    );
}

// ── GA-09a — Routes-as-types (typed href) ─────────────────────────────────

#[test]
fn routes_block_produces_route_ids() {
    let src = r#"
routes {
    "/" to Home
    "/users/:id" to UserProfile
}
fn dummy() to int { return 1 }
"#;
    let m = parse(lex(src)).expect("parse should succeed");
    let hir = vox_compiler::hir::lower::lower_module(&m);
    assert_eq!(
        hir.route_ids.len(),
        2,
        "expected 2 route_ids from routes block; got {:?}",
        hir.route_ids.iter().map(|r| &r.name).collect::<Vec<_>>()
    );
    let home = hir
        .route_ids
        .iter()
        .find(|r| r.name == "Home")
        .expect("Home route");
    assert_eq!(home.url_pattern, "/");
    assert!(home.params.is_empty());
    assert_eq!(home.analytics_slug, "home");

    let profile = hir
        .route_ids
        .iter()
        .find(|r| r.name == "UserProfile")
        .expect("UserProfile route");
    assert_eq!(profile.url_pattern, "/users/:id");
    assert_eq!(
        profile.params,
        vec![("id".to_string(), "string".to_string())]
    );
    assert_eq!(profile.analytics_slug, "user_profile");
}

#[test]
fn single_route_produces_one_route_id() {
    let src = r#"
routes {
    "/" to Home
}
fn dummy() to int { return 1 }
"#;
    let m = parse(lex(src)).expect("parse");
    let hir = vox_compiler::hir::lower::lower_module(&m);
    assert!(
        !hir.route_ids.is_empty(),
        "should have at least one route_id"
    );
    assert_eq!(hir.route_ids[0].analytics_slug, "home");
}

// ── GA-21 — @ai structured output validation ──────────────────────────────

#[test]
fn ai_structured_output_with_undeclared_type_warns() {
    let src = r#"
@ai(model = "gpt-4o", structured_output = TripPlan)
fn plan_trip() to int { return 1 }
"#;
    let m = parse(lex(src)).expect("parse should succeed");
    let ds = typecheck_ast_module(src, &m);
    let hit = ds
        .iter()
        .find(|d| d.code.as_deref() == Some("vox/ai/return-shape-not-codec'd"));
    assert!(
        hit.is_some(),
        "expected vox/ai/return-shape-not-codec'd for undeclared structured_output type; got {:?}",
        ds.iter().map(|d| d.code.as_deref()).collect::<Vec<_>>()
    );
}

#[test]
fn ai_without_structured_output_has_no_codec_diagnostic() {
    let src = r#"
@ai(model = "gpt-4o")
fn chat() to int { return 1 }
"#;
    let m = parse(lex(src)).expect("parse should succeed");
    let ds = typecheck_ast_module(src, &m);
    assert!(
        !ds.iter()
            .any(|d| d.code.as_deref() == Some("vox/ai/return-shape-not-codec'd")),
        "@ai without structured_output should not warn"
    );
}

#[test]
fn ai_parses_max_iterations_arg() {
    let src = r#"
@ai(model = "gpt-4o", structured_output = Plan, max_iterations = 5)
fn plan() to int { return 1 }
"#;
    let m = parse(lex(src)).expect("parse should succeed for @ai with max_iterations");
    let func = m.declarations.iter().find_map(|d| {
        if let vox_compiler::ast::decl::Decl::Function(f) = d {
            Some(f)
        } else {
            None
        }
    });
    let f = func.expect("should have parsed function");
    assert!(f.is_llm, "@ai should set is_llm");
    assert_eq!(f.ai_max_iterations, 5, "max_iterations should be 5");
    assert_eq!(f.ai_structured_output_type.as_deref(), Some("Plan"));
}

// ── GA-24 — @embed + Vector[N] dimension validation ───────────────────────

#[test]
fn embed_decorator_parses_args() {
    let src = r#"
@embed(model: "text-embedding-3-small", dimensions: 1536, source_field: "description")
fn embed_description() to int { return 1 }
"#;
    let m = parse(lex(src)).expect("parse should succeed for @embed with args");
    let func = m.declarations.iter().find_map(|d| {
        if let vox_compiler::ast::decl::Decl::Function(f) = d {
            Some(f)
        } else {
            None
        }
    });
    let f = func.expect("should have parsed function");
    let embed = f.embed.as_ref().expect("@embed should be captured");
    assert_eq!(embed.model, "text-embedding-3-small");
    assert_eq!(embed.dimensions, 1536);
    assert_eq!(embed.source_field, "description");
}

#[test]
fn embed_zero_dimensions_warns() {
    let src = r#"
@embed(model: "text-embedding-3-small", dimensions: 0, source_field: "body")
fn embed_fn() to int { return 1 }
"#;
    let m = parse(lex(src)).expect("parse should succeed");
    let ds = typecheck_ast_module(src, &m);
    let hit = ds
        .iter()
        .find(|d| d.code.as_deref() == Some("vox/embed/zero-dimensions"));
    assert!(
        hit.is_some(),
        "expected vox/embed/zero-dimensions for dimensions: 0; got {:?}",
        ds.iter().map(|d| d.code.as_deref()).collect::<Vec<_>>()
    );
}

#[test]
fn embed_nonzero_dimensions_is_clean() {
    let src = r#"
@embed(model: "text-embedding-3-small", dimensions: 1536, source_field: "body")
fn embed_fn() to int { return 1 }
"#;
    let m = parse(lex(src)).expect("parse should succeed");
    let ds = typecheck_ast_module(src, &m);
    assert!(
        !ds.iter()
            .any(|d| d.code.as_deref() == Some("vox/embed/zero-dimensions")),
        "@embed with valid dimensions should not warn"
    );
}
