use anyhow::Result;
use std::fs;

const VOXIGNORE_TEMPLATE: &str = r#"# .voxignore — SINGLE SOURCE OF TRUTH for AI context exclusion
#
# IMPORTANT: This file is the SSOT for what Vox's tools, agents, and IDEs
# should exclude from AI context. DO NOT edit .cursorignore, .aiignore, or
# .aiexclude directly. Edit this file, then run:
#
#   vox ci sync-ignore-files
#
# to regenerate all derived ignore files. CI will fail if derived files drift.
# See: docs/src/architecture/multi-repo-context-isolation-research-2026.md §3

# === BUILD ARTIFACTS ===
target/
target_*/
dist/
build/
node_modules/
__pycache__/
*.pyc

# === VCS INTERNALS ===
.git/
.jj/

# === SECRETS AND CREDENTIALS ===
.env
.env.*
*.pem
*.key

# === DATABASE FILES ===
*.db
*.db-wal
*.db-shm
*.sqlite

# === GENERATED / DERIVED FILES ===
*.lock
*.generated.*
vox-agent.json

# === SCRATCH / EPHEMERAL / LOGS ===
scratch/
tmp/
*.tmp
*.log
/artifacts/
"#;

const AGENTS_MD_TEMPLATE: &str = r#"# Agents Policy (Cross-Tool, Session-Critical)

## Scope
- Use this file for non-negotiable project policy that should apply in every session.
- Primary navigation should be mapped here.

## Research and Documentation Storage (IDE Agent Directive)
ALL research findings, architecture documents, and knowledge artifacts MUST be written to `docs/` in this repository — **not** to any IDE-private knowledge base (e.g., Antigravity's `~/.gemini/antigravity/knowledge/`). The `docs/` tree is the single source of truth for all project knowledge.

## AI Context Exclusion (SSOT)
`.voxignore` is the **single source of truth** for what files and directories should be excluded from AI context.
- Edit `.voxignore`; derive `.cursorignore`, `.aiignore`, `.aiexclude` via `vox ci sync-ignore-files`
- Do **not** edit derived ignore files directly — they are regenerated and tracked for drift
"#;

pub async fn run(name: Option<&str>) -> Result<()> {
    let project_name = name.unwrap_or_else(|| {
        std::env::current_dir()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .as_deref()
            .unwrap_or("my-project")
            .to_string()
            .leak()
    });

    let cwd = std::env::current_dir()?;
    let repo_dir = cwd; // We initialize in the current directory, similar to `git init`.

    // 1. Create .voxignore
    let voxignore_path = repo_dir.join(".voxignore");
    if !voxignore_path.exists() {
        fs::write(&voxignore_path, VOXIGNORE_TEMPLATE)?;
        println!("Created .voxignore (SSOT)");
    } else {
        println!(".voxignore already exists, skipping");
    }

    // 2. Derive other ignores
    vox_cli_ci::sync_ignore_files::run(&repo_dir, false)?;

    // 3. Create AGENTS.md
    let agents_path = repo_dir.join("AGENTS.md");
    if !agents_path.exists() {
        fs::write(&agents_path, AGENTS_MD_TEMPLATE)?;
        println!("Created AGENTS.md");
    } else {
        println!("AGENTS.md already exists, skipping");
    }

    // 4. Create .vox/
    let vox_dir = repo_dir.join(".vox");
    if !vox_dir.exists() {
        fs::create_dir_all(&vox_dir)?;
    }

    // 5. Create .vox/repositories.yaml
    let repo_yaml = vox_dir.join("repositories.yaml");
    if !repo_yaml.exists() {
        let repo_yaml_content = format!(
            "repositories:\n  - repository_id: {}\n    root_path: \".\"\n    access_mode: local\n    capabilities: [write]\n",
            project_name
        );
        fs::write(&repo_yaml, repo_yaml_content)?;
        println!("Created .vox/repositories.yaml");
    } else {
        println!(".vox/repositories.yaml already exists, skipping");
    }

    // 6. Create .vox/agents/
    let agents_dir = vox_dir.join("agents");
    if !agents_dir.exists() {
        fs::create_dir_all(&agents_dir)?;
        println!("Created .vox/agents/ directory");
    }

    // 7. Create .github/copilot-instructions.md
    let github_dir = repo_dir.join(".github");
    if !github_dir.exists() {
        fs::create_dir_all(&github_dir)?;
    }
    let copilot_instructions = github_dir.join("copilot-instructions.md");
    if !copilot_instructions.exists() {
        let copilot_content =
            "# Copilot Instructions\n\nSee AGENTS.md for primary cross-tool agent instructions.\n";
        fs::write(&copilot_instructions, copilot_content)?;
        println!("Created .github/copilot-instructions.md");
    } else {
        println!(".github/copilot-instructions.md already exists, skipping");
    }

    // 8. Handoffs dir
    let handoffs_dir = vox_dir.join("handoffs");
    if !handoffs_dir.exists() {
        fs::create_dir_all(&handoffs_dir)?;
        println!("Created .vox/handoffs/ directory");
    }

    println!(
        "✓ Initialized Vox repository scaffold for '{}'",
        project_name
    );

    Ok(())
}
