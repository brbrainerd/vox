use std::path::Path;

use vox_bounded_fs::read_utf8_path_capped;

use super::docs_sync::{ref_cli_vox_ci_section, ref_cli_vox_codex_section};
use super::mcp_wiring::{check_mcp_tool_wiring, extract_mcp_handler_tools};
use super::registry::{
    extract_mcp_registry_tool_names, parse_mcp_registry_read_role_eligible, parse_mcp_registry_yaml,
};
use super::validators::{
    check_dockerfiles_cargo_locked_policy, check_install_policy_surfaces,
    check_mcp_http_read_role_governance, check_operator_docs_no_legacy_vox_install_pm_nudge,
    check_packaging_pm_docs_no_resurrected_uv_copies, check_project_pm_commands_no_toolchain_lane,
    check_upgrade_toolchain_only, kebab_to_pascal,
};

#[test]
fn upgrade_rs_stays_toolchain_only() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repo root above crates/vox-cli");
    check_upgrade_toolchain_only(root).expect("upgrade.rs PM isolation");
}

#[test]
fn project_pm_files_avoid_toolchain_lane() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repo root above crates/vox-cli");
    check_project_pm_commands_no_toolchain_lane(root).expect("WP5 add/remove/update/lock/sync");
}

#[test]
fn operator_docs_avoid_legacy_vox_install_pm_nudge() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repo root above crates/vox-cli");
    check_operator_docs_no_legacy_vox_install_pm_nudge(root).expect("WP4 doc guard");
}

#[test]
fn install_policy_surfaces_align_with_docs() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repo root above crates/vox-cli");
    check_install_policy_surfaces(root).expect("vox-install-policy parity");
}

#[test]
fn dockerfiles_use_locked_cargo_when_lockfile_copied() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repo root above crates/vox-cli");
    check_dockerfiles_cargo_locked_policy(root).expect("Dockerfile --locked policy");
}

#[test]
fn packaging_pm_docs_uv_fragments_guard() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repo root above crates/vox-cli");
    check_packaging_pm_docs_no_resurrected_uv_copies(root).expect("WP6 doc fragments");
}

#[test]
fn kebab_pascal() {
    assert_eq!(kebab_to_pascal("stub-check"), "StubCheck");
    assert_eq!(kebab_to_pascal("fmt.check"), "FmtCheck");
}

#[test]
fn ref_cli_vox_ci_finds_manifest_backtick() {
    let md = "\n### `vox ci …`\n\n| `manifest` | x |\n\n### `vox dev`\n";
    let sec = ref_cli_vox_ci_section(md).expect("section");
    assert!(sec.contains("`manifest"));
}

#[test]
fn mcp_registry_yaml_tolerates_bracket_in_description() {
    let yaml = r#"
version: 1
tools:
  - name: "vox_bracket_test"
    description: "Description with ] bracket inside string"
    product_lane: data
"#;
    let tools = parse_mcp_registry_yaml(yaml).expect("parse");
    assert_eq!(tools, vec!["vox_bracket_test".to_string()]);
}

#[test]
fn mcp_registry_read_role_eligible_parser_filters_true_flags() {
    let yaml = r#"
version: 1
tools:
  - name: "vox_read_ok"
    description: "safe read tool"
    product_lane: ai
    http_read_role_eligible: true
  - name: "vox_write_only"
    description: "write tool"
    product_lane: ai
"#;
    let tools = parse_mcp_registry_read_role_eligible(yaml).expect("parse read-role");
    assert_eq!(tools, vec!["vox_read_ok".to_string()]);
}

#[test]
fn mcp_handler_extract_includes_alternation_arms() {
    let src = r#"
pub async fn handle_tool_call() {
    match name {
        "vox_config_get" | "vox_get_config" => { ok }
        "vox_other" => { ok }
        _ => { err }
    }
}
"#;
    let h = extract_mcp_handler_tools(src).expect("parse");
    assert!(h.contains("vox_config_get"));
    assert!(h.contains("vox_get_config"));
    assert!(h.contains("vox_other"));
}

#[test]
fn mcp_handler_default_arm_tolerates_indent() {
    let src = r#"pub async fn handle_tool_call() {
    match name {
			"vox_indented_only" => { ok }
			_ => { err }
    }
}"#;
    let h = extract_mcp_handler_tools(src).expect("parse");
    assert!(h.contains("vox_indented_only"));
}

#[test]
fn ref_cli_vox_codex_section_excludes_other_headings() {
    let md = "\n### `vox codex`\n\n| `import` | x |\n\n### `vox dev`\nverify unrelated\n";
    let sec = ref_cli_vox_codex_section(md).expect("codex section");
    assert!(sec.contains("`import"));
    assert!(!sec.contains("verify"));
}

#[test]
fn ref_cli_vox_ci_section_until_eof_when_last_heading() {
    let md = "### `vox ci …`\n\n| `manifest` | x |\n";
    let sec = ref_cli_vox_ci_section(md).expect("ci section");
    assert!(sec.contains("`manifest"));
}

/// Guard against drift in `vox-orchestrator-mcp` layout: full wiring must stay parseable by the compliance gate.
#[test]
fn mcp_extract_matches_workspace_vox_orchestrator_mcp_tools_mod_rs() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("vox-cli lives at crates/vox-cli");
    let base = repo_root.join("crates/vox-orchestrator-mcp/src");
    let mcp_mod =
        read_utf8_path_capped(&base.join("lib.rs")).expect("read vox-orchestrator-mcp lib.rs");
    let dispatch = read_utf8_path_capped(&base.join("dispatch.rs"))
        .expect("read vox-orchestrator-mcp dispatch.rs");
    let aliases = read_utf8_path_capped(&repo_root.join("contracts/mcp/tool-wire-aliases.v1.yaml"))
        .expect("read tool-wire-aliases.v1.yaml");
    let reg = extract_mcp_registry_tool_names(repo_root).expect("registry tools");
    let han = extract_mcp_handler_tools(&dispatch).expect("handler tools");
    let missing: Vec<&String> = reg.iter().filter(|t| !han.contains(*t)).collect();
    assert!(
        missing.is_empty(),
        "registry tools missing from handle_tool_call parse: {:?}",
        missing
    );
    check_mcp_tool_wiring(repo_root, &mcp_mod, &dispatch, &aliases).expect("mcp wiring + aliases");
}

#[test]
fn mcp_read_role_governance_profile_matches_registry() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("vox-cli lives at crates/vox-cli");
    check_mcp_http_read_role_governance(repo_root).expect("read-role governance profile");
}

/// T068/T069: `visible_alias` values in lib.rs must be registered in the operations catalog.
#[test]
fn latin_alias_parity_with_catalog() {
    use super::validators::check_latin_alias_parity_with_catalog;
    use vox_bounded_fs::read_utf8_path_capped;

    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repo root above crates/vox-cli");
    let lib_rs =
        read_utf8_path_capped(&repo_root.join("crates/vox-cli/src/lib.rs")).expect("read lib.rs");
    check_latin_alias_parity_with_catalog(repo_root, &lib_rs)
        .expect("T068/T069: visible_alias ↔ catalog latin_aliases parity");
}

/// T075-T082: Verify Latin command names are declared as visible_alias in lib.rs.
/// This ensures clap correctly routes `vox <latin>` to the same handler as `vox <english>`.
#[test]
fn latin_english_alias_declared_in_lib() {
    use vox_bounded_fs::read_utf8_path_capped;

    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repo root above crates/vox-cli");
    let lib_rs =
        read_utf8_path_capped(&repo_root.join("crates/vox-cli/src/lib.rs")).expect("read lib.rs");

    // T075: vox dei → orchestrator alias
    assert!(
        lib_rs.contains("visible_alias = \"orchestrator\""),
        "T075: `vox dei` must declare `visible_alias = \"orchestrator\"` in lib.rs"
    );
    // T077: vox secrets → clavis alias (deprecated backward-compat alias)
    assert!(
        lib_rs.contains("visible_alias = \"clavis\""),
        "T077: `vox secrets` must declare `visible_alias = \"clavis\"` in lib.rs"
    );
    // T078: vox oratio → speech alias
    assert!(
        lib_rs.contains("visible_alias = \"speech\""),
        "T078: `vox oratio` must declare `visible_alias = \"speech\"` in lib.rs"
    );
}

/// T084: Help output equivalence - verify that `dei` and `orchestrator` are both reachable
/// from the clap tree (via visible_alias the command appears in `--help` output).
#[test]
fn latin_aliases_appear_in_help_text() {
    use crate::VoxCliRoot;
    use clap::CommandFactory;

    let help = VoxCliRoot::command().render_long_help().to_string();

    // The Latin names and their aliases should both appear in help
    assert!(
        help.contains("fabrica") || help.contains("fab"),
        "T084: `fabrica` or its alias should appear in vox --help"
    );
    assert!(
        help.contains("secrets"),
        "T084: `secrets` should appear in vox --help"
    );
    // `clavis` is a visible_alias so it appears alongside secrets
    assert!(
        help.contains("clavis"),
        "T084: `clavis` (alias of secrets) should appear in vox --help"
    );
}
