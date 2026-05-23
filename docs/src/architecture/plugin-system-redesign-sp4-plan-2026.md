---
title: "Plugin System Redesign — SP4 Implementation Plan (2026)"
description: "Step-by-step implementation plan for Sub-Project 4: migrate the vox.compiler skill from vox-skills's compile-time builtins to a standalone skill-payload plugin loaded at runtime through vox-plugin-host."
category: "Architecture SSOTs"
status: "research"
training_eligible: true
training_rationale: "Concrete TDD task plan for SP4; companion to the parent design spec."
---

# Plugin System Redesign — SP4 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Parent spec:** [`plugin-system-redesign-2026.md`](plugin-system-redesign-2026.md)
**Predecessor plans:** [`SP1`](plugin-system-redesign-sp1-plan-2026.md) (catalog) AND [`SP2`](plugin-system-redesign-sp2-plan-2026.md) (host loader). Both must be merged before SP4 starts. SP4 is INDEPENDENT of SP3 — they can land in either order.

**Goal:** Migrate ONE built-in skill — `vox.compiler` — from [`vox-skills`](../../../crates/vox-skills/)'s compile-time `include_str!` registry into a standalone skill-payload plugin discovered and loaded at runtime by `vox-plugin-host`. Migrate `vox-orchestrator`, `vox-actor-runtime`, and `vox-integration-tests` consumers of that one skill to query through `vox-plugin-host`'s `SkillRegistry` instead of the old `vox_skills::SkillRegistry`. The other 8 skills stay in `vox-skills` and on the old code path until SP6.

**Architecture:** `crates/vox-plugin-skill-compiler/` becomes a directory-only plugin (no Rust crate, just `Plugin.toml` + `compiler.skill.md` lifted verbatim from `crates/vox-skills/skills/compiler.skill.md`). `vox-skills`'s `install_builtins()` stops registering `vox.compiler`. The orchestrator's startup path now calls `vox_plugin_host::discover()` against the install dir AND `vox_skills::install_builtins()` for the other 8 skills (transitional state). MCP tool dispatch for the three compiler tools (`vox_validate_file`, `vox_run_tests`, `vox_check_workspace`) routes through `vox_plugin_host::SkillRegistry` instead of `vox_skills::SkillRegistry`.

**Tech Stack:**
- `vox-plugin-api` and `vox-plugin-host` from SP2
- Existing `vox-skills` infrastructure (kept intact for the 8 unmigrated skills)
- `vox-orchestrator`, `vox-actor-runtime`, `vox-integration-tests` (consumers)

---

## File Structure

### New (directory plugin — no Cargo.toml)

| Path                                                                | Responsibility                                                          |
| ------------------------------------------------------------------- | ----------------------------------------------------------------------- |
| `crates/vox-plugin-skill-compiler/Plugin.toml`                      | Skill-payload manifest declaring 3 exposed tools.                       |
| `crates/vox-plugin-skill-compiler/compiler.skill.md`                | Verbatim copy of `crates/vox-skills/skills/compiler.skill.md`.           |

### Modified

| Path                                                                | Change                                                                  |
| ------------------------------------------------------------------- | ----------------------------------------------------------------------- |
| `crates/vox-skills/src/builtins.rs`                                 | Remove the `vox.compiler` entry from the embedded builtins array.       |
| `crates/vox-orchestrator/src/mcp_tools/server_state.rs`             | At startup, also `discover()` the install dir for skill plugins; merge into the existing skill registry view. |
| `crates/vox-orchestrator/src/bin/vox_orchestrator_d.rs`             | Same as server_state.rs (the `install_builtins` callsite at line 126).  |
| `crates/vox-orchestrator/src/mcp_tools/skills_tools.rs`             | MCP-tool dispatch consults `vox_plugin_host::SkillRegistry` for the compiler skill (still uses `vox_skills` for the other 8). |
| `crates/vox-actor-runtime/src/builtins/mod.rs`                            | If it references `vox.compiler` skill specifically, route through `vox_plugin_host` instead.  |
| `crates/vox-integration-tests/...`                                  | Find tests that exercise `vox.compiler` skill; migrate to use the new path. |
| `Cargo.toml` (workspace)                                            | (No change. `vox-plugin-host` is already a workspace dep from SP2.)     |
| Several Cargo.toml files                                            | Add `vox-plugin-host = { workspace = true }` to crates that need to query the new registry. |
| Workspace `[workspace] exclude = [...]`                             | Add `crates/vox-plugin-skill-compiler` to prevent cargo from treating the directory-only plugin as a Rust crate (same convention as the SP2 noop-skill). |

---

## Tasks

### Task 1: Map all current `vox.compiler` callsites

**Files:** none (research only).

Before touching anything, build a complete picture of where the `vox.compiler` skill is referenced.

- [ ] **Step 1:** Run:

```
rg "vox\.compiler|vox\\.compiler|compiler\.skill\.md|install_builtins" --type rust crates/
```

Capture every hit. Categorize each as:
- (a) Direct registration — code that adds the skill to a registry. Should be removed/redirected.
- (b) Direct lookup — code that looks up the skill by id `vox.compiler` (or any of its tools). Should consult the new registry.
- (c) Test fixture — code that constructs a fake compiler skill for testing. May or may not need migration.
- (d) Documentation reference — code comments or docs. Update only if needed for accuracy.

- [ ] **Step 2:** Write the call-graph summary as a comment block at the top of this plan's eventual implementation PR description (so reviewers can verify completeness).

### Task 2: Create the skill-plugin directory

**Files:** `crates/vox-plugin-skill-compiler/{Plugin.toml,compiler.skill.md}`.

- [ ] **Step 1:** Copy the content of `crates/vox-skills/skills/compiler.skill.md` to `crates/vox-plugin-skill-compiler/compiler.skill.md` verbatim. Do NOT modify the body — byte-for-byte identical.
- [ ] **Step 2:** Create `Plugin.toml`:

```toml
[plugin]
id = "skill-compiler"
name = "Vox Compiler Skill"
version = "0.1.0"
description = "Agent-facing skill describing the Vox compiler tools."
license = "Apache-2.0"

[plugin.host]
min-vox-version = "0.5.0"

[plugin.payload]
kind = "skill"
format-version = 1
skill-md = "compiler.skill.md"

[plugin.payload.tools]
exposes = ["vox_validate_file", "vox_run_tests", "vox_check_workspace"]
```

- [ ] **Step 3:** Verify cargo doesn't try to build the directory:

```
cargo check --workspace 2>&1 | grep -i "vox-plugin-skill-compiler"
```

If cargo complains about the missing Cargo.toml, add to root `Cargo.toml`:

```toml
[workspace]
exclude = ["crates/vox-plugin-skill-compiler"]
```

(SP2 likely already added a similar exclude for `vox-plugin-noop-skill`. Add this one to the same list.)

- [ ] **Step 4:** Verify `cargo run -q -p vox-cli -- ci plugin-skill-parity` recognizes the new plugin.
- [ ] **Step 5:** Commit: `feat(plugin-skill-compiler): add standalone skill plugin directory mirroring vox-skills/compiler.skill.md`.

### Task 3: Remove `vox.compiler` from `vox-skills` builtins

**Files:** `crates/vox-skills/src/builtins.rs`.

- [ ] **Step 1:** Find the array/list that registers built-in skills (the `include_str!` block from `crates/vox-skills/skills/`).
- [ ] **Step 2:** Remove the entry that registers `compiler.skill.md`. Leave the other 8 entries intact.
- [ ] **Step 3:** Do NOT delete `crates/vox-skills/skills/compiler.skill.md` itself yet. It's the source for SP2 Task 17 if anyone needs to compare. Deletion happens in SP6 along with the rest of `vox-skills`.

  Actually — for consistency, deleting it now prevents drift between the two copies. Decision: delete it after Task 2's verbatim copy is committed. Add the deletion to this commit.

- [ ] **Step 4:** `cargo build -p vox-skills` — green.
- [ ] **Step 5:** `cargo test -p vox-skills` — green (any tests that asserted compiler skill was a builtin will fail; those need migration in Task 6).
- [ ] **Step 6:** Commit: `refactor(vox-skills): drop vox.compiler from builtins (now lives as standalone plugin)`.

### Task 4: Wire `vox-plugin-host::discover()` into orchestrator startup

**Files:** `crates/vox-orchestrator/src/bin/vox_orchestrator_d.rs`, `crates/vox-orchestrator/src/mcp_tools/server_state.rs`.

The existing pattern (per Task 1 research):
- Line ~123 of `vox_orchestrator_d.rs`: `let registry = vox_skills::new_registry_arc();`
- Line ~126: `let _ = vox_skills::install_builtins(&registry_for_builtins).await;`

- [ ] **Step 1:** Add `vox-plugin-host = { workspace = true }` to `crates/vox-orchestrator/Cargo.toml` `[dependencies]`.
- [ ] **Step 2:** In both files (`vox_orchestrator_d.rs` and `server_state.rs`), after `install_builtins`, also call:

```rust
let plugin_install_dir = std::path::PathBuf::from(
    std::env::var("VOX_PLUGINS_DIR").unwrap_or_else(|_| {
        dirs::data_local_dir()
            .map(|p| p.join("vox").join("plugins").to_string_lossy().to_string())
            .unwrap_or_else(|_| "./vox-plugins".into())
    })
);
match vox_plugin_host::discover(&plugin_install_dir) {
    Ok(plugin_registry) => {
        // Merge plugin_registry.skills into the existing vox_skills registry view,
        // OR keep them as separate registries and have the dispatch layer (Task 5)
        // try one then the other. Cleanest: separate registries; merge in dispatch.
        state.plugin_registry = std::sync::Arc::new(plugin_registry);
    }
    Err(e) => tracing::warn!("plugin discover failed: {e}"),
}
```

- [ ] **Step 3:** Add the new field to whichever struct holds the orchestrator's registry handles. In `server_state.rs` add:

```rust
pub plugin_registry: Arc<vox_plugin_host::Registry>,
```

with `Default` impl producing an empty `Registry::new()`.

- [ ] **Step 4:** Test that the orchestrator starts cleanly even when no plugins exist (the install dir may be missing). Run any existing orchestrator startup test.
- [ ] **Step 5:** Commit: `feat(vox-orchestrator): scan plugin install dir at startup via vox-plugin-host::discover`.

### Task 5: Route compiler-tool MCP dispatch through the plugin registry

**Files:** `crates/vox-orchestrator/src/mcp_tools/skills_tools.rs` (and any sibling file that dispatches `vox_validate_file` / `vox_run_tests` / `vox_check_workspace`).

The existing dispatch consults `vox_skills::SkillRegistry`. After Task 3 the compiler skill is no longer in there; lookups for `vox.compiler` in the old registry now return None. The dispatch needs to fall back to `vox_plugin_host::Registry::skills`.

- [ ] **Step 1:** Identify the dispatch function. Add a fallback:

```rust
fn lookup_skill(state: &ServerState, id: &str) -> Option<SkillInfo> {
    // Try old registry first (8 remaining builtins).
    if let Some(s) = state.skill_registry.lookup(id) {
        return Some(s.into());
    }
    // Fall back to plugin host registry.
    state.plugin_registry.skills.lookup(id).ok().map(|s| s.into())
}
```

(The `Into` conversions and exact API names need to match what's in scope. Adapt as needed.)

- [ ] **Step 2:** Test: a request for tool `vox_validate_file` should still resolve and dispatch correctly.
- [ ] **Step 3:** Commit: `feat(vox-orchestrator): fall back to vox-plugin-host SkillRegistry for skill lookups`.

### Task 6: Update `vox-actor-runtime` consumer

**Files:** `crates/vox-actor-runtime/src/builtins/mod.rs` and any other location identified in Task 1.

If `vox-actor-runtime` directly references `vox.compiler` (vs going through orchestrator), apply the same fallback pattern.

- [ ] **Step 1:** Check the file. If no direct compiler reference, no change needed.
- [ ] **Step 2:** If reference exists, mirror Task 5's fallback pattern.
- [ ] **Step 3:** `cargo check -p vox-actor-runtime` — green.
- [ ] **Step 4:** Commit if changes were made.

### Task 7: Update `vox-integration-tests`

**Files:** Any test in `crates/vox-integration-tests/` that exercises `vox.compiler` skill (per Task 1 research).

- [ ] **Step 1:** For each affected test, install the `skill-compiler` plugin into a tempdir (mirror SP2 Task 17's `load_noop_skill.rs` pattern), construct the orchestrator with that plugin install dir, then run the existing assertions.
- [ ] **Step 2:** `cargo test -p vox-integration-tests` — green.
- [ ] **Step 3:** Commit: `test(integration): use vox-plugin-skill-compiler plugin install dir for compiler-skill tests`.

### Task 8: End-to-end MCP test

**Files:** `crates/vox-orchestrator/tests/skill_compiler_via_plugin.rs`.

Spin up the orchestrator with the compiler skill installed only as a plugin (not in the old vox-skills builtins), have an MCP client invoke `vox_validate_file`, and assert the response shape matches a baseline captured before SP4.

- [ ] **Step 1:** Capture the pre-SP4 baseline if not already in repo:

```
cargo test -p vox-orchestrator --test some_existing_compiler_test -- --capture-output > pre-sp4-baseline.json
```

(Use whatever existing test approximates this scenario. If none exists, write one against pre-SP4 main first.)

- [ ] **Step 2:** Write the new test that uses the plugin install dir.
- [ ] **Step 3:** Compare responses byte-by-byte (or field-by-field if non-deterministic fields like timestamps are present).
- [ ] **Step 4:** Verify PASS.
- [ ] **Step 5:** Commit.

### Task 9: Catalog and CI

`skill-compiler` is already in the catalog (SP1 Task 3). Verify the parity guards still pass.

- [ ] **Step 1:** `cargo run -q -p vox-cli -- ci plugin-catalog-parity` — pass.
- [ ] **Step 2:** `cargo run -q -p vox-cli -- ci plugin-skill-parity` — pass.
- [ ] **Step 3:** `cargo run -q -p vox-cli -- ci generate-plugin-catalog-docs --check` — pass.

### Task 10: Update `skill_marketplace.md` reference doc

The existing [`docs/src/reference/skill_marketplace.md`](../../../docs/src/reference/skill_marketplace.md) describes the old `vox-skills` SKILL.md format. Add a note (no full rewrite — that comes in SP6) mentioning that SP4 has begun the migration; the `vox.compiler` skill is now installed via the plugin system.

- [ ] **Step 1:** Edit the doc with a short callout box.
- [ ] **Step 2:** Run `cargo run -p vox-doc-pipeline` if this affects SUMMARY.md (it shouldn't — same file).
- [ ] **Step 3:** Commit.

### Task 11: Final acceptance

- [ ] **Step 1:** `cargo build --workspace` — green.
- [ ] **Step 2:** `cargo test -p vox-skills` — green (the 8 remaining builtins still work).
- [ ] **Step 3:** `cargo test -p vox-orchestrator` — green.
- [ ] **Step 4:** `cargo test -p vox-actor-runtime` — green.
- [ ] **Step 5:** `cargo test -p vox-integration-tests` — green.
- [ ] **Step 6:** All four CI guards green: `plugin-catalog-parity`, `plugin-abi-parity`, `plugin-skill-parity`, `generate-plugin-catalog-docs --check`.
- [ ] **Step 7:** Behavioral parity: an MCP client calling `vox_validate_file` gets the same response shape as before SP4.

If green: SP4 done. The other 7 skills (and the composite `populi-mesh` skill side) remain in `vox-skills`'s builtins and migrate as part of SP6.

---

## Spec coverage check (self-review)

| SP4 spec deliverable                                                             | Plan task |
| -------------------------------------------------------------------------------- | --------- |
| `crates/vox-plugin-skill-compiler/` directory with Plugin.toml + skill md        | 2         |
| `vox.compiler` removed from `vox-skills` builtins                                | 3         |
| Source `compiler.skill.md` reference removed                                     | 3         |
| Orchestrator scans plugin install dir + merges with existing skill registry      | 4, 5      |
| Runtime same migration                                                           | 6         |
| MCP tool aliasing (`vox_skill_*` continues to work)                              | (preserved by Task 5's fallback dispatch — old tools still reach old registry) |
| End-to-end MCP test                                                              | 8         |
| `vox.compiler` no longer in `vox-skills/skills/`                                 | 3         |
| Other 7 skills + ARS shim untouched                                              | (out of scope by design — SP6) |

All SP4 deliverables map to tasks. Largest implementation risk: Task 1's call-graph mapping. The orchestrator's skill consumption is wired through several files (per the rg from earlier reconnaissance: `server_state.rs`, `skills_tools.rs`, `bin/vox_orchestrator_d.rs`, `memory_tools/tests.rs`, `workspace_path.rs`, `openclaw_tools.rs`); missing one means a stale lookup that returns "skill not installed" at runtime. Task 1's exhaustive `rg` is the gating step.
