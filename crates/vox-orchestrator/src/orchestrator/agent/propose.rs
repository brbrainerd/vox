use crate::feedback::{FeedbackId, FeedbackKind, Surface};

impl crate::orchestrator::Orchestrator {
    /// Surface a mined recurring procedure as a non-blocking "save as skill?"
    /// suggestion in the NeedsYou inbox. Deduped by prompt; returns the new
    /// feedback id, or `None` if an identical proposal is already open.
    pub fn propose_skill(
        &self,
        name: &str,
        description: &str,
        session_id: Option<String>,
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
        );
        self.event_bus
            .emit(crate::events::AgentEventKind::FeedbackRequested {
                feedback_id: fid.0.clone(),
                kind: "skill_proposal".into(),
                gates: Vec::new(),
                surface: "needs_you".into(),
            });
        Some(fid)
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
        let f1 = orch.propose_skill("read-edit-run", desc, Some("s1".into()));
        assert!(f1.is_some());
        let open = orch.feedback().open_needs_you();
        assert!(open.iter().any(|f| f.kind == FeedbackKind::SkillProposal));
        let f2 = orch.propose_skill("read-edit-run", desc, Some("s1".into()));
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
