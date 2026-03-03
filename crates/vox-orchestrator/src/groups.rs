use std::path::{Path, PathBuf};

use globset::{Glob, GlobSet, GlobSetBuilder};

use crate::types::AgentId;

/// A named group of files that should be handled by the same agent.
///
/// Default groups correspond to Vox crate boundaries.
#[derive(Debug, Clone)]
pub struct AffinityGroup {
    /// Human-readable name (e.g., "parser", "codegen").
    pub name: String,
    /// Glob patterns matching files in this group.
    pub patterns: Vec<String>,
    /// Pre-assigned agent for this group (assigned on first use if None).
    pub default_agent: Option<AgentId>,
}

/// Registry of all affinity groups with compiled glob matchers.
pub struct AffinityGroupRegistry {
    groups: Vec<AffinityGroup>,
    matchers: Vec<GlobSet>,
}

impl AffinityGroupRegistry {
    /// Create a registry from a list of affinity groups.
    pub fn new(groups: Vec<AffinityGroup>) -> Self {
        let matchers = groups
            .iter()
            .map(|g| {
                let mut builder = GlobSetBuilder::new();
                for pattern in &g.patterns {
                    if let Ok(glob) = Glob::new(pattern) {
                        builder.add(glob);
                    } else {
                        tracing::warn!("Invalid glob pattern in group '{}': {}", g.name, pattern);
                    }
                }
                builder.build().unwrap_or_else(|_| {
                    GlobSetBuilder::new()
                        .build()
                        .expect("empty globset should always build")
                })
            })
            .collect();

        Self { groups, matchers }
    }

    /// Create a registry with the default Vox crate affinity groups as per Phase 7.
    pub fn defaults() -> Self {
        Self::new(vec![
            AffinityGroup {
                name: "lexer-parser-group".to_string(),
                patterns: vec![
                    "**/vox-lexer/**".to_string(),
                    "**/vox-parser/**".to_string(),
                    "**/vox-ast/**".to_string(),
                ],
                default_agent: None,
            },
            AffinityGroup {
                name: "typeck-group".to_string(),
                patterns: vec!["**/vox-typeck/**".to_string()],
                default_agent: None,
            },
            AffinityGroup {
                name: "hir-group".to_string(),
                patterns: vec!["**/vox-hir/**".to_string()],
                default_agent: None,
            },
            AffinityGroup {
                name: "codegen-rust-group".to_string(),
                patterns: vec!["**/vox-codegen-rust/**".to_string()],
                default_agent: None,
            },
            AffinityGroup {
                name: "codegen-ts-group".to_string(),
                patterns: vec!["**/vox-codegen-ts/**".to_string()],
                default_agent: None,
            },
            AffinityGroup {
                name: "runtime-group".to_string(),
                patterns: vec!["**/vox-runtime/**".to_string()],
                default_agent: None,
            },
            AffinityGroup {
                name: "orchestrator-group".to_string(),
                patterns: vec!["**/vox-orchestrator/**".to_string()],
                default_agent: None,
            },
            AffinityGroup {
                name: "pm-group".to_string(),
                patterns: vec!["**/vox-pm/**".to_string()],
                default_agent: None,
            },
            AffinityGroup {
                name: "lsp-group".to_string(),
                patterns: vec!["**/vox-lsp/**".to_string()],
                default_agent: None,
            },
        ])
    }

    /// Resolve a file path to its affinity group, if any.
    pub fn resolve(&self, path: &Path) -> Option<&AffinityGroup> {
        for (i, matcher) in self.matchers.iter().enumerate() {
            if matcher.is_match(path) {
                return Some(&self.groups[i]);
            }
        }
        None
    }

    /// Get all registered groups.
    pub fn groups(&self) -> &[AffinityGroup] {
        &self.groups
    }

    /// Find a group by name.
    pub fn find_by_name(&self, name: &str) -> Option<&AffinityGroup> {
        self.groups.iter().find(|g| g.name == name)
    }
}

/// Load affinity groups from VoxWorkspace members.
///
/// Each workspace member becomes its own affinity group with a glob
/// pattern matching all files under its directory.
pub fn groups_from_workspace_members(members: &[(String, PathBuf)]) -> Vec<AffinityGroup> {
    members
        .iter()
        .map(|(name, dir)| {
            let pattern = format!("{}/**", dir.display());
            AffinityGroup {
                name: name.clone(),
                patterns: vec![pattern],
                default_agent: None,
            }
        })
        .collect()
}

/// Dynamic auto-assign of a workspace mapping directly reading `Vox.toml` and creating groups per directory
pub fn auto_assign_groups(workspace_root: &Path) -> Vec<AffinityGroup> {
    let mut groups = Vec::new();

    // Fallback: read directories from target
    if let Ok(entries) = std::fs::read_dir(workspace_root.join("crates")) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    let group_name = format!("{}-group", name);
                    groups.push(AffinityGroup {
                        name: group_name,
                        patterns: vec![format!("{}/**", entry.path().display())],
                        default_agent: None,
                    });
                }
            }
        }
    }
    groups
}

/// Try to load Affinity Groups directly from a `Vox.toml` or similar file format.
pub fn load_from_config(path: &Path) -> Option<AffinityGroupRegistry> {
    // In a full implementation, you'd parse `Vox.toml` [affinity_groups] table here.
    // E.g.: `vox_pm::manifest::load_manifest(path)`
    // For now we'll simulate returning defaults if the file exists
    if path.exists() {
        Some(AffinityGroupRegistry::defaults())
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_resolve_parser_files() {
        let reg = AffinityGroupRegistry::defaults();
        let group = reg.resolve(Path::new("crates/vox-parser/src/grammar.rs"));
        assert!(group.is_some());
        assert_eq!(group.unwrap().name, "lexer-parser-group");
    }

    #[test]
    fn defaults_resolve_typeck_files() {
        let reg = AffinityGroupRegistry::defaults();
        let group = reg.resolve(Path::new("crates/vox-typeck/src/infer.rs"));
        assert!(group.is_some());
        assert_eq!(group.unwrap().name, "typeck-group");
    }

    #[test]
    fn defaults_resolve_codegen_files() {
        let reg = AffinityGroupRegistry::defaults();
        let group = reg.resolve(Path::new("crates/vox-codegen-rust/src/emit.rs"));
        assert!(group.is_some());
        assert_eq!(group.unwrap().name, "codegen-rust-group");
    }

    #[test]
    fn unknown_path_returns_none() {
        let reg = AffinityGroupRegistry::defaults();
        let group = reg.resolve(Path::new("random/path/file.txt"));
        assert!(group.is_none());
    }

    #[test]
    fn workspace_member_groups() {
        let members = vec![
            ("frontend".to_string(), PathBuf::from("packages/frontend")),
            ("backend".to_string(), PathBuf::from("packages/backend")),
        ];
        let groups = groups_from_workspace_members(&members);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].name, "frontend");
        assert!(groups[0].patterns[0].contains("packages/frontend"));
    }

    #[test]
    fn find_by_name() {
        let reg = AffinityGroupRegistry::defaults();
        assert!(reg.find_by_name("lexer-parser-group").is_some());
        assert!(reg.find_by_name("nonexistent").is_none());
    }
}
