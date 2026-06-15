use crate::server_state::ServerState;

pub(crate) fn memory_config_for_state(state: &ServerState) -> vox_orchestrator::MemoryConfig {
    state.orchestrator_config.memory.clone()
}
