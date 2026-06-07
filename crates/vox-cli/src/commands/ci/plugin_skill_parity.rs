//! `vox ci plugin-skill-parity [--write]`
//!
//! Walks `crates/` for any `Plugin.toml` declaring a skill or composite payload, asserts
//! the referenced `skill-md` file exists and is non-empty, and that `tools.exposes` is
//! non-empty.
//!
//! Also enforces **exposes-tools parity**: the `SKILL.md` frontmatter `vox-tools` list (which
//! the skill loader registers for agents, and which the skill body documents) is the single
//! source of truth, so each manifest `tools.exposes` must match it. `--write` rewrites the
//! manifest `exposes = [...]` line from the SKILL.md; without it the gate fails on drift.
//!
//! Note: SKILL.md is authoritative here (not the manifest) because the human-authored skill
//! body and its `vox-tools` frontmatter are what actually describe the real, registered tools;
//! manifests historically carried stale placeholder tool names.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct ManifestHead {
    plugin: PluginHead,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct PluginHead {
    #[allow(dead_code)]
    id: String,
    payload: PayloadHead,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
enum PayloadHead {
    Code {},
    Skill(SkillHead),
    Composite {
        #[serde(default)]
        skill: SkillHead,
    },
}

#[derive(Clone, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
struct SkillHead {
    #[serde(default)]
    skill_md: String,
    #[serde(default)]
    tools: ToolsHead,
}

#[derive(Clone, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
struct ToolsHead {
    #[serde(default)]
    exposes: Vec<String>,
}

/// The TOML frontmatter block of a SKILL.md (between the leading `---` fences), if present.
fn frontmatter_block(body: &str) -> Option<&str> {
    let rest = body.strip_prefix("---")?;
    let end = rest.find("\n---")?;
    Some(rest[..end].trim_start_matches(['\n', '\r']))
}

/// The `[metadata] vox-tools` list declared in a SKILL.md frontmatter, if present.
fn frontmatter_vox_tools(body: &str) -> Option<Vec<String>> {
    let fm = frontmatter_block(body)?;
    let val: toml::Value = fm.parse().ok()?;
    val.get("metadata")
        .and_then(|m| m.get("vox-tools"))
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|e| e.as_str().map(str::to_string))
                .collect()
        })
}

/// Replace the manifest `exposes = [...]` line (inside `[...tools]`) with one derived from the
/// SKILL.md `vox-tools` list, preserving the original line's leading indentation. Returns `None`
/// if no `exposes` line was found.
fn rewrite_manifest_exposes(raw: &str, tools: &[String]) -> Option<String> {
    let items: Vec<String> = tools.iter().map(|s| format!("\"{s}\"")).collect();
    let mut out: Vec<String> = Vec::new();
    let mut replaced = false;
    for line in raw.lines() {
        let trimmed = line.trim_start();
        if !replaced && (trimmed.starts_with("exposes =") || trimmed.starts_with("exposes=")) {
            let indent = &line[..line.len() - trimmed.len()];
            out.push(format!("{indent}exposes = [{}]", items.join(", ")));
            replaced = true;
            continue;
        }
        out.push(line.to_string());
    }
    if !replaced {
        return None;
    }
    let mut joined = out.join("\n");
    if raw.ends_with('\n') {
        joined.push('\n');
    }
    Some(joined)
}

pub fn run(write: bool) -> Result<()> {
    let mut errors: Vec<String> = Vec::new();
    let mut checked = 0usize;
    let mut rewritten = 0usize;

    let crates_root = Path::new("crates");
    if !crates_root.is_dir() {
        println!("✓ no crates/ dir; nothing to check");
        return Ok(());
    }

    for entry in walkdir::WalkDir::new(crates_root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name() == "Plugin.toml")
    {
        let path = entry.path();
        if path.components().any(|c| c.as_os_str() == "tests") {
            continue;
        }
        let raw =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let head: ManifestHead = match toml::from_str(&raw) {
            Ok(v) => v,
            Err(e) => {
                errors.push(format!("{}: parse error: {e}", path.display()));
                continue;
            }
        };
        let skill = match &head.plugin.payload {
            PayloadHead::Skill(s) => s.clone(),
            PayloadHead::Composite { skill } => skill.clone(),
            PayloadHead::Code {} => continue,
        };
        if skill.skill_md.is_empty() {
            errors.push(format!("{}: skill-md is empty", path.display()));
            continue;
        }
        let skill_md_path = path.parent().unwrap().join(&skill.skill_md);
        let body = match std::fs::read_to_string(&skill_md_path) {
            Ok(body) if body.trim().is_empty() => {
                errors.push(format!(
                    "{}: skill-md '{}' is empty",
                    path.display(),
                    skill.skill_md
                ));
                continue;
            }
            Ok(body) => body,
            Err(e) => {
                errors.push(format!(
                    "{}: skill-md '{}' not readable: {e}",
                    path.display(),
                    skill.skill_md,
                ));
                continue;
            }
        };
        if skill.tools.exposes.is_empty() {
            errors.push(format!("{}: tools.exposes is empty", path.display()));
        }

        // exposes-tools parity: SKILL.md vox-tools is authoritative; manifest tools.exposes must match.
        let manifest_set: BTreeSet<&str> = skill.tools.exposes.iter().map(String::as_str).collect();
        match frontmatter_vox_tools(&body) {
            None => errors.push(format!(
                "{}: SKILL.md '{}' has no `[metadata] vox-tools` frontmatter",
                path.display(),
                skill.skill_md
            )),
            Some(fm) => {
                let fm_set: BTreeSet<&str> = fm.iter().map(String::as_str).collect();
                if fm_set != manifest_set {
                    if write {
                        match rewrite_manifest_exposes(&raw, &fm) {
                            Some(updated) => {
                                std::fs::write(path, updated)
                                    .with_context(|| format!("writing {}", path.display()))?;
                                rewritten += 1;
                            }
                            None => errors.push(format!(
                                "{}: could not locate `exposes` line to rewrite",
                                path.display()
                            )),
                        }
                    } else {
                        errors.push(format!(
                            "{}: manifest tools.exposes {:?} != SKILL.md '{}' vox-tools {:?} (run `vox ci plugin-skill-parity --write`)",
                            path.display(),
                            skill.tools.exposes,
                            skill.skill_md,
                            fm,
                        ));
                    }
                }
            }
        }
        checked += 1;
    }

    if write {
        println!(
            "plugin-skill-parity: {rewritten} manifest tools.exposes list(s) synced from SKILL.md vox-tools ({checked} checked)"
        );
        return Ok(());
    }
    if errors.is_empty() {
        println!("✓ plugin-skill-parity ok ({checked} skill-bearing manifests checked)");
        Ok(())
    } else {
        for e in &errors {
            eprintln!("✗ {e}");
        }
        anyhow::bail!("plugin-skill-parity failed with {} error(s)", errors.len())
    }
}
