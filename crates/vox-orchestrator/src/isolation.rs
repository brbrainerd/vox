//! Multi-agent isolation strategy (spec §5.1). The orchestrator *chooses* a
//! strategy per workload from predicted overlap + task duration + config; this
//! module is the decision + per-agent assignment record. Enforcement lives in
//! `task_submit.rs` (locks) and `workspace.rs` (changes/branches).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::types::AgentId;

/// The three orchestrator-chosen isolation strategies (spec §5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum IsolationStrategy {
    /// §5.1(1) Shared change, file-partitioned. All agents on one jj change;
    /// the `FileLockManager` grants single-writer leases per file. Default for
    /// disjoint write-sets — zero branch/worktree overhead.
    #[default]
    SharedBranch,
    /// §5.1(2) Per-agent change, auto-rebased. Each agent gets its own jj change
    /// off the same base; merge-back records conflicts-as-data.
    SplitChanges,
    /// §5.1(3) Separate branches — classic isolation, cheap because jj branches
    /// are anonymous and rebasing is conflict-tolerant.
    SeparateBranches,
}

/// Per-workload + per-agent assignment of strategies. Chosen by the orchestrator,
/// overridable by config and (P4 GUI) by the user.
#[derive(Debug, Clone, Default)]
pub struct IsolationPlan {
    /// Strategy applied when an agent has no explicit override.
    pub default: IsolationStrategy,
    /// Per-agent overrides (config or GUI driven).
    pub per_agent: HashMap<AgentId, IsolationStrategy>,
}

impl IsolationPlan {
    /// Resolve the effective strategy for `agent`.
    pub fn strategy_for(&self, agent: AgentId) -> IsolationStrategy {
        self.per_agent.get(&agent).copied().unwrap_or(self.default)
    }
    /// Set (or clear, with `None`) a per-agent override.
    pub fn set_override(&mut self, agent: AgentId, strategy: Option<IsolationStrategy>) {
        match strategy {
            Some(s) => {
                self.per_agent.insert(agent, s);
            }
            None => {
                self.per_agent.remove(&agent);
            }
        }
    }
}

/// Choose a strategy for a workload (spec §5.1: "a function of predicted overlap,
/// task duration, and user policy — and is fully overridable").
///
/// `predicted_overlap` is the count of write-paths the new task shares with any
/// active agent (from `overlapping_paths()` / the affinity map). `config_default`
/// is the user/GUI-set baseline that wins absent a stronger signal.
pub fn choose_strategy(
    predicted_overlap: usize,
    long_running: bool,
    config_default: IsolationStrategy,
) -> IsolationStrategy {
    if long_running {
        IsolationStrategy::SeparateBranches
    } else if predicted_overlap > 0 {
        IsolationStrategy::SplitChanges
    } else {
        config_default
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disjoint_write_sets_pick_shared_branch() {
        // Two agents, no overlapping write paths -> the cheap shared-branch strategy.
        let s = choose_strategy(
            /* predicted_overlap */ 0,
            /* long_running */ false,
            IsolationStrategy::SharedBranch,
        );
        assert_eq!(s, IsolationStrategy::SharedBranch);
    }

    #[test]
    fn overlap_escalates_to_split_changes() {
        let s = choose_strategy(3, false, IsolationStrategy::SharedBranch);
        assert_eq!(s, IsolationStrategy::SplitChanges);
    }

    #[test]
    fn long_running_prefers_separate_branches() {
        let s = choose_strategy(0, true, IsolationStrategy::SharedBranch);
        assert_eq!(s, IsolationStrategy::SeparateBranches);
    }

    #[test]
    fn config_default_is_honored_when_no_signal_overrides() {
        // An explicit non-default config default wins absent overlap/long-running signal.
        let s = choose_strategy(0, false, IsolationStrategy::SeparateBranches);
        assert_eq!(s, IsolationStrategy::SeparateBranches);
    }

    #[test]
    fn plan_per_agent_override_resolves() {
        let mut plan = IsolationPlan {
            default: IsolationStrategy::SharedBranch,
            ..Default::default()
        };
        plan.set_override(AgentId(7), Some(IsolationStrategy::SeparateBranches));
        assert_eq!(
            plan.strategy_for(AgentId(7)),
            IsolationStrategy::SeparateBranches
        );
        assert_eq!(
            plan.strategy_for(AgentId(8)),
            IsolationStrategy::SharedBranch
        );
        plan.set_override(AgentId(7), None);
        assert_eq!(
            plan.strategy_for(AgentId(7)),
            IsolationStrategy::SharedBranch
        );
    }
}
