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

**Create:**
- `crates/vox-plugin-host/tests/plugin_skill_frontmatter_migration.rs` (characterization tests, one `#[test]` per migrated file)
- `assets/skills/using-superpowers/references/vox-axis-tools.md`

---

### Task 1: Delete the stray `populi.skill.md` duplicate

**Files:**
- Delete: `crates/vox-skills/skills/populi.skill.md`

- [ ] **Step 1: Confirm nothing references the file by path**

Run: `git grep -n "vox-skills/skills/populi" -- ':!docs/src/architecture/skill-ecosystem-audit-2026-08-01.md' ':!docs/superpowers/specs/2026-08-01-skill-ecosystem-improvements-design.md' ':!docs/superpowers/plans/2026-08-01-skill-ecosystem-phase1-consolidation.md'`
Expected: no output (only this plan's own docs mention the path).

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
    assert_eq!(bundle.manifest.id, "vox.git");
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
    assert_eq!(bundle.manifest.id, "vox.memory");
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
    assert_eq!(bundle.manifest.id, "vox.orchestrator");
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
    assert_eq!(bundle.manifest.id, "vox.rag");
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
    assert_eq!(bundle.manifest.id, "vox.testing");
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
    assert_eq!(bundle.manifest.id, "vox.testing.validate");
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
    assert_eq!(bundle.manifest.id, "vox.v0");
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
    assert_eq!(bundle.manifest.id, "vox.populi");
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

- [ ] **Step 3: Commit**

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

`vox-permissions` is omitted rather than an empty list — the original TOML had `"vox-permissions" = []`, and `vox_arr`'s `.filter(|l| !l.is_empty())` treats an empty declared list the same as an absent one (falls through to the legacy-key/empty-default path), so this is behavior-preserving.

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

**Files:** none (verification-only task, closes out the 9-file migration before moving to the remaining cleanup tasks)

- [ ] **Step 1: Run all 9 characterization tests together**

Run: `cargo test -p vox-plugin-host --test plugin_skill_frontmatter_migration`
Expected: `test result: ok. 9 passed; 0 failed`

- [ ] **Step 2: Run the compliance and parity gates**

Run: `vox ci agentskills-compliance`
Expected: `✓ agentskills-compliance ok (N skill files checked)` with 0 errors.

Run: `vox ci plugin-skill-parity`
Expected: exits 0 (no drift between plugin `Plugin.toml` tool declarations and `.skill.md` frontmatter `vox-tools`).

No commit — this task only verifies; nothing changes.

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

`claude-api/SKILL.md`'s pricing table is already self-aware of its own staleness (it says "cached: 2026-06-04" and points to `shared/live-sources.md` for a live lookup) — the fix is not to hand-edit numbers into a vendored, license-tracked file (that would silently diverge from the pinned upstream commit `57546260929473d4e0d1c1bb75297be2fdfa1949` and break the provenance `SOURCES.toml` exists to guarantee). The fix is to re-vendor against a newer upstream commit using the existing sync tooling.

- [ ] **Step 1: Find the current upstream HEAD commit**

Run: `git ls-remote https://github.com/anthropics/skills HEAD`
Expected: a line like `<sha>	HEAD`. Record `<sha>`.

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
Expected: the script re-pulls all 12 `anthropics/skills` entries (including `claude-api`) at the new pin and rewrites their directories under `assets/skills/`.

- [ ] **Step 4: Review the diff before committing**

Run: `git status --short assets/skills/` and `git diff --stat assets/skills/`
Expected: changes confined to the 12 vendored `anthropics/skills` directories (`algorithmic-art`, `brand-guidelines`, `canvas-design`, `claude-api`, `frontend-design`, `internal-comms`, `mcp-builder`, `skill-creator`, `slack-gif-creator`, `theme-factory`, `web-artifacts-builder`, `webapp-testing`) plus `SOURCES.toml`. If any `obra/superpowers`-sourced file changed, stop and investigate — that pin wasn't touched and shouldn't drift.

- [ ] **Step 5: Run the compliance gate**

Run: `vox ci agentskills-compliance`
Expected: no errors for any `assets/skills/*/SKILL.md`.

- [ ] **Step 6: Commit**

```bash
git add assets/skills/
git commit -m "chore(skills): re-vendor anthropics/skills to refresh stale claude-api pricing

Bumps assets/skills/SOURCES.toml's anthropics/skills pin and re-runs
vendor-skills.vox. claude-api's model/pricing table was cached
2026-06-04; hand-editing vendored, license-tracked content would have
diverged from the pinned upstream commit, so this re-vendors instead."
```

---

### Task 16: Add the Vox Axis / MENS tool-name mapping

**Files:**
- Create: `assets/skills/using-superpowers/references/vox-axis-tools.md`

- [ ] **Step 1: Enumerate the canonical Vox MCP base-tool names**

Run: `git grep -n "\"vox_read\|\"vox_write\|\"vox_edit\|\"vox_shell\|\"vox_grep\|\"vox_glob\|\"vox_bash" crates/vox-orchestrator-mcp/src/`

This session's research confirmed `vox_skill_use` (Skill-tool equivalent, `crates/vox-orchestrator-mcp/src/skills_tools.rs`) and the orchestrator skill's `vox_submit_task`/`vox_task_status`/`vox_agent_events` (Task-tool/subagent-dispatch equivalent, `crates/vox-plugin-skill-orchestrator/orchestrator.skill.md`), but did not enumerate the base file/shell/search tool names — use this step's grep output to fill those rows in Step 2 rather than guessing. If the grep returns nothing, check `crates/vox-orchestrator-mcp/src/lib.rs`'s tool-registration list directly for the base tool names before writing the table.

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

- [ ] **Step 3: Verify the file parses as intended (no compliance gate covers reference files, so this is a manual read-through)**

Run: `git diff --stat assets/skills/using-superpowers/`
Expected: one new file, `references/vox-axis-tools.md`, alongside the three existing `{codex,copilot,gemini}-tools.md`.

- [ ] **Step 4: Commit**

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

- [ ] **Step 1: Rewrite the "Key MCP tools" and "Graph verbs (CLI)" sections**

Replace lines 21–36 of `assets/skills/vox-graph/SKILL.md` (from `## Key MCP tools` through the closing ``` ``` `` of the CLI block) with:

```markdown
## Key MCP tools

| Tool | Purpose |
|---|---|
| `vox_graphify_status` | Read-only freshness report for graphify corpora |
| `vox_graphify_search` | Lexical search over an on-disk graphify corpus graph |
| `vox_graphify_query` | BFS neighbor expansion from seed node IDs |
| `vox_graphify_callers` / `vox_graphify_callees` | Direct caller/callee expansion from a seed node |
| `vox_graphify_path` | Shortest path between two node IDs |
| `vox_graphify_compare` | Diff two corpus manifests (node/edge/community delta) |

## Graph verbs (CLI)

```
vox graphify rebuild --corpus <id>   # rebuild the structural graph
vox graphify status                  # freshness report
vox graphify index --path <p> --id <id>   # index a new target as a corpus
vox graphify query <args>            # BFS neighbor expansion from the CLI
vox graphify coverage --corpus <id>  # coverage classification report
vox graphify refresh --corpus <id>   # rebuild-or-ingest based on staleness
```
```

- [ ] **Step 2: Update the "Rule" section's tool names to match**

Find the line:
```
**ALWAYS call `vox_search` or `vox_discover` first for connection/discovery questions.** Fall back to grep only when the graph gives no result (e.g., the corpus is stale or the file is not yet indexed).
```
Replace with:
```
**ALWAYS call `vox_graphify_search` or `vox_graphify_query` first for connection/discovery questions.** Fall back to grep only when the graph gives no result (e.g., the corpus is stale or the file is not yet indexed).
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
- **"Where is X called?"** — `vox_graphify_query` over the `repo-code-graph` corpus (BFS from the fn node) instead of grep.
- **"What surfaces expose command Y?"** — `vox_graphify_query` with seed `cmd:Y` → follows edges to `surface:` nodes.
- **"What is related to Z?"** — `vox_graphify_query` with seed of Z's node id → returns neighbors + their coverage class.
```

- [ ] **Step 4: Note the rename as a fast-follow, not a blocker**

Add a line after the "Determinism firewall" section:
```markdown

## Naming note

This skill's tool names (`vox_graphify_*`) match what's registered in `crates/vox-orchestrator-mcp/src/graph_tools.rs` today. A `vox_search_*` rename was drafted (see `docs/superpowers/plans/2026-06-27-vox-graph-rename-and-manifest-plan-vg1.md`) but never carried out in code — if that rename lands later, update this skill in the same change, not before.
```

- [ ] **Step 5: Verify no other file references the now-corrected old names as if they were real**

Run: `git grep -rn "vox_search_query\|vox_search_path\|vox_search_status" -- ':!docs/src/architecture/skill-ecosystem-audit-2026-08-01.md' ':!docs/superpowers/specs/2026-08-01-skill-ecosystem-improvements-design.md' ':!docs/superpowers/plans/2026-08-01-skill-ecosystem-phase1-consolidation.md'`
Expected: no output (or only hits inside `docs/superpowers/plans/2026-06-27-vox-graph-rename-and-manifest-plan-vg1.md`, which documents the drafted-but-not-executed rename and is correctly left alone).

- [ ] **Step 6: Commit**

```bash
git add assets/skills/vox-graph/SKILL.md
git commit -m "fix(skills): correct vox-graph SKILL.md tool/CLI names to match reality

The skill documented vox_search_query/vox_discover/vox_search_path/
vox_search_status and a \`vox search\` CLI subgroup that don't exist -
graph_tools.rs registers vox_graphify_* tools and vox-cli exposes
\`vox graphify <verb>\`. The skill was written ahead of a drafted
rename that was never carried out in code."
```

---

### Task 18: Final acceptance sweep

**Files:** none (verification-only)

- [ ] **Step 1: Run the full skill-format gates**

Run: `vox ci agentskills-compliance`
Expected: `✓ agentskills-compliance ok (N skill files checked)`, 0 errors.

Run: `vox ci plugin-skill-parity`
Expected: exits 0.

- [ ] **Step 2: Run skill discovery to confirm no orphans introduced**

Run: `vox skill discover`
Expected: completes without flagging the deleted files (`spec-document-reviewer-prompt.md`, `plan-document-reviewer-prompt.md`, `CREATION-LOG.md`, the stray `populi.skill.md`) as missing/broken references.

- [ ] **Step 3: Run the full plugin-host test suite**

Run: `cargo test -p vox-plugin-host`
Expected: all tests pass, including the 9 new characterization tests and the existing `skill_parser` test suite (unaffected by this phase's changes).

- [ ] **Step 4: Confirm the working tree is clean**

Run: `git status --short`
Expected: no output — every task in this plan ends with its own commit.

No commit for this task — it's the closing verification, not a change.

---

## Self-Review

**1. Spec coverage.** Design spec §2 items 1–6 all map to tasks: item 1 (dedupe) → Task 1; item 2 (frontmatter migration) → Tasks 2–12; item 3 (orphaned files) → Tasks 13–14; item 4 (claude-api staleness) → Task 15; item 5 (vox-axis-tools.md) → Task 16; item 6 (graphify tool-name fix) → Task 17. Task 18 covers the spec's §9 testing/acceptance criteria.

**2. Placeholder scan.** The only bracketed placeholders in this plan are inside Task 16's `vox-axis-tools.md` template (`<fill from Step 1's grep output>`) — deliberate, because this session's research confirmed the Skill-tool and Task-tool mappings but did not enumerate the base file/shell/search tool names, and inventing them would be less honest than directing the implementer to a concrete, runnable discovery command. Every other code/config block in this plan is complete, sourced from files read directly during planning.

**3. Type consistency.** `SkillCategory::Custom(String)` (Task 2's `rag`/`v0`/`populi-mesh` assertions) and `SkillPermission` variants (`ReadFiles`, `WriteFiles`, `ShellExec`, `Network`, `DbRead`, `DbWrite`) match `crates/vox-plugin-host/src/skill_parser.rs`'s `parse_category`/`parse_permission` exactly, confirmed by reading that file's match arms directly, not inferred.
