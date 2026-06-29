use anyhow::{Context, Result};
use std::path::PathBuf;

use vox_bounded_fs::read_utf8_path_capped;

use super::registry::make_registry;

pub async fn list() -> Result<()> {
    let registry = make_registry().await;
    let skills = registry.list(None);

    if skills.is_empty() {
        println!("No skills installed.");
        println!("  Install from file: vox skill install <path/to/skill.skill.md>");
        return Ok(());
    }

    println!("Installed skills ({}):\n", skills.len());
    for skill in &skills {
        println!(
            "  {:30} {:10} [{:?}]  {}",
            skill.id, skill.version, skill.category, skill.description
        );
        if !skill.tools.is_empty() {
            println!("    tools: {}", skill.tools.join(", "));
        }
    }
    Ok(())
}

pub async fn install(path: &PathBuf) -> Result<()> {
    let registry = make_registry().await;

    let content = read_utf8_path_capped(path)
        .with_context(|| format!("Failed to read skill file: {}", path.display()))?;

    let bundle = vox_openclaw_runtime::parser::parse_skill_md(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse skill file: {e}"))?;

    let result = registry.install_bundle(&bundle).await?;
    if result.already_installed {
        println!(
            "✓ Skill '{}' (v{}) already installed",
            result.id, result.version
        );
    } else {
        println!(
            "✓ Installed skill '{}' v{} (hash: {})",
            result.id,
            result.version,
            &result.hash[..8.min(result.hash.len())]
        );
    }
    Ok(())
}

pub async fn add(source: &str, global: bool, skill: Option<&str>) -> Result<()> {
    let ws_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let installed = vox_plugin_host::user_install::install_to_user_root(source, &ws_root, global, skill)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    for s in &installed {
        println!("✓ Added '{}' → {}", s.name, s.dest.display());
    }
    println!(
        "{} skill(s) added. They load on next daemon start, or run `vox skill list`.",
        installed.len()
    );
    Ok(())
}

pub async fn uninstall(id: &str) -> Result<()> {
    let registry = make_registry().await;
    let result = registry.uninstall(id).await?;
    if result.was_installed {
        println!("✓ Uninstalled '{}'", id);
    } else {
        println!("  Skill '{}' was not installed", id);
    }
    Ok(())
}

pub async fn search(query: &str) -> Result<()> {
    let registry = make_registry().await;
    let hits = registry.search(query);
    if hits.is_empty() {
        println!("No skills matching '{}'", query);
    } else {
        println!("Skills matching '{}' ({}):\n", query, hits.len());
        for skill in &hits {
            println!(
                "  {:30} {}  ({})",
                skill.id, skill.version, skill.description
            );
        }
    }
    Ok(())
}

pub async fn info(id: &str) -> Result<()> {
    let registry = make_registry().await;
    match registry.get(id) {
        Some(skill) => {
            println!("Skill: {}", skill.id);
            println!("  Name:        {}", skill.name);
            println!("  Version:     {}", skill.version);
            println!("  Author:      {}", skill.author);
            println!("  Description: {}", skill.description);
            println!("  Category:    {:?}", skill.category);
            println!("  Tags:        {}", skill.tags.join(", "));
            println!("  Tools:       {}", skill.tools.join(", "));
        }
        None => println!("Skill '{}' is not installed", id),
    }
    Ok(())
}

pub async fn create(name: &str) -> Result<()> {
    let id = name.to_lowercase().replace(' ', "-");
    let filename = format!("{}.skill.md", id);
    let path = PathBuf::from(&filename);

    if path.exists() {
        anyhow::bail!("{} already exists", filename);
    }

    let template = format!(
        "---\nid = \"my-org.{id}\"\nname = \"{name}\"\nversion = \"0.1.0\"\nauthor = \"your-name\"\ndescription = \"A short description of what this skill does\"\ncategory = \"utilities\"\ntools = [\"example_tool\"]\ntags = [\"example\", \"custom\"]\npermissions = [\"read_files\"]\n---\n\n# {name}\n\nDescribe what this skill does and when to use it.\n\n## Tools\n\n### `example_tool`\n\nA brief description of what this tool does.\n",
        id = id,
        name = name,
    );

    std::fs::write(&path, &template)?;
    println!("✓ Created {}", filename);
    println!("  Edit the frontmatter and markdown body, then:");
    println!("  vox skill install {}", filename);
    Ok(())
}
