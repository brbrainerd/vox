//! `vox agent` — register, list, and inspect AI agent definitions.

use anyhow::{Context, Result};
use vox_pm::{AgentDefEntry, CodeStore};

const DEFAULT_DB: &str = ".vox/store.db";

async fn connect() -> Result<CodeStore> {
    if let (Ok(url), Ok(token)) = (
        std::env::var("VOX_TURSO_URL"),
        std::env::var("VOX_TURSO_TOKEN"),
    ) {
        CodeStore::open_remote(&url, &token)
            .await
            .context("Failed to connect to remote store")
    } else {
        std::fs::create_dir_all(".vox").ok();
        CodeStore::open(DEFAULT_DB)
            .await
            .context("Failed to open local store")
    }
}

fn print_agent(a: &AgentDefEntry) {
    let vis = if a.is_public { "public" } else { "private" };
    println!("  {} (v{}) [{vis}]", a.name, a.version);
    if let Some(ref desc) = a.description {
        println!("    {}", desc);
    }
}

/// Register an agent definition.
pub async fn create(
    name: &str,
    description: Option<&str>,
    system_prompt: Option<&str>,
    tools: Option<&str>,
    model_config: Option<&str>,
    is_public: bool,
) -> Result<()> {
    let store = connect().await?;
    let id = format!("agent-{name}");
    store
        .register_agent(
            &id,
            name,
            description,
            system_prompt,
            tools,
            model_config,
            Some("local-user"),
            "0.1.0",
            is_public,
        )
        .await?;
    println!("✓ Registered agent '{name}' (id: {id})");
    Ok(())
}

/// List all registered agents.
pub async fn list() -> Result<()> {
    let store = connect().await?;
    let agents = store.list_agents().await?;
    if agents.is_empty() {
        println!("No agents registered.");
    } else {
        println!("{} agents:", agents.len());
        for a in &agents {
            print_agent(a);
        }
    }
    Ok(())
}

/// Get details of a specific agent.
pub async fn info(id: &str) -> Result<()> {
    let store = connect().await?;
    let agent = store.get_agent(id).await?;
    println!("Agent: {} (v{})", agent.name, agent.version);
    if let Some(ref desc) = agent.description {
        println!("Description: {desc}");
    }
    if let Some(ref prompt) = agent.system_prompt {
        println!("System prompt: {}", &prompt[..prompt.len().min(200)]);
    }
    if let Some(ref tools) = agent.tools {
        println!("Tools: {tools}");
    }
    if let Some(ref config) = agent.model_config {
        println!("Model config: {config}");
    }
    println!("Public: {}", agent.is_public);
    Ok(())
}

/// Dynamically generate Vox agent definitions based on workspace crates.
pub async fn generate() -> Result<()> {
    use std::fs;
    use std::path::Path;

    let crates_dir = Path::new("crates");
    if !crates_dir.exists() {
        println!("No crates directory found. Make sure you are in the workspace root.");
        return Ok(());
    }

    let agents_dir = Path::new(".vox/agents");
    fs::create_dir_all(agents_dir).context("Failed to create .vox/agents directory")?;

    for entry in fs::read_dir(crates_dir)? {
        let entry = entry?;
        if entry.path().is_dir() {
            let crate_name = entry.file_name().to_string_lossy().to_string();
            let cargo_toml_path = entry.path().join("Cargo.toml");

            if cargo_toml_path.exists() {
                let cargo_toml = fs::read_to_string(&cargo_toml_path)?;
                let mut description = format!("{} component", crate_name);

                for line in cargo_toml.lines() {
                    let line = line.trim();
                    if line.starts_with("description") {
                        if let Some(desc) = line.split('=').nth(1) {
                            description = desc.trim().trim_matches('"').to_string();
                            break; // Stop at first description
                        }
                    }
                }

                let md_path = agents_dir.join(format!("{crate_name}.md"));

                let content = format!(
                    "---\nname: {}\nmodel: gemini-2.5-flash-preview\npermission:\n  write: allow\n  bash: allow\n  edit: allow\nscope:\n  - {}/**\n---\n\nYou are the specialist for {}. Your domain is {}.\n\nFocus exclusively on this component and its specific responsibilities within the Vox compilation pipeline.",
                    crate_name, entry.path().display().to_string().replace("\\", "/"), description, entry.path().display().to_string().replace("\\", "/")
                );

                fs::write(&md_path, content)?;
                println!(
                    "✓ Generated agent definition for {crate_name} at {}",
                    md_path.display()
                );
            }
        }
    }

    // Phase 9: Include Orchestrator and Visualizer meta-agents
    let orch_path = agents_dir.join("orchestrator.md");
    if !orch_path.exists() {
        let content = "---\nname: orchestrator\nmodel: gemini-2.5-pro\npermission:\n  write: allow\n  bash: allow\n  edit: allow\nscope:\n  - crates/**\n---\n\nYou are the Meta-Orchestrator. Manage other task-specific agents and distribute workload.\n";
        fs::write(&orch_path, content)?;
        println!("✓ Generated meta-agent definition for orchestrator");
    }

    let vis_path = agents_dir.join("visualizer.md");
    if !vis_path.exists() {
        let content = "---\nname: visualizer\nmodel: gemini-2.0-flash-lite\npermission:\n  write: deny\n  bash: deny\n  edit: deny\nscope:\n  - docs/dashboard/**\n---\n\nYou are the Visualizer. Your role is read-only, generating dashboard data, reports, and tracking agent gamification metrics.\n";
        fs::write(&vis_path, content)?;
        println!("✓ Generated meta-agent definition for visualizer");
    }

    println!("Generation complete. Vox agent definitions are available in .vox/agents/");
    Ok(())
}
