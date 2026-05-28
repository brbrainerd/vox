use clap::Subcommand;

#[derive(Subcommand)]
pub enum PlanCmd {
    /// Generate a new V2 plan via the PlanningOrchestrator.
    Create {
        /// Human readable goal the agent should plan out
        goal: String,

        /// Auto-approve execution bypassing the `RequiresApproval` block.
        #[arg(long)]
        approve: bool,
    },
    /// Re-evaluate an existing PlanId to unblock exceptions.
    Replan {
        /// Active Plan Session ID
        session_id: String,
    },
    /// Status view of current queued plans and telemetry via CLI
    Status {
        /// Optional Plan Session ID to scope output
        session_id: Option<String>,
    },
}

pub async fn dispatch(cmd: PlanCmd) -> anyhow::Result<()> {
    match cmd {
        PlanCmd::Create { goal, approve } => {
            tracing::info!("Synthesizing plan for goal: {}", goal);
            let nodes = vox_orchestrator::planning::synthesizer::synthesize_plan_nodes(&goal);
            let json = serde_json::to_string_pretty(&nodes)?;
            println!("Plan synthesized:\n{}", json);
            if approve {
                tracing::info!("Auto-approve enabled. (Execution engine not fully wired yet)");
            }
        }
        PlanCmd::Replan { session_id } => {
            tracing::info!("Re-planning session: {}", session_id);
            println!("Replan requested for {}", session_id);
            
            let config = vox_orchestrator::session::SessionConfig {
                persist: true,
                ..Default::default()
            };
            let mut manager = vox_orchestrator::session::SessionManager::new(config)?;
            
            if let Ok(db) = vox_db::VoxDb::connect_default_sync() {
                manager.set_db(std::sync::Arc::new(db));
            }

            if let Err(e) = manager.load(&session_id).await {
                anyhow::bail!("Failed to load session {}: {}", session_id, e);
            }
            if let Some(session) = manager.get(&session_id) {
                println!("Session {} loaded successfully from VoxDb.", session_id);
                println!("Turns recorded: {}", session.turn_count);
                println!("Tokens utilized: {}", session.total_tokens);
            }
        }
        PlanCmd::Status { session_id } => {
            tracing::info!("Status requested for session: {:?}", session_id);
            println!("Status view not fully wired.");
        }
    }
    Ok(())
}
