//! Plan/Act/Verify phase-boundary intervention methods (Track D).
//!
//! These let a human (via GUI or daemon RPC) approve a plan, skip verification,
//! or force verification on a running task. All methods publish a
//! `AgentEventKind::PavPhaseChanged` event on the EventBus.

use crate::orchestrator::OrchestratorError;
use crate::planning::phase_loop::PhaseLoop;
use crate::types::TaskId;

impl crate::orchestrator::Orchestrator {
    /// Advance a task's PAV loop from Planning → Acting (user approved the plan).
    pub fn pav_advance_to_acting(&self, task_id: TaskId) -> Result<(), OrchestratorError> {
        self.with_pav_loop(task_id, |loop_, _tid| {
            loop_.advance_to_acting();
        })
    }

    /// Skip the Verifying phase for a task (if currently Verifying → Done).
    pub fn pav_skip_verify(&self, task_id: TaskId) -> Result<(), OrchestratorError> {
        self.with_pav_loop(task_id, |loop_, _tid| {
            loop_.skip_verify();
        })
    }

    /// Force the Verifying phase (Acting → Verifying), overriding any skip.
    pub fn pav_force_verify(&self, task_id: TaskId) -> Result<(), OrchestratorError> {
        self.with_pav_loop(task_id, |loop_, _tid| {
            loop_.force_verify();
        })
    }

    /// Internal helper: locate a task by id, mutate its PAV loop via `f`,
    /// write the updated state back, and emit `PavPhaseChanged`.
    fn with_pav_loop(
        &self,
        task_id: TaskId,
        f: impl FnOnce(&mut PhaseLoop, TaskId),
    ) -> Result<(), OrchestratorError> {
        let agent_id = crate::sync_lock::rw_read(&self.task_assignments)
            .get(&task_id)
            .copied()
            .ok_or(OrchestratorError::TaskNotFound(task_id))?;

        let agents = crate::sync_lock::rw_read(&self.agents);
        let queue_lock = agents
            .get(&agent_id)
            .ok_or(OrchestratorError::AgentNotFound(agent_id))?;
        let mut queue = crate::sync_lock::rw_write(queue_lock);

        // Try in-progress first (current_task_mut), then queued (find_task_mut).
        let task = if let Some(t) = queue.current_task_mut().filter(|t| t.id == task_id) {
            t
        } else if let Some(t) = queue.find_task_mut(task_id) {
            t
        } else {
            return Err(OrchestratorError::TaskNotFound(task_id));
        };

        // Build a PhaseLoop from existing state or a fresh default.
        let mut loop_ = if let Some(state) = task.pav_loop.take() {
            PhaseLoop::from_state(state)
        } else {
            PhaseLoop::new()
        };

        f(&mut loop_, task_id);

        let new_phase = loop_.phase();
        task.pav_loop = Some(loop_.into_state());

        // Publish the phase transition.
        self.event_bus
            .emit(crate::events::AgentEventKind::PavPhaseChanged {
                task_id,
                phase: new_phase,
            });

        tracing::debug!(
            target: "vox.orchestrator.pav",
            %task_id,
            %agent_id,
            phase = ?new_phase,
            "PAV phase updated"
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::planning::phase_loop::{PavLoopState, PavPhase};
    use crate::types::TaskId;

    /// Verify PhaseLoop state transitions used by pav_advance_to_acting,
    /// pav_skip_verify, and pav_force_verify. These are unit tests of the
    /// pure logic; integration tests require a running orchestrator.
    #[test]
    fn approve_plan_advances_to_acting() {
        use crate::planning::phase_loop::PhaseLoop;
        let mut loop_ = PhaseLoop::new(); // starts Planning, verify_required=true
        loop_.advance_to_acting();
        assert_eq!(loop_.phase(), PavPhase::Acting);
    }

    #[test]
    fn skip_verify_during_verifying_goes_to_done() {
        use crate::planning::phase_loop::PhaseLoop;
        let mut loop_ = PhaseLoop::new();
        loop_.advance_to_acting();
        loop_.advance_to_verifying();
        assert_eq!(loop_.phase(), PavPhase::Verifying);
        loop_.skip_verify();
        // skip_verify jumps to Done if already Verifying
        assert_eq!(loop_.phase(), PavPhase::Done);
    }

    #[test]
    fn force_verify_while_acting_goes_to_verifying() {
        use crate::planning::phase_loop::PhaseLoop;
        use crate::mode::{ClutchProfile, RiskPosture};
        // High risk → verify_required=false for High clutch
        let mut loop_ = PhaseLoop::start(ClutchProfile::Free, RiskPosture::High);
        assert_eq!(loop_.phase(), PavPhase::Acting);
        loop_.force_verify();
        assert_eq!(loop_.phase(), PavPhase::Verifying);
    }

    #[test]
    fn pav_loop_state_serde_roundtrip() {
        let state = PavLoopState {
            phase: PavPhase::Planning,
            verify_required: true,
            verify_skipped: false,
        };
        let json = serde_json::to_string(&state).unwrap();
        let back: PavLoopState = serde_json::from_str(&json).unwrap();
        assert_eq!(back.phase, PavPhase::Planning);
        assert!(back.verify_required);
    }

    /// Verify the TaskId type wraps correctly (exercises the TaskId(task_id) pattern
    /// used in the daemon handler).
    #[test]
    fn task_id_wraps_u64() {
        let tid = TaskId(42_u64);
        assert_eq!(tid.0, 42);
    }
}
