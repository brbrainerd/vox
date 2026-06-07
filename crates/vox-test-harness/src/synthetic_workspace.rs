//! Generalized synthetic Cargo workspace builder for integration tests.
//!
//! Lets a test stand up a real-on-disk Cargo workspace with N member crates
//! in a `TempRoot`, then point an external tool (`vox-arch-check`,
//! `vox-code-audit`, `vox-cli`, etc.) at it instead of the live workspace.
//! The pattern landed first in `crates/vox-arch-check/tests/helpers/fixture.rs`
//! during the test-suite perf rework; promoting it here so other crates can
//! reuse the same primitives without rebuilding the wheel.
//!
//! # Example
//! ```rust,ignore
//! use vox_test_harness::synthetic_workspace::{SyntheticWorkspaceBuilder, MemberSpec};
//!
//! let ws = SyntheticWorkspaceBuilder::new()
//!     .member(MemberSpec::library("vox-alpha"))
//!     .member(
//!         MemberSpec::library("vox-beta")
//!             .with_dep("vox-alpha")
//!             .with_description("Custom description for vox-beta with enough characters."),
//!     )
//!     .with_git_stub()
//!     .with_changelog("0.1.0", "2026-01-01")
//!     .build()?;
//! // ws.root() is the workspace root; ws.member_path("vox-alpha") is its crate dir.
//! ```

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::temp_root::TempRoot;

const DEFAULT_LONG_DESCRIPTION: &str =
    "A synthetic workspace member produced by vox_test_harness::synthetic_workspace.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberKind {
    /// `src/lib.rs` (library crate). Default.
    Library,
    /// `src/main.rs` (binary crate).
    Binary,
}

#[derive(Debug, Clone)]
pub struct MemberSpec {
    pub name: String,
    pub description: Option<String>,
    pub deps: Vec<String>,
    pub kind: MemberKind,
    pub source_override: Option<String>,
}

impl MemberSpec {
    pub fn library(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            deps: Vec::new(),
            kind: MemberKind::Library,
            source_override: None,
        }
    }

    pub fn binary(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            deps: Vec::new(),
            kind: MemberKind::Binary,
            source_override: None,
        }
    }

    /// Set the Cargo.toml `description` field. Use this to test description-rule
    /// violations: pass a string shorter than the rule threshold.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Declare a path dependency on another workspace member.
    pub fn with_dep(mut self, dep: impl Into<String>) -> Self {
        self.deps.push(dep.into());
        self
    }

    /// Override the default `lib.rs` / `main.rs` body. Use to test code-pattern
    /// rules (forbidden patterns, docstring lint, etc.).
    pub fn with_source(mut self, body: impl Into<String>) -> Self {
        self.source_override = Some(body.into());
        self
    }
}

pub struct SyntheticWorkspaceBuilder {
    members: Vec<MemberSpec>,
    git_stub: bool,
    changelog: Option<(String, String)>,
    extras: Vec<(PathBuf, String)>,
}

impl Default for SyntheticWorkspaceBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl SyntheticWorkspaceBuilder {
    pub fn new() -> Self {
        Self {
            members: Vec::new(),
            git_stub: false,
            changelog: None,
            extras: Vec::new(),
        }
    }

    pub fn member(mut self, spec: MemberSpec) -> Self {
        self.members.push(spec);
        self
    }

    /// Write a `.git/HEAD` stub so callers that invoke `git` commands in the
    /// fixture directory don't fall through to the host repo.
    pub fn with_git_stub(mut self) -> Self {
        self.git_stub = true;
        self
    }

    /// Write a minimal `CHANGELOG.md` with a single released version. Required
    /// by callers that read the changelog for release-date / tag context.
    pub fn with_changelog(mut self, version: impl Into<String>, date: impl Into<String>) -> Self {
        self.changelog = Some((version.into(), date.into()));
        self
    }

    /// Drop an arbitrary file into the workspace at `rel_path` (relative to root).
    pub fn with_extra_file(
        mut self,
        rel_path: impl Into<PathBuf>,
        contents: impl Into<String>,
    ) -> Self {
        self.extras.push((rel_path.into(), contents.into()));
        self
    }

    pub fn build(self) -> Result<SyntheticWorkspace> {
        let temp = TempRoot::new().context("create temp root")?;
        let root = temp.path().to_path_buf();

        // Root Cargo.toml: virtual workspace listing every member.
        let mut ws_toml = String::from("[workspace]\nmembers = [");
        for (i, m) in self.members.iter().enumerate() {
            if i > 0 {
                ws_toml.push_str(", ");
            }
            ws_toml.push_str(&format!("\"crates/{}\"", m.name));
        }
        ws_toml.push_str("]\nresolver = \"2\"\n");
        fs::write(root.join("Cargo.toml"), ws_toml).context("write workspace Cargo.toml")?;

        // Cargo.lock stub — some tools key cache state on it.
        fs::write(root.join("Cargo.lock"), "# workspace lock\n")?;

        if let Some((version, date)) = self.changelog {
            fs::write(
                root.join("CHANGELOG.md"),
                format!("## [{version}] - {date}\n\n- initial\n"),
            )?;
        }

        if self.git_stub {
            fs::create_dir_all(root.join(".git"))?;
            fs::write(root.join(".git/HEAD"), "ref: refs/heads/main\n")?;
        }

        for member in &self.members {
            write_member(&root, member)?;
        }

        for (rel, contents) in self.extras {
            let path = root.join(rel);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&path, contents)?;
        }

        Ok(SyntheticWorkspace { temp })
    }
}

fn write_member(root: &Path, spec: &MemberSpec) -> Result<()> {
    let crate_dir = root.join("crates").join(&spec.name);
    fs::create_dir_all(crate_dir.join("src"))?;

    let description = spec
        .description
        .clone()
        .unwrap_or_else(|| DEFAULT_LONG_DESCRIPTION.to_string());

    let mut cargo_toml = format!(
        "[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2021\"\ndescription = \"{}\"\n",
        spec.name,
        description.replace('"', "\\\""),
    );
    if !spec.deps.is_empty() {
        cargo_toml.push_str("\n[dependencies]\n");
        for dep in &spec.deps {
            cargo_toml.push_str(&format!("{dep} = {{ path = \"../{dep}\" }}\n"));
        }
    }
    fs::write(crate_dir.join("Cargo.toml"), cargo_toml)?;

    let source = spec
        .source_override
        .clone()
        .unwrap_or_else(|| match spec.kind {
            MemberKind::Library => {
                format!("//! {} library.\npub fn placeholder() {{}}\n", spec.name)
            }
            MemberKind::Binary => format!("//! {} binary.\nfn main() {{}}\n", spec.name),
        });
    let source_name = match spec.kind {
        MemberKind::Library => "lib.rs",
        MemberKind::Binary => "main.rs",
    };
    fs::write(crate_dir.join("src").join(source_name), source)?;
    Ok(())
}

pub struct SyntheticWorkspace {
    temp: TempRoot,
}

impl SyntheticWorkspace {
    pub fn root(&self) -> &Path {
        self.temp.path()
    }

    /// Absolute path of a member crate's directory.
    pub fn member_path(&self, name: &str) -> PathBuf {
        self.root().join("crates").join(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_two_member_workspace() {
        let ws = SyntheticWorkspaceBuilder::new()
            .member(MemberSpec::library("vox-alpha"))
            .member(MemberSpec::library("vox-beta").with_dep("vox-alpha"))
            .build()
            .unwrap();
        assert!(ws.root().join("Cargo.toml").is_file());
        assert!(ws.root().join("Cargo.lock").is_file());
        assert!(ws.member_path("vox-alpha").join("Cargo.toml").is_file());
        assert!(ws.member_path("vox-alpha").join("src/lib.rs").is_file());
        let beta_toml = fs::read_to_string(ws.member_path("vox-beta").join("Cargo.toml")).unwrap();
        assert!(beta_toml.contains("vox-alpha = { path = \"../vox-alpha\" }"));
    }

    #[test]
    fn with_description_round_trips() {
        let ws = SyntheticWorkspaceBuilder::new()
            .member(MemberSpec::library("alpha").with_description("short"))
            .build()
            .unwrap();
        let cargo = fs::read_to_string(ws.member_path("alpha").join("Cargo.toml")).unwrap();
        assert!(cargo.contains("description = \"short\""));
    }

    #[test]
    fn with_git_stub_and_changelog() {
        let ws = SyntheticWorkspaceBuilder::new()
            .with_git_stub()
            .with_changelog("0.2.0", "2026-05-28")
            .member(MemberSpec::binary("voxup"))
            .build()
            .unwrap();
        assert!(ws.root().join(".git/HEAD").is_file());
        let changelog = fs::read_to_string(ws.root().join("CHANGELOG.md")).unwrap();
        assert!(changelog.contains("[0.2.0] - 2026-05-28"));
        assert!(ws.member_path("voxup").join("src/main.rs").is_file());
    }

    #[test]
    fn extra_files_land_at_expected_path() {
        let ws = SyntheticWorkspaceBuilder::new()
            .member(MemberSpec::library("alpha"))
            .with_extra_file(
                "docs/src/architecture/layers.toml",
                "[crates.alpha]\nlayer = 0\n",
            )
            .build()
            .unwrap();
        let layers =
            fs::read_to_string(ws.root().join("docs/src/architecture/layers.toml")).unwrap();
        assert!(layers.contains("[crates.alpha]"));
    }

    #[test]
    fn source_override_replaces_default() {
        let ws = SyntheticWorkspaceBuilder::new()
            .member(MemberSpec::library("alpha").with_source("//! Custom.\npub fn special() {}\n"))
            .build()
            .unwrap();
        let source = fs::read_to_string(ws.member_path("alpha").join("src/lib.rs")).unwrap();
        assert!(source.contains("pub fn special"));
    }
}
