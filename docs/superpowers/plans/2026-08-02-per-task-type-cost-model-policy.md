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

**⚠️ REVISED after adversarial review (2026-08-02):** the original version of this task modeled `category_policy`/`source_policy` as `Option<(ClutchProfile, RiskPosture)>` — a whole pair, present or absent together. That's wrong: it makes clutch and risk resolve as a *bundle* per precedence level, contradicting this plan's own spec, which explicitly promises "clutch and risk resolve independently... a task can inherit its category's clutch but its source's risk." It's also incompatible with Task 4.3's GUI panel, which lets an operator set just the clutch dropdown and leave risk at "(inherit)" per row — that only means anything if risk really can fall through independently. The fix: `resolve_task_policy` takes each axis at each level as its own `Option`, six parameters instead of four wrapped pairs.

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
            Some(ClutchProfile::Genius), Some(RiskPosture::Low),
            Some(ClutchProfile::Free), Some(RiskPosture::High),
            Some(ClutchProfile::Efficiency), Some(RiskPosture::Moderate),
        );
        assert_eq!(clutch, ClutchProfile::Genius);
        assert_eq!(risk, RiskPosture::Low);
    }

    #[test]
    fn category_policy_wins_over_source_policy() {
        let (clutch, risk) = resolve_task_policy(
            None, None,
            Some(ClutchProfile::Balanced), Some(RiskPosture::Moderate),
            Some(ClutchProfile::Free), Some(RiskPosture::High),
        );
        assert_eq!(clutch, ClutchProfile::Balanced);
        assert_eq!(risk, RiskPosture::Moderate);
    }

    #[test]
    fn source_policy_wins_when_no_category_policy() {
        let (clutch, risk) = resolve_task_policy(
            None, None,
            None, None,
            Some(ClutchProfile::Free), Some(RiskPosture::High),
        );
        assert_eq!(clutch, ClutchProfile::Free);
        assert_eq!(risk, RiskPosture::High);
    }

    #[test]
    fn falls_back_to_global_default_when_nothing_set() {
        let (clutch, risk) = resolve_task_policy(None, None, None, None, None, None);
        assert_eq!(clutch, ClutchProfile::Balanced);
        assert_eq!(risk, RiskPosture::Moderate);
    }

    #[test]
    fn axes_resolve_independently_across_levels_real_case() {
        // A category policy supplies ONLY clutch (its risk axis is None — the
        // realistic partial-override case Task 4.3's GUI produces when an
        // operator sets the clutch dropdown and leaves risk at "(inherit)").
        // Risk must fall through past category to source, NOT default straight
        // to Moderate — this is the exact property the previous (buggy) design
        // could not express, because it only ever passed whole pairs.
        let (clutch, risk) = resolve_task_policy(
            None, None,
            Some(ClutchProfile::Efficiency), None,
            None, Some(RiskPosture::High),
        );
        assert_eq!(clutch, ClutchProfile::Efficiency, "category's clutch axis wins");
        assert_eq!(risk, RiskPosture::High, "category had no risk axis, so source's risk axis is used, not the global default");
    }

    #[test]
    fn explicit_clutch_and_category_risk_combine_across_different_levels() {
        let (clutch, risk) = resolve_task_policy(
            Some(ClutchProfile::Genius), None,
            None, Some(RiskPosture::Low),
            Some(ClutchProfile::Free), Some(RiskPosture::High),
        );
        assert_eq!(clutch, ClutchProfile::Genius, "explicit clutch wins outright");
        assert_eq!(risk, RiskPosture::Low, "explicit risk unset, category risk wins over source risk");
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
/// `AgentTask::resolved_clutch()`/`resolved_risk()`). Each of the three levels
/// takes clutch and risk as SEPARATE `Option`s (not a paired tuple) so an
/// override that only sets one axis lets the other keep falling through —
/// callers compute the category/source arguments via
/// `effective_category_policy()`/`effective_source_policy()` below, which
/// return the same per-axis shape.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn resolve_task_policy(
    explicit_clutch: Option<ClutchProfile>,
    explicit_risk: Option<RiskPosture>,
    category_clutch: Option<ClutchProfile>,
    category_risk: Option<RiskPosture>,
    source_clutch: Option<ClutchProfile>,
    source_risk: Option<RiskPosture>,
) -> (ClutchProfile, RiskPosture) {
    let clutch = explicit_clutch
        .or(category_clutch)
        .or(source_clutch)
        .unwrap_or(ClutchProfile::Balanced);
    let risk = explicit_risk
        .or(category_risk)
        .or(source_risk)
        .unwrap_or(RiskPosture::Moderate);
    (clutch, risk)
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-orchestrator --lib mode:: 2>test_out.log; tail -60 test_out.log`
Expected: PASS (all `mode.rs` tests, including the 6 new ones).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator/src/mode.rs
git commit -m "feat(vox-orchestrator): add resolve_task_policy precedence resolver (per-axis independent)"
```

### Task 1.3: Override-merge helpers (bridges Vox.toml overrides with the compiled tables)

**Files:**
- Modify: `crates/vox-orchestrator/src/mode.rs`
- Modify: `crates/vox-orchestrator/src/config/orchestrator_fields.rs`
- Modify: `crates/vox-orchestrator/src/config/mod.rs` (re-export — see Step 1b; missing this is a compile-blocking gap the original version of this task had)
- Modify: `crates/vox-orchestrator/src/config/impl_default.rs` (see Step 1c; also compile-blocking if skipped)

- [ ] **Step 1: Write the failing test.** First add the override types this test needs — append to `orchestrator_fields.rs`, near the top after the existing imports (before the `OrchestratorConfig` struct definition):

```rust
/// One override entry: a clutch and/or risk label (parsed via
/// `ClutchProfile::from_label`/`RiskPosture::from_label`). Either may be
/// `None` — an override can set just one axis, letting the other fall through
/// to the next precedence level. This is exactly the shape Task 4.3's GUI
/// panel produces (independent clutch/risk dropdowns, each with an
/// "(inherit)" option).
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

- [ ] **Step 1b (compile-blocking, do not skip): export the new types.** `crates/vox-orchestrator/src/config/mod.rs` re-exports from `orchestrator_fields` via an explicit named list, not a glob (`pub use orchestrator_fields::{FieldType, OrchestratorConfig, OrchestratorConfigField};` at line 21) — `TaskPolicyOverrides`/`TaskPolicyEntry` are invisible as `crate::config::TaskPolicyOverrides` anywhere outside `orchestrator_fields.rs` itself until this list is extended. Change that line to:

```rust
pub use orchestrator_fields::{
    FieldType, OrchestratorConfig, OrchestratorConfigField, TaskPolicyEntry, TaskPolicyOverrides,
};
```

- [ ] **Step 1c (compile-blocking, do not skip): update the hand-written `Default` impl.** `OrchestratorConfig` explicitly does NOT derive `Default` — `crates/vox-orchestrator/src/config/impl_default.rs` has an exhaustive `Self { enabled: true, max_agents: 8, ..., test_decision_policy: crate::planning::TestDecisionPolicy::default(), ... }` struct literal covering every field by name. Adding `task_policy` to the struct without adding a matching line here is `E0063` (missing field). Add, next to the `test_decision_policy` line:

```rust
            task_policy: crate::config::TaskPolicyOverrides::default(),
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
        let (clutch, risk) = effective_category_policy(&overrides, TaskCategory::CodeGen);
        assert_eq!(clutch, Some(ClutchProfile::Free));
        assert_eq!(risk, Some(RiskPosture::High));
    }

    #[test]
    fn missing_category_override_and_no_compiled_default_is_none() {
        let overrides = TaskPolicyOverrides::default();
        assert_eq!(effective_category_policy(&overrides, TaskCategory::Research), (None, None));
    }

    #[test]
    fn malformed_override_label_falls_through_to_none() {
        let mut source = HashMap::new();
        source.insert(
            "Automated".to_string(),
            TaskPolicyEntry { clutch: Some("turbo".to_string()), risk: None },
        );
        let overrides = TaskPolicyOverrides { category: HashMap::new(), source };
        // "turbo" doesn't parse; risk was never set — neither axis resolves.
        assert_eq!(effective_source_policy(&overrides, TriggerSource::Automated), (None, None));
    }

    #[test]
    fn partial_override_sets_one_axis_and_leaves_the_other_none() {
        // The property Task 4.3's GUI relies on: setting only the clutch
        // dropdown for a category must NOT force a risk value — risk stays
        // `None` here so resolve_task_policy can let it fall through further.
        let mut category = HashMap::new();
        category.insert(
            "Research".to_string(),
            TaskPolicyEntry { clutch: Some("genius".to_string()), risk: None },
        );
        let overrides = TaskPolicyOverrides { category, source: HashMap::new() };
        assert_eq!(
            effective_category_policy(&overrides, TaskCategory::Research),
            (Some(ClutchProfile::Genius), None),
            "a clutch-only override must resolve clutch and leave risk as None, not force a paired default"
        );
    }

    #[test]
    fn unknown_category_key_warns_once_and_falls_through() {
        // Regression guard for the spec's "unknown category/source name ...
        // warned once, mirrors unregistered_llm_env's warn-once pattern"
        // requirement — this test only proves the fallthrough half (no panic,
        // resolves to (None, None)); the warn call itself is exercised via
        // `tracing_test` capture if available, or left as a visual check on
        // `RUST_LOG=warn cargo test -- --nocapture` since this crate doesn't
        // currently depend on a tracing-capture test helper.
        let mut category = HashMap::new();
        category.insert("NotARealCategory".to_string(), TaskPolicyEntry { clutch: Some("free".to_string()), risk: Some("high".to_string()) });
        let overrides = TaskPolicyOverrides { category, source: HashMap::new() };
        assert_eq!(effective_category_policy(&overrides, TaskCategory::CodeGen), (None, None));
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-orchestrator --lib mode::effective_policy_tests 2>test_out.log; tail -40 test_out.log`
Expected: FAIL — `effective_category_policy`/`effective_source_policy` not found.

- [ ] **Step 3: Write minimal implementation.** Append to `mode.rs`:

```rust
/// Merge the live Vox.toml override (if any and parseable) with the compiled
/// `DEFAULT_CATEGORY_POLICY` for one category. Returns each axis
/// INDEPENDENTLY as its own `Option` — an override that sets only `clutch`
/// resolves `(Some(_), None)`, not a forced pair, so `resolve_task_policy`
/// can let the unset axis fall through to the source-level policy. Logs a
/// `tracing::warn!` once per lookup when the override map has an entry for
/// this key but neither axis parses (a malformed/typo'd label), per the
/// spec's "not a silent no-op" requirement.
#[must_use]
pub fn effective_category_policy(
    overrides: &crate::config::TaskPolicyOverrides,
    category: crate::types::TaskCategory,
) -> (Option<ClutchProfile>, Option<RiskPosture>) {
    let key = format!("{category:?}");
    if let Some(entry) = overrides.category.get(&key) {
        let clutch = entry.clutch.as_deref().and_then(ClutchProfile::from_label);
        let risk = entry.risk.as_deref().and_then(RiskPosture::from_label);
        if clutch.is_none() && risk.is_none() && (entry.clutch.is_some() || entry.risk.is_some()) {
            tracing::warn!(category = %key, clutch = ?entry.clutch, risk = ?entry.risk, "task_policy category override has an unparseable clutch/risk label; falling through to compiled default");
        }
        if clutch.is_some() || risk.is_some() {
            let default = DEFAULT_CATEGORY_POLICY.iter().find(|p| p.category == category);
            return (
                clutch.or_else(|| default.map(|p| p.clutch)),
                risk.or_else(|| default.map(|p| p.risk)),
            );
        }
    }
    match DEFAULT_CATEGORY_POLICY.iter().find(|p| p.category == category) {
        Some(p) => (Some(p.clutch), Some(p.risk)),
        None => (None, None),
    }
}

/// Same merge as [`effective_category_policy`], for `TriggerSource`.
#[must_use]
pub fn effective_source_policy(
    overrides: &crate::config::TaskPolicyOverrides,
    source: TriggerSource,
) -> (Option<ClutchProfile>, Option<RiskPosture>) {
    let key = format!("{source:?}");
    if let Some(entry) = overrides.source.get(&key) {
        let clutch = entry.clutch.as_deref().and_then(ClutchProfile::from_label);
        let risk = entry.risk.as_deref().and_then(RiskPosture::from_label);
        if clutch.is_none() && risk.is_none() && (entry.clutch.is_some() || entry.risk.is_some()) {
            tracing::warn!(source = %key, clutch = ?entry.clutch, risk = ?entry.risk, "task_policy source override has an unparseable clutch/risk label; falling through to compiled default");
        }
        if clutch.is_some() || risk.is_some() {
            let default = DEFAULT_SOURCE_POLICY.iter().find(|p| p.source == source);
            return (
                clutch.or_else(|| default.map(|p| p.clutch)),
                risk.or_else(|| default.map(|p| p.risk)),
            );
        }
    }
    match DEFAULT_SOURCE_POLICY.iter().find(|p| p.source == source) {
        Some(p) => (Some(p.clutch), Some(p.risk)),
        None => (None, None),
    }
}
```

Add `use serde::{Deserialize, Serialize};` to `orchestrator_fields.rs` if not already imported (check the top of the file first — it already derives `Serialize, Deserialize` on `OrchestratorConfig` itself, so this import already exists). Confirm `mode.rs` has `tracing` in scope (it's a workspace-standard dependency used throughout `vox-orchestrator`; if this specific file has no prior `tracing::` call, add `use tracing;` is unnecessary since `tracing::warn!` is a fully-qualified macro path — just confirm the crate dependency exists in `Cargo.toml`, which it does everywhere else in this crate).

**Version-skew risk — fixed directly, ahead of this task (2026-08-02), not deferred.** `OrchestratorConfig` previously carried `#[serde(deny_unknown_fields, default)]` at the struct level. `crates/vox-orchestrator/src/config/orchestrator_fields.rs`'s own test `unknown_scope_enforcement_does_not_wipe_whole_section` documents that this combination caused a real production incident (PR #349): one unrecognized/bad key failed the whole `[orchestrator]` section's parse, silently resetting every setting to defaults. Adding `task_policy` would have carried the identical risk for any older binary reading a Vox.toml written by a newer one. Rather than accept and document this as inherited, it's been fixed at the source: `deny_unknown_fields` is removed from `OrchestratorConfig`, and a `#[serde(flatten, skip_serializing)] pub unrecognized_fields: std::collections::BTreeMap<String, toml::Value>` field now absorbs any key that doesn't match a named field instead of failing the parse; `load_from_toml` (`crates/vox-orchestrator/src/config/impl_load.rs`) logs a `tracing::warn!` listing the ignored keys when this is non-empty, so drift stays observable instead of silent. A regression test (`unrecognized_orchestrator_key_does_not_wipe_the_section`) proves an unrelated setting survives a wholly-unknown sibling key. This fix landed in `orchestrator_fields.rs`/`impl_default.rs`/`impl_load.rs` as part of this review pass — Task 1.3's own `git add`/commit list above should include these three files if you're replaying this plan from a clean branch where the fix isn't already present; if you're executing on top of a branch where this review's fixes are already committed, there is nothing left to do here.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-orchestrator --lib mode:: 2>test_out.log; tail -60 test_out.log`
Expected: PASS (all `mode.rs` tests, including the 5 new ones).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator/src/mode.rs crates/vox-orchestrator/src/config/orchestrator_fields.rs crates/vox-orchestrator/src/config/mod.rs crates/vox-orchestrator/src/config/impl_default.rs crates/vox-orchestrator/src/config/impl_load.rs
git commit -m "feat(vox-orchestrator): TaskPolicyOverrides on OrchestratorConfig + per-axis effective-policy merge helpers; fix deny_unknown_fields version-skew wiping the whole [orchestrator] section"
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
        let (category_clutch, category_risk) =
            crate::mode::effective_category_policy(overrides, self.task_category);
        let source = self.trigger_source.unwrap_or(crate::mode::TriggerSource::Interactive);
        let (source_clutch, source_risk) = crate::mode::effective_source_policy(overrides, source);
        crate::mode::resolve_task_policy(
            self.clutch_profile,
            self.risk_posture,
            category_clutch,
            category_risk,
            source_clutch,
            source_risk,
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

**Scope note (found by adversarial review, not fixed in this task):** the identical "gate on an explicit hint, ignore category/source policy" bug also exists in `crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/attention_fields.rs:86-92` (approval-tier escalation only consults `task.resolved_risk()` when `task.risk_posture.is_some()`) and `crates/vox-orchestrator/src/orchestrator/task_dispatch/complete/success/socrates.rs:47-59` (grounding/Socrates enforcement, same gate). Task 2.4 below fixes both, reusing the exact same `resolved_policy()`/`effective_source_policy()` machinery this task wires into `runtime.rs` — without Task 2.4, a category/source *risk* policy changes cost preference but has no effect on approval strictness or grounding/Socrates enforcement, which undercuts half of what the spec asks for (configurable risk posture per task type).

**Grounding (confirmed live):** `crates/vox-orchestrator/src/runtime.rs:619-634` (`AiTaskProcessor::process`) is the real consumption site (line numbers verified against the live file during this review; a first draft of this plan cited 628-644, off by the width of an intervening comment block — reconfirm against the file you actually have checked out, since comments shift):

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
    let (category_clutch, category_risk) =
        crate::mode::effective_category_policy(overrides, task.task_category);
    let source = task.trigger_source.unwrap_or(crate::mode::TriggerSource::Interactive);
    let (source_clutch, source_risk) = crate::mode::effective_source_policy(overrides, source);
    if task.clutch_profile.is_none()
        && category_clutch.is_none()
        && source_clutch.is_none()
    {
        return (global_default, false);
    }
    let (clutch, _risk) = crate::mode::resolve_task_policy(
        task.clutch_profile,
        task.risk_posture,
        category_clutch,
        category_risk,
        source_clutch,
        source_risk,
    );
    let rc = clutch.resolve();
    (rc.cost_preference, rc.force_free_pool)
}
```

Replace the `let mut cost_pref = ...` / `let force_free_pool = if task.clutch_profile.is_some() { ... } else { false };` block (the exact lines shown in this task's "Grounding" quote above — verify against the live file, since the citation drifted once already) in `AiTaskProcessor::process` with:

```rust
        let overrides = crate::sync_lock::rw_read(&*self.orchestrator.config).task_policy.clone();
        let global_default = crate::sync_lock::rw_read(&*self.orchestrator.config).cost_preference;
        let (mut cost_pref, force_free_pool) = resolve_task_cost_policy(&task, &overrides, global_default);
```

(Leave the risk/`ModelLean::Intelligence` `if matches!(...)` block immediately after unchanged — it already runs unconditionally today via `task.resolved_risk()`'s own built-in `Moderate` fallback and needs no fix.)

- [ ] **Step 5: Run to verify it passes**

Run: `cargo test -p vox-orchestrator --lib runtime:: 2>test_out.log; tail -100 test_out.log`
Expected: PASS — including every pre-existing `runtime.rs` test (this is the zero-regression gate; if any pre-existing test asserted the old `else { false }` behavior for a task that now resolves a category/source policy, read it and confirm the new behavior is what that test *should* want, per the confirmed-Economy assumption from Step 1 — do not weaken the new test to make an outdated assertion pass).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-orchestrator/src/runtime.rs
git commit -m "fix(vox-orchestrator): AgentTask execution honors task-type cost policy even without an explicit clutch hint"
```

### Task 2.4: Apply the same fix to approval-tier escalation and grounding/Socrates enforcement

**Why this task exists:** flagged by adversarial review — `attention_fields.rs` and `socrates.rs` have the exact same "only consult resolved_risk when risk_posture is explicitly Some" gate that Task 2.3 just fixed in `runtime.rs`. Skipping these means a category/source *risk* policy (e.g. "Automated tasks get Low risk posture, forcing extra grounding") silently has zero effect outside cost preference — the risk half of this plan's stated goal stays unfulfilled without it. Both fixes reuse `resolved_policy()`/`effective_source_policy()` already built in Tasks 1.3/2.2; there is no new resolution logic here, only two more call sites consuming it.

**Files:**
- Modify: `crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/attention_fields.rs:82-92`
- Modify: `crates/vox-orchestrator/src/orchestrator/task_dispatch/complete/success/socrates.rs:47-59`

- [ ] **Step 1: Read both files' current gate logic in full** (`rg -n "resolved_risk|risk_posture" crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/attention_fields.rs crates/vox-orchestrator/src/orchestrator/task_dispatch/complete/success/socrates.rs`) and confirm the exact current shape before editing — both were read during this plan's review pass, but re-confirm at execution time since these files may have changed since. In `attention_fields.rs`, the gate is `match task.risk_posture { Some(_) => { let risk_tier = task.resolved_risk().approval.to_approval_tier(); tier.max_strictness(risk_tier) } None => tier }`. In `socrates.rs`, it's `task.risk_posture.map(|_| resolved.grounding_enforce).unwrap_or(config....)` (and the equivalent for `socrates_enforce`).

- [ ] **Step 2: Write the failing test for `attention_fields.rs`.** Add near its existing tests:

```rust
    #[test]
    fn category_or_source_risk_policy_escalates_approval_tier_without_explicit_hint() {
        let mut task = AgentTask::new(TaskId(1), "t", TaskPriority::Normal, vec![]);
        task.trigger_source = Some(crate::mode::TriggerSource::Automated);
        let mut source = std::collections::HashMap::new();
        source.insert(
            "Automated".to_string(),
            crate::config::TaskPolicyEntry { clutch: None, risk: Some("low".to_string()) },
        );
        let overrides = crate::config::TaskPolicyOverrides { category: std::collections::HashMap::new(), source };
        let tier = TaskPriority::Normal.default_approval_tier(); // or whatever this file's existing baseline-tier helper is named — confirm against live code
        let escalated = effective_approval_tier(&task, &overrides, tier); // new small function, Step 3
        assert_eq!(escalated, crate::mode::RiskPosture::Low.resolve().approval.to_approval_tier());
    }
```

*(The baseline-tier helper name (`default_approval_tier` above is a placeholder) must be confirmed against the real function computing `tier` before this test is written for real — read the call site immediately before the current `match task.risk_posture { ... }` block to find it.)*

- [ ] **Step 3: Extract and fix.** Replace the `match task.risk_posture { Some(_) => ..., None => tier }` block with a small function mirroring `resolve_task_cost_policy`'s shape:

```rust
fn effective_approval_tier(
    task: &crate::types::AgentTask,
    overrides: &crate::config::TaskPolicyOverrides,
    baseline: ApprovalTier, // exact type name per this file's existing import
) -> ApprovalTier {
    let (_category_clutch, category_risk) =
        crate::mode::effective_category_policy(overrides, task.task_category);
    let source = task.trigger_source.unwrap_or(crate::mode::TriggerSource::Interactive);
    let (_source_clutch, source_risk) = crate::mode::effective_source_policy(overrides, source);
    let risk = task.risk_posture.or(category_risk).or(source_risk);
    match risk {
        Some(r) => baseline.max_strictness(r.resolve().approval.to_approval_tier()),
        None => baseline,
    }
}
```

Call it in place of the old `match`, passing in the `overrides` fetched the same way Task 2.3 fetches them (`crate::sync_lock::rw_read(&*self.orchestrator.config).task_policy.clone()`, or whatever this file's existing config-access pattern is — confirm at execution time, this file may read config differently than `runtime.rs`).

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-orchestrator --lib orchestrator::task_dispatch::submit::attention_fields:: 2>test_out.log; tail -60 test_out.log`
Expected: PASS, including all pre-existing tests in the file unchanged.

- [ ] **Step 5: Repeat Steps 2-4 for `socrates.rs`**, using the same `task.risk_posture.or(category_risk).or(source_risk)` fallthrough in place of `task.risk_posture.map(|_| ...).unwrap_or(...)` for both `grounding_enforce` and `socrates_enforce`. Write one test per field (`category_or_source_risk_policy_enables_grounding_without_explicit_hint`, `..._enables_socrates_without_explicit_hint`), run `cargo test -p vox-orchestrator --lib orchestrator::task_dispatch::complete::success::socrates::`, confirm PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/attention_fields.rs crates/vox-orchestrator/src/orchestrator/task_dispatch/complete/success/socrates.rs
git commit -m "fix(vox-orchestrator): approval-tier + grounding/Socrates enforcement honor task-type risk policy"
```

---

## Phase 3 — MCP wiring

### Task 3.1: `SubmitTaskParams` carries the hints; `submission.rs` forwards them

**Security note (raised by adversarial review, deliberately accepted, not fixed here):** exposing `clutch`/`risk` on the generic `vox_task_submit` MCP tool means any caller of that tool can set `risk: "high"`, which (via `crates/vox-orchestrator/src/orchestrator/task_dispatch/complete/success/socrates.rs`) unconditionally disables grounding/Socrates enforcement with no floor — unlike the sibling approval-tier path (`attention_fields.rs`), which already protects against a permissive risk hint *demoting* below a trust-classified baseline via `max_strictness`. This is a real widening of who can reach that knob, but it is consistent with, not worse than, `vox_task_submit`'s existing trust model: `model_override` (pick an arbitrary model) and `budget.max_cost_usd` are already exposed on the same tool with no per-caller authorization check today. Hardening the grounding/Socrates read to floor against a `max_strictness`-style baseline (mirroring `attention_fields.rs`) is a reasonable follow-up, but it's a change to existing, pre-plan behavior (not something this plan introduces) and is out of scope here — noted so the tradeoff is a conscious call, not a blind spot.

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/params.rs`
- Modify: `crates/vox-orchestrator-mcp/src/task_tools/submission.rs`

**Grounding fix (found by adversarial review):** the real function that builds `TaskEnqueueHints` from `SubmitTaskParams` is `pub fn enqueue_hints_from_submit_params(params: &SubmitTaskParams) -> Option<TaskEnqueueHints>` (`submission.rs:90`) — **one argument**, not the five-argument `task_enqueue_hints_from_params(&params, category, campaign_id, benchmark_tier, description)` an earlier draft of this task guessed. Also: `SubmitTaskParams` derives only `#[derive(Debug, Deserialize, JsonSchema)]` — **no `Default`, no `Clone`** — this is a deliberate, file-wide pattern (every MCP param struct in `params.rs` is `Deserialize`-only, since these only ever arrive via JSON-RPC input, never constructed by hand in production code). A test using `SubmitTaskParams { ..., ..Default::default() }` will not compile. Build the test value via `serde_json::from_value` instead, matching how these structs are actually meant to be constructed.

- [ ] **Step 1: Write the failing test.** Add to `submission.rs`'s existing test module (find it with `rg '#\[cfg\(test\)\]' crates/vox-orchestrator-mcp/src/task_tools/submission.rs` — if none exists yet, create one at the bottom of the file):

```rust
#[cfg(test)]
mod trigger_source_forwarding_tests {
    use super::*;
    use crate::params::SubmitTaskParams;

    #[test]
    fn forwards_clutch_risk_trigger_source_hints() {
        let params: SubmitTaskParams = serde_json::from_value(serde_json::json!({
            "description": "t",
            "clutch": "free",
            "risk": "high",
            "trigger_source": "automated",
        }))
        .expect("valid SubmitTaskParams JSON");
        let hints = enqueue_hints_from_submit_params(&params)
            .expect("hints should be produced when clutch/risk/trigger_source are set");
        assert_eq!(hints.clutch.as_deref(), Some("free"));
        assert_eq!(hints.risk.as_deref(), Some("high"));
        assert_eq!(hints.trigger_source.as_deref(), Some("automated"));
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-orchestrator-mcp --lib task_tools::submission::trigger_source_forwarding_tests 2>test_out.log; tail -30 test_out.log`
Expected: FAIL — `SubmitTaskParams` has no `clutch`/`risk`/`trigger_source` fields (and the guard at the top of `enqueue_hints_from_submit_params`, currently returning `None` when every optional field is empty, needs `clutch`/`risk`/`trigger_source` added to its emptiness check or the test's values won't be enough to avoid an early `None`).

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

- [ ] **Step 5: Fix each flagged call site.** **Corrected prediction (an earlier draft of this task guessed wrong here, verified during adversarial review):** `rg 'McpChatModelResolution\s*\{' crates/vox-orchestrator-mcp` finds 27 construction sites across 14 files, but 26 of them already use `..Default::default()` (which absorbs a new field with a `Default` impl automatically — no compile error). **Only one site is exhaustive and will actually be flagged: `crates/vox-orchestrator-mcp/src/models_tools.rs:74-84`.** Add `trigger_source: vox_orchestrator::mode::TriggerSource::Interactive,` to that struct literal, next to its `clutch: None,`/`risk: None,` fields. Still run the build first (Step 4) and treat its output as authoritative — this correction is based on a live grep at review time, not a guarantee about what the file looks like when you execute this task — but do not expect `model_route_policy/tests.rs` to need any change; all 7 of its `McpChatModelResolution` literals already use `..Default::default()`.

- [ ] **Step 6: Run to verify it passes**

Run: `cargo test -p vox-orchestrator-mcp --lib 2>test_out.log; tail -100 test_out.log`
Expected: PASS (full crate test suite, confirming no construction site was missed).

- [ ] **Step 7: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/types.rs crates/vox-orchestrator-mcp/src/models_tools.rs
git commit -m "feat(vox-orchestrator-mcp): McpChatModelResolution carries trigger_source (always Interactive today)"
```

### Task 3.3: Resolve the policy at the top of `resolve_mcp_chat_model_sync_inner`

**⚠️ CRITICAL FIX after adversarial review (2026-08-02) — the original version of this task shipped a silent production regression.** Unconditionally setting `res.clutch = Some(clutch)`/`res.risk = Some(risk)` (where `clutch` falls back to `ClutchProfile::Balanced` when nothing else applies, since the compiled tables start empty) changes `build_selection_request`'s behavior for the *default, most common* case: today, when `res.clutch` is `None` (true everywhere in production right now) and the global `cost_preference` is `Economy` (the actual default — see `crates/vox-orchestrator/src/config/defaults.rs`, "Free-by-default policy"), selection uses `SelectionAxes::COST_FIRST` (70/15/15). Once `res.clutch` is forced to `Some(Balanced)` unconditionally, that same call now takes the `effective_axes(Balanced, Moderate)` branch = `(33,33,34)` — cost weight drops from 70 to 33 for every default-config chat call, with **no explicit policy anywhere causing it**. Every existing test in the 756-line `model_route_policy/tests.rs` suite explicitly overrides `cost_preference` to `Performance`, so none of them exercise the branch that actually changes — this regression would have shipped with a fully green test suite. The fix mirrors Task 2.3's own guard exactly: only override `res.clutch`/`res.risk` when an actual policy source applies (explicit, category, or source) — otherwise leave them `None` so the existing `build_selection_request` fallback (`COST_FIRST`/`BALANCED` based on `preference`) is untouched, preserving today's behavior byte-for-byte until an operator actually configures something.

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/resolve.rs:197-220`

- [ ] **Step 1: Write the regression test FIRST — this is the test that would have caught the bug above.** Find (or add, if none of the 8 `OrchestratorConfig::for_testing()` fixtures in `model_route_policy/tests.rs` cover this) a test using the *default* `cost_preference` (i.e. do NOT override it to `Performance`, unlike every existing fixture in that file) with `McpChatModelResolution { ..Default::default() }` (no explicit clutch/risk), and assert the selection still prefers the cheap/free model exactly as it does today:

```rust
    #[test]
    fn default_economy_preference_with_no_clutch_keeps_cost_first_selection() {
        // Regression guard: with Economy (the real default) and no clutch
        // anywhere (explicit, category, or source), selection must still use
        // COST_FIRST axes exactly as it does today — Task 3.3's wiring below
        // must NOT force a Balanced-axes selection for the unconfigured case.
        let orch = /* build via this file's existing test-orchestrator helper, WITHOUT
                       overriding cost_preference — confirm the exact constructor name
                       against live code; every current fixture in this file overrides
                       cost_preference to Performance, so this is a genuinely new setup */;
        let registry = tiny_registry_with_free_and_paid(); // or this file's equivalent fixture
        let (model, _) = resolve_mcp_chat_model_sync(&orch, "generate a parser", None, McpChatModelResolution {
            task_category: TaskCategory::CodeGen,
            ..Default::default()
        }).expect("resolves");
        assert!(model.is_free, "with no policy configured anywhere, Economy default + no clutch must still pick the free/cheap model (COST_FIRST), matching today's behavior exactly");
    }
```

*(The exact helper names for building a test `Orchestrator`/registry with default, non-overridden `cost_preference` must be confirmed against live code in `model_route_policy/tests.rs` before this test is written for real — every existing fixture overrides `cost_preference`, so this is new setup, not a copy-paste of an existing one.)*

- [ ] **Step 2: Run to verify it passes on the CURRENT (unmodified) code** — this proves the test actually captures today's real behavior before you touch anything:

Run: `cargo test -p vox-orchestrator-mcp --lib default_economy_preference_with_no_clutch_keeps_cost_first_selection 2>test_out.log; tail -30 test_out.log`
Expected: PASS (this is a characterization test of existing behavior, not new functionality — it must pass before Step 4's change and continue passing after).

- [ ] **Step 3: Write the dead-path documentation test** (same as the original version of this task — still useful, just no longer the only test):

```rust
    #[test]
    fn clutch_and_risk_are_populated_before_selection_when_a_policy_applies() {
        // Contrast with Step 1's test: when a category/source policy DOES
        // apply, res.clutch/res.risk must become Some so build_selection_request
        // actually uses effective_axes — this is the dead-path fix, scoped to
        // only fire when something real applies.
        let (clutch, risk) = vox_orchestrator::mode::resolve_task_policy(
            None, None, None, None, Some(vox_orchestrator::mode::ClutchProfile::Free), Some(vox_orchestrator::mode::RiskPosture::High),
        );
        assert_eq!(clutch, vox_orchestrator::mode::ClutchProfile::Free);
        assert_eq!(risk, vox_orchestrator::mode::RiskPosture::High);
    }
```

- [ ] **Step 4: Wire the real call site, WITH the guard.** In `resolve.rs`, immediately after `let mut res = res;` (currently line 211, before the `force_free_pool` check at line 216), insert:

```rust
    {
        let overrides = /* however this function already reads OrchestratorConfig —
                            it does this at line 264-267 via orch.config_handle() +
                            vox_orchestrator::sync_lock::rw_read(&*config_handle);
                            read the guard ONCE here and pull .task_policy off it,
                            reusing that single read for the `preference` lookup
                            immediately below it too instead of reading twice */;
        let (category_clutch, category_risk) =
            vox_orchestrator::mode::effective_category_policy(&overrides.task_policy, res.task_category);
        let (source_clutch, source_risk) =
            vox_orchestrator::mode::effective_source_policy(&overrides.task_policy, res.trigger_source);
        // Guard mirrors Task 2.3's resolve_task_cost_policy exactly: only override
        // res.clutch/res.risk when a real policy applies somewhere. Leaving both
        // None when nothing applies preserves today's COST_FIRST/BALANCED legacy
        // branch in build_selection_request untouched — this is what Step 1's
        // regression test enforces.
        if res.clutch.is_some() || category_clutch.is_some() || source_clutch.is_some() {
            let (clutch, risk) = vox_orchestrator::mode::resolve_task_policy(
                res.clutch, res.risk,
                category_clutch, category_risk,
                source_clutch, source_risk,
            );
            res.clutch = Some(clutch);
            res.risk = Some(risk);
        }
    }
```

- [ ] **Step 5: Run the full resolve.rs / model_route_policy test suite**

Run: `cargo test -p vox-orchestrator-mcp --lib llm_bridge::model_route_policy:: 2>test_out.log; tail -150 test_out.log`
Expected: PASS — all pre-existing tests in `model_route_policy/tests.rs` (756 lines) unchanged, PLUS Step 1's new regression test still passing (proving the guard actually preserved default behavior) AND Step 3's test passing (proving the dead path is genuinely fixed when a policy exists). If Step 1's test fails after Step 4's change, the guard is wrong — do not weaken or delete that test to make it pass; fix the guard.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/resolve.rs crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs
git commit -m "feat(vox-orchestrator-mcp): resolve task-type policy before model selection, only when one applies (fixes dead effective_axes path without changing the unconfigured default)"
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

**Before starting this task:** if this is the first `vox-gui` build/test in this worktree, run `vox run scripts/gui-build.vox` once — per `AGENTS.md`'s documented "Perennial Bug Pattern," `cargo build`/`test -p vox-gui` fails inside `tauri-build` the first time any `git worktree add` builds it, because the release `vox` binary Tauri bundles as a sidecar (and `ui/dist`) don't exist yet in a fresh worktree's `target/`. This applies to every `cargo test -p vox-gui --bins` step in this phase and Phase 5, not just this one — it's called out here since it's the first one.

**⚠️ FIX after adversarial review (2026-08-02): these commands must also signal the orchestrator daemon to reload, like `set_orchestrator_config` already does.** The orchestrator runs as a separate long-lived process (`PersistentDaemon`) holding its own `Arc<RwLock<OrchestratorConfig>>` — `AiTaskProcessor::process` (Task 2.3's wiring) reads that live daemon-process config directly, not a value the GUI process can mutate in place. `set_orchestrator_config` (the function this task mirrors) accounts for this: after writing Vox.toml and bumping the local snapshot, it *also* fires a `tokio::spawn`ned, fire-and-forget RPC (`call_orchestrator_daemon(&daemon, orch_daemon_method::RELOAD_CONFIG, ...)`) so the running daemon actually picks up the change. The original version of this task wrote Vox.toml and updated the GUI-process-local snapshot but never told the daemon to reload — a GUI-set override would silently have no effect on real task execution until the daemon was separately restarted, undermining the spec's own "live override" framing. Both commands below now take a `daemon` parameter and include the reload call, exactly mirroring `set_orchestrator_config`'s existing pattern.

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
/// `TriggerSource` Debug name (e.g. `"CodeGen"`, `"Automated"`). Signals the
/// running orchestrator daemon to reload afterward (fire-and-forget), exactly
/// like `set_orchestrator_config` does — without this, the write only affects
/// what a future daemon restart picks up, not the live process.
#[tauri::command]
pub async fn set_task_policy_override(
    app_handle: tauri::AppHandle,
    scope_kind: String,
    scope_key: String,
    clutch: Option<String>,
    risk: Option<String>,
    daemon: tauri::State<'_, Arc<PersistentDaemon>>,
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

    // Fire-and-forget: tell the running daemon to reload so this override
    // affects real task execution immediately, not just after a restart.
    let daemon: Arc<PersistentDaemon> = daemon.inner().clone();
    tokio::spawn(async move {
        let _ = call_orchestrator_daemon(&daemon, orch_daemon_method::RELOAD_CONFIG, serde_json::json!({})).await;
    });

    Ok(())
}

/// Remove one override (`scope_kind`/`scope_key` as in [`set_task_policy_override`]).
/// Same daemon-reload signal as `set_task_policy_override`.
#[tauri::command]
pub async fn clear_task_policy_override(
    app_handle: tauri::AppHandle,
    scope_kind: String,
    scope_key: String,
    daemon: tauri::State<'_, Arc<PersistentDaemon>>,
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

    let daemon: Arc<PersistentDaemon> = daemon.inner().clone();
    tokio::spawn(async move {
        let _ = call_orchestrator_daemon(&daemon, orch_daemon_method::RELOAD_CONFIG, serde_json::json!({})).await;
    });

    Ok(())
}
```

(`manifest.orchestrator`'s type is confirmed `Option<toml::Table>` — `toml::Table` is a type alias for `toml::map::Map<String, toml::Value>` — so `toml::map::Map::new()` above is correct as written, verified against `crates/vox-package-types/src/manifest.rs` during this plan's review; no further check needed at execution time.)

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

**⚠️ FIX after adversarial review (2026-08-02): the original version of this task had no way to CREATE a first override.** The spec explicitly requires "a minimal panel... listing current overrides as rows... plus an 'add override' control offering the categories/sources that don't yet have one." The original component only ever rendered `Object.entries(overrides.category/source)` — since a fresh install has zero overrides, the panel rendered an empty table with no way to populate it, short of hand-editing Vox.toml. The version below adds the missing control: a dropdown of not-yet-configured category/source names plus clutch/risk selects and an "Add" button.

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

  it("offers only not-yet-configured categories/sources in the add control, and adds one", async () => {
    render(<TaskPolicySection />);
    await waitFor(() => expect(screen.getByText(/CodeGen/i)).toBeInTheDocument());

    // CodeGen already has an override (from the mock) — it must not appear as
    // an addable option; Automated (a source) must, since none are configured.
    const addScopeSelect = screen.getByLabelText(/add override for/i);
    const optionLabels = Array.from(addScopeSelect.querySelectorAll("option")).map((o) => o.textContent);
    expect(optionLabels.some((l) => l?.includes("CodeGen"))).toBe(false);
    expect(optionLabels.some((l) => l?.includes("Automated"))).toBe(true);

    fireEvent.change(addScopeSelect, { target: { value: "source:Automated" } });
    fireEvent.click(screen.getByRole("button", { name: /^add$/i }));

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("set_task_policy_override", {
        scopeKind: "source",
        scopeKey: "Automated",
        clutch: undefined,
        risk: undefined,
      })
    );
  });
});
```

- [ ] **Step 3: Run to verify it fails**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/surfaces/Settings/TaskPolicySection.test.tsx 2>test_out.log; tail -60 test_out.log`
Expected: FAIL — `TaskPolicySection` module not found.

- [ ] **Step 4: Write minimal implementation** `TaskPolicySection.tsx`. `ALL_CATEGORIES` mirrors `contracts/orchestration/model-routing.v1.yaml::task_categories` plus the codegen'd default `General` — this is a static, hand-maintained list (matching how `driveConsole.ts` already hardcodes `CLUTCH_DETENTS`/`RISK_POSTURES` rather than fetching them), so it will drift if a category is renamed in the YAML; acceptable for a minimal panel, revisit if that YAML changes often. `ALL_SOURCES` mirrors `TriggerSource`'s four variants exactly:

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

// Mirrors contracts/orchestration/model-routing.v1.yaml::task_categories + the
// codegen'd #[default] General variant. Hand-maintained, like driveConsole.ts's
// CLUTCH_DETENTS/RISK_POSTURES — update if that YAML's category list changes.
const ALL_CATEGORIES = [
  "General", "CodeGen", "Testing", "Debugging", "TypeChecking", "Research",
  "Parsing", "Review", "Ars", "Planning", "InterAgent", "ToolOrchestration",
  "Visus", "CodeEffortJudge", "Chat",
];
// Mirrors crate::mode::TriggerSource's four variants exactly (Debug-style names).
const ALL_SOURCES = ["Interactive", "Automated", "Subagent", "Mesh"];

export function TaskPolicySection() {
  const [overrides, setOverrides] = useState<TaskPolicyOverrides>({ category: {}, source: {} });
  const [addScope, setAddScope] = useState("");

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

  const addableCategories = ALL_CATEGORIES.filter((c) => !(c in overrides.category));
  const addableSources = ALL_SOURCES.filter((s) => !(s in overrides.source));

  const handleAdd = () => {
    if (!addScope) return;
    const [scopeKind, scopeKey] = addScope.split(":") as ["category" | "source", string];
    setOverride(scopeKind, scopeKey, undefined, undefined);
    setAddScope("");
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

      <div>
        <label htmlFor="task-policy-add-scope">Add override for</label>
        <select id="task-policy-add-scope" value={addScope} onChange={(e) => setAddScope(e.target.value)}>
          <option value="">(choose a category or source)</option>
          {addableCategories.map((c) => (
            <option key={`category:${c}`} value={`category:${c}`}>
              Category: {c}
            </option>
          ))}
          {addableSources.map((s) => (
            <option key={`source:${s}`} value={`source:${s}`}>
              Source: {s}
            </option>
          ))}
        </select>
        <button onClick={handleAdd} disabled={!addScope}>
          Add
        </button>
      </div>
    </section>
  );
}
```

- [ ] **Step 5: Run to verify it passes**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/surfaces/Settings/TaskPolicySection.test.tsx 2>test_out.log; tail -60 test_out.log`
Expected: PASS (3 tests).

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

**⚠️ FIX after adversarial review (2026-08-02): the original version of this task guessed, incorrectly, that `TaskCategory` had no public string parser and left the implementation deliberately unfinished.** It does have one, already used in production: `crates/vox-orchestrator/build.rs` generates `impl std::str::FromStr for TaskCategory` (never errors — lowercases the input, matches against the category list, falls back to `General` for anything unrecognized), and `crates/vox-orchestrator/src/orch_daemon/mod.rs` already calls `.parse::<TaskCategory>()` on a JSON string for exactly this kind of task-category hint. This also resolves what looked like a casing mismatch between this task and Task 5.2 (`category: "Chat"`, capitalized) — the parser lowercases internally, so it accepts either casing. No new parser needed; the version below just uses `.parse()` directly.

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
        // way to the global default, exactly like resolve_task_policy(None, None, None, None, None, None).
        assert_eq!(dto.clutch, "balanced");
        assert_eq!(dto.risk, "moderate");
    }

    #[test]
    fn unknown_category_or_source_labels_fall_back_to_global_default() {
        // TaskCategory::from_str never errors (falls back to General for an
        // unrecognized string), so this exercises "recognized category with no
        // configured policy," not a parse failure — still must land on the
        // same global default since nothing is configured for General either.
        let dto = resolve_default_task_policy("NotARealCategory".to_string(), "not_a_real_source".to_string());
        assert_eq!(dto.clutch, "balanced");
        assert_eq!(dto.risk, "moderate");
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-gui --bins resolve_default_task_policy 2>test_out.log; tail -30 test_out.log`
Expected: FAIL — function not found.

- [ ] **Step 3: Write the implementation.** Add to `orchestrator.rs`:

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
/// never drift from what would actually happen. `category` is any string
/// `TaskCategory::from_str` accepts (case-insensitive, never errors — falls
/// back to `General`); `source` is a `TriggerSource::from_label` string
/// (`"interactive"`/`"automated"`/`"subagent"`/`"mesh"`, case-insensitive) —
/// an unrecognized `source` falls back to `Interactive` via the same
/// `unwrap_or(TriggerSource::Interactive)` pattern `resolved_policy()` uses
/// elsewhere, not a parse error.
#[tauri::command]
pub fn resolve_default_task_policy(category: String, source: String) -> DefaultTaskPolicyDto {
    use vox_orchestrator::types::TaskCategory;
    use vox_orchestrator::mode::TriggerSource;

    let overrides = vox_orchestrator::config::OrchestratorConfig::snapshot().task_policy;
    let category: TaskCategory = category.parse().unwrap_or_default();
    let source = TriggerSource::from_label(&source).unwrap_or(TriggerSource::Interactive);

    let (category_clutch, category_risk) =
        vox_orchestrator::mode::effective_category_policy(&overrides, category);
    let (source_clutch, source_risk) =
        vox_orchestrator::mode::effective_source_policy(&overrides, source);
    let (clutch, risk) = vox_orchestrator::mode::resolve_task_policy(
        None, None,
        category_clutch, category_risk,
        source_clutch, source_risk,
    );
    DefaultTaskPolicyDto {
        clutch: clutch_label(clutch).to_string(),
        risk: risk_label(risk).to_string(),
    }
}
```

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

**Note:** `category: 'Chat'` (capitalized) below is fine as written — Task 5.1's `resolve_default_task_policy` parses it via `TaskCategory::from_str`, which lowercases its input before matching, so both `'Chat'` and `'chat'` resolve identically. No casing convention to reconcile here.

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

## Parallelism and orchestration for executing this plan

**Dependency shape:** this plan is a single dependency chain, not a fan-out. Phase 1 (`mode.rs`/`orchestrator_fields.rs`) is a hard prerequisite for everything downstream (every later phase calls `resolve_task_policy`/`effective_category_policy`/`effective_source_policy`). Within Phase 1, Tasks 1.1→1.2→1.3→1.4 are sequential (1.2 uses 1.1's enum; 1.3 uses 1.2's resolver). Phase 2 depends on Phase 1. Phase 3 depends on Phase 1 (not Phase 2 — the MCP path and the `AgentTask` path are independent consumers of the same Phase 1 resolver). Phase 4 depends on Phase 1 (for the types) but not Phase 2/3. Phase 5 depends on Phase 4's Tauri-command pattern. Phase 6 depends on everything.

**Genuinely parallel-safe pairs (disjoint files, no cross-item dependency):** once Phase 1 is merged, **Phase 2 (`vox-orchestrator`: `tasks.rs`, `runtime.rs`, `attention_fields.rs`, `socrates.rs`) and Phase 3 (`vox-orchestrator-mcp`: `params.rs`, `submission.rs`, `model_route_policy/*`)** touch entirely different crates and could run as two parallel Agent-tool subagents. Phase 4 (`vox-gui` backend commands) could also start in parallel with Phase 2/3 once Phase 1's types exist, since it only needs `TaskPolicyOverrides`/the resolver signatures, not anything Phase 2/3 produce. Within Phase 2, Task 2.4 (`attention_fields.rs` + `socrates.rs`) is disjoint from Task 2.3 (`runtime.rs`) and could run alongside it. Task 4.3 (frontend) has a soft dependency on Task 4.1/4.2 (the commands it calls) existing, but could be scaffolded against a mock in parallel and integrated after. Any step ambiguous enough to need a human checkpoint rather than unattended execution: **Task 3.3** (the guard fix is the single highest-consequence correctness decision in this plan — a human should review the diff before it merges, not just trust green tests) and **Task 2.4** (touches two files this plan's author did not have live output from a compiler for, unlike everywhere else — the "confirm against live code" caveats in that task are real, not decorative).

**Does this plan warrant the Workflow tool for its own execution?** No, and it's worth saying explicitly rather than leaving it unaddressed. Workflow earns its overhead for genuine multi-stage fan-out with a real barrier (a discovery step gating a transform step), a large volume of independent similarly-shaped sub-items (a golden-corpus, a bulk migration across dozens of call sites), or a verify-after-generate loop needing adversarial cross-checking. This plan is ~19 tasks in a single, mostly-sequential dependency chain (Phase 1 gates everything; the two genuinely-parallel branches — Phase 2 and Phase 3 — are two items, not a "volume" of similarly-shaped sub-items). The one place this plan touches something workflow-shaped — Task 2.4 fixing the same bug pattern at two more call sites after Task 2.3 fixed the first instance — is exactly two sites, found by one review pass, not an open-ended sweep; a `rg` for the same gate pattern (`\.risk_posture\.map\(\|_\|`/`\.clutch_profile\.is_some\(\)` outside the three files this plan already touches) is a reasonable one-line addition to Task 2.4 if you want confidence there's a fourth site, but even that's a single grep, not a fan-out. Recommending Workflow here — a resumable background pipeline for a linear ~19-task plan a human can review step-by-step — would itself be the scope/YAGNI violation this review was asked to watch for. Use `subagent-driven-development` (fresh subagent per task, review between tasks) or `executing-plans` (batch with checkpoints), per this plan's own header.

---

## Self-Review

**Note: this section was rewritten after an adversarial multi-dimension review (2026-08-02) found and fixed 5 blockers and 8 majors in the original version of this plan (see the "Adversarial Review — Fixes Applied" section below for the full list). The bullets below describe the plan as it stands now, post-fix, not the original draft.**

- **Spec coverage:** `TriggerSource` enum → Task 1.1. Resolver + precedence (now genuinely per-axis independent, matching the spec's explicit claim — see Fix #4 below) → Task 1.2. Storage (compiled defaults + Vox.toml overrides, adjusted to live in `vox-orchestrator` per the grounding correction noted at the top; `deny_unknown_fields` version-skew risk fixed at the source, not just documented) → Task 1.3. `AgentTask` wiring, including the two sibling call sites (`attention_fields.rs`/`socrates.rs`) the original draft missed → Phase 2. MCP-direct wiring (the concrete "CI/CD inbox" example from the spec's data-flow section), now WITHOUT the silent default-behavior regression the original draft shipped → Phase 3. GUI panel, now WITH the "add override" control the spec explicitly required → Phase 4. Chat unification, now WITH a daemon-reload signal so a GUI override actually reaches the live process → Phase 5. Deferred-per-spec items (llm_bridge consolidation, full Band B, combo rules, tenant policy) remain untouched, verified by absence.
- **Placeholder scan:** the remaining "confirm against live code" call-outs (Task 3.3's config-handle read pattern, Task 4.3's styling conventions, Task 5.2's test-scaffolding reuse, Task 2.4's exact gate-code shape in two files not read during this review) are each a concrete, bounded, one-file check with a specific `rg`/read command attached — not an open-ended TODO. Task 5.1's previous `unimplemented!()` placeholder is gone — it now has a complete, verified implementation using `TaskCategory::from_str` (confirmed to exist and work as needed, not guessed).
- **Type consistency:** `resolve_task_policy` now takes 6 independent `Option` parameters (`explicit_clutch, explicit_risk, category_clutch, category_risk, source_clutch, source_risk`) — every call site (`effective_category_policy`/`effective_source_policy`'s own callers in Tasks 1.3's tests, 2.2, 2.3, 3.3, 5.1) was updated to the new signature and independently re-checked in this pass, not just asserted. `effective_category_policy`/`effective_source_policy` now return `(Option<ClutchProfile>, Option<RiskPosture>)` instead of the old `Option<(ClutchProfile, RiskPosture)>` — every caller destructures the pair, not the old `Option`-of-tuple.

## Adversarial Review — Fixes Applied (2026-08-02)

A 6-dimension independent review (correctness-vs-live-code, security, test coverage, operational readiness, scope/YAGNI, spec-plan consistency) plus direct verification against the live repo found the following. Every item below was independently confirmed against real files before being acted on; none is taken on the reviewing agent's word alone.

**Fixed in the plan document:**
1. **[blocker]** `TaskPolicyOverrides`/`TaskPolicyEntry` were never added to `crates/vox-orchestrator/src/config/mod.rs`'s explicit (non-glob) re-export list — invisible as `crate::config::TaskPolicyOverrides` anywhere outside `orchestrator_fields.rs`. Fixed: Task 1.3 now edits `mod.rs`.
2. **[blocker]** `OrchestratorConfig` doesn't derive `Default` — it has a hand-written, exhaustive literal in `impl_default.rs`. A new field without a matching line there is `E0063`. Fixed: Task 1.3 now edits `impl_default.rs`.
3. **[blocker]** Task 3.1's test called a function with the wrong name and arity (`task_enqueue_hints_from_params(&params, None, None, None, "t")` vs. the real `enqueue_hints_from_submit_params(&params)`), and constructed `SubmitTaskParams` via `..Default::default()` on a struct that (by deliberate, file-wide design) derives no `Default`. Fixed: corrected function name/arity, test now builds the value via `serde_json::from_value`.
4. **[blocker, spec-contradicting]** The original `resolve_task_policy`/`effective_category_policy`/`effective_source_policy` modeled clutch+risk as an inseparable pair per precedence level (`Option<(ClutchProfile, RiskPosture)>`), so a category policy that only set clutch would force risk to jump straight to the global default, never consulting the source-level policy — contradicting the spec's explicit "clutch and risk resolve independently" claim and silently breaking what Task 4.3's GUI (independent clutch/risk dropdowns) promises. Fixed: refactored to per-axis independent `Option`s throughout Phase 1 and every downstream caller.
5. **[blocker, silent regression]** Task 3.3's original wiring unconditionally forced `res.clutch`/`res.risk` to `Some(_)`, changing `build_selection_request`'s axes from `COST_FIRST` (70/15/15, today's actual default-config behavior) to `BALANCED` (33/33/34) for every unconfigured chat call — invisible to all 8 existing tests, which all override `cost_preference` away from the default. Fixed: added the same "only override when a real policy applies" guard Task 2.3 already had, plus a new regression test that would have caught this.
6. **[major]** Task 3.2 predicted the compiler would flag `models_tools.rs` AND `model_route_policy/tests.rs`; a live `rg` across all 27 `McpChatModelResolution` construction sites in 14 files found only `models_tools.rs` is exhaustive (everything else, including all 7 sites in `tests.rs`, already uses `..Default::default()`). Fixed: corrected the prediction and removed the unnecessary `tests.rs` edit/commit.
7. **[major]** Task 4.2's new commands wrote Vox.toml and bumped the GUI-process-local snapshot but never signaled the orchestrator daemon (a separate long-lived process) to reload — a GUI-set override would have had no effect on real task execution until a daemon restart. Fixed: both commands now take a `daemon` parameter and fire the same `RELOAD_CONFIG` RPC `set_orchestrator_config` already uses.
8. **[major, spec-contradicting]** Task 4.3's GUI panel had no way to create a first override — it only rendered pre-existing entries, and the spec explicitly requires an "add override" control. Fixed: added an add-scope dropdown (offering only not-yet-configured categories/sources) + Add button, with a test.
9. **[major]** Two more call sites (`attention_fields.rs` approval-tier escalation, `socrates.rs` grounding/Socrates enforcement) have the identical "only consult risk policy when an explicit hint is set" bug Task 2.3 fixes in `runtime.rs` — left unfixed, a category/source *risk* policy would have no effect outside cost preference. Fixed: added Task 2.4, reusing the same resolver machinery.
10. **[major]** Task 5.1 incorrectly asserted `TaskCategory` had no public string parser and shipped a deliberately-incomplete `unimplemented!()` placeholder; a working `impl FromStr for TaskCategory` already exists and is already used in production. Fixed: Task 5.1 now has a complete implementation using `.parse()`; this also resolved an apparent casing mismatch with Task 5.2 (the parser lowercases internally, so it doesn't matter).
11. **[major, documentation]** The original Self-Review's "Type consistency" bullet misattributed a `resolve_task_policy` call to Task 1.3 (which doesn't call it — it calls `effective_category_policy`/`effective_source_policy`, the resolver's *inputs*) and miscounted "six uses" against its own five-item list. Fixed: rewritten (see above).
12. **[minor]** Several quoted line-number citations had drifted from the live file by small amounts (e.g. `runtime.rs:628-644` vs. the real `619-634`). Fixed where cited exactly; loosened to "verify against the live file" language elsewhere, since line numbers drift structurally in any plan written before execution.

**Fixed in actual code, not just documented (per explicit user instruction mid-review):**
13. **[blocker-adjacent operational risk]** `OrchestratorConfig` had `#[serde(deny_unknown_fields, default)]`, and its own test suite documents a real production incident (PR #349) where an unrecognized value wiped the *entire* `[orchestrator]` section to defaults. Adding `task_policy` would have carried the identical exposure for the *key* case (an older binary reading a newer one's Vox.toml). Rather than accept this as inherited risk, it was fixed directly: `deny_unknown_fields` removed; a `#[serde(flatten, skip_serializing)] unrecognized_fields: BTreeMap<String, toml::Value>` field now absorbs unrecognized keys instead of failing the parse; `load_from_toml` logs a warning listing what was ignored. Landed in `crates/vox-orchestrator/src/config/{orchestrator_fields.rs,impl_default.rs,impl_load.rs}`, with a new regression test (`unrecognized_orchestrator_key_does_not_wipe_the_section`). Verified: `cargo test -p vox-orchestrator --lib config::` — 1079 tests, all passing, including the new one, with no regressions from removing `deny_unknown_fields`.

**Flagged, not fixed (deliberate, with reasoning given):**
- Security: exposing `risk` on the generic `vox_task_submit` MCP tool widens who can disable grounding/Socrates enforcement (no floor, unlike the sibling approval-tier code). Judged consistent with, not worse than, that tool's existing trust model (`model_override`/`budget` are already ungated the same way) — noted in Task 3.1, not redesigned.
- The spec's "vox doctor check for malformed entries" testing requirement is partially addressed (a `tracing::warn!` fires on unparseable override labels, per Task 1.3) but a dedicated `vox doctor` CLI surface was not added — that would require grounding in the doctor-check architecture beyond what this review pass covered.
- `crates/vox-orchestrator/src/orchestrator_policy.rs`'s pre-existing `TODO(risk-safety-budget)` (budget-gate aggressiveness from `ClutchProfile` is still unwired at the one call site that would consume it) predates this plan and is out of scope — noted for completeness, not a new gap this plan introduces.

**Could not be verified either way:** the exact current shape of `attention_fields.rs`'s baseline-tier-computation call site and `socrates.rs`'s config-access pattern (Task 2.4) — flagged in that task as needing a live read at execution time, since this review's agents described the *gate logic* precisely but not every surrounding line, and no one on this review pass re-verified those two files as thoroughly as the ones with actual compiler output attached (Phase 1's `deny_unknown_fields` fix, which was compiled and tested directly).
