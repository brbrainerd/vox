use crate::budget::DriftDecision;
use crate::orchestrator::Orchestrator;
use crate::types::{AgentId, TaskId};

impl Orchestrator {
    /// Issues a cryptographic tool receipt for an agent to perform a specific tool call.
    /// This prevents agents from hallucinating tool outputs that were never executed.
    pub fn issue_tool_receipt(
        &self,
        agent_id: AgentId,
        tool_name: &str,
        args_json: &str,
    ) -> String {
        let ledger = crate::sync_lock::rw_read(&*self.tool_ledger);
        ledger
            .issue_intent(agent_id, tool_name, args_json)
            .receipt_id
    }

    /// Records the result of a tool execution in an existing receipt.
    pub fn fulfill_tool_receipt(&self, receipt_id: &str, result_json: &str) -> bool {
        let ledger = crate::sync_lock::rw_read(&*self.tool_ledger);
        ledger.fulfill_intent(receipt_id, result_json).is_ok()
    }

    /// Verifies that a tool receipt was indeed issued by this orchestrator.
    pub fn verify_tool_receipt(&self, receipt_id: &str) -> bool {
        let ledger = crate::sync_lock::rw_read(&*self.tool_ledger);
        ledger.verify(receipt_id).is_ok()
    }

    /// Records an agent's output iteration for semantic drift detection.
    /// If drift is detected (a "doom-loop"), returns a decision to halt or warn.
    pub fn record_agent_iteration(
        &self,
        agent_id: AgentId,
        output_text: &str,
        is_tool_call: bool,
    ) -> DriftDecision {
        let budget = crate::sync_lock::rw_read(&*self.budget_manager);
        budget.record_iteration_output(agent_id, output_text, is_tool_call)
    }

    /// Checks if a task should be routed to a specific model based on privacy requirements.
    pub fn privacy_check_routing(
        &self,
        _task_id: TaskId,
        pii_detected: bool,
    ) -> crate::privacy_router::PrivacyRoutingDecision {
        let router = crate::sync_lock::rw_read(&*self.privacy_router);
        router.route(pii_detected)
    }

    /// Performs a consensus check via a second "Judge" model for a high-stakes task.
    pub async fn judge_consensus(
        &self,
        _task_id: TaskId,
        _input: &str,
        _output: &str,
    ) -> crate::judge_model::JudgeVerdict {
        let judge = crate::sync_lock::rw_read(&*self.judge_model);
        // In a real implementation, this would trigger an actual LLM call.
        // For now, we use the policy-driven default.
        judge.policy.to_verdict()
    }

    /// Acquires a generic resource lock and broadcasts the event to the bulletin board.
    pub fn acquire_resource_lock(
        &self,
        agent_id: AgentId,
        resource_id: &str,
        kind: crate::locks::ResourceLockKind,
        ttl_ms: u64,
    ) -> bool {
        match self
            .resource_locks
            .try_acquire(resource_id, agent_id, kind, ttl_ms)
        {
            Ok(_) => {
                self.bulletin
                    .publish(crate::types::AgentMessage::ResourceLockAcquired {
                        agent_id,
                        resource_id: resource_id.to_string(),
                    });
                // Wire LockAcquired to the event bus so activity_log receives it.
                self.event_bus.emit(crate::events::AgentEventKind::LockAcquired {
                    agent_id,
                    path: std::path::PathBuf::from(resource_id),
                    exclusive: matches!(kind, crate::locks::ResourceLockKind::Exclusive),
                });
                true
            }
            Err(_) => false,
        }
    }

    /// Releases a generic resource lock and broadcasts the event to the bulletin board.
    pub fn release_resource_lock(&self, agent_id: AgentId, resource_id: &str) {
        self.resource_locks.release(resource_id, agent_id);
        self.bulletin
            .publish(crate::types::AgentMessage::ResourceLockReleased {
                agent_id,
                resource_id: resource_id.to_string(),
            });
        // Wire LockReleased to the event bus so activity_log receives it.
        self.event_bus.emit(crate::events::AgentEventKind::LockReleased {
            agent_id,
            path: std::path::PathBuf::from(resource_id),
        });
    }
}
