//! `vox catalog` — explicit multi-repo catalog management (`.vox/repositories.yaml`).

use anyhow::{Context, Result};
use clap::Subcommand;
use std::path::{Path, PathBuf};

use vox_repository::{RepoAccessMode, RepoCatalog, RepositoryDescriptor};

#[derive(Subcommand, Debug)]
pub enum CatalogCmd {
    /// Resolve and print the current repo catalog.
    List,
    /// Add a local workspace-relative path to the catalog.
    Add {
        /// The path to add (must be relative to the workspace root or absolute)
        path: String,
        /// The identifier for the repository (e.g. 'my-repo')
        #[arg(long = "id")]
        repository_id: Option<String>,
        /// The display name for the repository
        #[arg(long)]
        name: Option<String>,
    },
    /// Remove a repository from the catalog by its path or id.
    Remove {
        /// The path or repository id to remove
        target: String,
    },
    /// Focus the session on a specific repository in the catalog.
    Focus {
        /// The repository id to set as primary (or empty to clear focus)
        repository_id: Option<String>,
    },
}

fn load_catalog(root: &Path) -> Result<(PathBuf, RepoCatalog)> {
    let manifest_path = vox_repository::repo_catalog_manifest_path(root);
    if manifest_path.exists() {
        let text = std::fs::read_to_string(&manifest_path)
            .with_context(|| format!("Failed to read {}", manifest_path.display()))?;
        let catalog = serde_yaml::from_str::<RepoCatalog>(&text)
            .with_context(|| format!("Failed to parse {}", manifest_path.display()))?;
        Ok((manifest_path, catalog))
    } else {
        Ok((
            manifest_path,
            RepoCatalog {
                schema_version: vox_repository::REPO_CATALOG_SCHEMA_VERSION,
                primary_repository_id: None,
                repositories: Vec::new(),
            },
        ))
    }
}

fn save_catalog(path: &Path, catalog: &RepoCatalog) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_yaml::to_string(catalog)?;
    std::fs::write(path, text)?;
    Ok(())
}

fn current_repo_root() -> Result<std::path::PathBuf> {
    let cwd = std::env::current_dir()?;
    Ok(vox_repository::discover_repository_or_fallback(&cwd).root)
}

pub async fn run(cmd: CatalogCmd) -> Result<()> {
    let repo_root = current_repo_root()?;
    match cmd {
        CatalogCmd::List => {
            let catalog = vox_repository::resolve_repo_catalog(&repo_root)?;
            if crate::pipeline::global_json_enabled() {
                println!("{}", serde_json::to_string(&catalog)?);
            } else {
                println!("{}", serde_json::to_string_pretty(&catalog)?);
            }
        }
        CatalogCmd::Add {
            path,
            repository_id,
            name,
        } => {
            let (manifest, mut catalog) = load_catalog(&repo_root)?;
            if catalog
                .repositories
                .iter()
                .any(|r| r.root_path == Some(path.clone()))
            {
                println!("Path '{}' is already in the catalog.", path);
                return Ok(());
            }
            let display_name = name.unwrap_or_else(|| path.clone());
            catalog.repositories.push(RepositoryDescriptor {
                display_name,
                repository_id,
                root_path: Some(path.clone()),
                access_mode: RepoAccessMode::Local,
                capabilities: Vec::new(),
                default_ref: None,
                origin_url: None,
                metadata: None,
                provider: None,
                remote: None,
            });
            save_catalog(&manifest, &catalog)?;
            println!("Added '{}' to repo catalog.", path);
        }
        CatalogCmd::Remove { target } => {
            let (manifest, mut catalog) = load_catalog(&repo_root)?;
            let orig_len = catalog.repositories.len();
            catalog.repositories.retain(|r| {
                r.root_path != Some(target.clone())
                    && r.repository_id.as_deref() != Some(target.as_str())
            });
            if catalog.repositories.len() < orig_len {
                save_catalog(&manifest, &catalog)?;
                println!("Removed '{}' from repo catalog.", target);
            } else {
                println!("Could not find '{}' in the catalog.", target);
            }
        }
        CatalogCmd::Focus { repository_id } => {
            let (manifest, mut catalog) = load_catalog(&repo_root)?;
            if let Some(id) = repository_id.as_deref() {
                let exists = catalog
                    .repositories
                    .iter()
                    .any(|r| r.repository_id.as_deref() == Some(id));
                // We permit setting a focus that hasn't been explicitly declared, since it might refer to the root itself.
                if !exists {
                    println!(
                        "Note: Repository ID '{}' does not match any entry currently listed in the catalog array. Setting it anyway.",
                        id
                    );
                }
            }
            catalog.primary_repository_id = repository_id.clone();
            save_catalog(&manifest, &catalog)?;
            if let Some(id) = repository_id {
                println!("Set primary repository focus to '{}'.", id);
            } else {
                println!("Cleared primary repository focus.");
            }
        }
    }
    Ok(())
}
