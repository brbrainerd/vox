//! Clap surface for `vox skill` (ARS + local registry; requires `--features ars`).

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

/// Subcommands for `vox skill`.
#[derive(Parser)]
pub enum SkillCmd {
    /// List installed skills.
    List,
    /// Install from a `.skill.md` path.
    Install {
        #[arg(required = true)]
        path: PathBuf,
    },
    /// Add a skill from a git URL or local path into `.vox/skills`.
    Add {
        #[arg(required = true)]
        source: String,
        /// Install into `~/.vox/skills` instead of the workspace.
        #[arg(long, default_value_t = false)]
        global: bool,
        /// Install only the skill whose `name` matches.
        #[arg(long)]
        skill: Option<String>,
    },
    /// Uninstall by skill id.
    Uninstall {
        #[arg(required = true)]
        id: String,
    },
    /// Search installed skills.
    Search {
        #[arg(required = true)]
        query: String,
    },
    /// Show one skill by id.
    Info {
        #[arg(required = true)]
        id: String,
    },
    /// Scaffold a new `.skill.md` template.
    Create {
        #[arg(required = true)]
        name: String,
    },
    /// Evaluate an ephemeral task body (sandbox harness).
    EvalTask {
        #[arg(required = true)]
        body: String,
        #[arg(long)]
        input_json: Option<String>,
    },
    /// Promote an ephemeral task into `skill_manifests` in Codex.
    Promote {
        #[arg(required = true)]
        session_id: String,
        #[arg(required = true)]
        task_id: String,
        #[arg(required = true)]
        name: String,
    },
    /// Run a skill by id with optional JSON input.
    Run {
        #[arg(required = true)]
        id: String,
        #[arg(long)]
        input_json: Option<String>,
        #[arg(long, default_value_t = false)]
        workflow: bool,
    },
    /// Assemble a context bundle and print it (uses default Codex when needed).
    ContextAssemble {
        #[arg(required = true)]
        tier: String,
        #[arg(long)]
        policy_json: Option<String>,
        #[arg(long)]
        agent_id: Option<String>,
    },
    /// Scan workspace for `.skill.md` files.
    Discover,
}

/// Dispatch `vox skill …`.
pub async fn run(cmd: SkillCmd) -> Result<()> {
    use super::ars;
    match cmd {
        SkillCmd::List => ars::list().await,
        SkillCmd::Install { path } => ars::install(&path).await,
        SkillCmd::Add {
            source,
            global,
            skill,
        } => ars::add(&source, global, skill.as_deref()).await,
        SkillCmd::Uninstall { id } => ars::uninstall(&id).await,
        SkillCmd::Search { query } => ars::search(&query).await,
        SkillCmd::Info { id } => ars::info(&id).await,
        SkillCmd::Create { name } => ars::create(&name).await,
        SkillCmd::EvalTask { body, input_json } => {
            ars::eval_task(&body, input_json.as_deref()).await
        }
        SkillCmd::Promote {
            session_id,
            task_id,
            name,
        } => ars::promote_skill(&session_id, &task_id, &name).await,
        SkillCmd::Run {
            id,
            input_json,
            workflow,
        } => ars::run(&id, input_json.as_deref(), workflow).await,
        SkillCmd::ContextAssemble {
            tier,
            policy_json,
            agent_id,
        } => ars::context_assemble(&tier, policy_json.as_deref(), agent_id.as_deref()).await,
        SkillCmd::Discover => ars::discover().await,
    }
}
