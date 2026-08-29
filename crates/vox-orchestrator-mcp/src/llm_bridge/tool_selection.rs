//! Task 1.2 (harness implementation spec,
//! `docs/src/architecture/vox-harness-implementation-spec-2026-07-31.md` §3.2):
//! deterministic, unit-testable tool-selection filter for a single chat turn.
//!
//! Vox's MCP tool registry ([`vox_mcp_registry::TOOL_REGISTRY`]) has 330
//! entries. Published research on tool-selection accuracy (cited in the
//! implementation spec) documents degradation once a model is offered more
//! than roughly 30-50 tools in one turn — 330 is 6.6x that. This module does
//! NOT send anything to a model; it only narrows `TOOL_REGISTRY` down to a
//! small relevant subset for a turn, which a *future* task (1.3, the agent
//! loop) will use to build the wire-format tool list actually sent.
//!
//! Filters are applied in a fixed order: permission -> lane -> active-skill
//! allowlist -> cap. See [`select_tools_for_turn`].

use vox_mcp_registry::McpToolRegistryEntry;
use vox_skills::SkillRegistry;

use crate::skill_permissions::check_skill_tool_permission;

/// Thin re-export of `vox_skills::new_registry_arc` so `vox-cli`'s
/// `tool-cap-never-exceeds-cap` golden task (`vox harness eval`) can build a
/// fixture [`SkillRegistry`] for [`select_tools_for_turn`] without taking a
/// direct (optional, `ars`-feature-gated) dependency on `vox-skills` itself —
/// `vox-orchestrator-mcp` already depends on it unconditionally.
#[must_use]
pub fn new_registry_arc_for_eval() -> std::sync::Arc<SkillRegistry> {
    vox_skills::new_registry_arc()
}

/// Default cap on the number of tools offered per turn.
///
/// Chosen well under the ~30-50-tool degradation threshold cited in the
/// implementation spec, while leaving headroom for the always-on skill
/// infrastructure tools (`vox_skill_*`, `vox_chat_*`) that
/// [`check_skill_tool_permission`] never filters out. 40 sits comfortably
/// inside that range without hugging its upper edge.
pub const DEFAULT_MAX_TOOLS: usize = 40;

/// Wire-string used by [`crate::permission_modes::PermissionMode::from_wire`]
/// for its read-only/planning mode. `PermissionMode::Plan` is documented there
/// as "Read-only/planning mode" — the closest existing canonical concept to
/// "read-only" in this crate's permission vocabulary, so this filter reuses
/// the literal `"plan"` rather than inventing a new string. Note that for the
/// *dispatch* HITL gate `Plan` behaves identically to `Ask` (auto-approves
/// nothing); here we additionally interpret it as "restrict the tool surface
/// to `http_read_role_eligible` tools only". If task 1.3 (the agent loop)
/// establishes a different / more granular read-only vocabulary, this
/// constant should be reconciled with it then.
pub const READ_ONLY_PERMISSION_MODE: &str = "plan";

/// Everything needed to filter [`vox_mcp_registry::TOOL_REGISTRY`] down to a
/// small, relevant subset for one chat turn.
#[derive(Debug, Clone)]
pub struct TurnContext {
    /// Wire-level permission mode string (see
    /// `DispatchRequest::permission_mode` / `PermissionMode::from_wire`).
    /// `Some(READ_ONLY_PERMISSION_MODE)` restricts selection to
    /// `http_read_role_eligible` tools only. Any other value (including
    /// `None`) is a no-op for this filter — permission-mode enforcement for
    /// non-read-only modes already happens later, in `dispatch.rs`'s HITL
    /// gate, not here.
    pub permission_mode: Option<String>,
    /// Which `product_lane`s are relevant for this turn (e.g. a plain chat
    /// turn might want `["ai", "app"]` but not `"platform"`/`"interop"`).
    /// An empty vec means "no lane restriction" (all lanes pass) rather than
    /// "nothing passes" — callers that want zero tools should not call this
    /// function at all.
    pub lanes: Vec<&'static str>,
    /// The currently active skill, if any, for the per-skill allowlist
    /// filter (see [`check_skill_tool_permission`]).
    pub active_skill_id: Option<String>,
    /// Hard cap on the number of tools returned. See [`DEFAULT_MAX_TOOLS`].
    pub max_tools: usize,
    /// Tool-name prefixes to exclude from the candidate set entirely, applied
    /// BEFORE the `max_tools` cap (unlike a post-hoc filter, this guarantees
    /// excluded tools never consume a cap slot). Callers that must never offer
    /// a given family of tools — e.g. the agent loop excluding `vox_chat_*` to
    /// prevent unbounded re-entrant recursion into `run_agent_turn` — should
    /// list those prefixes here rather than filtering the already-capped
    /// result, which can silently shrink the effective tool count below
    /// `max_tools` and can drop a genuinely useful tool that only lost its cap
    /// slot to an excluded one. Empty by default (no exclusion).
    pub exclude_name_prefixes: Vec<&'static str>,
    /// Tool names hoisted to the front of the candidate list, applied BEFORE
    /// the `max_tools` cap. Registry order is alphabetical, so a plain
    /// truncation silently makes late-alphabet tools unreachable no matter how
    /// relevant they are — `vox_spawn_agent` sits at index ~166 of the ~188
    /// candidates a default chat turn produces, well past the 40-tool cut.
    /// Listing a name here guarantees it a cap slot.
    ///
    /// Pins are subject to every other filter (read-only permission, lane,
    /// active-skill allowlist, prefix exclusion) — a pin is a priority hint,
    /// not a permission bypass. Names not present in the registry, or filtered
    /// out by an earlier step, are simply ignored.
    pub pin_names: Vec<&'static str>,
}

impl Default for TurnContext {
    /// A plain chat turn: `ai` + `app` lanes, no active skill, default cap,
    /// no permission restriction, no name-prefix exclusions.
    fn default() -> Self {
        Self {
            permission_mode: None,
            lanes: vec!["ai", "app"],
            active_skill_id: None,
            max_tools: DEFAULT_MAX_TOOLS,
            exclude_name_prefixes: Vec::new(),
            pin_names: Vec::new(),
        }
    }
}

impl TurnContext {
    fn is_read_only(&self) -> bool {
        self.permission_mode.as_deref() == Some(READ_ONLY_PERMISSION_MODE)
    }

    fn lane_allowed(&self, lane: &str) -> bool {
        self.lanes.is_empty() || self.lanes.contains(&lane)
    }
}

/// Select the tool subset for one chat turn.
///
/// Applies, in order:
/// 1. Permission filter — read-only mode keeps only `http_read_role_eligible`
///    tools.
/// 2. Lane filter — keeps only tools whose `product_lane` is in
///    `ctx.lanes` (no-op if `ctx.lanes` is empty).
/// 3. Active-skill filter — drops any tool [`check_skill_tool_permission`]
///    denies for `ctx.active_skill_id` (skill-infrastructure tools such as
///    `vox_skill_list`/`vox_chat_*` are never denied by that function, so
///    they always survive this step regardless of the active skill).
/// 4. Name-prefix exclusion — drops any tool whose name starts with one of
///    `ctx.exclude_name_prefixes` (no-op if empty).
/// 5. Pin hoist — moves any survivor named in `ctx.pin_names` to the front,
///    preserving registry order within each group (no-op if empty).
/// 6. Cap — truncates to the first `ctx.max_tools` survivors.
///
/// Both the exclusion filter and the pin hoist run BEFORE the cap: excluded
/// tools never consume a cap slot, and pinned tools are always inside it.
/// Capping first and adjusting after would shrink the effective tool count
/// below `max_tools` and could bump a genuinely usable tool out for no benefit.
///
/// Apart from the pin hoist, step 6 is a plain truncation over registry
/// (alphabetical) order, not a relevance ranking. `pin_names` exists precisely
/// because that makes late-alphabet tools unreachable; a future task may want
/// a real relevance sort instead.
#[must_use]
pub fn select_tools_for_turn(
    registry: &'static [McpToolRegistryEntry],
    skill_registry: &SkillRegistry,
    ctx: &TurnContext,
) -> Vec<&'static McpToolRegistryEntry> {
    let (pinned, rest): (Vec<_>, Vec<_>) = registry
        .iter()
        .filter(|entry| !ctx.is_read_only() || entry.http_read_role_eligible)
        .filter(|entry| ctx.lane_allowed(entry.product_lane))
        .filter(|entry| {
            check_skill_tool_permission(skill_registry, ctx.active_skill_id.as_deref(), entry.name)
                .is_none()
        })
        .filter(|entry| {
            !ctx.exclude_name_prefixes
                .iter()
                .any(|prefix| entry.name.starts_with(prefix))
        })
        .partition(|entry| ctx.pin_names.contains(&entry.name));
    pinned.into_iter().chain(rest).take(ctx.max_tools).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use vox_mcp_registry::TOOL_REGISTRY;
    use vox_plugin_api::skill::LoadedSkill;
    use vox_skills::{SkillManifest, new_registry_arc};

    fn install_restrictive_skill(reg: &SkillRegistry) {
        reg.install(LoadedSkill {
            plugin_id: "narrow-skill".to_string(),
            format_version: 1,
            manifest: SkillManifest {
                id: "narrow-skill".to_string(),
                name: "narrow-skill".to_string(),
                version: "1.0.0".to_string(),
                description: "only allows one domain tool".to_string(),
                tools: vec!["vox_git_status".to_string()],
                ..Default::default()
            },
            body: String::new(),
            exposed_tools: vec!["vox_git_status".to_string()],
        });
    }

    #[test]
    fn default_context_on_real_registry_is_nonempty_and_capped() {
        let reg = new_registry_arc();
        let ctx = TurnContext::default();
        let selected = select_tools_for_turn(TOOL_REGISTRY, &reg, &ctx);
        assert!(!selected.is_empty());
        assert!(selected.len() <= ctx.max_tools);
    }

    #[test]
    fn read_only_mode_keeps_only_http_read_role_eligible_tools() {
        let reg = new_registry_arc();
        let ctx = TurnContext {
            permission_mode: Some(READ_ONLY_PERMISSION_MODE.to_string()),
            lanes: vec![],
            active_skill_id: None,
            max_tools: TOOL_REGISTRY.len(),
            exclude_name_prefixes: vec![],
            pin_names: vec![],
        };
        let selected = select_tools_for_turn(TOOL_REGISTRY, &reg, &ctx);
        assert!(!selected.is_empty());
        assert!(selected.iter().all(|t| t.http_read_role_eligible));
    }

    #[test]
    fn lane_filter_excludes_tools_outside_requested_lanes() {
        let reg = new_registry_arc();
        let allowed: HashSet<&'static str> = ["data", "interop"].into_iter().collect();
        let ctx = TurnContext {
            permission_mode: None,
            lanes: allowed.iter().copied().collect(),
            active_skill_id: None,
            max_tools: TOOL_REGISTRY.len(),
            exclude_name_prefixes: vec![],
            pin_names: vec![],
        };
        let selected = select_tools_for_turn(TOOL_REGISTRY, &reg, &ctx);
        assert!(!selected.is_empty());
        assert!(selected.iter().all(|t| allowed.contains(t.product_lane)));
        // Sanity: the registry does contain tools outside {data, interop}
        // (e.g. ai/app/platform/workflow lanes), so this filter is doing real work.
        assert!(
            TOOL_REGISTRY
                .iter()
                .any(|t| !allowed.contains(t.product_lane))
        );
    }

    #[test]
    fn active_skill_allowlist_shrinks_the_selected_set() {
        let reg = new_registry_arc();
        install_restrictive_skill(&reg);
        let ctx_no_skill = TurnContext {
            permission_mode: None,
            lanes: vec![],
            active_skill_id: None,
            max_tools: TOOL_REGISTRY.len(),
            exclude_name_prefixes: vec![],
            pin_names: vec![],
        };
        let ctx_with_skill = TurnContext {
            active_skill_id: Some("narrow-skill".to_string()),
            ..ctx_no_skill.clone()
        };
        let without_skill = select_tools_for_turn(TOOL_REGISTRY, &reg, &ctx_no_skill);
        let with_skill = select_tools_for_turn(TOOL_REGISTRY, &reg, &ctx_with_skill);
        assert!(with_skill.len() < without_skill.len());
    }

    #[test]
    fn skill_infrastructure_tools_survive_restrictive_active_skill() {
        let reg = new_registry_arc();
        install_restrictive_skill(&reg);
        let ctx = TurnContext {
            permission_mode: None,
            lanes: vec![],
            active_skill_id: Some("narrow-skill".to_string()),
            max_tools: TOOL_REGISTRY.len(),
            exclude_name_prefixes: vec![],
            pin_names: vec![],
        };
        let selected = select_tools_for_turn(TOOL_REGISTRY, &reg, &ctx);
        let selected_names: HashSet<&str> = selected.iter().map(|t| t.name).collect();
        for infra in ["vox_skill_list", "vox_skill_search"] {
            if TOOL_REGISTRY.iter().any(|t| t.name == infra) {
                assert!(
                    selected_names.contains(infra),
                    "expected skill-infrastructure tool {infra} to survive the active-skill filter"
                );
            }
        }
        for name in TOOL_REGISTRY.iter().map(|t| t.name) {
            if name.starts_with("vox_chat_") {
                assert!(
                    selected_names.contains(name),
                    "expected vox_chat_* tool {name} to survive the active-skill filter"
                );
            }
        }
    }

    /// Regression: an excluded prefix (e.g. `vox_chat_*` in the agent loop) must
    /// not consume a cap slot. Before the fix, excluding `vox_chat_*` happened
    /// AFTER `select_tools_for_turn`'s `.take(max_tools)`, so any `vox_chat_*`
    /// entries within the first `max_tools` registry-order entries would occupy a
    /// slot and then be discarded by the caller's post-filter — leaving turns
    /// with fewer than `max_tools` usable tools and potentially pushing a
    /// genuinely useful tool just past the cap boundary out of reach. Now the
    /// exclusion runs inside `select_tools_for_turn`, before the cap, so the
    /// full `max_tools` budget is always spent on genuinely-usable tools (up to
    /// however many survive every other filter).
    #[test]
    fn excluded_prefix_does_not_consume_a_cap_slot() {
        let reg = new_registry_arc();

        // Sanity: the real registry does contain vox_chat_* entries within
        // registry order, so this test exercises the bug it targets.
        assert!(
            TOOL_REGISTRY
                .iter()
                .any(|t| t.name.starts_with("vox_chat_")),
            "test assumption: TOOL_REGISTRY must contain at least one vox_chat_* tool"
        );

        // A small cap chosen so that, in plain registry order without exclusion,
        // at least one vox_chat_* tool would land inside the cap window (proving
        // the old post-filter bug would have shrunk the result below the cap).
        let small_cap = TOOL_REGISTRY
            .iter()
            .position(|t| t.name.starts_with("vox_chat_"))
            .map(|idx| idx + 1)
            .unwrap_or(TOOL_REGISTRY.len())
            .max(1);

        let ctx = TurnContext {
            permission_mode: None,
            lanes: vec![],
            active_skill_id: None,
            max_tools: small_cap,
            exclude_name_prefixes: vec!["vox_chat_"],
            pin_names: vec![],
        };
        let selected = select_tools_for_turn(TOOL_REGISTRY, &reg, &ctx);

        assert!(
            selected.iter().all(|t| !t.name.starts_with("vox_chat_")),
            "excluded vox_chat_* tools must never appear in the result"
        );
        assert_eq!(
            selected.len(),
            small_cap,
            "the cap slot vacated by an excluded tool must be backfilled by the \
             next genuinely-usable tool, not left empty — got {} of {small_cap} \
             expected usable tools",
            selected.len()
        );
    }

    /// The three delegation tools the chat agent loop pins.
    const DELEGATION_TOOLS: [&str; 3] = ["vox_spawn_agent", "vox_submit_task", "vox_task_status"];

    /// Regression for the reachability bug this pin hatch exists to fix: on a
    /// default chat turn (`ai`+`app` lanes, cap 40) registry order is
    /// alphabetical, so `vox_spawn_agent` sat at candidate index ~166 of ~188
    /// and was never offered. Fails without `pin_names`.
    #[test]
    fn default_chat_turn_offers_the_delegation_tools_when_pinned() {
        let reg = new_registry_arc();
        let ctx = TurnContext {
            pin_names: DELEGATION_TOOLS.to_vec(),
            exclude_name_prefixes: vec!["vox_chat_"],
            ..TurnContext::default()
        };
        let selected = select_tools_for_turn(TOOL_REGISTRY, &reg, &ctx);
        let names: HashSet<&str> = selected.iter().map(|t| t.name).collect();
        for tool in DELEGATION_TOOLS {
            assert!(
                names.contains(tool),
                "pinned delegation tool {tool} missing from a default chat turn's \
                 offered set ({} tools)",
                selected.len()
            );
        }
    }

    #[test]
    fn pinning_does_not_exceed_the_cap() {
        let reg = new_registry_arc();
        let unpinned = select_tools_for_turn(TOOL_REGISTRY, &reg, &TurnContext::default());
        let ctx = TurnContext {
            pin_names: DELEGATION_TOOLS.to_vec(),
            ..TurnContext::default()
        };
        let selected = select_tools_for_turn(TOOL_REGISTRY, &reg, &ctx);
        assert_eq!(selected.len(), unpinned.len());
        assert_eq!(selected.len(), ctx.max_tools);
    }

    #[test]
    fn pinning_a_tool_already_inside_the_cap_does_not_duplicate_it() {
        let reg = new_registry_arc();
        let base = select_tools_for_turn(TOOL_REGISTRY, &reg, &TurnContext::default());
        let already_in = base[0].name;
        let ctx = TurnContext {
            pin_names: vec![already_in],
            ..TurnContext::default()
        };
        let selected = select_tools_for_turn(TOOL_REGISTRY, &reg, &ctx);
        assert_eq!(
            selected.iter().filter(|t| t.name == already_in).count(),
            1,
            "pinned tool {already_in} appeared more than once"
        );
        assert_eq!(selected.len(), base.len());
    }

    #[test]
    fn pinning_is_applied_before_the_cap() {
        let reg = new_registry_arc();
        let ctx = TurnContext {
            max_tools: 1,
            pin_names: vec!["vox_spawn_agent"],
            ..TurnContext::default()
        };
        let selected = select_tools_for_turn(TOOL_REGISTRY, &reg, &ctx);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].name, "vox_spawn_agent");
    }

    /// The bug itself, pinned down so it cannot silently return.
    ///
    /// Without `pin_names`, a default chat turn's alphabetical truncation drops
    /// every delegation tool — which is why `run_agent_turn` pins them. If this
    /// ever starts failing it means the cap, the registry order, or the
    /// candidate count changed enough to make the pin redundant: verify that,
    /// then delete this test and the pin together. It is a canary for *why* the
    /// pin exists, not a requirement that the truncation stay broken.
    #[test]
    fn unpinned_chat_turn_cannot_reach_the_delegation_tools() {
        let reg = new_registry_arc();
        let ctx = TurnContext {
            exclude_name_prefixes: vec!["vox_chat_"],
            ..TurnContext::default()
        };
        let selected = select_tools_for_turn(TOOL_REGISTRY, &reg, &ctx);
        let names: HashSet<&str> = selected.iter().map(|t| t.name).collect();
        for tool in DELEGATION_TOOLS {
            assert!(
                !names.contains(tool),
                "{tool} is now reachable unpinned — the alphabetical-truncation \
                 bug the pin works around may be gone; re-verify before removing \
                 the pin (offered set was {} tools)",
                selected.len()
            );
        }
    }

    #[test]
    fn delegation_tools_survive_a_restrictive_active_skill() {
        let reg = new_registry_arc();
        install_restrictive_skill(&reg);
        let ctx = TurnContext {
            active_skill_id: Some("narrow-skill".to_string()),
            pin_names: DELEGATION_TOOLS.to_vec(),
            ..TurnContext::default()
        };
        let names: HashSet<&str> = select_tools_for_turn(TOOL_REGISTRY, &reg, &ctx)
            .iter()
            .map(|t| t.name)
            .collect();
        for tool in DELEGATION_TOOLS {
            assert!(names.contains(tool), "{tool} removed by the active skill");
        }
    }
}
