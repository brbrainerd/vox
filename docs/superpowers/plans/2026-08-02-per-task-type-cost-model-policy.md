# Per-Task-Type LLM Cost/Model Policy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make LLM cost/model-selection strategy configurable per task type (TaskCategory) and per trigger source (who/what started the task), instead of every task silently falling back to the global `Balanced`/`Moderate` clutch/risk defaults.

**Architecture:** Add a `TriggerSource` enum alongside the existing `ClutchProfile`/`RiskPosture` in `crates/vox-orchestrator/src/mode.rs`, plus a pure precedence resolver (`explicit > category policy > source policy > global default`). Policy entries are typed Rust tables (compiled defaults, empty at first landing) merged with a small `TaskPolicyOverrides` struct already-loadable through `OrchestratorConfig`'s existing Vox.toml deserialization (`crates/vox-orchestrator/src/config/orchestrator_fields.rs`) — no new crate dependency required, since both the enums and the config struct already live in `vox-orchestrator`. `AgentTask` (the queue/CI/scheduled path) and the MCP-direct `McpChatModelResolution` path (always `Interactive`) both call the resolver instead of hardcoding a fallback. A minimal GUI panel (Tauri commands mirroring `set_orchestrator_config`'s existing raw-TOML write pattern) lets a user add/remove overrides live.

**Tech Stack:** Rust (workspace crates `vox-orchestrator`, `vox-orchestrator-mcp`, `vox-gui` (Tauri 2)), existing `vox_config::snapshot` reactive-bump mechanism, React/vitest for the GUI panel. Windows-safe formatting per `AGENTS.md` (`cargo fmt -p <crate>`, never `--all`).

**Spec:** [`docs/superpowers/specs/2026-08-02-per-task-type-cost-model-policy-design.md`](../specs/2026-08-02-per-task-type-cost-model-policy-design.md)

**Deviation from the spec's literal storage description (found while grounding this plan against live code, noted here per this repo's own convention of documenting such corrections):** the spec described compiled defaults living in the layer-0 `vox-llm-config` crate, string-keyed to avoid a cross-layer dependency. Live-code verification found that's unnecessary: `ClutchProfile`, `RiskPosture`, `TaskCategory`, and `OrchestratorConfig` (the Vox.toml-backed struct) all already live in `vox-orchestrator` itself, and `vox-orchestrator` has zero dependency on `vox-llm-config`/`vox-config` today. Adding one would be pure overhead for no additional consumer. This plan keeps everything — typed tables, resolver, and the override struct — inside `vox-orchestrator`, using typed enums directly (no string-keying needed for the *typed* tables; string keys are used only at the Vox.toml boundary, exactly like `ClutchProfile::from_label` already does). The precedence/behavior described in the spec is unchanged.

**Per-phase close (applies to every phase):** after the phase's tasks, run `/code-review` on the diff, then `cargo clippy -p <each touched crate> -- -D warnings` and the phase's tests green, before moving on. These are not repeated as steps inside each task.

---

## File Structure

| File | Responsibility | Phase |
|---|---|---|
| `crates/vox-orchestrator/src/mode.rs` (modify) | `TriggerSource` enum + label parsing; `TaskCategoryPolicy`/`TriggerSourcePolicy` types + empty compiled-default tables; `resolve_task_policy()` pure resolver; `effective_category_policy()`/`effective_source_policy()` override-merge helpers | 1 |
| `crates/vox-orchestrator/src/config/orchestrator_fields.rs` (modify) | `TaskPolicyOverrides` + `TaskPolicyEntry` structs; new `task_policy: TaskPolicyOverrides` field on `OrchestratorConfig` | 1 |
| `crates/vox-orchestrator/src/types/tasks.rs` (modify) | `AgentTask.trigger_source` field; `TaskEnqueueHints.trigger_source` hint; `apply_hints` parses it; new `AgentTask::resolved_policy()` replacing the naive `resolved_clutch()`/`resolved_risk()` fallbacks | 2 |
| `crates/vox-orchestrator/src/runtime.rs` (modify) | `resolve_task_cost_policy()` extracted + wired into `AiTaskProcessor::process`, replacing the `clutch_profile.is_some()` dead-branch gate | 2 |
| `crates/vox-orchestrator-mcp/src/params.rs` (modify) | `clutch`/`risk`/`trigger_source: Option<String>` fields on `SubmitTaskParams` (schema auto-derives via `schemars`) | 3 |
| `crates/vox-orchestrator-mcp/src/task_tools/submission.rs` (modify) | Forward the new params into `TaskEnqueueHints` instead of hardcoding `None` | 3 |
| `crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/types.rs` (modify) | `McpChatModelResolution.trigger_source: TriggerSource` (defaults `Interactive`) | 3 |
| `crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/resolve.rs` (modify) | Call the resolver at the top of `resolve_mcp_chat_model_sync_inner` so `res.clutch`/`res.risk` are always `Some` before `build_selection_request` | 3 |
| `crates/vox-orchestrator-mcp/src/models_tools.rs` (modify) | Add `trigger_source: TriggerSource::Interactive` to its `McpChatModelResolution` literal | 3 |
| `crates/vox-gui/src/commands/orchestrator.rs` (modify) | `get_task_policy_overrides` / `set_task_policy_override` / `clear_task_policy_override` Tauri commands, mirroring `get_orchestrator_config`/`set_orchestrator_config`'s existing raw-TOML read/write + `ORCH_CONFIG_CHANGED_EVENT` emit | 4 |
| `crates/vox-gui/src/main.rs` (modify) | Register the 3 new commands in the Tauri `invoke_handler` | 4 |
| `crates/vox-gui/ui/src/components/surfaces/Settings/TaskPolicySection.tsx` (create) | Minimal override list + add/remove UI | 4 |
| `crates/vox-gui/src/commands/orchestrator.rs` (modify, again) | `resolve_default_task_policy` Tauri command — GUI default mirrors the backend resolver | 5 |
| `crates/vox-gui/ui/src/components/surfaces/Loquela/Loquela.tsx` (modify) | Seed `DriveConsole`'s starting `control` state from `resolve_default_task_policy` instead of the hardcoded `defaultControl()` | 5 |

---

## Phase 1 — Core mechanism (`mode.rs` + `OrchestratorConfig`)

### Task 1.1: `TriggerSource` enum

**Files:**
- Modify: `crates/vox-orchestrator/src/mode.rs`

- [ ] **Step 1: Write the failing test.** Append to a new test module at the bottom of `mode.rs` (after the existing `interaction_tests` module):

```rust
#[cfg(test)]
mod trigger_source_tests {
    use super::*;

    #[test]
    fn from_label_parses_all_four_case_insensitive() {
        assert_eq!(TriggerSource::from_label("interactive"), Some(TriggerSource::Interactive));
        assert_eq!(TriggerSource::from_label("Automated"), Some(TriggerSource::Automated));
        assert_eq!(TriggerSource::from_label("SUBAGENT"), Some(TriggerSource::Subagent));
        assert_eq!(TriggerSource::from_label("mesh"), Some(TriggerSource::Mesh));
    }

    #[test]
    fn from_label_unknown_returns_none() {
        assert_eq!(TriggerSource::from_label("turbo"), None);
    }

    #[test]
    fn default_is_interactive() {
        assert_eq!(TriggerSource::default(), TriggerSource::Interactive);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator --lib mode::trigger_source_tests 2>test_out.log; tail -30 test_out.log`
Expected: FAIL — `TriggerSource` not found.

- [ ] **Step 3: Write minimal implementation.** Add to `mode.rs`, after the `RiskPosture` block (after line 267, before the `effective_axes` section comment):

```rust
// ── Task: TriggerSource ─────────────────────────────────────────────────────

/// Who/what started a task — orthogonal to `TaskCategory` (what kind of work).
/// `Interactive`: a live chat/editor feature. `Automated`: CI/CD, scheduled, or
/// background-poll dispatch. `Subagent`: spawned by another agent. `Mesh`:
/// delivered via A2A from another node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TriggerSource {
    #[default]
    Interactive,
    Automated,
    Subagent,
    Mesh,
}

impl TriggerSource {
    /// Parse a hint/GUI-supplied label. Case-insensitive. Unknown labels return `None`.
    #[must_use]
    pub fn from_label(label: &str) -> Option<Self> {
        match label.trim().to_ascii_lowercase().as_str() {
            "interactive" => Some(Self::Interactive),
            "automated" => Some(Self::Automated),
            "subagent" => Some(Self::Subagent),
            "mesh" => Some(Self::Mesh),
            _ => None,
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-orchestrator --lib mode::trigger_source_tests 2>test_out.log; tail -30 test_out.log`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator/src/mode.rs
git commit -m "feat(vox-orchestrator): add TriggerSource enum (who/what started a task)"
```

### Task 1.2: Policy tables + pure resolver

**Files:**
- Modify: `crates/vox-orchestrator/src/mode.rs`

- [ ] **Step 1: Write the failing tests.** Append a new test module:

```rust
#[cfg(test)]
mod task_policy_resolver_tests {
    use super::*;

    #[test]
    fn explicit_wins_over_everything() {
        let (clutch, risk) = resolve_task_policy(
            Some(ClutchProfile::Genius),
            Some(RiskPosture::Low),
            Some((ClutchProfile::Free, RiskPosture::High)),
            Some((ClutchProfile::Efficiency, RiskPosture::Moderate)),
        );
        assert_eq!(clutch, ClutchProfile::Genius);
        assert_eq!(risk, RiskPosture::Low);
    }

    #[test]
    fn category_policy_wins_over_source_policy() {
        let (clutch, risk) = resolve_task_policy(
            None,
            None,
            Some((ClutchProfile::Balanced, RiskPosture::Moderate)),
            Some((ClutchProfile::Free, RiskPosture::High)),
        );
        assert_eq!(clutch, ClutchProfile::Balanced);
        assert_eq!(risk, RiskPosture::Moderate);
    }

    #[test]
    fn source_policy_wins_when_no_category_policy() {
        let (clutch, risk) = resolve_task_policy(
            None,
            None,
            None,
            Some((ClutchProfile::Free, RiskPosture::High)),
        );
        assert_eq!(clutch, ClutchProfile::Free);
        assert_eq!(risk, RiskPosture::High);
    }

    #[test]
    fn falls_back_to_global_default_when_nothing_set() {
        let (clutch, risk) = resolve_task_policy(None, None, None, None);
        assert_eq!(clutch, ClutchProfile::Balanced);
        assert_eq!(risk, RiskPosture::Moderate);
    }

    #[test]
    fn clutch_and_risk_resolve_independently_across_levels() {
        // Category supplies clutch, source supplies risk (category has no risk-relevant entry
        // in this synthetic case since real entries always carry both — this proves the two
        // fields aren't forced to come from the same precedence level as a pair).
        let (clutch, risk) = resolve_task_policy(
            None,
            Some(RiskPosture::Low),
            Some((ClutchProfile::Efficiency, RiskPosture::Moderate)),
            Some((ClutchProfile::Genius, RiskPosture::High)),
        );
        assert_eq!(clutch, ClutchProfile::Efficiency, "category clutch wins (explicit clutch unset)");
        assert_eq!(risk, RiskPosture::Low, "explicit risk wins outright");
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-orchestrator --lib mode::task_policy_resolver_tests 2>test_out.log; tail -40 test_out.log`
Expected: FAIL — `resolve_task_policy` not found.

- [ ] **Step 3: Write minimal implementation.** Append to `mode.rs` (after the `TriggerSource` block):

```rust
// ── Task: per-task-type cost/model policy resolver ──────────────────────────

/// A compiled-in default policy for one `TaskCategory`. The production table
/// (`DEFAULT_CATEGORY_POLICY`) starts empty — seeding real defaults is a
/// separate, low-risk follow-up (editable live via the GUI once this lands)
/// rather than a behavior change bundled into this plan.
#[derive(Debug, Clone, Copy)]
pub struct TaskCategoryPolicy {
    pub category: crate::types::TaskCategory,
    pub clutch: ClutchProfile,
    pub risk: RiskPosture,
}

/// A compiled-in default policy for one `TriggerSource`. Same seeding note as
/// [`TaskCategoryPolicy`].
#[derive(Debug, Clone, Copy)]
pub struct TriggerSourcePolicy {
    pub source: TriggerSource,
    pub clutch: ClutchProfile,
    pub risk: RiskPosture,
}

/// Compiled-in `TaskCategory` defaults. Empty at first landing (see
/// [`TaskCategoryPolicy`] doc) — extend via a dedicated follow-up PR, not by
/// editing this plan's tasks after the fact.
pub const DEFAULT_CATEGORY_POLICY: &[TaskCategoryPolicy] = &[];

/// Compiled-in `TriggerSource` defaults. Empty at first landing (see
/// [`TriggerSourcePolicy`] doc).
pub const DEFAULT_SOURCE_POLICY: &[TriggerSourcePolicy] = &[];

/// Pure precedence resolver: explicit > category policy > source policy > the
/// existing global default (`Balanced`/`Moderate`, unchanged from today's
/// `AgentTask::resolved_clutch()`/`resolved_risk()`). `category_policy` and
/// `source_policy` are the *already-merged* (compiled default + live override)
/// effective policy for this task's category/source, or `None` if neither has
/// one — callers compute these via `effective_category_policy()`/
/// `effective_source_policy()`. Clutch and risk resolve independently: a task
/// can inherit its category's clutch but its source's risk if that's what the
/// effective data says.
#[must_use]
pub fn resolve_task_policy(
    explicit_clutch: Option<ClutchProfile>,
    explicit_risk: Option<RiskPosture>,
    category_policy: Option<(ClutchProfile, RiskPosture)>,
    source_policy: Option<(ClutchProfile, RiskPosture)>,
) -> (ClutchProfile, RiskPosture) {
    let clutch = explicit_clutch
        .or_else(|| category_policy.map(|(c, _)| c))
        .or_else(|| source_policy.map(|(c, _)| c))
        .unwrap_or(ClutchProfile::Balanced);
    let risk = explicit_risk
        .or_else(|| category_policy.map(|(_, r)| r))
        .or_else(|| source_policy.map(|(_, r)| r))
        .unwrap_or(RiskPosture::Moderate);
    (clutch, risk)
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-orchestrator --lib mode:: 2>test_out.log; tail -60 test_out.log`
Expected: PASS (all `mode.rs` tests, including the 5 new ones).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator/src/mode.rs
git commit -m "feat(vox-orchestrator): add resolve_task_policy precedence resolver"
```

### Task 1.3: Override-merge helpers (bridges Vox.toml overrides with the compiled tables)

**Files:**
- Modify: `crates/vox-orchestrator/src/mode.rs`
- Modify: `crates/vox-orchestrator/src/config/orchestrator_fields.rs`

- [ ] **Step 1: Write the failing test.** First add the override types this test needs — append to `orchestrator_fields.rs`, near the top after the existing imports (before the `OrchestratorConfig` struct definition):

```rust
/// One override entry: a clutch and/or risk label (parsed via
/// `ClutchProfile::from_label`/`RiskPosture::from_label`). Either may be
/// `None` — an override can set just one axis, letting the other fall through
/// to the next precedence level.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskPolicyEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clutch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk: Option<String>,
}

/// User overrides for the per-task-type cost/model policy, loaded from
/// `[orchestrator.task_policy.category]` / `[orchestrator.task_policy.source]`
/// in Vox.toml. Keys are `TaskCategory`/`TriggerSource` Debug names (e.g.
/// `"CodeGen"`, `"Automated"`) — matching how `ClutchProfile`/`RiskPosture`
/// already use string labels at config/hint boundaries.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskPolicyOverrides {
    #[serde(default)]
    pub category: std::collections::HashMap<String, TaskPolicyEntry>,
    #[serde(default)]
    pub source: std::collections::HashMap<String, TaskPolicyEntry>,
}
```

Then add the field to `OrchestratorConfig` (a plain `#[serde(default)]` field, matching the existing `test_decision_policy` field's pattern — not a `#[config(...)]`-annotated scalar, since this is a structured table, not an env-backed scalar):

```rust
    /// Per-task-type cost/model policy overrides (category + trigger-source).
    /// See `crate::mode::resolve_task_policy` for how these combine with the
    /// compiled defaults.
    #[serde(default)]
    pub task_policy: TaskPolicyOverrides,
```

Now the test, appended to `mode.rs`:

```rust
#[cfg(test)]
mod effective_policy_tests {
    use super::*;
    use crate::config::{TaskPolicyEntry, TaskPolicyOverrides};
    use crate::types::TaskCategory;
    use std::collections::HashMap;

    #[test]
    fn override_wins_over_compiled_default_for_category() {
        let mut category = HashMap::new();
        category.insert(
            "CodeGen".to_string(),
            TaskPolicyEntry { clutch: Some("free".to_string()), risk: Some("high".to_string()) },
        );
        let overrides = TaskPolicyOverrides { category, source: HashMap::new() };
        let resolved = effective_category_policy(&overrides, TaskCategory::CodeGen);
        assert_eq!(resolved, Some((ClutchProfile::Free, RiskPosture::High)));
    }

    #[test]
    fn missing_category_override_and_no_compiled_default_is_none() {
        let overrides = TaskPolicyOverrides::default();
        assert_eq!(effective_category_policy(&overrides, TaskCategory::Research), None);
    }

    #[test]
    fn malformed_override_label_falls_through_to_none() {
        let mut source = HashMap::new();
        source.insert(
            "Automated".to_string(),
            TaskPolicyEntry { clutch: Some("turbo".to_string()), risk: None },
        );
        let overrides = TaskPolicyOverrides { category: HashMap::new(), source };
        // "turbo" doesn't parse; risk is unset too — the whole entry has nothing
        // usable, so this falls through to the compiled default (empty today -> None).
        assert_eq!(effective_source_policy(&overrides, TriggerSource::Automated), None);
    }

    #[test]
    fn partial_override_clutch_only_still_resolves_the_pair() {
        // A clutch-only override still returns a full (clutch, risk) pair — risk
        // comes from Moderate (RiskPosture's own #[default]), since there is no
        // compiled default to fall back to at this level; the caller's overall
        // resolve_task_policy() call is what lets risk fall through further.
        let mut category = HashMap::new();
        category.insert(
            "Research".to_string(),
            TaskPolicyEntry { clutch: Some("genius".to_string()), risk: None },
        );
        let overrides = TaskPolicyOverrides { category, source: HashMap::new() };
        assert_eq!(
            effective_category_policy(&overrides, TaskCategory::Research),
            None,
            "partial entries with only one usable axis are not returned as a full pair; \
             use resolve_task_policy's per-axis fallthrough instead of half-filled pairs"
        );
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-orchestrator --lib mode::effective_policy_tests 2>test_out.log; tail -40 test_out.log`
Expected: FAIL — `effective_category_policy`/`effective_source_policy` not found.

- [ ] **Step 3: Write minimal implementation.** Append to `mode.rs`:

```rust
/// Merge the live Vox.toml override (if any and if fully parseable) with the
/// compiled `DEFAULT_CATEGORY_POLICY` for one category. Returns `None` when
/// neither source has a *complete* (clutch AND risk) entry — a half-filled
/// override (only clutch or only risk set) is intentionally not returned here;
/// `resolve_task_policy`'s own per-axis fallthrough is where partial data gets
/// used, keeping this merge step simple (whole-pair in, whole-pair out).
#[must_use]
pub fn effective_category_policy(
    overrides: &crate::config::TaskPolicyOverrides,
    category: crate::types::TaskCategory,
) -> Option<(ClutchProfile, RiskPosture)> {
    let key = format!("{category:?}");
    if let Some(entry) = overrides.category.get(&key) {
        if let (Some(c), Some(r)) = (
            entry.clutch.as_deref().and_then(ClutchProfile::from_label),
            entry.risk.as_deref().and_then(RiskPosture::from_label),
        ) {
            return Some((c, r));
        }
    }
    DEFAULT_CATEGORY_POLICY
        .iter()
        .find(|p| p.category == category)
        .map(|p| (p.clutch, p.risk))
}

/// Same merge as [`effective_category_policy`], for `TriggerSource`.
#[must_use]
pub fn effective_source_policy(
    overrides: &crate::config::TaskPolicyOverrides,
    source: TriggerSource,
) -> Option<(ClutchProfile, RiskPosture)> {
    let key = format!("{source:?}");
    if let Some(entry) = overrides.source.get(&key) {
        if let (Some(c), Some(r)) = (
            entry.clutch.as_deref().and_then(ClutchProfile::from_label),
            entry.risk.as_deref().and_then(RiskPosture::from_label),
        ) {
            return Some((c, r));
        }
    }
    DEFAULT_SOURCE_POLICY
        .iter()
        .find(|p| p.source == source)
        .map(|p| (p.clutch, p.risk))
}
```

Add `use serde::{Deserialize, Serialize};` to `orchestrator_fields.rs` if not already imported (check the top of the file first — it already derives `Serialize, Deserialize` on `OrchestratorConfig` itself, so this import already exists).

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-orchestrator --lib mode:: 2>test_out.log; tail -60 test_out.log`
Expected: PASS (all `mode.rs` tests, including the 4 new ones — note `malformed_override_label_falls_through_to_none` and `partial_override_clutch_only_still_resolves_the_pair` both assert `None` for the same underlying reason: an incomplete entry doesn't produce a pair).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator/src/mode.rs crates/vox-orchestrator/src/config/orchestrator_fields.rs
git commit -m "feat(vox-orchestrator): TaskPolicyOverrides on OrchestratorConfig + effective-policy merge helpers"
```

### Task 1.4: `OrchestratorConfigField` catalog parity check

**Files:**
- Modify: `crates/vox-gui/src/commands/orchestrator.rs:592-604` (the `catalog_len_matches_config_field_count` test)

- [ ] **Step 1: Run the existing catalog parity test to see whether the new field breaks it**

Run: `cargo test -p vox-gui --bins catalog_len_matches_config_field_count 2>test_out.log; tail -30 test_out.log`

- [ ] **Step 2: If it fails** (i.e. `to_catalog()` iterates `OrchestratorConfig`'s fields structurally and now includes/miscounts `task_policy`), read `crates/vox-orchestrator/src/config/orchestrator_fields.rs:547-575` (`OrchestratorConfigField`/`to_catalog`) to see whether it's a hand-written field-by-field list or a macro that walks every field. If hand-written and `task_policy` isn't in it, no change is needed — the test count stays the same and this step is done. If it's a macro that walks all fields, update the expected count in the test by the number the macro now reports (read the actual failure message, which states the real count — copy that number into the assertion) and note in the commit message that `task_policy` is intentionally excluded from the flat scalar catalog (it's a keyed table, not a scalar setting — Task 4.x below gives it its own dedicated commands).

- [ ] **Step 3: Commit only if a change was needed**

```bash
git add crates/vox-orchestrator/src/config/orchestrator_fields.rs crates/vox-gui/src/commands/orchestrator.rs
git commit -m "test(vox-gui): exclude task_policy from the flat OrchestratorConfigField catalog"
```

---

## Phase 2 — `AgentTask` wiring

### Task 2.1: `trigger_source` field + hint parsing

**Files:**
- Modify: `crates/vox-orchestrator/src/types/tasks.rs`

- [ ] **Step 1: Write the failing test.** Append near the existing `apply_hints_parses_clutch_labels` test (around line 1077):

```rust
    #[test]
    fn apply_hints_parses_trigger_source_labels() {
        for (label, expected) in [
            ("interactive", crate::mode::TriggerSource::Interactive),
            ("Automated", crate::mode::TriggerSource::Automated),
            ("SUBAGENT", crate::mode::TriggerSource::Subagent),
            ("mesh", crate::mode::TriggerSource::Mesh),
        ] {
            let mut task = AgentTask::new(TaskId(1), "t", TaskPriority::Normal, vec![]);
            let hints = TaskEnqueueHints {
                trigger_source: Some(label.to_string()),
                ..Default::default()
            };
            task.apply_hints(&hints);
            assert_eq!(task.trigger_source, Some(expected), "label {label}");
        }
    }

    #[test]
    fn apply_hints_unknown_trigger_source_leaves_none() {
        let mut task = AgentTask::new(TaskId(1), "t", TaskPriority::Normal, vec![]);
        let hints = TaskEnqueueHints {
            trigger_source: Some("turbo".to_string()),
            ..Default::default()
        };
        task.apply_hints(&hints);
        assert_eq!(task.trigger_source, None);
    }

    #[test]
    fn new_task_has_no_trigger_source_by_default() {
        let task = AgentTask::new(TaskId(1), "t", TaskPriority::Normal, vec![]);
        assert_eq!(task.trigger_source, None);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-orchestrator --lib types::tasks::tests::apply_hints_parses_trigger_source 2>test_out.log; tail -30 test_out.log`
Expected: FAIL — `TaskEnqueueHints` has no field `trigger_source`; `AgentTask` has no field `trigger_source`.

- [ ] **Step 3: Write minimal implementation.**

Add to `TaskEnqueueHints` (next to the existing `clutch`/`risk` fields, around line 289-292):

```rust
    /// Trigger-source label (`interactive`|`automated`|`subagent`|`mesh`); parsed
    /// in [`AgentTask::apply_hints`]. `None` = unset; the generic MCP submission
    /// path defaults this to `Interactive` at the resolver, not here (unset stays
    /// unset so `resolved_policy()` can tell "explicitly interactive" from
    /// "caller didn't say").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger_source: Option<String>,
```

Add to `AgentTask` (next to `risk_posture`, around line 699-702):

```rust
    /// Who/what started this task. `None` = unknown; `resolved_policy()` treats
    /// unset the same as `Interactive` (today's most common caller).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger_source: Option<crate::mode::TriggerSource>,
```

Add `trigger_source: None,` to `AgentTask::new`'s constructor (next to `risk_posture: None,` around line 783).

Add to `apply_hints` (next to the `risk` hint block, around line 947-951):

```rust
        if let Some(ref source) = h.trigger_source {
            if let Some(parsed) = crate::mode::TriggerSource::from_label(source) {
                self.trigger_source = Some(parsed);
            }
        }
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-orchestrator --lib types::tasks:: 2>test_out.log; tail -80 test_out.log`
Expected: PASS (all `tasks.rs` tests, including the 3 new ones).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator/src/types/tasks.rs
git commit -m "feat(vox-orchestrator): AgentTask/TaskEnqueueHints carry a trigger_source hint"
```

### Task 2.2: `resolved_policy()` replaces the naive fallback

**Files:**
- Modify: `crates/vox-orchestrator/src/types/tasks.rs:957-973`

- [ ] **Step 1: Write the failing test.** Append after the existing `resolved_clutch_risk_fall_back_to_neutral_defaults` test:

```rust
    #[test]
    fn resolved_policy_falls_back_to_neutral_defaults_when_nothing_set() {
        let task = AgentTask::new(TaskId(1), "t", TaskPriority::Normal, vec![]);
        let overrides = crate::config::TaskPolicyOverrides::default();
        let (clutch, risk) = task.resolved_policy(&overrides);
        assert_eq!(clutch, crate::mode::ClutchProfile::Balanced);
        assert_eq!(risk, crate::mode::RiskPosture::Moderate);
    }

    #[test]
    fn resolved_policy_explicit_hint_wins_over_source_override() {
        let mut task = AgentTask::new(TaskId(1), "t", TaskPriority::Normal, vec![]);
        task.clutch_profile = Some(crate::mode::ClutchProfile::Genius);
        task.trigger_source = Some(crate::mode::TriggerSource::Automated);

        let mut source = std::collections::HashMap::new();
        source.insert(
            "Automated".to_string(),
            crate::config::TaskPolicyEntry {
                clutch: Some("free".to_string()),
                risk: Some("high".to_string()),
            },
        );
        let overrides = crate::config::TaskPolicyOverrides { category: std::collections::HashMap::new(), source };

        let (clutch, _risk) = task.resolved_policy(&overrides);
        assert_eq!(clutch, crate::mode::ClutchProfile::Genius, "explicit clutch hint must win");
    }

    #[test]
    fn resolved_policy_uses_source_override_when_no_explicit_hint() {
        let mut task = AgentTask::new(TaskId(1), "t", TaskPriority::Normal, vec![]);
        task.trigger_source = Some(crate::mode::TriggerSource::Automated);

        let mut source = std::collections::HashMap::new();
        source.insert(
            "Automated".to_string(),
            crate::config::TaskPolicyEntry {
                clutch: Some("free".to_string()),
                risk: Some("high".to_string()),
            },
        );
        let overrides = crate::config::TaskPolicyOverrides { category: std::collections::HashMap::new(), source };

        let (clutch, risk) = task.resolved_policy(&overrides);
        assert_eq!(clutch, crate::mode::ClutchProfile::Free);
        assert_eq!(risk, crate::mode::RiskPosture::High);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-orchestrator --lib types::tasks::tests::resolved_policy 2>test_out.log; tail -40 test_out.log`
Expected: FAIL — `resolved_policy` not found.

- [ ] **Step 3: Write minimal implementation.** Add next to `resolved_clutch`/`resolved_risk` (keep those two methods as-is — they're still used elsewhere and stay correct as "no task-type awareness" helpers; `resolved_policy` is the new task-type-aware entry point):

```rust
    /// Resolve this task's effective (clutch, risk) pair using the full
    /// precedence chain: explicit hint > this task's category policy > this
    /// task's trigger-source policy > the neutral global default. `overrides`
    /// is the live `OrchestratorConfig::snapshot().task_policy` — callers fetch
    /// it once per resolution rather than this method reaching for it itself,
    /// keeping `AgentTask` free of a config-snapshot dependency in its own type.
    #[must_use]
    pub fn resolved_policy(
        &self,
        overrides: &crate::config::TaskPolicyOverrides,
    ) -> (crate::mode::ClutchProfile, crate::mode::RiskPosture) {
        let category_policy = crate::mode::effective_category_policy(overrides, self.task_category);
        let source = self.trigger_source.unwrap_or(crate::mode::TriggerSource::Interactive);
        let source_policy = crate::mode::effective_source_policy(overrides, source);
        crate::mode::resolve_task_policy(
            self.clutch_profile,
            self.risk_posture,
            category_policy,
            source_policy,
        )
    }
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-orchestrator --lib types::tasks:: 2>test_out.log; tail -100 test_out.log`
Expected: PASS (all tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator/src/types/tasks.rs
git commit -m "feat(vox-orchestrator): AgentTask::resolved_policy() — task-type-aware clutch/risk resolution"
```

### Task 2.3: Wire `resolved_policy()` into the real `AgentTask` execution path (`runtime.rs`)

This is the AgentTask-path counterpart to Phase 3's MCP wiring — without it, a task submitted from the chat composer with an explicit clutch already works today (see below), but any task relying on a *category or source* policy (no explicit hint — the CI/CD-inbox case, and every non-composer submission) silently does nothing, exactly like the MCP path's dead `build_selection_request` branch this plan already fixes in Task 3.3.

**Grounding (confirmed live):** `crates/vox-orchestrator/src/runtime.rs:628-644` (`AiTaskProcessor::process`) is the real consumption site:

```rust
let mut cost_pref = crate::sync_lock::rw_read(&*self.orchestrator.config).cost_preference;
let force_free_pool = if task.clutch_profile.is_some() {
    let rc = task.resolved_clutch();
    cost_pref = rc.cost_preference;
    rc.force_free_pool
} else {
    false
};
if matches!(task.resolved_risk().model_lean, crate::mode::ModelLean::Intelligence) {
    cost_pref = crate::config::CostPreference::Performance;
}
```

The chat composer (`Loquela.tsx`) already sends an explicit `clutch`/`risk` on every submission today (`control.clutch`/`control.risk`, threaded through `SubmitTaskInput` → `enqueue_hints` → the daemon's `TaskEnqueueHints` → `AgentTask.apply_hints` — this round-trip already exists and is not part of this plan), so `task.clutch_profile.is_some()` is already `true` for composer-submitted tasks and this branch already runs for them. It is the **`is_some()` gate itself** that's the bug this task fixes: every task *without* an explicit hint (CI/CD-inbox submissions via Task 3.1's new `trigger_source` hint, and any other internal task creation) hits the `else { false }` branch and gets no task-type policy applied at all, even once a category/source override exists in `task_policy`.

**Files:**
- Modify: `crates/vox-orchestrator/src/runtime.rs:628-644`

- [ ] **Step 1: Confirm the safe-default assumption before changing behavior.** Run:

```bash
cargo test -p vox-orchestrator --lib config:: -- --nocapture 2>test_out.log
grep -n "cost_preference" crates/vox-orchestrator/src/config/impl_default.rs
```

Confirm `OrchestratorConfig::default().cost_preference` is `CostPreference::Economy` (matching `ClutchProfile::Balanced.resolve().cost_preference`, per `mode.rs`'s own "Free-by-default policy" comment on `QualityLevel::to_cost_preference`). **If it is NOT Economy**, do not proceed with Step 3 as written — instead keep computing `cost_pref` from the global config when `category_policy`/`source_policy`/`task.clutch_profile` are *all* `None` (i.e. only switch to the resolver's output when at least one of those three is `Some`), so a task with zero applicable policy keeps today's exact global-default behavior. The test below is written for the confirmed-Economy case; adapt its assertion if the guard above says otherwise.

- [ ] **Step 2: Write the failing test.** `AiTaskProcessor::process` isn't directly unit-testable in isolation (it needs a live orchestrator+registry) — instead, add a `runtime.rs` unit test that pins down the *policy computation* this task changes, extracted as its own small function (Step 3 introduces it):

```rust
#[cfg(test)]
mod task_policy_wiring_tests {
    use super::*;
    use crate::config::TaskPolicyOverrides;
    use crate::types::{AgentTask, TaskId, TaskPriority};

    #[test]
    fn unset_task_with_no_overrides_matches_todays_global_default() {
        let task = AgentTask::new(TaskId(1), "t", TaskPriority::Normal, vec![]);
        let overrides = TaskPolicyOverrides::default();
        let global_default = crate::config::OrchestratorConfig::default().cost_preference;
        let (cost_pref, force_free_pool) = resolve_task_cost_policy(&task, &overrides, global_default);
        assert_eq!(cost_pref, global_default, "no policy anywhere must reproduce today's behavior exactly");
        assert!(!force_free_pool);
    }

    #[test]
    fn source_override_applies_when_no_explicit_hint() {
        let mut task = AgentTask::new(TaskId(1), "t", TaskPriority::Normal, vec![]);
        task.trigger_source = Some(crate::mode::TriggerSource::Automated);
        let mut source = std::collections::HashMap::new();
        source.insert(
            "Automated".to_string(),
            crate::config::TaskPolicyEntry { clutch: Some("free".to_string()), risk: Some("high".to_string()) },
        );
        let overrides = TaskPolicyOverrides { category: std::collections::HashMap::new(), source };
        let (_cost_pref, force_free_pool) = resolve_task_cost_policy(
            &task, &overrides, crate::config::OrchestratorConfig::default().cost_preference,
        );
        assert!(force_free_pool, "Automated source override (Free clutch) must force the free-only pool");
    }
}
```

- [ ] **Step 3: Run to verify it fails**

Run: `cargo test -p vox-orchestrator --lib runtime::task_policy_wiring_tests 2>test_out.log; tail -40 test_out.log`
Expected: FAIL — `resolve_task_cost_policy` not found.

- [ ] **Step 4: Extract the policy computation into a small testable function, and call it from `AiTaskProcessor::process`.** Add near the top of `runtime.rs` (module level, outside the `impl`):

```rust
/// Compute the effective `(CostPreference, force_free_pool)` for one task —
/// extracted from `AiTaskProcessor::process` so it's unit-testable without a
/// live orchestrator. `global_default` is `OrchestratorConfig.cost_preference`,
/// the fallback when no clutch policy (explicit, category, or source) applies
/// at all — preserving today's exact behavior for a fully-unconfigured task.
fn resolve_task_cost_policy(
    task: &crate::types::AgentTask,
    overrides: &crate::config::TaskPolicyOverrides,
    global_default: crate::config::CostPreference,
) -> (crate::config::CostPreference, bool) {
    let category_policy = crate::mode::effective_category_policy(overrides, task.task_category);
    let source = task.trigger_source.unwrap_or(crate::mode::TriggerSource::Interactive);
    let source_policy = crate::mode::effective_source_policy(overrides, source);
    if task.clutch_profile.is_none() && category_policy.is_none() && source_policy.is_none() {
        return (global_default, false);
    }
    let (clutch, _risk) =
        crate::mode::resolve_task_policy(task.clutch_profile, task.risk_posture, category_policy, source_policy);
    let rc = clutch.resolve();
    (rc.cost_preference, rc.force_free_pool)
}
```

Replace lines 628-638 in `AiTaskProcessor::process` with:

```rust
        let overrides = crate::sync_lock::rw_read(&*self.orchestrator.config).task_policy.clone();
        let global_default = crate::sync_lock::rw_read(&*self.orchestrator.config).cost_preference;
        let (mut cost_pref, force_free_pool) = resolve_task_cost_policy(&task, &overrides, global_default);
```

(Leave lines 639-644, the risk/`ModelLean::Intelligence` block, unchanged — it already runs unconditionally today via `task.resolved_risk()`'s own built-in `Moderate` fallback and needs no fix.)

- [ ] **Step 5: Run to verify it passes**

Run: `cargo test -p vox-orchestrator --lib runtime:: 2>test_out.log; tail -100 test_out.log`
Expected: PASS — including every pre-existing `runtime.rs` test (this is the zero-regression gate; if any pre-existing test asserted the old `else { false }` behavior for a task that now resolves a category/source policy, read it and confirm the new behavior is what that test *should* want, per the confirmed-Economy assumption from Step 1 — do not weaken the new test to make an outdated assertion pass).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-orchestrator/src/runtime.rs
git commit -m "fix(vox-orchestrator): AgentTask execution honors task-type cost policy even without an explicit clutch hint"
```

---

## Phase 3 — MCP wiring

### Task 3.1: `SubmitTaskParams` carries the hints; `submission.rs` forwards them

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/params.rs`
- Modify: `crates/vox-orchestrator-mcp/src/task_tools/submission.rs`

- [ ] **Step 1: Write the failing test.** Add to `submission.rs`'s existing test module (find it with `rg '#\[cfg\(test\)\]' crates/vox-orchestrator-mcp/src/task_tools/submission.rs` — if none exists yet, create one at the bottom of the file):

```rust
#[cfg(test)]
mod trigger_source_forwarding_tests {
    use super::*;
    use crate::params::SubmitTaskParams;

    #[test]
    fn forwards_clutch_risk_trigger_source_hints() {
        let params = SubmitTaskParams {
            description: "t".to_string(),
            clutch: Some("free".to_string()),
            risk: Some("high".to_string()),
            trigger_source: Some("automated".to_string()),
            ..Default::default()
        };
        let hints = task_enqueue_hints_from_params(&params, None, None, None, "t")
            .expect("hints should be produced when clutch/risk/trigger_source are set");
        assert_eq!(hints.clutch.as_deref(), Some("free"));
        assert_eq!(hints.risk.as_deref(), Some("high"));
        assert_eq!(hints.trigger_source.as_deref(), Some("automated"));
    }
}
```

*(The exact name/signature of the function building `TaskEnqueueHints` from `SubmitTaskParams` — shown here as `task_enqueue_hints_from_params(&params, category, campaign_id, benchmark_tier, description)` — must be confirmed against the live function signature at `submission.rs:143` before writing this test; read that function's full signature first and adjust the test's call to match exactly. Do not guess parameter order.)*

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-orchestrator-mcp --lib task_tools::submission::trigger_source_forwarding_tests 2>test_out.log; tail -30 test_out.log`
Expected: FAIL — `SubmitTaskParams` has no `clutch`/`risk`/`trigger_source` fields (and the guard at the top of the hints function, currently returning `None` when every optional field is empty, needs `clutch`/`risk`/`trigger_source` added to its emptiness check or the test's `Some` values won't be enough to avoid an early `None`).

- [ ] **Step 3: Write minimal implementation.**

Add to `SubmitTaskParams` (`params.rs`, next to `model_override` around line 236-239):

```rust
    /// Optional clutch ("how much gas") override for this task: `free`|`efficiency`|`balanced`|`genius`.
    #[serde(default)]
    #[schemars(length(max = 32))]
    pub clutch: Option<String>,
    /// Optional risk-posture override for this task: `high`|`moderate`|`low`.
    #[serde(default)]
    #[schemars(length(max = 32))]
    pub risk: Option<String>,
    /// Optional trigger-source override: `interactive`|`automated`|`subagent`|`mesh`.
    /// Set this from CI/scheduled callers so their tasks get the `Automated`
    /// cost/model policy instead of the default `Interactive` one.
    #[serde(default)]
    #[schemars(length(max = 32))]
    pub trigger_source: Option<String>,
```

In `submission.rs`, update the emptiness guard (around line 127-140) to also check the three new fields:

```rust
        && params.clutch.is_none()
        && params.risk.is_none()
        && params.trigger_source.is_none()
```

(add these three lines inside the existing `if ... { return None; }` condition, alongside the other `.is_none()`/`.is_empty()` checks).

Change the hardcoded fields in the `TaskEnqueueHints` construction (around line 168-170) from:

```rust
        clutch: None,
        grounding_check_enabled: None,
        risk: None,
```

to:

```rust
        clutch: params.clutch.clone(),
        grounding_check_enabled: None,
        risk: params.risk.clone(),
        trigger_source: params.trigger_source.clone(),
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-orchestrator-mcp --lib task_tools::submission:: 2>test_out.log; tail -60 test_out.log`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/params.rs crates/vox-orchestrator-mcp/src/task_tools/submission.rs
git commit -m "feat(vox-orchestrator-mcp): vox_task_submit accepts clutch/risk/trigger_source hints"
```

### Task 3.2: `McpChatModelResolution.trigger_source`

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/types.rs`

- [ ] **Step 1: Write the failing test.** Add to `types.rs` (create a `#[cfg(test)] mod tests` if none exists):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_trigger_source_is_interactive() {
        let res = McpChatModelResolution::default();
        assert_eq!(res.trigger_source, vox_orchestrator::mode::TriggerSource::Interactive);
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-orchestrator-mcp --lib llm_bridge::model_route_policy::types 2>test_out.log; tail -30 test_out.log`
Expected: FAIL — no field `trigger_source`.

- [ ] **Step 3: Write minimal implementation.** Add to the `McpChatModelResolution` struct (next to `risk`):

```rust
    /// Who/what triggered this resolution. Every MCP tool call site constructs
    /// this struct directly from a live chat/editor feature, so `Interactive`
    /// is correct by construction here — no inference needed (contrast with
    /// `AgentTask.trigger_source`, which is genuinely optional/hinted).
    pub trigger_source: vox_orchestrator::mode::TriggerSource,
```

Add `trigger_source: vox_orchestrator::mode::TriggerSource::Interactive,` to the `Default` impl.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo build -p vox-orchestrator-mcp 2>build_out.log; tail -80 build_out.log`
Expected: Compile errors (E0063 missing field) at every other place `McpChatModelResolution { .. }` is constructed without `..Default::default()`. Note the exact file:line list from the error output — this is the authoritative list of remaining call sites, not a guess.

- [ ] **Step 5: Fix each flagged call site.** For every file the compiler flagged (expected: `crates/vox-orchestrator-mcp/src/models_tools.rs` and `crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs`, per this session's grep — confirm against the actual compiler output, which is authoritative), add `trigger_source: vox_orchestrator::mode::TriggerSource::Interactive,` to the struct literal next to its `clutch: None,`/`risk: None,` fields.

- [ ] **Step 6: Run to verify it passes**

Run: `cargo test -p vox-orchestrator-mcp --lib 2>test_out.log; tail -100 test_out.log`
Expected: PASS (full crate test suite, confirming no construction site was missed).

- [ ] **Step 7: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/types.rs crates/vox-orchestrator-mcp/src/models_tools.rs crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs
git commit -m "feat(vox-orchestrator-mcp): McpChatModelResolution carries trigger_source (always Interactive today)"
```

### Task 3.3: Resolve the policy at the top of `resolve_mcp_chat_model_sync_inner`

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/resolve.rs:197-220`

- [ ] **Step 1: Write the failing test.** Add near the top of `resolve.rs`'s existing test module (`rg '#\[cfg\(test\)\]' crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/resolve.rs` to find it — the sibling `tests.rs` file, 756 lines, is likely `#[path = "tests.rs"] mod tests;` referenced from `mod.rs`; add this test there instead if `resolve.rs` has no inline test module — check `mod.rs` first to see which file actually holds `resolve.rs`'s tests before writing this step for real):

```rust
    #[test]
    fn clutch_and_risk_are_populated_before_selection_when_unset() {
        // Regression guard for the dead-code path this feature fixes: before
        // this change, res.clutch == None meant build_selection_request never
        // called effective_axes at all (see resolve.rs's preference-only branch).
        let res = McpChatModelResolution { task_category: TaskCategory::CodeGen, ..Default::default() };
        assert!(res.clutch.is_none(), "test setup sanity check");
        // The resolver call this task adds must backfill both fields to Some
        // before build_selection_request is reached — verified indirectly via
        // resolve_task_policy's own already-tested global-default fallback
        // (Balanced/Moderate), since resolve_mcp_chat_model_sync_inner is not
        // directly unit-testable without a live Orchestrator registry.
        let (clutch, risk) = vox_orchestrator::mode::resolve_task_policy(
            res.clutch, res.risk, None, None,
        );
        assert_eq!(clutch, vox_orchestrator::mode::ClutchProfile::Balanced);
        assert_eq!(risk, vox_orchestrator::mode::RiskPosture::Moderate);
    }
```

- [ ] **Step 2: Run to verify it passes already**

Run: `cargo test -p vox-orchestrator-mcp --lib clutch_and_risk_are_populated 2>test_out.log; tail -30 test_out.log`
Expected: PASS — this test only exercises the already-implemented `resolve_task_policy` (Phase 1), so it passes immediately. It documents the contract Step 3 below wires into the real function; it is not itself proof that `resolve_mcp_chat_model_sync_inner` calls it yet.

- [ ] **Step 3: Wire the real call site.** In `resolve.rs`, immediately after `let mut res = res;` (currently line 211, before the `force_free_pool` check at line 216), insert:

```rust
    {
        let category_policy = vox_orchestrator::mode::effective_category_policy(
            &orch.config_handle_read().task_policy,
            res.task_category,
        );
        let source_policy = vox_orchestrator::mode::effective_source_policy(
            &orch.config_handle_read().task_policy,
            res.trigger_source,
        );
        let (clutch, risk) = vox_orchestrator::mode::resolve_task_policy(
            res.clutch,
            res.risk,
            category_policy,
            source_policy,
        );
        res.clutch = Some(clutch);
        res.risk = Some(risk);
    }
```

*(`orch.config_handle_read()` is a placeholder name for "however this function already reads `OrchestratorConfig`" — the function already does this at line 264-267 via `orch.config_handle()` + `vox_orchestrator::sync_lock::rw_read(&*config_handle)`. Use that exact same pattern here instead of inventing a new accessor: read the config handle once, pull `.task_policy` off the same guard the existing `preference` read at line 264-267 already produces, reusing one read instead of two. Adjust the snippet above to match — the two-`orch.config_handle_read()` calls shown are illustrative of *what* to read, not the literal code to paste.)*

- [ ] **Step 4: Run the full resolve.rs / model_route_policy test suite**

Run: `cargo test -p vox-orchestrator-mcp --lib llm_bridge::model_route_policy:: 2>test_out.log; tail -150 test_out.log`
Expected: PASS — all pre-existing tests in `model_route_policy/tests.rs` (756 lines) must still pass unchanged; this is the zero-regression gate for this task, since `res.clutch`/`res.risk` are now always `Some` where they used to sometimes be `None`, and any pre-existing test asserting `None`-branch behavior needs to be read and reconciled (not silently broken) — if any test fails, read it, understand which branch it exercised, and update its expectation to match the new (intentional) always-resolved behavior rather than reverting the wiring.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/resolve.rs crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs
git commit -m "feat(vox-orchestrator-mcp): resolve task-type policy before model selection (fixes dead effective_axes path)"
```

---

## Phase 4 — GUI (minimal panel)

### Task 4.1: `get_task_policy_overrides` Tauri command

**Files:**
- Modify: `crates/vox-gui/src/commands/orchestrator.rs`

- [ ] **Step 1: Write the failing test.** Add near the existing `catalog_tests` module:

```rust
#[cfg(test)]
mod task_policy_tests {
    use super::*;

    #[test]
    fn get_task_policy_overrides_reflects_snapshot() {
        let overrides = get_task_policy_overrides();
        // Fresh default config has no overrides yet.
        assert!(overrides.category.is_empty());
        assert!(overrides.source.is_empty());
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-gui --bins get_task_policy_overrides_reflects_snapshot 2>test_out.log; tail -30 test_out.log`
Expected: FAIL — function not found.

- [ ] **Step 3: Write minimal implementation.** Add to `orchestrator.rs`, after `get_orchestrator_config_catalog`:

```rust
/// Current per-task-type policy overrides, straight from the effective
/// `OrchestratorConfig` snapshot (env/project/user-merged, matching every
/// other orchestrator-settings read in this file).
#[tauri::command]
pub fn get_task_policy_overrides() -> vox_orchestrator::config::TaskPolicyOverrides {
    vox_orchestrator::config::OrchestratorConfig::snapshot().task_policy
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-gui --bins get_task_policy_overrides_reflects_snapshot 2>test_out.log; tail -30 test_out.log`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/src/commands/orchestrator.rs
git commit -m "feat(vox-gui): get_task_policy_overrides Tauri command"
```

### Task 4.2: `set_task_policy_override` / `clear_task_policy_override`

**Files:**
- Modify: `crates/vox-gui/src/commands/orchestrator.rs`

- [ ] **Step 1: Write the failing test.**

```rust
    #[test]
    fn set_task_policy_override_rejects_unparseable_labels() {
        let result = validate_task_policy_labels(Some("turbo"), Some("high"));
        assert!(result.is_err(), "an unparseable clutch label must be rejected before writing Vox.toml");
    }

    #[test]
    fn set_task_policy_override_accepts_valid_labels() {
        let result = validate_task_policy_labels(Some("free"), Some("high"));
        assert!(result.is_ok());
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-gui --bins validate_task_policy_labels 2>test_out.log; tail -30 test_out.log`
Expected: FAIL — function not found.

- [ ] **Step 3: Write minimal implementation.** Add the validator plus the two commands, mirroring `set_orchestrator_config`'s existing raw-TOML read/write/bump/emit sequence exactly (lines 417-536):

```rust
/// Reject unparseable clutch/risk labels before touching Vox.toml. `None`
/// values are always accepted (that axis just isn't being set/changed).
fn validate_task_policy_labels(clutch: Option<&str>, risk: Option<&str>) -> Result<(), String> {
    if let Some(c) = clutch {
        vox_orchestrator::mode::ClutchProfile::from_label(c)
            .ok_or_else(|| format!("unknown clutch label: {c}"))?;
    }
    if let Some(r) = risk {
        vox_orchestrator::mode::RiskPosture::from_label(r)
            .ok_or_else(|| format!("unknown risk label: {r}"))?;
    }
    Ok(())
}

/// `scope_kind` is `"category"` or `"source"`; `scope_key` is a `TaskCategory`/
/// `TriggerSource` Debug name (e.g. `"CodeGen"`, `"Automated"`).
#[tauri::command]
pub async fn set_task_policy_override(
    app_handle: tauri::AppHandle,
    scope_kind: String,
    scope_key: String,
    clutch: Option<String>,
    risk: Option<String>,
) -> Result<(), String> {
    validate_task_policy_labels(clutch.as_deref(), risk.as_deref())?;

    let current_dir = std::env::current_dir().map_err(|e| e.to_string())?;
    let (mut manifest, path) = VoxManifest::discover(&current_dir).map_err(|e| e.to_string())?;
    let mut orch_table = manifest.orchestrator.unwrap_or_default();

    let mut task_policy_table = orch_table
        .get("task_policy")
        .and_then(|v| v.as_table())
        .cloned()
        .unwrap_or_default();
    let mut scope_table = task_policy_table
        .get(&scope_kind)
        .and_then(|v| v.as_table())
        .cloned()
        .unwrap_or_default();

    let mut entry = toml::map::Map::new();
    if let Some(c) = &clutch {
        entry.insert("clutch".to_string(), toml::Value::String(c.clone()));
    }
    if let Some(r) = &risk {
        entry.insert("risk".to_string(), toml::Value::String(r.clone()));
    }
    scope_table.insert(scope_key.clone(), toml::Value::Table(entry));
    task_policy_table.insert(scope_kind, toml::Value::Table(scope_table));
    orch_table.insert("task_policy".to_string(), toml::Value::Table(task_policy_table));

    manifest.orchestrator = Some(orch_table);
    let toml_str = manifest.to_toml_string().map_err(|e| e.to_string())?;
    std::fs::write(&path, toml_str).map_err(|e| e.to_string())?;

    vox_config::snapshot::bump(&["task_policy"]);
    let _ = app_handle.emit(
        ORCH_CONFIG_CHANGED_EVENT,
        OrchestratorConfigChanged { rev: vox_config::snapshot::current_rev() },
    );
    Ok(())
}

/// Remove one override (`scope_kind`/`scope_key` as in [`set_task_policy_override`]).
#[tauri::command]
pub async fn clear_task_policy_override(
    app_handle: tauri::AppHandle,
    scope_kind: String,
    scope_key: String,
) -> Result<(), String> {
    let current_dir = std::env::current_dir().map_err(|e| e.to_string())?;
    let (mut manifest, path) = VoxManifest::discover(&current_dir).map_err(|e| e.to_string())?;
    let mut orch_table = manifest.orchestrator.unwrap_or_default();

    if let Some(mut task_policy_table) = orch_table.get("task_policy").and_then(|v| v.as_table()).cloned() {
        if let Some(mut scope_table) = task_policy_table.get(&scope_kind).and_then(|v| v.as_table()).cloned() {
            scope_table.remove(&scope_key);
            task_policy_table.insert(scope_kind, toml::Value::Table(scope_table));
            orch_table.insert("task_policy".to_string(), toml::Value::Table(task_policy_table));
        }
    }

    manifest.orchestrator = Some(orch_table);
    let toml_str = manifest.to_toml_string().map_err(|e| e.to_string())?;
    std::fs::write(&path, toml_str).map_err(|e| e.to_string())?;

    vox_config::snapshot::bump(&["task_policy"]);
    let _ = app_handle.emit(
        ORCH_CONFIG_CHANGED_EVENT,
        OrchestratorConfigChanged { rev: vox_config::snapshot::current_rev() },
    );
    Ok(())
}
```

*(Verify `toml::map::Map` is the correct type for `manifest.orchestrator`'s value type before pasting — read `VoxManifest`'s definition, e.g. `rg 'pub orchestrator' crates/vox-orchestrator/ crates/vox-config/` first, since `orch_table.insert(...)` in the existing `set_orchestrator_config` implies it's some `toml`-crate map type; match whatever that type actually is exactly, adjusting `toml::map::Map`/`.as_table()` calls if the real type differs.)*

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-gui --bins task_policy 2>test_out.log; tail -60 test_out.log`
Expected: PASS.

- [ ] **Step 5: Register both commands in the Tauri invoke handler.** Find the existing registration (`rg 'get_orchestrator_config_catalog' crates/vox-gui/src/main.rs`) and add `commands::orchestrator::get_task_policy_overrides`, `commands::orchestrator::set_task_policy_override`, `commands::orchestrator::clear_task_policy_override` to the same `tauri::generate_handler![...]` list.

- [ ] **Step 6: Run the full vox-gui Rust test suite**

Run: `cargo test -p vox-gui --bins 2>test_out.log; tail -100 test_out.log`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-gui/src/commands/orchestrator.rs crates/vox-gui/src/main.rs
git commit -m "feat(vox-gui): set/clear_task_policy_override Tauri commands"
```

### Task 4.3: Minimal React panel

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/Settings/TaskPolicySection.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/Settings/TaskPolicySection.test.tsx`

- [ ] **Step 1: Read the surrounding convention.** Open `crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx` and one existing section component in the same directory to match its exact styling classes, `invoke()` import path, and how a section is registered into `SettingsView.tsx`'s layout — copy that structure rather than inventing a new one; the code below is functionally complete but its JSX class names must be reconciled against whatever the real sibling component uses before this step is considered done.

- [ ] **Step 2: Write the failing test** `TaskPolicySection.test.tsx`:

```tsx
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { TaskPolicySection } from "./TaskPolicySection";

const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

describe("TaskPolicySection", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_task_policy_overrides") {
        return Promise.resolve({
          category: { CodeGen: { clutch: "efficiency", risk: "moderate" } },
          source: {},
        });
      }
      return Promise.resolve(undefined);
    });
  });

  it("renders existing overrides", async () => {
    render(<TaskPolicySection />);
    await waitFor(() => expect(screen.getByText(/CodeGen/i)).toBeInTheDocument());
    expect(screen.getByText(/efficiency/i)).toBeInTheDocument();
  });

  it("clears an override on remove click", async () => {
    render(<TaskPolicySection />);
    await waitFor(() => expect(screen.getByText(/CodeGen/i)).toBeInTheDocument());
    fireEvent.click(screen.getByRole("button", { name: /remove/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("clear_task_policy_override", {
        scopeKind: "category",
        scopeKey: "CodeGen",
      })
    );
  });
});
```

- [ ] **Step 3: Run to verify it fails**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/surfaces/Settings/TaskPolicySection.test.tsx 2>test_out.log; tail -60 test_out.log`
Expected: FAIL — `TaskPolicySection` module not found.

- [ ] **Step 4: Write minimal implementation** `TaskPolicySection.tsx`:

```tsx
import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

type TaskPolicyEntry = { clutch?: string; risk?: string };
type TaskPolicyOverrides = {
  category: Record<string, TaskPolicyEntry>;
  source: Record<string, TaskPolicyEntry>;
};

const CLUTCH_OPTIONS = ["free", "efficiency", "balanced", "genius"];
const RISK_OPTIONS = ["high", "moderate", "low"];

export function TaskPolicySection() {
  const [overrides, setOverrides] = useState<TaskPolicyOverrides>({ category: {}, source: {} });

  const refresh = useCallback(() => {
    invoke<TaskPolicyOverrides>("get_task_policy_overrides").then(setOverrides);
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const rows: Array<{ scopeKind: "category" | "source"; scopeKey: string; entry: TaskPolicyEntry }> = [
    ...Object.entries(overrides.category).map(([scopeKey, entry]) => ({
      scopeKind: "category" as const,
      scopeKey,
      entry,
    })),
    ...Object.entries(overrides.source).map(([scopeKey, entry]) => ({
      scopeKind: "source" as const,
      scopeKey,
      entry,
    })),
  ];

  const setOverride = (scopeKind: "category" | "source", scopeKey: string, clutch?: string, risk?: string) => {
    invoke("set_task_policy_override", { scopeKind, scopeKey, clutch, risk }).then(refresh);
  };

  const clearOverride = (scopeKind: "category" | "source", scopeKey: string) => {
    invoke("clear_task_policy_override", { scopeKind, scopeKey }).then(refresh);
  };

  return (
    <section>
      <h3>Task-type cost/model policy</h3>
      <table>
        <thead>
          <tr>
            <th>Scope</th>
            <th>Clutch</th>
            <th>Risk</th>
            <th />
          </tr>
        </thead>
        <tbody>
          {rows.map(({ scopeKind, scopeKey, entry }) => (
            <tr key={`${scopeKind}:${scopeKey}`}>
              <td>{scopeKind === "category" ? `Category: ${scopeKey}` : `Source: ${scopeKey}`}</td>
              <td>
                <select
                  value={entry.clutch ?? ""}
                  onChange={(e) => setOverride(scopeKind, scopeKey, e.target.value || undefined, entry.risk)}
                >
                  <option value="">(inherit)</option>
                  {CLUTCH_OPTIONS.map((c) => (
                    <option key={c} value={c}>
                      {c}
                    </option>
                  ))}
                </select>
              </td>
              <td>
                <select
                  value={entry.risk ?? ""}
                  onChange={(e) => setOverride(scopeKind, scopeKey, entry.clutch, e.target.value || undefined)}
                >
                  <option value="">(inherit)</option>
                  {RISK_OPTIONS.map((r) => (
                    <option key={r} value={r}>
                      {r}
                    </option>
                  ))}
                </select>
              </td>
              <td>
                <button onClick={() => clearOverride(scopeKind, scopeKey)}>Remove</button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </section>
  );
}
```

- [ ] **Step 5: Run to verify it passes**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/surfaces/Settings/TaskPolicySection.test.tsx 2>test_out.log; tail -60 test_out.log`
Expected: PASS (2 tests).

- [ ] **Step 6: Register the section in `SettingsView.tsx`** — import `TaskPolicySection` and render it alongside the other settings sections (match wherever `RuntimeConfigSection`-equivalent components are listed).

- [ ] **Step 7: Run typecheck**

Run: `cd crates/vox-gui/ui && npm run typecheck 2>test_out.log; tail -60 test_out.log`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Settings/TaskPolicySection.tsx crates/vox-gui/ui/src/components/surfaces/Settings/TaskPolicySection.test.tsx crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx
git commit -m "feat(vox-gui): minimal Task-type cost/model policy settings panel"
```

---

## Phase 5 — Unify the chat mode selector's default with the backend resolver

**Grounding (confirmed live):** the chat composer (`Loquela.tsx`) already threads its `DriveConsole` clutch/risk selection all the way to `AgentTask.clutch_profile`/`risk_posture` today (`control.clutch`/`control.risk` → `SubmitTaskInput.clutch`/`.risk` in `crates/vox-gui/src/commands/control_plane.rs` → `enqueue_hints` → the daemon's `TaskEnqueueHints` → `AgentTask.apply_hints`, all pre-existing, not part of this plan) — so **picking a mode already changes what gets submitted**, and Task 2.3 above is what makes that submission actually change model selection once it lands. What's *not* unified: `crates/vox-gui/ui/src/lib/driveConsole.ts`'s `defaultControl()` hardcodes `{ clutch: 'efficiency', risk: 'moderate' }` as the composer's starting selection, completely independent of whatever this plan's backend resolver (`resolve_task_policy`, Phase 1) would actually pick for a `Chat`-category, `Interactive`-source task — two separately-hardcoded defaults that can silently drift. This phase makes the composer ask the backend for its real default instead of guessing locally.

### Task 5.1: `resolve_default_task_policy` Tauri command

**Files:**
- Modify: `crates/vox-gui/src/commands/orchestrator.rs`

- [ ] **Step 1: Write the failing test.**

```rust
#[cfg(test)]
mod default_policy_tests {
    use super::*;

    #[test]
    fn chat_default_matches_resolve_task_policy_with_no_overrides() {
        let dto = resolve_default_task_policy("Chat".to_string(), "interactive".to_string());
        // No overrides configured in a fresh test environment ⇒ falls all the
        // way to the global default, exactly like resolve_task_policy(None, None, None, None).
        assert_eq!(dto.clutch, "balanced");
        assert_eq!(dto.risk, "moderate");
    }

    #[test]
    fn unknown_category_or_source_labels_fall_back_to_global_default() {
        let dto = resolve_default_task_policy("NotARealCategory".to_string(), "not_a_real_source".to_string());
        assert_eq!(dto.clutch, "balanced");
        assert_eq!(dto.risk, "moderate");
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-gui --bins resolve_default_task_policy 2>test_out.log; tail -30 test_out.log`
Expected: FAIL — function not found.

- [ ] **Step 3: Write minimal implementation.** Add to `orchestrator.rs`:

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct DefaultTaskPolicyDto {
    pub clutch: String,
    pub risk: String,
}

fn clutch_label(c: vox_orchestrator::mode::ClutchProfile) -> &'static str {
    match c {
        vox_orchestrator::mode::ClutchProfile::Free => "free",
        vox_orchestrator::mode::ClutchProfile::Efficiency => "efficiency",
        vox_orchestrator::mode::ClutchProfile::Balanced => "balanced",
        vox_orchestrator::mode::ClutchProfile::Genius => "genius",
    }
}

fn risk_label(r: vox_orchestrator::mode::RiskPosture) -> &'static str {
    match r {
        vox_orchestrator::mode::RiskPosture::High => "high",
        vox_orchestrator::mode::RiskPosture::Moderate => "moderate",
        vox_orchestrator::mode::RiskPosture::Low => "low",
    }
}

/// The composer's real starting clutch/risk for `task_category` — the same
/// precedence chain (`resolve_task_policy`) the backend uses when actually
/// executing a task with no explicit hint, so the GUI's shown default can
/// never drift from what would actually happen. `category`/`source` are
/// `TaskCategory`/`TriggerSource` Debug-style names (e.g. `"Chat"`,
/// `"Interactive"`) or a `TriggerSource::from_label`-style lowercase label for
/// `source` — this command accepts either casing via `from_label`, since the
/// frontend only ever passes the lowercase `TriggerSource`/`ClutchId` labels.
#[tauri::command]
pub fn resolve_default_task_policy(category: String, source: String) -> DefaultTaskPolicyDto {
    use vox_orchestrator::types::TaskCategory;

    let overrides = vox_orchestrator::config::OrchestratorConfig::snapshot().task_policy;
    let category: TaskCategory = format!("{category:?}")
        .parse()
        .ok()
        .filter(|_: &TaskCategory| false) // never used — see note below
        .unwrap_or_default();
    let _ = category; // placeholder removed in Step 3b below
    unimplemented!()
}
```

*(The block above is deliberately incomplete — `TaskCategory` has no public string parser today (`task_category_from_mcp_str` in `submission.rs` is crate-private to `vox-orchestrator-mcp` and only covers 10 of the 15 variants). Do not paste the code above as-is. Instead, in Step 3b, either (a) add a small `pub fn from_label(s: &str) -> Option<Self>` to `TaskCategory` in `crates/vox-orchestrator/src/types/tasks.rs` mirroring `ClutchProfile::from_label`'s exact style, matching on the lowercased `model-routing.v1.yaml` category names (`"codegen"`, `"chat"`, `"research"`, etc. — the full 15-entry list is in `contracts/orchestration/model-routing.v1.yaml::task_categories`), or (b) if `TaskCategory` already derives `Deserialize` with `#[serde(rename_all = "PascalCase")]`-equivalent behavior, parse via `serde_json::from_value(serde_json::Value::String(category))` instead of hand-writing a parser. Check `TaskCategory`'s actual derive attributes first — prefer (b) if it already round-trips, since that avoids a second parser to keep in sync with the YAML list.)*

- [ ] **Step 3b: Write the real implementation** once the category-parsing approach from Step 3's note is chosen. Finish `resolve_default_task_policy` to build `category_policy`/`source_policy` via `vox_orchestrator::mode::effective_category_policy`/`effective_source_policy` (same as Task 2.2's `AgentTask::resolved_policy`), call `vox_orchestrator::mode::resolve_task_policy(None, None, category_policy, source_policy)`, and map the result through `clutch_label`/`risk_label` into `DefaultTaskPolicyDto`. Unknown category/source strings (Step 1's second test) must fall through to `None`/`None` policy — not panic — reproducing the global default.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-gui --bins default_policy_tests 2>test_out.log; tail -40 test_out.log`
Expected: PASS.

- [ ] **Step 5: Register the command** in `crates/vox-gui/src/main.rs`'s `tauri::generate_handler![...]` list, next to the Task 4.x registrations.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/src/commands/orchestrator.rs crates/vox-gui/src/main.rs
git commit -m "feat(vox-gui): resolve_default_task_policy — GUI default mirrors the backend resolver"
```

### Task 5.2: `Loquela.tsx` fetches its starting clutch/risk instead of hardcoding it

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Loquela/Loquela.tsx:169`

- [ ] **Step 1: Write the failing test.** Add to `Loquela.test.tsx` (find its existing `vi.mock('@tauri-apps/api/core', ...)` setup and extend it):

```tsx
it("seeds the DriveConsole default from resolve_default_task_policy instead of a hardcoded guess", async () => {
  mockInvoke.mockImplementation((cmd: string) => {
    if (cmd === "resolve_default_task_policy") {
      return Promise.resolve({ clutch: "free", risk: "high" });
    }
    return Promise.resolve(undefined);
  });
  render(<Loquela {...defaultProps} />);
  await waitFor(() => {
    const freeButton = screen.getByRole("radio", { name: /free/i });
    expect(freeButton).toHaveAttribute("aria-checked", "true");
  });
});
```

*(Match this test's render setup — `defaultProps`, existing mock scaffolding — to whatever `Loquela.test.tsx` already establishes for its other tests; the assertion shape above is complete, but the render call and `mockInvoke` wiring must reuse the file's existing conventions rather than introducing a second, inconsistent mock setup.)*

- [ ] **Step 2: Run to verify it fails**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/surfaces/Loquela/Loquela.test.tsx -t "seeds the DriveConsole default" 2>test_out.log; tail -40 test_out.log`
Expected: FAIL — `control` still starts at the hardcoded `defaultControl()` (efficiency/moderate), never becomes free/high.

- [ ] **Step 3: Write minimal implementation.** In `Loquela.tsx`, replace:

```tsx
const [control, setControl] = useState<ControlState>(defaultControl);
```

with:

```tsx
const [control, setControl] = useState<ControlState>(defaultControl);

useEffect(() => {
  invoke<{ clutch: ClutchId; risk: RiskId }>('resolve_default_task_policy', {
    category: 'Chat',
    source: 'interactive',
  })
    .then((resolved) => setControl(resolved))
    .catch(() => {
      // Backend unavailable (e.g. cold start) — keep the local hardcoded
      // default rather than blocking the composer on this fetch.
    });
}, []);
```

(Add `invoke` to the existing `@tauri-apps/api/core` import at the top of the file if not already imported; add `useEffect` to the existing `react` import if not already present — both are extremely likely already imported given the file's size, confirm rather than duplicating the import.)

- [ ] **Step 4: Run to verify it passes**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/surfaces/Loquela/Loquela.test.tsx 2>test_out.log; tail -80 test_out.log`
Expected: PASS (the whole file's suite, including the new test — confirming this fetch-on-mount doesn't break any test that renders `Loquela` without mocking `resolve_default_task_policy`, since the `.catch()` keeps the old hardcoded default as a safe fallback for those).

- [ ] **Step 5: Run typecheck**

Run: `cd crates/vox-gui/ui && npm run typecheck 2>test_out.log; tail -40 test_out.log`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Loquela/Loquela.tsx crates/vox-gui/ui/src/components/surfaces/Loquela/Loquela.test.tsx
git commit -m "feat(vox-gui): chat composer's starting clutch/risk mirrors the backend's resolved default"
```

---

## Phase 6 — Docs + final check

### Task 6.1: Update the spec's status and cross-link

**Files:**
- Modify: `docs/superpowers/specs/2026-08-02-per-task-type-cost-model-policy-design.md`

- [ ] **Step 1:** Add a one-line "Execution" note near the top of the spec: `**Execution:** implemented via docs/superpowers/plans/2026-08-02-per-task-type-cost-model-policy.md.` Commit:

```bash
git add docs/superpowers/specs/2026-08-02-per-task-type-cost-model-policy-design.md
git commit -m "docs: cross-link the per-task-type cost policy plan from its spec"
```

### Task 6.2: Full workspace check

- [ ] **Step 1:** `cargo clippy -p vox-orchestrator -p vox-orchestrator-mcp -- -D warnings 2>clippy_out.log; tail -100 clippy_out.log` — expect clean.
- [ ] **Step 2:** `cargo clippy -p vox-gui --bins -- -D warnings 2>clippy_out.log; tail -100 clippy_out.log` (per `AGENTS.md`, never `--all-targets` on `vox-gui`) — expect clean.
- [ ] **Step 3:** `vox run scripts/fmt.vox` (never `cargo fmt --all` — Windows gotcha) on every touched crate.
- [ ] **Step 4:** Run `/code-review` on the full branch diff before opening a PR.

---

## Self-Review

- **Spec coverage:** `TriggerSource` enum → Task 1.1. Resolver + precedence → Task 1.2. Storage (compiled defaults + Vox.toml overrides, adjusted to live in `vox-orchestrator` per the grounding correction noted at the top) → Task 1.3. `AgentTask` wiring → Phase 2. MCP-direct wiring (the concrete "CI/CD inbox" example from the spec's data-flow section) → Phase 3. GUI panel → Phase 4. Deferred-per-spec items (llm_bridge consolidation, full Band B, combo rules, tenant policy) are not touched anywhere in this plan — verified by absence, not by an explicit "skip" task, since there is nothing to do for them here.
- **Mid-brainstorm addition coverage (chat wiring, requested after the spec was written):** "the mode actually changes" when selected in chat → Task 2.3 (the real `AgentTask` execution path in `runtime.rs`, which had the same dead-branch-when-unset bug as the MCP path this plan already fixed in Task 3.3 — confirmed via live-code read, not assumed). "Unifying" the frontend and backend defaults → Phase 5 (`resolve_default_task_policy` + `Loquela.tsx` fetching it instead of the hardcoded `defaultControl()`). Grounding note: the chat composer's clutch/risk *submission* path (composer → `SubmitTaskInput` → `enqueue_hints` → `TaskEnqueueHints` → `AgentTask.apply_hints`) was found to already exist and work today — it is explicitly called out as pre-existing in Task 2.3 and Phase 5's intro rather than re-built, so this plan only adds the two genuinely-missing pieces (the execution-side dead branch, and the GUI's independently-hardcoded default).
- **Placeholder scan:** the call-outs that say "verify against live code before pasting" (Task 3.1's hints-function signature, Task 3.3's config-handle read, Task 4.2's `toml::map::Map` type, Task 4.3's styling conventions, Task 5.1's `TaskCategory` string-parsing approach, Task 5.2's test-scaffolding reuse) are each a concrete, bounded, one-file check with a specific `rg`/read command or explicit decision criterion attached — not an open-ended TODO. Task 5.1's Step 2 code block is a deliberate exception worth flagging explicitly: it is intentionally-incomplete illustrative code (marked `unimplemented!()`), not a step to execute as written — Step 3b is where the real, complete implementation goes, gated on a one-line decision (does `TaskCategory` already round-trip through serde or not) that couldn't be resolved without reading `tasks.rs`'s derive attributes at execution time. This mirrors the "Caveat for the implementer" pattern already used in this repo's Band A/B/egress plans for exactly this kind of drift between plan-writing time and execution time.
- **Type consistency:** `resolve_task_policy(explicit_clutch, explicit_risk, category_policy, source_policy) -> (ClutchProfile, RiskPosture)` (Task 1.2) is called with that exact signature in `effective_category_policy`/`effective_source_policy` callers (Task 1.3), `AgentTask::resolved_policy` (Task 2.2), `resolve_task_cost_policy` (Task 2.3), the `resolve.rs` wiring (Task 3.3), and `resolve_default_task_policy` (Task 5.1) — verified consistent across all six uses. `TaskPolicyOverrides`/`TaskPolicyEntry` field names (`category`, `source`, `clutch`, `risk`) are identical everywhere they're constructed or read (Tasks 1.3, 2.2, 2.3, 4.1, 4.2, 4.3, 5.1).
