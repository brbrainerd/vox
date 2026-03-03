// Integration test for session <-> agent lifecycle
#[cfg(test)]
mod tests {
    use crate::{Orchestrator, OrchestratorConfig};

    #[tokio::test]
    async fn test_agent_session_lifecycle() {
        // Setup a mock orchestrator config
        let config = OrchestratorConfig::default();

        let mut orchestrator = Orchestrator::new(config);

        // 1. Map session to a new agent
        let agent_id = orchestrator.spawn_agent("TestAgentSession").unwrap();
        let session_id = "vox_sess_123456";

        // Ensure successful map
        assert!(orchestrator
            .map_agent_session(agent_id, session_id.to_string())
            .is_ok());

        // 2. Fetch status, check if session is tracked
        let status = orchestrator.status();
        let agent_has_session = status
            .agents
            .iter()
            .any(|a| a.agent_session_id.as_deref() == Some(session_id));

        assert!(
            agent_has_session,
            "Agent session was not tracked by agent status"
        );
    }
}
