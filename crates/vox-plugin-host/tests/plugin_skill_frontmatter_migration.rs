//! Characterization tests for crates/vox-plugin-skill-*/*.skill.md frontmatter.
//! These assert the parsed SkillManifest is unchanged across the TOML->YAML
//! frontmatter migration (docs/superpowers/plans/2026-08-01-skill-ecosystem-phase1-consolidation.md).

use vox_plugin_host::skill_parser::parse_skill_md;
use vox_plugin_types::skill_manifest::{SkillCategory, SkillPermission};

#[test]
fn compiler_skill_manifest() {
    let content = include_str!("../../vox-plugin-skill-compiler/compiler.skill.md");
    let bundle = parse_skill_md(content).expect("parse compiler.skill.md");
    assert_eq!(bundle.manifest.name, "skill-compiler");
    assert_eq!(
        bundle.manifest.description,
        "Compiles Vox source files and runs cargo check/build for the workspace."
    );
    assert_eq!(bundle.manifest.id, "vox.compiler");
    assert_eq!(bundle.manifest.version, "0.1.0");
    assert_eq!(bundle.manifest.author, "vox-team");
    assert_eq!(bundle.manifest.category, SkillCategory::Compiler);
    assert_eq!(
        bundle.manifest.tools,
        vec!["vox_validate_file", "vox_run_tests", "vox_check_workspace"]
    );
    assert_eq!(
        bundle.manifest.tags,
        vec!["compile", "build", "cargo", "check"]
    );
    assert_eq!(
        bundle.manifest.permissions,
        vec![SkillPermission::ReadFiles, SkillPermission::ShellExec]
    );
}

#[test]
fn git_skill_manifest() {
    let content = include_str!("../../vox-plugin-skill-git/git.skill.md");
    let bundle = parse_skill_md(content).expect("parse git.skill.md");
    assert_eq!(bundle.manifest.name, "skill-git");
    assert_eq!(
        bundle.manifest.description,
        "Git workflow assistance: status, diff, commit messaging, branch management, and file ownership."
    );
    assert_eq!(bundle.manifest.id, "vox.git");
    assert_eq!(bundle.manifest.version, "0.1.0");
    assert_eq!(bundle.manifest.author, "vox-team");
    assert_eq!(bundle.manifest.category, SkillCategory::Git);
    assert_eq!(
        bundle.manifest.tools,
        vec![
            "vox_my_files",
            "vox_claim_file",
            "vox_transfer_file",
            "vox_check_file_owner"
        ]
    );
    assert_eq!(
        bundle.manifest.tags,
        vec!["git", "version-control", "branch", "diff", "commit"]
    );
    assert_eq!(
        bundle.manifest.permissions,
        vec![
            SkillPermission::ReadFiles,
            SkillPermission::WriteFiles,
            SkillPermission::ShellExec
        ]
    );
}

#[test]
fn memory_skill_manifest() {
    let content = include_str!("../../vox-plugin-skill-memory/memory.skill.md");
    let bundle = parse_skill_md(content).expect("parse memory.skill.md");
    assert_eq!(bundle.manifest.name, "skill-memory");
    assert_eq!(
        bundle.manifest.description,
        "Persistent agent memory — store and recall facts, search logs, manage sessions."
    );
    assert_eq!(bundle.manifest.id, "vox.memory");
    assert_eq!(bundle.manifest.version, "0.1.0");
    assert_eq!(bundle.manifest.author, "vox-team");
    assert_eq!(bundle.manifest.category, SkillCategory::Database);
    assert_eq!(
        bundle.manifest.tools,
        vec![
            "vox_memory_store",
            "vox_memory_recall",
            "vox_memory_search",
            "vox_memory_log",
            "vox_memory_list_keys",
            "vox_knowledge_query",
            "vox_session_create",
            "vox_session_list",
            "vox_session_info",
            "vox_session_compact",
            "vox_session_cleanup"
        ]
    );
    assert_eq!(
        bundle.manifest.tags,
        vec!["memory", "recall", "session", "knowledge", "facts"]
    );
    assert_eq!(
        bundle.manifest.permissions,
        vec![SkillPermission::DbRead, SkillPermission::DbWrite]
    );
}

#[test]
fn orchestrator_skill_manifest() {
    let content = include_str!("../../vox-plugin-skill-orchestrator/orchestrator.skill.md");
    let bundle = parse_skill_md(content).expect("parse orchestrator.skill.md");
    assert_eq!(bundle.manifest.name, "skill-orchestrator");
    assert_eq!(
        bundle.manifest.description,
        "Multi-agent orchestration: submit tasks, check status, rebalance, monitor budgets and queues."
    );
    assert_eq!(bundle.manifest.id, "vox.orchestrator");
    assert_eq!(bundle.manifest.version, "0.1.0");
    assert_eq!(bundle.manifest.author, "vox-team");
    assert_eq!(bundle.manifest.category, SkillCategory::Monitoring);
    assert_eq!(
        bundle.manifest.tools,
        vec![
            "vox_submit_task",
            "vox_task_status",
            "vox_orchestrator_status",
            "vox_complete_task",
            "vox_fail_task",
            "vox_cancel_task",
            "vox_rebalance",
            "vox_queue_status",
            "vox_lock_status",
            "vox_budget_status",
            "vox_agent_events"
        ]
    );
    assert_eq!(
        bundle.manifest.tags,
        vec!["orchestrator", "tasks", "agents", "budget", "queue"]
    );
    assert_eq!(
        bundle.manifest.permissions,
        vec![SkillPermission::DbRead, SkillPermission::DbWrite]
    );
}

#[test]
fn rag_skill_manifest() {
    let content = include_str!("../../vox-plugin-skill-rag/rag.skill.md");
    let bundle = parse_skill_md(content).expect("parse rag.skill.md");
    assert_eq!(bundle.manifest.name, "skill-rag");
    assert_eq!(
        bundle.manifest.description,
        "Multi-modal Visual Retrieval-Augmented Generation RAG handler orchestrating queries to connected intelligent backends."
    );
    assert_eq!(bundle.manifest.id, "vox.rag");
    assert_eq!(bundle.manifest.version, "0.1.0");
    assert_eq!(bundle.manifest.author, "vox-team");
    assert_eq!(
        bundle.manifest.category,
        SkillCategory::Custom("research".to_string())
    );
    assert_eq!(bundle.manifest.tools, vec!["vox_visual_rag_query"]);
    assert_eq!(
        bundle.manifest.tags,
        vec!["rag", "visual", "vision", "image", "multimodal", "search"]
    );
    assert_eq!(bundle.manifest.permissions, Vec::<SkillPermission>::new());
}

#[test]
fn testing_skill_manifest() {
    let content = include_str!("../../vox-plugin-skill-testing/testing.skill.md");
    let bundle = parse_skill_md(content).expect("parse testing.skill.md");
    assert_eq!(bundle.manifest.name, "skill-testing");
    assert_eq!(
        bundle.manifest.description,
        "Runs tests, displays coverage summaries, and validates test output for Vox crates."
    );
    assert_eq!(bundle.manifest.id, "vox.testing");
    assert_eq!(bundle.manifest.version, "0.1.0");
    assert_eq!(bundle.manifest.author, "vox-team");
    assert_eq!(bundle.manifest.category, SkillCategory::Testing);
    assert_eq!(bundle.manifest.tools, vec!["vox_run_tests", "vox_test_all"]);
    assert_eq!(
        bundle.manifest.tags,
        vec!["test", "coverage", "ci", "validation"]
    );
    assert_eq!(
        bundle.manifest.permissions,
        vec![SkillPermission::ReadFiles, SkillPermission::ShellExec]
    );
}

#[test]
fn testing_validate_skill_manifest() {
    let content = include_str!("../../vox-plugin-skill-testing-validate/testing.validate.skill.md");
    let bundle = parse_skill_md(content).expect("parse testing.validate.skill.md");
    assert_eq!(bundle.manifest.name, "skill-testing-validate");
    assert_eq!(
        bundle.manifest.description,
        "Executes the 5-stage delivery gate pipeline to autonomously validate and heal Vox code."
    );
    assert_eq!(bundle.manifest.id, "vox.testing.validate");
    assert_eq!(bundle.manifest.version, "0.1.0");
    assert_eq!(bundle.manifest.author, "vox-team");
    assert_eq!(bundle.manifest.category, SkillCategory::Testing);
    assert_eq!(
        bundle.manifest.tools,
        vec!["vox_validate_file", "vox_validate_source"]
    );
    assert_eq!(
        bundle.manifest.tags,
        vec!["test", "validation", "self-healing", "ars"]
    );
    // Note: "ai_invoke" is not a recognized SkillPermission variant (parse_permission
    // has no match arm for it) — it was silently dropped before this migration too.
    // Migration preserves the declared value in the frontmatter; it does not fix
    // this pre-existing parser gap, which is out of scope for Phase 1.
    assert_eq!(
        bundle.manifest.permissions,
        vec![
            SkillPermission::ReadFiles,
            SkillPermission::WriteFiles,
            SkillPermission::ShellExec
        ]
    );
}

#[test]
fn v0_skill_manifest() {
    let content = include_str!("../../vox-plugin-skill-v0/v0.skill.md");
    let bundle = parse_skill_md(content).expect("parse v0.skill.md");
    assert_eq!(bundle.manifest.name, "skill-v0");
    assert_eq!(
        bundle.manifest.description,
        "Generate React/Vox UI components from text prompts using the v0.dev API."
    );
    assert_eq!(bundle.manifest.id, "vox.v0");
    assert_eq!(bundle.manifest.version, "0.1.0");
    // No vox-author key in the source frontmatter; parser defaults to "unknown".
    assert_eq!(bundle.manifest.author, "unknown");
    assert_eq!(
        bundle.manifest.category,
        SkillCategory::Custom("UI".to_string())
    );
    assert_eq!(
        bundle.manifest.tools,
        vec!["vox_generate_code", "vox_write_file"]
    );
    assert_eq!(bundle.manifest.permissions, vec![SkillPermission::Network]);
}

#[test]
fn populi_mesh_skill_manifest() {
    let content = include_str!("../../vox-plugin-populi-mesh/populi.skill.md");
    let bundle = parse_skill_md(content).expect("parse populi-mesh populi.skill.md");
    assert_eq!(bundle.manifest.name, "populi-mesh");
    assert_eq!(
        bundle.manifest.description,
        "Align mens node labels with orchestrator task hints and inspect local/remote registry visibility."
    );
    assert_eq!(bundle.manifest.id, "vox.populi");
    assert_eq!(bundle.manifest.version, "0.1.0");
    assert_eq!(bundle.manifest.author, "vox-team");
    assert_eq!(
        bundle.manifest.category,
        SkillCategory::Custom("infrastructure".to_string())
    );
    assert_eq!(
        bundle.manifest.tools,
        vec![
            "vox_populi_local_status",
            "vox_orchestrator_status",
            "vox_submit_task"
        ]
    );
    assert_eq!(
        bundle.manifest.tags,
        vec!["mens", "labels", "gpu", "federation", "workers"]
    );
    assert_eq!(bundle.manifest.permissions, vec![SkillPermission::DbRead]);
}
