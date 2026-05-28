//! Workspace-wide affinity groups: globs that route files to preferred agents.
//!
//! [`AffinityGroupRegistry`] compiles patterns from repository layout (Cargo, Node, etc.)
//! so the orchestrator can keep related edits on one agent.

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

fn repo_relative_glob(repo_root: &Path, dir: &Path) -> String {
    let rel = dir.strip_prefix(repo_root).unwrap_or(dir);
    let s = rel.to_string_lossy().replace('\\', "/");
    let s = s.trim_matches('/').to_string();
    if s.is_empty() {
        "**/*".to_string()
    } else {
        format!("{s}/**")
    }
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
                    "**/vox-compiler/src/lexer/**".to_string(),
                    "**/vox-compiler/src/parser/**".to_string(),
                    "**/vox-compiler/src/ast/**".to_string(),
                ],
                default_agent: None,
            },
            AffinityGroup {
                name: "typeck-group".to_string(),
                patterns: vec!["**/vox-compiler/src/typeck/**".to_string()],
                default_agent: None,
            },
            AffinityGroup {
                name: "hir-group".to_string(),
                patterns: vec!["**/vox-compiler/src/hir/**".to_string()],
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
                patterns: vec!["**/vox-actor-runtime/**".to_string()],
                default_agent: None,
            },
            AffinityGroup {
                name: "orchestrator-group".to_string(),
                patterns: vec!["**/vox-orchestrator/**".to_string()],
                default_agent: None,
            },
            AffinityGroup {
                name: "pm-group".to_string(),
                patterns: vec!["**/vox-package/**".to_string()],
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

    /// Build affinity groups from on-disk repository layout (Cargo workspace members, `crates/`, or catch-all).
    ///
    /// Used when the orchestrator should adapt to an external or polyglot repo instead of hardcoded Vox crate names.
    pub fn detect_from_repository_layout(repo_root: &Path) -> Self {
        let member_dirs = vox_repository::cargo_workspace_member_dirs(repo_root);
        if !member_dirs.is_empty() {
            let groups: Vec<AffinityGroup> = member_dirs
                .into_iter()
                .filter_map(|p| {
                    let name = p.file_name()?.to_string_lossy().into_owned();
                    let rg = repo_relative_glob(repo_root, &p);
                    Some(AffinityGroup {
                        name: format!("{name}-group"),
                        patterns: vec![
                            format!("**/crates/{name}/**"),
                            format!("crates/{name}/**"),
                            format!("**/{name}/**"),
                            rg,
                        ],
                        default_agent: None,
                    })
                })
                .collect();
            if !groups.is_empty() {
                return Self::new(groups);
            }
        }

        let node = vox_repository::node_workspace_packages(repo_root);
        if !node.is_empty() {
            let groups: Vec<AffinityGroup> = node
                .into_iter()
                .map(|(name, p)| {
                    let pat = repo_relative_glob(repo_root, &p);
                    AffinityGroup {
                        name: format!("node-{name}"),
                        patterns: vec![pat],
                        default_agent: None,
                    }
                })
                .collect();
            return Self::new(groups);
        }

        let py = vox_repository::python_roots(repo_root);
        if !py.is_empty() {
            let groups: Vec<AffinityGroup> = py
                .into_iter()
                .map(|(name, p)| AffinityGroup {
                    name: format!("{name}-group"),
                    patterns: vec![repo_relative_glob(repo_root, &p)],
                    default_agent: None,
                })
                .collect();
            return Self::new(groups);
        }

        let go = vox_repository::go_roots(repo_root);
        if !go.is_empty() {
            let groups: Vec<AffinityGroup> = go
                .into_iter()
                .map(|(name, p)| AffinityGroup {
                    name: format!("{name}-group"),
                    patterns: vec![repo_relative_glob(repo_root, &p)],
                    default_agent: None,
                })
                .collect();
            return Self::new(groups);
        }

        let crates_dir = repo_root.join("crates");
        if crates_dir.is_dir() {
            let mut groups = Vec::new();
            if let Ok(rd) = std::fs::read_dir(&crates_dir) {
                for ent in rd.flatten() {
                    let p = ent.path();
                    if p.join("Cargo.toml").is_file() {
                        let name = p.file_name().unwrap().to_string_lossy().into_owned();
                        groups.push(AffinityGroup {
                            name: format!("{name}-group"),
                            patterns: vec![
                                format!("**/crates/{name}/**"),
                                format!("crates/{name}/**"),
                            ],
                            default_agent: None,
                        });
                    }
                }
            }
            if !groups.is_empty() {
                return Self::new(groups);
            }
        }

        Self::new(vec![AffinityGroup {
            name: "workspace".to_string(),
            patterns: vec!["**/*".to_string()],
            default_agent: None,
        }])
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

/// Load affinity groups from `Vox.toml` when an `affinity_groups` array is present.
///
/// Expected shape:
///
/// ```toml
/// [[affinity_groups]]
/// name = "my-group"
/// patterns = ["crates/foo/**", "docs/**"]
/// ```
///
/// If the file is missing, invalid TOML, or `affinity_groups` is absent or empty, returns `None`.
/// Callers should fall back to [`AffinityGroupRegistry::detect_from_repository_layout`] or
/// [`AffinityGroupRegistry::defaults`].
pub fn load_from_config(path: &Path) -> Option<AffinityGroupRegistry> {
    let raw = vox_bounded_fs::read_utf8_path_capped(path).ok()?;
    let value: toml::Value = raw.parse().ok()?;
    let root = value.as_table()?;
    let ag = root.get("affinity_groups")?;
    let arr = ag.as_array()?;
    if arr.is_empty() {
        return None;
    }
    let mut groups = Vec::new();
    for item in arr {
        let t = item.as_table()?;
        let Some(name) = t
            .get("name")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let patterns: Vec<String> = match t.get("patterns") {
            Some(toml::Value::Array(a)) => a
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect(),
            Some(toml::Value::String(s)) => vec![s.clone()],
            None => continue,
            Some(_) => return None,
        };
        if patterns.is_empty() {
            continue;
        }
        groups.push(AffinityGroup {
            name,
            patterns,
            default_agent: None,
        });
    }
    if groups.is_empty() {
        None
    } else {
        Some(AffinityGroupRegistry::new(groups))
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
        let group = reg.resolve(Path::new("crates/vox-compiler/src/parser/mod.rs"));
        assert!(group.is_some());
        assert_eq!(group.unwrap().name, "lexer-parser-group");
    }

    #[test]
    fn defaults_resolve_typeck_files() {
        let reg = AffinityGroupRegistry::defaults();
        let group = reg.resolve(Path::new("crates/vox-compiler/src/typeck/infer.rs"));
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

    #[test]
    fn detect_from_layout_matches_member_crate_paths() {
        use std::fs;
        let d = tempfile::TempDir::new().expect("tempdir");
        fs::write(
            d.path().join("Cargo.toml"),
            r#"[workspace]
members = ["crates/*"]
resolver = "2"
"#,
        )
        .expect("root");
        let c = d.path().join("crates");
        fs::create_dir_all(c.join("alpha")).expect("mkdir");
        fs::write(
            c.join("alpha").join("Cargo.toml"),
            "[package]\nname = \"alpha\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .expect("crate");
        let reg = AffinityGroupRegistry::detect_from_repository_layout(d.path());
        let g = reg.resolve(Path::new("crates/alpha/src/lib.rs"));
        assert!(g.is_some());
        assert_eq!(g.unwrap().name, "alpha-group");
    }

    #[test]
    fn load_from_config_parses_affinity_groups() {
        use std::fs;
        let d = tempfile::TempDir::new().expect("tempdir");
        let path = d.path().join("Vox.toml");
        fs::write(
            &path,
            r#"[[affinity_groups]]
name = "docs"
patterns = ["docs/**"]

[[affinity_groups]]
name = "single-glob"
patterns = "legacy/*.md"
"#,
        )
        .expect("write Vox.toml");
        let reg = load_from_config(&path).expect("registry");
        let names: Vec<_> = reg.groups().iter().map(|g| g.name.as_str()).collect();
        assert!(names.contains(&"docs"));
        assert!(names.contains(&"single-glob"));
        let g = reg.resolve(Path::new("docs/foo.md"));
        assert_eq!(g.expect("match").name, "docs");
    }

    #[test]
    fn load_from_config_missing_or_empty_returns_none() {
        use std::fs;
        let d = tempfile::TempDir::new().expect("tempdir");
        let path = d.path().join("Vox.toml");
        fs::write(&path, "[vox]\nmodel = \"x\"\n").unwrap();
        assert!(load_from_config(&path).is_none());
        fs::write(&path, "affinity_groups = []\n").unwrap();
        assert!(load_from_config(&path).is_none());
    }

    #[test]
    fn detect_layout_node_workspaces() {
        use std::fs;
        let d = tempfile::TempDir::new().expect("tempdir");
        fs::write(
            d.path().join("package.json"),
            r#"{"name":"root","workspaces":["packages/*"]}"#,
        )
        .unwrap();
        let pkg_a = d.path().join("packages").join("a");
        fs::create_dir_all(&pkg_a).unwrap();
        fs::write(pkg_a.join("package.json"), "{}").unwrap();
        let reg = AffinityGroupRegistry::detect_from_repository_layout(d.path());
        let names: Vec<_> = reg.groups().iter().map(|g| g.name.as_str()).collect();
        assert!(
            names.contains(&"node-a"),
            "expected node-a group, got {names:?}"
        );
    }
}
