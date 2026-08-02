# Skill Ecosystem Phase 1: Content Audit & Consolidation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every skill file in the repo is canonical-format (per AGENTS.md's YAML+`metadata.vox-*` rule), has no dead cross-references, has no duplicate SSOT, and the superpowers skill set is portable to Vox Axis/MENS. This phase removes debt found by the audit; it adds no new capability.

**Architecture:** Nine `crates/vox-plugin-skill-*/*.skill.md` (+ `vox-plugin-populi-mesh`) files migrate from `[metadata]`-nested TOML frontmatter to spec-compliant YAML with a `metadata` map of `vox-*` keys — a format the parser (`vox-plugin-host::skill_parser`) already accepts (it tries TOML, falls back to YAML) and a format `vox ci agentskills-compliance` already validates for these exact files. Each migration is guarded by a small characterization test asserting the parsed `SkillManifest` fields are unchanged before and after. One duplicate skill file, four dead/stale files, one missing portability doc, and one confirmed tool-name bug round out the phase.

**Tech Stack:** Rust (`vox-plugin-host` crate, `cargo test`), YAML/TOML frontmatter, `vox ci` CLI gates.

---

## Spec coverage

This plan implements Phase 1, items 1–6 of `docs/superpowers/specs/2026-08-01-skill-ecosystem-improvements-design.md` §2. Two items from that spec are refined here with facts gathered during planning (both docs already corrected to match): the populi dedupe keeps the plugin-crate copy (not the `vox-skills` copy — it's the one `Plugin.toml`/`catalog.toml` actually load), and the graphify tool-name issue is a confirmed bug (not a "verify" step) fixed by rewriting the skill's documented tool/CLI names to match what's registered today.

## File Structure

**Delete:**
- `crates/vox-skills/skills/populi.skill.md` (stray duplicate, unreferenced by any sync script/test/catalog entry)
- `assets/skills/brainstorming/spec-document-reviewer-prompt.md` (orphaned)
- `assets/skills/writing-plans/plan-document-reviewer-prompt.md` (orphaned)
- `assets/skills/systematic-debugging/CREATION-LOG.md` (authoring log, not a runtime reference)

**Modify (frontmatter migration, TOML → YAML):**
- `crates/vox-plugin-skill-compiler/compiler.skill.md`
- `crates/vox-plugin-skill-git/git.skill.md`
- `crates/vox-plugin-skill-memory/memory.skill.md`
- `crates/vox-plugin-skill-orchestrator/orchestrator.skill.md`
- `crates/vox-plugin-skill-rag/rag.skill.md`
- `crates/vox-plugin-skill-testing/testing.skill.md`
- `crates/vox-plugin-skill-testing-validate/testing.validate.skill.md`
- `crates/vox-plugin-skill-v0/v0.skill.md`
- `crates/vox-plugin-populi-mesh/populi.skill.md`

**Modify (content fix):**
- `assets/skills/vox-graph/SKILL.md` (correct tool/CLI names)
- `assets/skills/SOURCES.toml` (bump anthropics/skills pin)
- `NOTICE` (repo root — keep pinned-commit line in sync with `SOURCES.toml`)
- `crates/vox-cli-ci/src/plugin_skill_parity.rs` (conditional — only if Task 12 Step 0 finds it's still TOML-only after the YAML migration)
- `crates/vox-skills/skills/superpowers/*.skill.md` (conditional — only if Task 12b finds a file that fails to parse or genuinely needs a `metadata` block)

**Create:**
- `crates/vox-plugin-host/tests/plugin_skill_frontmatter_migration.rs` (characterization tests, one `#[test]` per migrated file)
- `assets/skills/using-superpowers/references/vox-axis-tools.md`

---

### Task 1: Delete the stray `populi.skill.md` duplicate

**Files:**
- Delete: `crates/vox-skills/skills/populi.skill.md`

- [ ] **Step 1: Confirm nothing references the file by path**

Run: `git grep -n "vox-skills/skills/populi" -- ':!docs/src/architecture/skill-ecosystem-audit-2026-08-01.md' ':!docs/superpowers/specs/2026-08-01-skill-ecosystem-improvements-design.md' ':!docs/superpowers/plans/2026-08-01-skill-ecosystem-phase1-consolidation.md'`
Expected: one hit, `docs/agents/doc-inventory.json`, listing `crates/vox-skills/skills/populi.skill.md` as a `path` entry. This file is machine-generated (`crates/vox-doc-inventory/src/bin/doc-inventory-generate.rs`) and regenerates on its own schedule, so this hit does not block the deletion — it is not a real reference. Any *other* hit (source code, another skill, a catalog entry) means the deletion is unsafe and must be investigated before Step 2.

- [ ] **Step 2: Delete the file**

Run: `git rm crates/vox-skills/skills/populi.skill.md`

- [ ] **Step 3: Commit**

```bash
git commit -m "chore(skills): remove stray duplicate populi.skill.md

The load-bearing copy is crates/vox-plugin-populi-mesh/populi.skill.md
(referenced by Plugin.toml and catalog.toml); this copy was an
unreferenced stray."
```

---

### Task 2: Add characterization tests for the 9 plugin-skill manifests

**Files:**
- Create: `crates/vox-plugin-host/tests/plugin_skill_frontmatter_migration.rs`
- Test: same file (this task IS the test file)

- [ ] **Step 1: Write the characterization tests**

```rust
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
    assert_eq!(bundle.manifest.tags, vec!["compile", "build", "cargo", "check"]);
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
    assert_eq!(bundle.manifest.category, SkillCategory::Custom("research".to_string()));
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
    assert_eq!(bundle.manifest.tags, vec!["test", "coverage", "ci", "validation"]);
    assert_eq!(
        bundle.manifest.permissions,
        vec![SkillPermission::ReadFiles, SkillPermission::ShellExec]
    );
}

#[test]
fn testing_validate_skill_manifest() {
    let content =
        include_str!("../../vox-plugin-skill-testing-validate/testing.validate.skill.md");
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
    assert_eq!(bundle.manifest.category, SkillCategory::Custom("UI".to_string()));
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
    assert_eq!(bundle.manifest.category, SkillCategory::Custom("infrastructure".to_string()));
    assert_eq!(
        bundle.manifest.tools,
        vec!["vox_populi_local_status", "vox_orchestrator_status", "vox_submit_task"]
    );
    assert_eq!(
        bundle.manifest.tags,
        vec!["mens", "labels", "gpu", "federation", "workers"]
    );
    assert_eq!(bundle.manifest.permissions, vec![SkillPermission::DbRead]);
}
```

- [ ] **Step 2: Run the tests to confirm they pass against the current TOML frontmatter (baseline)**

Run: `cargo test -p vox-plugin-host --test plugin_skill_frontmatter_migration`
Expected: all 9 tests PASS. This is the safety net — the parser already accepts TOML, so these are green *before* migration. Each test must stay green *after* its file's migration in the tasks below.

- [ ] **Step 3: Format the new test file**

Run: `cargo fmt -p vox-plugin-host` (never `cargo fmt --all` — see AGENTS.md's fmt policy). This is a new Rust file and must satisfy the same per-crate formatting gate (`vox ci pre-push` → `check_fmt`) as every other Rust file in the workspace before it's committed.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-plugin-host/tests/plugin_skill_frontmatter_migration.rs
git commit -m "test(skills): add characterization tests for plugin-skill manifests

Baseline for the TOML->YAML frontmatter migration in the next 9 tasks
- asserts parsed SkillManifest fields before any format change."
```

---

### Task 3: Migrate `compiler.skill.md` to canonical YAML frontmatter

**Files:**
- Modify: `crates/vox-plugin-skill-compiler/compiler.skill.md`

- [ ] **Step 1: Rewrite the frontmatter block**

Replace lines 1–13 of `crates/vox-plugin-skill-compiler/compiler.skill.md` (everything from the opening `---` through the closing `---`) with:

```yaml
---
name: skill-compiler
description: "Compiles Vox source files and runs cargo check/build for the workspace."
metadata:
  vox-id: vox.compiler
  vox-version: "0.1.0"
  vox-author: vox-team
  vox-category: compiler
  vox-tools:
    - vox_validate_file
    - vox_run_tests
    - vox_check_workspace
  vox-tags:
    - compile
    - build
    - cargo
    - check
  vox-permissions:
    - read_files
    - shell_exec
---
```

Leave the Markdown body (everything from `# Vox Compiler Skill` onward) unchanged.

- [ ] **Step 2: Run the characterization test for this file**

Run: `cargo test -p vox-plugin-host --test plugin_skill_frontmatter_migration compiler_skill_manifest`
Expected: PASS (same assertions as before the migration — parsed fields are unchanged).

- [ ] **Step 3: Run the compliance gate**

Run: `vox ci agentskills-compliance`
Expected: `✓ agentskills-compliance ok (N skill files checked)` — no errors for `compiler.skill.md`.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-plugin-skill-compiler/compiler.skill.md
git commit -m "chore(skills): migrate compiler.skill.md to canonical YAML frontmatter

Per AGENTS.md, TOML frontmatter (including [metadata]-nested TOML) is
the legacy fallback, not a co-equal format. Parsed manifest fields are
unchanged (see plugin_skill_frontmatter_migration.rs)."
```

---

### Task 4: Migrate `git.skill.md` to canonical YAML frontmatter

**Files:**
- Modify: `crates/vox-plugin-skill-git/git.skill.md`

- [ ] **Step 1: Rewrite the frontmatter block**

Replace lines 1–13 with:

```yaml
---
name: skill-git
description: "Git workflow assistance: status, diff, commit messaging, branch management, and file ownership."
metadata:
  vox-id: vox.git
  vox-version: "0.1.0"
  vox-author: vox-team
  vox-category: git
  vox-tools:
    - vox_my_files
    - vox_claim_file
    - vox_transfer_file
    - vox_check_file_owner
  vox-tags:
    - git
    - version-control
    - branch
    - diff
    - commit
  vox-permissions:
    - read_files
    - write_files
    - shell_exec
---
```

- [ ] **Step 2: Run the characterization test**

Run: `cargo test -p vox-plugin-host --test plugin_skill_frontmatter_migration git_skill_manifest`
Expected: PASS.

- [ ] **Step 3: Run the compliance gate**

Run: `vox ci agentskills-compliance`
Expected: no errors for `git.skill.md`.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-plugin-skill-git/git.skill.md
git commit -m "chore(skills): migrate git.skill.md to canonical YAML frontmatter"
```

---

### Task 5: Migrate `memory.skill.md` to canonical YAML frontmatter

**Files:**
- Modify: `crates/vox-plugin-skill-memory/memory.skill.md`

- [ ] **Step 1: Rewrite the frontmatter block**

Replace lines 1–13 with:

```yaml
---
name: skill-memory
description: "Persistent agent memory — store and recall facts, search logs, manage sessions."
metadata:
  vox-id: vox.memory
  vox-version: "0.1.0"
  vox-author: vox-team
  vox-category: database
  vox-tools:
    - vox_memory_store
    - vox_memory_recall
    - vox_memory_search
    - vox_memory_log
    - vox_memory_list_keys
    - vox_knowledge_query
    - vox_session_create
    - vox_session_list
    - vox_session_info
    - vox_session_compact
    - vox_session_cleanup
  vox-tags:
    - memory
    - recall
    - session
    - knowledge
    - facts
  vox-permissions:
    - db_read
    - db_write
---
```

- [ ] **Step 2: Run the characterization test**

Run: `cargo test -p vox-plugin-host --test plugin_skill_frontmatter_migration memory_skill_manifest`
Expected: PASS.

- [ ] **Step 3: Run the compliance gate**

Run: `vox ci agentskills-compliance`
Expected: no errors for `memory.skill.md`.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-plugin-skill-memory/memory.skill.md
git commit -m "chore(skills): migrate memory.skill.md to canonical YAML frontmatter"
```

---

### Task 6: Migrate `orchestrator.skill.md` to canonical YAML frontmatter

**Files:**
- Modify: `crates/vox-plugin-skill-orchestrator/orchestrator.skill.md`

- [ ] **Step 1: Rewrite the frontmatter block**

Replace lines 1–13 with:

```yaml
---
name: skill-orchestrator
description: "Multi-agent orchestration: submit tasks, check status, rebalance, monitor budgets and queues."
metadata:
  vox-id: vox.orchestrator
  vox-version: "0.1.0"
  vox-author: vox-team
  vox-category: monitoring
  vox-tools:
    - vox_submit_task
    - vox_task_status
    - vox_orchestrator_status
    - vox_complete_task
    - vox_fail_task
    - vox_cancel_task
    - vox_rebalance
    - vox_queue_status
    - vox_lock_status
    - vox_budget_status
    - vox_agent_events
  vox-tags:
    - orchestrator
    - tasks
    - agents
    - budget
    - queue
  vox-permissions:
    - db_read
    - db_write
---
```

- [ ] **Step 2: Run the characterization test**

Run: `cargo test -p vox-plugin-host --test plugin_skill_frontmatter_migration orchestrator_skill_manifest`
Expected: PASS.

- [ ] **Step 3: Run the compliance gate**

Run: `vox ci agentskills-compliance`
Expected: no errors for `orchestrator.skill.md`.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-plugin-skill-orchestrator/orchestrator.skill.md
git commit -m "chore(skills): migrate orchestrator.skill.md to canonical YAML frontmatter"
```

---

### Task 7: Migrate `rag.skill.md` to canonical YAML frontmatter

**Files:**
- Modify: `crates/vox-plugin-skill-rag/rag.skill.md`

- [ ] **Step 1: Rewrite the frontmatter block**

Replace lines 1–13 with:

```yaml
---
name: skill-rag
description: "Multi-modal Visual Retrieval-Augmented Generation RAG handler orchestrating queries to connected intelligent backends."
metadata:
  vox-id: vox.rag
  vox-version: "0.1.0"
  vox-author: vox-team
  vox-category: research
  vox-tools:
    - vox_visual_rag_query
  vox-tags:
    - rag
    - visual
    - vision
    - image
    - multimodal
    - search
---
```

`vox-permissions` is omitted rather than an empty list — the original TOML had `"vox-permissions" = []`, and the standalone permissions-parsing logic in `skill_parser.rs` (not the `vox_arr` closure, which only backs `tools`/`tags`/`dependencies`) applies its own `.filter(|l| !l.is_empty())`, treating an empty declared list the same as an absent one (falls through to the empty-default path), so this is behavior-preserving.

- [ ] **Step 2: Run the characterization test**

Run: `cargo test -p vox-plugin-host --test plugin_skill_frontmatter_migration rag_skill_manifest`
Expected: PASS.

- [ ] **Step 3: Run the compliance gate**

Run: `vox ci agentskills-compliance`
Expected: no errors for `rag.skill.md`.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-plugin-skill-rag/rag.skill.md
git commit -m "chore(skills): migrate rag.skill.md to canonical YAML frontmatter"
```

---

### Task 8: Migrate `testing.skill.md` to canonical YAML frontmatter

**Files:**
- Modify: `crates/vox-plugin-skill-testing/testing.skill.md`

- [ ] **Step 1: Rewrite the frontmatter block**

Replace lines 1–13 with:

```yaml
---
name: skill-testing
description: "Runs tests, displays coverage summaries, and validates test output for Vox crates."
metadata:
  vox-id: vox.testing
  vox-version: "0.1.0"
  vox-author: vox-team
  vox-category: testing
  vox-tools:
    - vox_run_tests
    - vox_test_all
  vox-tags:
    - test
    - coverage
    - ci
    - validation
  vox-permissions:
    - read_files
    - shell_exec
---
```

- [ ] **Step 2: Run the characterization test**

Run: `cargo test -p vox-plugin-host --test plugin_skill_frontmatter_migration testing_skill_manifest`
Expected: PASS.

- [ ] **Step 3: Run the compliance gate**

Run: `vox ci agentskills-compliance`
Expected: no errors for `testing.skill.md`.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-plugin-skill-testing/testing.skill.md
git commit -m "chore(skills): migrate testing.skill.md to canonical YAML frontmatter"
```

---

### Task 9: Migrate `testing.validate.skill.md` to canonical YAML frontmatter

**Files:**
- Modify: `crates/vox-plugin-skill-testing-validate/testing.validate.skill.md`

- [ ] **Step 1: Rewrite the frontmatter block**

Replace lines 1–13 with:

```yaml
---
name: skill-testing-validate
description: "Executes the 5-stage delivery gate pipeline to autonomously validate and heal Vox code."
metadata:
  vox-id: vox.testing.validate
  vox-version: "0.1.0"
  vox-author: vox-team
  vox-category: testing
  vox-tools:
    - vox_validate_file
    - vox_validate_source
  vox-tags:
    - test
    - validation
    - self-healing
    - ars
  vox-permissions:
    - read_files
    - write_files
    - shell_exec
    - ai_invoke
---
```

- [ ] **Step 2: Run the characterization test**

Run: `cargo test -p vox-plugin-host --test plugin_skill_frontmatter_migration testing_validate_skill_manifest`
Expected: PASS.

- [ ] **Step 3: Run the compliance gate**

Run: `vox ci agentskills-compliance`
Expected: no errors for `testing.validate.skill.md`.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-plugin-skill-testing-validate/testing.validate.skill.md
git commit -m "chore(skills): migrate testing.validate.skill.md to canonical YAML frontmatter"
```

---

### Task 10: Migrate `v0.skill.md` to canonical YAML frontmatter

**Files:**
- Modify: `crates/vox-plugin-skill-v0/v0.skill.md`

- [ ] **Step 1: Rewrite the frontmatter block**

Replace lines 1–11 with:

```yaml
---
name: skill-v0
description: "Generate React/Vox UI components from text prompts using the v0.dev API."
metadata:
  vox-id: vox.v0
  vox-version: "0.1.0"
  vox-category: UI
  vox-tools:
    - vox_generate_code
    - vox_write_file
  vox-permissions:
    - network
---
```

No `vox-author` key — the original TOML didn't declare one either (parser defaults to `"unknown"`); this migration preserves that, not invents an author.

- [ ] **Step 2: Run the characterization test**

Run: `cargo test -p vox-plugin-host --test plugin_skill_frontmatter_migration v0_skill_manifest`
Expected: PASS.

- [ ] **Step 3: Run the compliance gate**

Run: `vox ci agentskills-compliance`
Expected: no errors for `v0.skill.md`.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-plugin-skill-v0/v0.skill.md
git commit -m "chore(skills): migrate v0.skill.md to canonical YAML frontmatter"
```

---

### Task 11: Migrate `populi-mesh/populi.skill.md` to canonical YAML frontmatter

**Files:**
- Modify: `crates/vox-plugin-populi-mesh/populi.skill.md`

- [ ] **Step 1: Rewrite the frontmatter block**

Replace lines 1–13 with:

```yaml
---
name: populi-mesh
description: "Align mens node labels with orchestrator task hints and inspect local/remote registry visibility."
metadata:
  vox-id: vox.populi
  vox-version: "0.1.0"
  vox-author: vox-team
  vox-category: infrastructure
  vox-tools:
    - vox_populi_local_status
    - vox_orchestrator_status
    - vox_submit_task
  vox-tags:
    - mens
    - labels
    - gpu
    - federation
    - workers
  vox-permissions:
    - db_read
---
```

- [ ] **Step 2: Run the characterization test**

Run: `cargo test -p vox-plugin-host --test plugin_skill_frontmatter_migration populi_mesh_skill_manifest`
Expected: PASS.

- [ ] **Step 3: Run the compliance gate**

Run: `vox ci agentskills-compliance`
Expected: no errors for `populi.skill.md` — this is also the check that `name: populi-mesh` still matches the `vox-plugin-populi-mesh` crate directory's expected short-name (`populi-mesh`), which the migration must not change.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-plugin-populi-mesh/populi.skill.md
git commit -m "chore(skills): migrate populi-mesh/populi.skill.md to canonical YAML frontmatter"
```

---

### Task 12: Run the full plugin-skill test and compliance sweep

**Files:**
- Modify (conditionally): `crates/vox-cli-ci/src/plugin_skill_parity.rs` (only if Step 0 finds it still TOML-only; see below)
- Otherwise none — verification-only task, closes out the 9-file migration before moving to the remaining cleanup tasks

- [ ] **Step 0: Fix `plugin-skill-parity`'s TOML-only frontmatter parser (prerequisite — this task will fail without it)**

`crates/vox-cli-ci/src/plugin_skill_parity.rs`'s `frontmatter_vox_tools()` parses the frontmatter block strictly as TOML (`let val: toml::Value = fm.parse().ok()?;`, no YAML fallback), unlike `vox-plugin-host::skill_parser`'s `parse_frontmatter`, which tries TOML first and falls back to YAML. After Tasks 3–11 migrate all 9 files' frontmatter to YAML, `frontmatter_vox_tools()` will fail to parse every one of them and return `None`, causing `vox ci plugin-skill-parity` to report "has no `[metadata] vox-tools` frontmatter" for all 9 files and exit non-zero — the opposite of Step 2's expectation below. Before running Step 2, update `frontmatter_vox_tools()` to mirror `skill_parser::parse_frontmatter`'s TOML-first-then-YAML-fallback strategy so it accepts the now-canonical YAML frontmatter. Run `cargo test -p vox-cli-ci` (or the crate's existing parity test suite, e.g. `crates/vox-cli/tests/plugin_skill_parity_smoke.rs`) after the change to confirm no regression for files that still use plain YAML elsewhere in the repo.

- [ ] **Step 1: Run all 9 characterization tests together**

Run: `cargo test -p vox-plugin-host --test plugin_skill_frontmatter_migration`
Expected: `test result: ok. 9 passed; 0 failed`

- [ ] **Step 2: Run the compliance and parity gates**

Run: `vox ci agentskills-compliance`
Expected: `✓ agentskills-compliance ok (N skill files checked)` with 0 errors.

Run: `vox ci plugin-skill-parity`
Expected: exits 0 (no drift between plugin `Plugin.toml` tool declarations and `.skill.md` frontmatter `vox-tools`) — only reachable once Step 0's YAML-fallback fix has landed.

- [ ] **Step 3: Confirm each migrated file's Markdown body is unchanged**

For each of the 9 files touched in Tasks 3–11, run `git diff <path>` and confirm the diff's only hunk is within the frontmatter block (from the opening `---` through the closing `---`), with no changes to the Markdown body below it. The characterization tests in Task 2 only assert parsed `SkillManifest` struct fields (sourced solely from frontmatter) and `vox ci agentskills-compliance` doesn't inspect body content either, so this is the only check in the plan that would catch an accidental body edit made alongside a frontmatter migration.

No commit — this task only verifies (plus the conditional Step 0 fix, committed on its own if made).

```bash
# Only if Step 0 required a code change:
git add crates/vox-cli-ci/src/plugin_skill_parity.rs
git commit -m "fix(vox-cli-ci): accept YAML frontmatter in plugin-skill-parity

frontmatter_vox_tools() was TOML-only; the 9 plugin-skill manifests
migrated to canonical YAML frontmatter in Tasks 3-11 of this plan
would otherwise all fail this gate's parse step."
```

---

### Task 12b: Verify the 13 `superpowers/*.skill.md` files against the parser SSOT

**Files:** none by default — only modify a given file if Step 1 finds it fails to parse, or Step 2 finds a genuine need for a Vox extension field

Spec item 2's third sub-bullet (`docs/superpowers/specs/2026-08-01-skill-ecosystem-improvements-design.md`) requires verifying the 13 already-YAML files under `crates/vox-skills/skills/superpowers/` (`antigravity-pipeline`, `brainstorming`, `deep-research`, `delegate-gemini`, `dispatching-parallel-agents`, `executing-plans`, `requesting-code-review`, `research`, `subagent-driven-development`, `test-driven-development`, `using-git-worktrees`, `verification-before-completion`, `writing-plans`) against the parser SSOT (`vox-plugin-host::skill_parser`), and adding an empty/absent `metadata` block only where a Vox extension field is actually needed — not speculatively. No task before this one in the plan touches this directory; this task closes that gap.

- [ ] **Step 1: Confirm each file parses cleanly under the SSOT parser**

Run: `cargo test -p vox-plugin-host` (the crate's existing `skill_parser` test suite already exercises `parse_skill_md`/`parse_frontmatter` generically). Then, for each of the 13 files, read its frontmatter and confirm it is plain YAML `name` + `description` with no `metadata` block, consistent with what the spec states is already true today. If any file fails to parse or its frontmatter doesn't match this description, note it and fix only that file's frontmatter (not its Markdown body) using the same YAML shape as Tasks 3–11.

- [ ] **Step 2: Decide whether any file genuinely needs a Vox extension field**

Per the spec's own instruction, only add a `metadata` block to a file if this phase's work actually depends on a `vox-*` field being present on it (e.g. Task 16's Vox Axis/MENS tool mapping does not require any per-skill frontmatter field — it maps tool *names*, not skill metadata). If no such dependency exists for any of the 13 files, leave all of them unmodified and record that in this task's commit message (or, if nothing changed, note it in Task 18's acceptance sweep instead of committing).

- [ ] **Step 3: Commit (only if a file was modified)**

```bash
# Only if Step 1 or Step 2 required a change to one or more files:
git add crates/vox-skills/skills/superpowers/<changed-file>.skill.md
git commit -m "chore(skills): verify superpowers/*.skill.md against parser SSOT

Spec item 2's third sub-bullet requires verifying the 13 superpowers
skill files against vox-plugin-host::skill_parser and adding metadata
blocks only where a Vox extension field is actually needed."
```

If no file needed changing, skip the commit — this task is then verification-only, like Task 12.

---

### Task 13: Delete orphaned reviewer-prompt templates

**Files:**
- Delete: `assets/skills/brainstorming/spec-document-reviewer-prompt.md`
- Delete: `assets/skills/writing-plans/plan-document-reviewer-prompt.md`

- [ ] **Step 1: Confirm both are unreferenced**

Run: `git grep -n "spec-document-reviewer-prompt\|plan-document-reviewer-prompt" -- ':!docs/src/architecture/skill-ecosystem-audit-2026-08-01.md' ':!docs/superpowers/specs/2026-08-01-skill-ecosystem-improvements-design.md' ':!docs/superpowers/plans/2026-08-01-skill-ecosystem-phase1-consolidation.md'`
Expected: no output.

- [ ] **Step 2: Delete both files**

Run: `git rm assets/skills/brainstorming/spec-document-reviewer-prompt.md assets/skills/writing-plans/plan-document-reviewer-prompt.md`

- [ ] **Step 3: Commit**

```bash
git commit -m "chore(skills): remove orphaned reviewer-prompt templates

Neither spec-document-reviewer-prompt.md nor plan-document-reviewer-prompt.md
is referenced by their sibling SKILL.md or anything else in the repo.
Phase 3 (review-gate integration, per the improvement roadmap) will
author fresh dispatch templates if/when it needs them."
```

---

### Task 14: Relocate `systematic-debugging/CREATION-LOG.md`

**Files:**
- Delete: `assets/skills/systematic-debugging/CREATION-LOG.md`

- [ ] **Step 1: Confirm it's an authoring log, not a runtime reference**

Run: `git grep -n "CREATION-LOG" -- assets/skills/systematic-debugging/SKILL.md`
Expected: no output — the skill's own body never links to it.

- [ ] **Step 2: Delete it**

Run: `git rm assets/skills/systematic-debugging/CREATION-LOG.md`

The file's history (how the skill was authored) remains available via `git log --follow` on the deleted path; it doesn't need to be preserved in a new location since it was never a shipped runtime reference.

- [ ] **Step 3: Commit**

```bash
git commit -m "chore(skills): remove systematic-debugging/CREATION-LOG.md

Authoring log from the upstream obra/superpowers vendoring, not a
runtime reference - unlinked from SKILL.md and not needed at runtime.
Still available via git history."
```

---

### Task 15: Refresh the stale `claude-api` vendored pin

**Files:**
- Modify: `assets/skills/SOURCES.toml`
- Modify: `assets/skills/claude-api/**` (regenerated by the sync script, not hand-edited)
- Modify: `NOTICE` (repo root — pinned-commit line must match the new pin)

`claude-api/SKILL.md`'s pricing table is already self-aware of its own staleness (it says "cached: 2026-06-04" and points to `shared/live-sources.md` for a live lookup) — the fix is not to hand-edit numbers into a vendored, license-tracked file (that would silently diverge from the pinned upstream commit `57546260929473d4e0d1c1bb75297be2fdfa1949` and break the provenance `SOURCES.toml` exists to guarantee). The fix is to re-vendor against a newer upstream commit using the existing sync tooling.

**Caveats the implementer must account for:**
- **Network access required.** Step 1 (`git ls-remote`) and Step 3 (`vox run scripts/vendor-skills.vox`, which does `git fetch --depth 1` against `github.com` for every source in `SOURCES.toml`) both require live outbound network access to `github.com`. There is no offline/local-cache fallback for the `anthropics/skills` source specifically — `vendor-skills.vox`'s `--local-cache` flag only substitutes a local checkout for `obra/superpowers`. If this task is run in a network-isolated environment, both steps will fail outright; there is no documented workaround in this phase.
- **The re-vendor is not scoped to `claude-api`.** `vendor-skills.vox` has no per-skill filter — running it re-fetches and fully overwrites (`remove_tree` + `copy_tree`, not an incremental diff) all 12 `anthropics/skills` entries sharing this one pin, *and*, because the script's `main()` loop has no per-source skip either, it also re-fetches and overwrites every `obra/superpowers`-sourced skill at that source's own (unchanged) pin. Since `obra/superpowers` still contains, at its current pin, the exact files Tasks 13 and 14 just deleted (`brainstorming/spec-document-reviewer-prompt.md`, `writing-plans/plan-document-reviewer-prompt.md`, `systematic-debugging/CREATION-LOG.md`), running the sync script here will silently recreate them on disk. **Steps 4 and 6 below scope the review and the `git add` to the `anthropics/skills` paths only, specifically to avoid re-committing them.**
- **The pin is not a deliberately reviewed commit.** `git ls-remote ... HEAD` records whatever the upstream default branch tip happens to be at the moment this task runs — not a commit chosen after reviewing what changed. Because `clone_at_pin()` does a shallow `git fetch --depth 1`, there is no local history to inspect the commit range between the old and new pin. Step 4 below requires a content-level look at the other 11 skills' diffs, not just a path-confinement check, precisely because nothing upstream of this task has reviewed what's in that commit range.

- [ ] **Step 1: Find the current upstream HEAD commit**

Run: `git ls-remote https://github.com/anthropics/skills HEAD`
Expected: a line like `<sha>	HEAD`. Record `<sha>` and the date this was run (for the commit message in Step 7) — this pin is "whatever HEAD is right now," not a specific reviewed release, so the record matters for anyone auditing this later.

- [ ] **Step 2: Update the pin in SOURCES.toml**

In `assets/skills/SOURCES.toml`, change line 12 from:
```toml
pin = "57546260929473d4e0d1c1bb75297be2fdfa1949"
```
to:
```toml
pin = "<sha from Step 1>"
```

- [ ] **Step 3: Re-run the vendor sync**

Run: `vox run scripts/vendor-skills.vox`
Expected: the script re-pulls all 12 `anthropics/skills` entries (including `claude-api`) at the new pin, and — because the script has no per-source scoping — also re-pulls every `obra/superpowers`-sourced skill at its own (unchanged) pin, which will recreate the files Tasks 13–14 deleted. This is expected at this point in the run; Steps 4 and 6 handle it.

- [ ] **Step 4: Review the diff before committing — content, not just paths**

Run: `git status --short assets/skills/` and `git diff --stat assets/skills/`.

First, confirm the *anthropics/skills* portion of the diff is confined to the 12 vendored directories (`algorithmic-art`, `brand-guidelines`, `canvas-design`, `claude-api`, `frontend-design`, `internal-comms`, `mcp-builder`, `skill-creator`, `slack-gif-creator`, `theme-factory`, `web-artifacts-builder`, `webapp-testing`) plus `SOURCES.toml`.

Then, for each of the 11 changed directories other than `claude-api`, run `git diff assets/skills/<dir>/` and actually read the content-level diff — not just the path list. This phase's pin bump was only reviewed/motivated for `claude-api`'s staleness; nothing upstream has looked at what changed in the other 11 skills sharing this pin, so this read-through is the only content-level check in the plan for that.

Separately, confirm `git status --short` shows the Task 13/14-deleted paths (`assets/skills/brainstorming/spec-document-reviewer-prompt.md`, `assets/skills/writing-plans/plan-document-reviewer-prompt.md`, `assets/skills/systematic-debugging/CREATION-LOG.md`) as untracked/recreated. If any of them reappear, delete them again now (`git rm -f <path>` or `rm <path>` if untracked) before proceeding — they must not be re-added in Step 6.

- [ ] **Step 5: Re-verify the upstream license is still redistributable**

`vendor-skills.vox`'s `license_upstream_path()` blindly copies whichever `LICENSE`/`LICENSE.txt`/`LICENSE.md` file it finds at the new pinned commit into `LICENSE.upstream`, with no check that the terms match the previously-recorded license. Run `git diff assets/skills/claude-api/LICENSE.upstream` (and spot-check the other 11 changed skills' `LICENSE.upstream` files) and confirm the license text is still the same OSS license `SOURCES.toml` currently declares (Apache-2.0). If any `LICENSE.upstream` diff shows a license change, stop — do not commit — and treat it as a licensing review, not a routine re-vendor.

- [ ] **Step 6: Run the compliance gate**

Run: `vox ci agentskills-compliance`
Expected: no errors for any `assets/skills/*/SKILL.md`.

Optionally, confirm the resync actually touched the specific staleness this task exists to fix: `git grep -n "cached: 2026-06-04" assets/skills/claude-api/SKILL.md` — expect no output (a positive signal the cached-date note moved forward, not just that the script ran without error).

- [ ] **Step 7: Update NOTICE's pinned-commit line**

`NOTICE` (repo root) independently states `Pinned at commit 57546260929473d4e0d1c1bb75297be2fdfa1949. See assets/skills/SOURCES.toml for full provenance.` — this is a second, human-readable provenance record alongside `SOURCES.toml`'s machine-readable `pin` field, and no automated step keeps them in sync. Edit `NOTICE` to replace the old commit hash with the `<sha>` recorded in Step 1, so `NOTICE` and `SOURCES.toml` agree.

- [ ] **Step 8: Commit**

```bash
git add assets/skills/algorithmic-art assets/skills/brand-guidelines assets/skills/canvas-design \
        assets/skills/claude-api assets/skills/frontend-design assets/skills/internal-comms \
        assets/skills/mcp-builder assets/skills/skill-creator assets/skills/slack-gif-creator \
        assets/skills/theme-factory assets/skills/web-artifacts-builder assets/skills/webapp-testing \
        assets/skills/SOURCES.toml NOTICE
git commit -m "chore(skills): re-vendor anthropics/skills to refresh stale claude-api pricing

Bumps assets/skills/SOURCES.toml's anthropics/skills pin (and NOTICE's
matching pinned-commit line) and re-runs vendor-skills.vox. claude-api's
model/pricing table was cached 2026-06-04; hand-editing vendored,
license-tracked content would have diverged from the pinned upstream
commit, so this re-vendors instead. Scoped to the anthropics/skills
directories only — the vendor script also touched obra/superpowers'
unchanged-pin tree, which is intentionally excluded from this commit
so it doesn't revert Tasks 13-14's deletions."
```

Deliberately **not** `git add assets/skills/` broadly — that would also stage whatever the vendor sync recreated under the `obra/superpowers`-sourced skills, undoing Tasks 13 and 14.

---

### Task 16: Add the Vox Axis / MENS tool-name mapping

**Files:**
- Create: `assets/skills/using-superpowers/references/vox-axis-tools.md`

**Scope note:** this task documents an existing-but-undocumented mapping between Claude Code tool names and already-registered Vox MCP tools/CLI verbs — it adds no new tool, no new MCP endpoint, and no new CLI verb. It is a documentation-only unblock for a scenario ("superpowers skills readable under Vox Axis/MENS") that the underlying tools already support; nothing here is new runtime capability in the sense the phase Goal's "adds no new capability" line is guarding against (new code paths, new permissions, new services).

- [ ] **Step 1: Enumerate the canonical Vox MCP base-tool names**

Run: `git grep -n "\"vox_read\|\"vox_write\|\"vox_edit\|\"vox_shell\|\"vox_grep\|\"vox_glob\|\"vox_bash" crates/vox-orchestrator-mcp/src/`

This session's research confirmed `vox_skill_use` (Skill-tool equivalent, `crates/vox-orchestrator-mcp/src/skills_tools.rs`) and the orchestrator skill's `vox_submit_task`/`vox_task_status`/`vox_agent_events` (Task-tool/subagent-dispatch equivalent, `crates/vox-plugin-skill-orchestrator/orchestrator.skill.md`), but did not enumerate the base file/shell/search tool names — use this step's grep output to fill those rows in Step 2 rather than guessing. If the grep returns nothing, check `crates/vox-orchestrator-mcp/src/lib.rs`'s tool-registration list directly for the base tool names before writing the table.

If a given Claude Code base tool (`Read`/`Write`/`Edit`/`Bash`/`Grep`/`Glob`) has no 1:1 Vox MCP equivalent — e.g. one Vox tool covers two Claude Code tools, or several Vox tools jointly cover one, or nothing covers it at all — do not force a false 1:1 row. Write the actual N:1, 1:N, or "no direct equivalent found" relationship in that row, following the same honest-gap pattern already used for the `TodoWrite` row below.

- [ ] **Step 2: Write the mapping file**

Follow the exact structure of `assets/skills/using-superpowers/references/gemini-tools.md`. Populate the `Read`/`Write`/`Edit`/`Bash`/`Grep`/`Glob` rows from Step 1's findings; the remaining rows are confirmed by this session's audit:

```markdown
# Vox Axis / MENS Tool Mapping

Skills use Claude Code tool names. When you encounter these in a skill while running under
Vox Axis (MENS or any other model in the Vox orchestrator), use your platform equivalent:

| Skill references | Vox Axis / MENS equivalent |
|-----------------|----------------------|
| `Read` (file reading) | <fill from Step 1's grep output> |
| `Write` (file creation) | <fill from Step 1's grep output> |
| `Edit` (file editing) | <fill from Step 1's grep output> |
| `Bash` (run commands) | <fill from Step 1's grep output> |
| `Grep` (search file content) | <fill from Step 1's grep output> |
| `Glob` (search files by name) | <fill from Step 1's grep output> |
| `TodoWrite` (task tracking) | No direct equivalent found as of 2026-08-01 — track progress in prose or via `vox_memory_log` |
| `Skill` tool (invoke a skill) | `vox_skill_use` |
| `Task` tool (dispatch subagent) | `vox_submit_task` (submit) + `vox_task_status` (poll) — see `crates/vox-plugin-skill-orchestrator/orchestrator.skill.md` |

## No native subagent-review loop

Vox Axis's `vox_submit_task`/`vox_task_status` is fire-and-poll, not a blocking dispatch-and-return
like Claude Code's `Task` tool. Skills that assume synchronous subagent dispatch
(`subagent-driven-development`, `dispatching-parallel-agents`) need to poll `vox_task_status`
until completion instead of awaiting a return value directly.

## Additional Vox Axis tools with no Claude Code equivalent

| Tool | Purpose |
|------|---------|
| `vox_memory_store` / `vox_memory_recall` | Persist and retrieve facts across sessions (`crates/vox-plugin-skill-memory/memory.skill.md`) |
| `vox_populi_local_status` | Inspect mens worker mesh labels/registry visibility |
| `vox_graphify_status` / `vox_graphify_query` | Graph-first structural search (see the `graphify` skill once Phase 2 lands it natively; today this is the Claude-Code-only `vox-graph` skill's tool set) |
```

- [ ] **Step 3: Confirm no placeholder text survived into the committed file**

Run: `git grep -n "<fill from" assets/skills/using-superpowers/references/vox-axis-tools.md`
Expected: no output. Step 2's template deliberately ships with literal `<fill from Step 1's grep output>` placeholders in the `Read`/`Write`/`Edit`/`Bash`/`Grep`/`Glob` rows; this check confirms they were actually replaced with Step 1's findings before committing, since `git diff --stat` alone (Step 4) only confirms the file exists, not that its content is complete.

- [ ] **Step 4: Verify the file parses as intended (no compliance gate covers reference files, so this is a manual read-through)**

Run: `git diff --stat assets/skills/using-superpowers/`
Expected: one new file, `references/vox-axis-tools.md`, alongside the three existing `{codex,copilot,gemini}-tools.md`.

- [ ] **Step 5: Commit**

```bash
git add assets/skills/using-superpowers/references/vox-axis-tools.md
git commit -m "docs(skills): add Vox Axis/MENS tool-name mapping for superpowers skills

Mirrors the existing codex-tools.md/copilot-tools.md/gemini-tools.md
pattern. Without this, the superpowers skill set (brainstorming,
writing-plans, subagent-driven-development, etc.) has no documented
path to running under Vox Axis/MENS."
```

---

### Task 17: Fix the confirmed `vox-graph` tool-name/CLI mismatch

**Files:**
- Modify: `assets/skills/vox-graph/SKILL.md`

**Corrected premise (adversarial review found the plan's original version of this task factually wrong):** the MCP tool-name keys actually registered in `crates/vox-orchestrator-mcp/src/dispatch.rs` and `input_schemas.rs` today are `vox_search_status`, `vox_search_structural`, `vox_search_neighbors`, `vox_search_path`, `vox_search_callers`, `vox_search_callees`, `vox_search_compare`, `vox_search_rebuild` — the `vox_search_*` rename already landed (commit `c18c7139a6`, "Vox Search (1/2): rename + engine + CLI + MCP + contracts (#406)", 2026-06-30) and is enforced by two passing guard tests (`crates/vox-orchestrator-mcp/tests/vox_search_dispatch.rs` and `crates/vox-cli/tests/vox_search_rename.rs`). `vox_graphify_*` strings appear only in stale Rust doc-comments on internal function names in `graph_tools.rs` — they are never dispatched. Separately, the canonical top-level CLI subcommand is `vox graph` (`crates/vox-cli/src/lib.rs`: `#[command(name = "graph", alias = "graphify", alias = "search")]`) — `graphify` and `search` are backward-compatible aliases, not the current name. This task now targets the actually-correct names instead.

- [ ] **Step 1: Rewrite the "Key MCP tools" and "Graph verbs (CLI)" sections**

Replace lines 21–36 of `assets/skills/vox-graph/SKILL.md` (from `## Key MCP tools` through the closing ``` ``` `` of the CLI block) with:

```markdown
## Key MCP tools

| Tool | Purpose |
|---|---|
| `vox_search_status` | Read-only freshness report for the structural corpus |
| `vox_search_structural` | Lexical search over an on-disk corpus graph |
| `vox_search_neighbors` | BFS neighbor expansion from seed node IDs |
| `vox_search_callers` / `vox_search_callees` | Direct caller/callee expansion from a seed node |
| `vox_search_path` | Shortest path between two node IDs |
| `vox_search_compare` | Diff two corpus manifests (node/edge/community delta) |
| `vox_search_rebuild` | Rebuild the structural graph for a corpus |

## Graph verbs (CLI)

```
vox graph rebuild --corpus <id>      # rebuild the structural graph
vox graph status                     # freshness report
vox graph index <path> --id <id>     # index a new target as a corpus (path is positional, not a --path flag)
vox graph query <args>               # lexical search over the corpus (same lexical scorer as vox_search_structural)
vox graph coverage --corpus <id>     # coverage classification report
vox graph refresh --corpus <id>      # rebuild-or-ingest based on staleness
```

`graph` is the canonical subcommand name (`crates/vox-cli/src/lib.rs`); `graphify` and `search` are backward-compatible aliases only — do not use them in new documentation.
```

- [ ] **Step 2: Update the "Rule" section's tool names to match**

Find the line:
```
**ALWAYS call `vox_search` or `vox_discover` first for connection/discovery questions.** Fall back to grep only when the graph gives no result (e.g., the corpus is stale or the file is not yet indexed).
```
Replace with:
```
**ALWAYS call `vox_search_structural` or `vox_search_neighbors` first for connection/discovery questions.** Fall back to grep only when the graph gives no result (e.g., the corpus is stale or the file is not yet indexed).
```

- [ ] **Step 3: Update the three bulleted "When to use this skill" examples to use the corrected names**

Find:
```
- **"Where is X called?"** — `vox_search` over the `repo-code-graph` corpus (BFS from the fn node) instead of grep.
- **"What surfaces expose command Y?"** — `vox_discover` with seed `cmd:Y` → follows edges to `surface:` nodes.
- **"What is related to Z?"** — `vox_discover` with seed of Z's node id → returns neighbors + their coverage class.
```
Replace with:
```
- **"Where is X called?"** — `vox_search_neighbors` over the `repo-code-graph` corpus (BFS from the fn node) instead of grep.
- **"What surfaces expose command Y?"** — `vox_search_neighbors` with seed `cmd:Y` → follows edges to `surface:` nodes.
- **"What is related to Z?"** — `vox_search_neighbors` with seed of Z's node id → returns neighbors + their coverage class.
```

- [ ] **Step 4: Replace the "no `graph` infix" footnote — it directly contradicts the corrected CLI block**

Find the line immediately after the CLI block (currently reads, verbatim): `*(There is no `graph` infix — `vox search <verb>` IS the graph subgroup, per vs1. See G6.)*` — this line sits outside the lines 21–36 range Step 1 replaces, so it survives untouched unless removed here. It is now false: `graph` is the canonical infix (Step 1), and `search` is only an alias. Delete this line entirely (its claim has no replacement — there is nothing accurate left to say here once the CLI block above is correct).

- [ ] **Step 5: Add a naming note recording that this fix corrects a documentation bug, not a pending rename**

Add a line after the "Determinism firewall" section:
```markdown

## Naming note

This skill's tool names (`vox_search_*`) and CLI form (`vox graph <verb>`) match what's actually registered in `crates/vox-orchestrator-mcp/src/dispatch.rs`/`input_schemas.rs` and `crates/vox-cli/src/lib.rs` today. A prior version of this skill (and of the plan task that produced this fix) incorrectly claimed a `vox_graphify_*`/`vox graphify` naming was current and that a `vox_search_*` rename was still pending — that rename landed via #406 over a month before this correction; this file was simply out of date.
```

- [ ] **Step 6: Positively cross-check the corrected names against `dispatch.rs`, not just grep for absence of the old ones**

Run: `git grep -n '"vox_search_' crates/vox-orchestrator-mcp/src/dispatch.rs` and confirm the 8 keys it returns (`vox_search_status`, `vox_search_structural`, `vox_search_neighbors`, `vox_search_path`, `vox_search_callers`, `vox_search_callees`, `vox_search_compare`, `vox_search_rebuild`) are exactly the 8 tool names just written into the "Key MCP tools" table in Step 1 — same set, same spelling, nothing extra or missing. This positive cross-check catches drift in either direction; a name-absence-only grep (e.g. for stale `vox_graphify_*` strings) would not catch a case where SKILL.md and `dispatch.rs` disagree on some *other* name.

Then run: `git grep -rn "vox_graphify_status\|vox_graphify_search\|vox_graphify_query\|vox_graphify_callers\|vox_graphify_callees\|vox_graphify_path\|vox_graphify_compare\|vox_graphify_rebuild" assets/skills/vox-graph/SKILL.md`
Expected: no output — confirms none of the previously-wrong `vox_graphify_*` names remain in the corrected file.

- [ ] **Step 7: Commit**

```bash
git add assets/skills/vox-graph/SKILL.md
git commit -m "fix(skills): correct vox-graph SKILL.md tool/CLI names to match reality

The skill documented vox_graphify_* tool names and a \`vox graphify\`
CLI form. Neither matches what's registered: dispatch.rs/input_schemas.rs
dispatch on vox_search_* (renamed via #406, 2026-06-30), and vox-cli's
canonical subcommand is \`vox graph\` (graphify/search are aliases).
Also removes a footnote claiming there is no \`graph\` infix, which
directly contradicted the corrected CLI block."
```

---

### Task 18: Final acceptance sweep

**Files:** none (verification-only)

- [ ] **Step 1: Run the full skill-format gates**

Run: `vox ci agentskills-compliance`
Expected: `✓ agentskills-compliance ok (N skill files checked)`, 0 errors.

Run: `vox ci plugin-skill-parity`
Expected: exits 0.

- [ ] **Step 2: Run skill discovery to confirm no stray skill files were left behind**

`vox skill discover` (`crates/vox-cli/src/commands/extras/ars/discover.rs`) is only compiled in under the non-default `ars` cargo feature (`crates/vox-cli/Cargo.toml`'s `default = ["keyring-store", "script-execution"]` does not include it) — build/run with `cargo run -p vox-cli --features ars -- skill discover` (or an equivalent `--features ars` build), not a default-build `vox skill discover`, or the command won't exist.

Also note what this command actually checks: it walks the tree for `*.skill.md` files and diffs their `id`s against the installed-skill registry (installed vs. not-installed) — it does **not** perform any reference-integrity or broken-link check, so it cannot "flag" a deleted file as missing. The correct expectation is narrower: run it and confirm none of the deleted files (`spec-document-reviewer-prompt.md`, `plan-document-reviewer-prompt.md`, `CREATION-LOG.md`, the stray `crates/vox-skills/skills/populi.skill.md`) shows up in its output at all (they aren't `*.skill.md` files walked by this command in the first place, except the deleted `populi.skill.md` — confirm it's absent from the listing, not merely "not flagged as broken").

- [ ] **Step 3: Run the full plugin-host test suite**

Run: `cargo test -p vox-plugin-host`
Expected: all tests pass, including the 9 new characterization tests and the existing `skill_parser` test suite (unaffected by this phase's changes).

- [ ] **Step 4: Confirm the working tree is clean**

Run: `git status --short`
Expected: no output — every task in this plan ends with its own commit.

- [ ] **Step 5: Manual MENS smoke test (spec §9 acceptance criterion — not covered by any automated step above)**

The design spec (`docs/superpowers/specs/2026-08-01-skill-ecosystem-improvements-design.md`, item 6's Testing/acceptance paragraph and §9) requires, as a Phase 1 acceptance criterion, that a fresh MENS session under Vox Axis can successfully invoke at least one superpowers skill end-to-end using the new `vox-axis-tools.md` mapping from Task 16. This is a manual smoke test (MENS harness automation is out of scope for this phase) and has no corresponding automated command in Steps 1–4 above. Start a fresh MENS session, have it read `assets/skills/using-superpowers/references/vox-axis-tools.md`, and have it invoke one superpowers skill (e.g. `brainstorming` or `writing-plans`) end-to-end using the tool-name substitutions the mapping documents. Record the outcome (pass/fail, and what broke if it failed) before considering this phase complete — do not claim Phase 1 acceptance without having actually run this.

No commit for this task — it's the closing verification, not a change.

---

## Self-Review

**1. Spec coverage.** Design spec §2 items 1–6 all map to tasks: item 1 (dedupe) → Task 1; item 2 (frontmatter migration, all three sub-bullets: populi TOML, 9 plugin-skill TOML files, and the 13 `superpowers/*.skill.md` SSOT-verification sub-bullet) → Tasks 2–12 plus Task 12b; item 3 (orphaned files) → Tasks 13–14; item 4 (claude-api staleness) → Task 15; item 5 (vox-axis-tools.md) → Task 16; item 6 (graphify/graph tool-name fix) → Task 17. Task 18 covers the spec's §9 testing/acceptance criteria, including the manual MENS smoke test (Step 5) that an earlier version of this plan omitted.

**2. Placeholder scan.** The only bracketed placeholders in this plan are inside Task 16's `vox-axis-tools.md` template (`<fill from Step 1's grep output>`) — deliberate, because this session's research confirmed the Skill-tool and Task-tool mappings but did not enumerate the base file/shell/search tool names, and inventing them would be less honest than directing the implementer to a concrete, runnable discovery command. Every other code/config block in this plan is complete, sourced from files read directly during planning.

**3. Type consistency.** `SkillCategory::Custom(String)` (Task 2's `rag`/`v0`/`populi-mesh` assertions) and `SkillPermission` variants (`ReadFiles`, `WriteFiles`, `ShellExec`, `Network`, `DbRead`, `DbWrite`) match `crates/vox-plugin-host/src/skill_parser.rs`'s `parse_category`/`parse_permission` exactly, confirmed by reading that file's match arms directly, not inferred.

---

## Parallel execution guidance

This section summarizes the adversarial-review parallelism analysis for a future `superpowers:subagent-driven-development` execution of this plan. It tells you which tasks can be dispatched to concurrent subagents and which must run in the foreground (single-threaded) because of a real ordering dependency or an unresolved ambiguity.

### Parallel-safe groups

**Group A — the 9 frontmatter migrations (Tasks 3–11).** Each task edits exactly one distinct `crates/vox-plugin-skill-*/*.skill.md` (or `vox-plugin-populi-mesh/populi.skill.md`) file, in a different directory, with no cross-references between them. They only share a read dependency on the test file Task 2 creates (via `include_str!` in the characterization tests) — **Task 2 must finish and be committed first**, but once it has, all 9 of Tasks 3–11 can run as concurrent subagents with zero file overlap.

**Group B — six independent-path tasks (Tasks 1, 13, 14, 15, 16, 17).** These touch fully disjoint paths: `crates/vox-skills/skills/populi.skill.md` (Task 1, distinct from the `vox-plugin-populi-mesh` copy Task 11 edits); `assets/skills/brainstorming/` and `assets/skills/writing-plans/` reviewer-prompt files (Task 13); `assets/skills/systematic-debugging/CREATION-LOG.md` (Task 14); `assets/skills/SOURCES.toml` plus the 12 named `anthropics/skills` vendor directories and `NOTICE` (Task 15); a brand-new file `assets/skills/using-superpowers/references/vox-axis-tools.md` (Task 16); and `assets/skills/vox-graph/SKILL.md` (Task 17). None of these paths intersect each other, Task 2's test file, or Tasks 3–11's skill.md files, and none has a stated precondition on another task's output. This group has no dependency on Task 2 either, so in principle it can start at time zero alongside (or before) Task 2/Group A — the plan's task numbering reflects authoring order, not a file-based ordering requirement.

**Task 12b (added during adversarial review) fits into Group B by the same file-disjointness logic** — it only touches `crates/vox-skills/skills/superpowers/*.skill.md`, a path no other task in the plan touches, so it can run concurrently with Group A and the rest of Group B. It was not part of the original parallelism analysis (the task didn't exist yet) but the same reasoning applies cleanly.

### Must run sequentially (foreground)

- **Task 2** — must complete and commit before any of Tasks 3–11 (Group A) start; it creates the characterization-test file they're all graded against.
- **Task 12** — must run after all of Tasks 3–11 (and its own new Step 0 fix to `plugin_skill_parity.rs`, if needed) are committed; it's the sweep that closes out the 9-file migration.
- **Task 18** — must run last; it's the plan's final acceptance sweep and depends on every other task's work being committed, including Task 12b, Task 15, Task 16, and Task 17.

### Ambiguous — flag for human confirmation, don't parallelize blindly

- **Task 12's compliance/parity gates report on skill files repo-wide** ("N skill files checked"), but the plan doesn't state whether that scan is sensitive to Group B's state — e.g. whether the stray, still-undeleted `crates/vox-skills/skills/populi.skill.md` (Task 1) would be counted or flagged before Task 1 runs. If it would, Task 12 implicitly needs Task 1 (and possibly other Group B tasks) done first, which would make Group A and Group B **not** fully independent. Confirm the compliance gate's scope with a human before running Task 12 concurrently with an unfinished Group B.
- **Task 15's `vox run scripts/vendor-skills.vox` re-vendor step has a wider write footprint than its own verification catches ahead of time** — the plan only discovers its blast radius *after* running it (`git status --short`/`git diff --stat`, confirmed during this review to also silently touch the `obra/superpowers`-sourced tree Tasks 13–14 already deleted from). Because of this, **do not run Task 15 concurrently with Task 13 or Task 14** — run it either strictly before them (so there's nothing for it to silently recreate) or strictly after them with the scoped `git add`/review steps added in this revision (Steps 4, 6, 8) followed exactly. If Task 15 is dispatched to a subagent in parallel with Group B, a human should confirm Tasks 13/14 have already committed their deletions first, or restrict Task 15's own commit to the exact `anthropics/skills` paths listed in its Step 8 `git add` (already narrowed in this revision, not `git add assets/skills/` broadly).
