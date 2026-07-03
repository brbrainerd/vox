use crate::feedback::{FeedbackId, FeedbackKind, Surface};

impl crate::orchestrator::Orchestrator {
    /// Surface a mined recurring procedure as a non-blocking "save as skill?"
    /// suggestion in the NeedsYou inbox. Deduped by prompt; returns the new
    /// feedback id, or `None` if an identical proposal is already open.
    ///
    /// Does **not** broadcast `FeedbackRequested` on the event bus — this
    /// method is sync and cannot `.await` the durable oplog write, so callers
    /// must durably record the transition first via `record_operation`, then
    /// call [`Orchestrator::emit_feedback_requested`] with the returned id
    /// (T1.2: Tier-A durable-before-broadcast).
    pub fn propose_skill(
        &self,
        name: &str,
        description: &str,
        session_id: Option<String>,
        meta: Option<serde_json::Value>,
    ) -> Option<FeedbackId> {
        let prompt = format!(
            "Recurring procedure '{name}': {description}. Consider saving it as a reusable skill."
        );
        if self
            .feedback()
            .open_needs_you()
            .iter()
            .any(|f| f.kind == FeedbackKind::SkillProposal && f.prompt == prompt)
        {
            return None;
        }
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let fid = self.feedback().register(
            FeedbackKind::SkillProposal,
            prompt,
            vec!["Dismiss".to_string()], // sub-project 4 adds "Save as skill"
            Vec::new(),                  // non-blocking: no gates
            None,
            0.0,
            0,
            Surface::NeedsYou,
            session_id,
            None,
            ts,
            meta,
        );
        Some(fid)
    }

    /// Broadcast the `FeedbackRequested` bus event for a skill proposal
    /// previously registered by [`Orchestrator::propose_skill`]. Callers MUST
    /// call this only *after* durably recording the transition (via
    /// `record_operation`) — see the T1.2 tier-A contract on
    /// [`crate::events::is_tier_a`].
    pub fn emit_feedback_requested_skill_proposal(&self, feedback_id: &FeedbackId) {
        self.event_bus
            .emit(crate::events::AgentEventKind::FeedbackRequested {
                feedback_id: feedback_id.0.clone(),
                kind: "skill_proposal".into(),
                gates: Vec::new(),
                surface: "needs_you".into(),
            });
    }
}

#[cfg(test)]
mod tests {
    use crate::config::OrchestratorConfig;
    use crate::feedback::FeedbackKind;
    use crate::orchestrator::Orchestrator;

    #[test]
    fn propose_skill_registers_needs_you_and_dedups() {
        let orch = Orchestrator::new(OrchestratorConfig::for_testing());
        let desc = "Recurring procedure: read → edit → run (seen 4× across 2 sessions)";
        let f1 = orch.propose_skill("read-edit-run", desc, Some("s1".into()), None);
        assert!(f1.is_some());
        let open = orch.feedback().open_needs_you();
        assert!(open.iter().any(|f| f.kind == FeedbackKind::SkillProposal));
        let f2 = orch.propose_skill("read-edit-run", desc, Some("s1".into()), None);
        assert!(f2.is_none(), "duplicate proposal must be skipped");
        assert_eq!(
            orch.feedback()
                .open_needs_you()
                .iter()
                .filter(|f| f.kind == FeedbackKind::SkillProposal)
                .count(),
            1
        );
    }
}
