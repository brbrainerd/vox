//! `vox skill` — manage Vox skills from the CLI.

use anyhow::{Context, Result};
use std::path::PathBuf;

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

    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read skill file: {}", path.display()))?;

    let bundle = vox_skills::parser::parse_skill_md(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse skill file: {e}"))?;

    let result = registry.install(&bundle).await?;
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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn make_registry() -> vox_skills::SkillRegistry {
    let registry = vox_skills::SkillRegistry::new();
    // Auto-load all 5 built-in skills (embedded via include_str! at compile time)
    let _ = vox_skills::install_builtins(&registry).await;
    registry
}

/// 9.4 — Skill auto-discovery: scan the crate graph for `.skill.md` files
/// and suggest installable skills not yet in the registry.
pub async fn discover() -> Result<()> {
    use owo_colors::OwoColorize;
    use std::collections::HashSet;

    let registry = make_registry().await;
    let installed: HashSet<String> = registry.list(None).into_iter().map(|s| s.id).collect();

    // Walk workspace for .skill.md files (up to 6 levels deep)
    let workspace_root = std::env::current_dir().unwrap_or_default();
    let mut found: Vec<(std::path::PathBuf, String)> = Vec::new();

    walk_for_skills(&workspace_root, 0, 6, &mut found);

    if found.is_empty() {
        println!("{}", "No .skill.md files found in the workspace.".dimmed());
        println!("  Create one with: {}", "vox skill create <name>".cyan());
        return Ok(());
    }

    let new_count = found
        .iter()
        .filter(|(_, id)| !installed.contains(id))
        .count();
    println!(
        "\n{} Found {} skill file(s) ({} not yet installed)\n",
        "🔍".bold(),
        found.len(),
        new_count
    );

    for (path, id) in &found {
        let is_installed = installed.contains(id);
        let rel = path.strip_prefix(&workspace_root).unwrap_or(path);
        if is_installed {
            println!(
                "  {} {:<32} [{}]",
                "✅".green(),
                id.dimmed(),
                rel.display().to_string().dimmed()
            );
        } else {
            println!(
                "  {} {:<32}  {} {}",
                "📦".yellow(),
                id.yellow(),
                rel.display().to_string().dimmed(),
                "← not installed".bright_yellow()
            );
            println!(
                "     {} vox skill install {}",
                "→".dimmed(),
                rel.display().to_string().cyan()
            );
        }
    }

    if new_count > 0 {
        println!(
            "\nInstall all: {}",
            "for f in $(find . -name '*.skill.md'); do vox skill install $f; done".cyan()
        );
    }

    Ok(())
}

fn walk_for_skills(
    dir: &std::path::Path,
    depth: usize,
    max_depth: usize,
    out: &mut Vec<(std::path::PathBuf, String)>,
) {
    if depth > max_depth {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        // Skip hidden dirs, target/, node_modules/
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with('.') || name == "target" || name == "node_modules" {
                continue;
            }
        }
        if path.is_dir() {
            walk_for_skills(&path, depth + 1, max_depth, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            let fname = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if fname.ends_with(".skill.md") {
                // Quick parse: extract `id = "..."` from frontmatter
                if let Ok(content) = std::fs::read_to_string(&path) {
                    let id = extract_skill_id(&content)
                        .unwrap_or_else(|| fname.trim_end_matches(".skill.md").to_string());
                    out.push((path, id));
                }
            }
        }
    }
}

fn extract_skill_id(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("id") {
            // Handle: id = "foo.bar" or id = 'foo.bar'
            if let Some(rest) = trimmed.strip_prefix("id").map(|s| s.trim()) {
                if let Some(rest) = rest.strip_prefix('=') {
                    let val = rest.trim().trim_matches('"').trim_matches('\'').to_string();
                    if !val.is_empty() {
                        return Some(val);
                    }
                }
            }
        }
    }
    None
}
