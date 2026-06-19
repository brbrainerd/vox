//! Dedup installed skills and validate the MCP↔skill SSOT. Operates on a provided
//! slice of `SkillManifest` (caller supplies; v1 loads from a JSON file). Wiring to
//! a live `SkillRegistry` is a deferred follow-up.

use vox_plugin_types::skill_manifest::SkillManifest;
use vox_similarity::{Fragment, FragmentKind, LshIndex};

use crate::candidate::{Candidate, CandidateKind};
use crate::options::DiscoverOptions;
use std::collections::HashSet;

/// Build the comparable text for a skill manifest.
fn manifest_text(m: &SkillManifest) -> String {
    let mut parts = vec![m.name.clone(), m.description.clone()];
    parts.extend(m.tags.iter().cloned());
    parts.extend(m.tools.iter().cloned());
    parts.join(" ")
}

/// Find installed skills that overlap heavily (near-duplicate skills).
pub fn dedup_skills(manifests: &[SkillManifest], opts: &DiscoverOptions) -> Vec<Candidate> {
    let mut index = LshIndex::new(opts.bands, opts.rows);
    for m in manifests {
        let text = manifest_text(m);
        let frag = Fragment::new(
            m.id.clone(),
            FragmentKind::InstalledSkill,
            text,
            m.id.clone(),
            opts.shingle_k,
            opts.num_hashes(),
        );
        index.insert(frag);
    }

    let mut out = Vec::new();
    for cluster in index.cluster(2, opts.min_jaccard) {
        let members: Vec<String> = cluster
            .members
            .iter()
            .map(|&i| index.fragment(i).source_ref.clone())
            .collect();
        let score = vox_similarity::mean_pairwise_jaccard(&index, &cluster.members);
        out.push(Candidate {
            kind: CandidateKind::DuplicatesInstalled,
            members,
            score,
            suggested_action: "These installed skills overlap — consider consolidating or reusing one"
                .to_string(),
            draft_frontmatter: None,
        });
    }
    out
}

/// The set of all known MCP tool names (registry + skill + orchestrator tool lists).
fn known_tool_names() -> HashSet<String> {
    let mut set = HashSet::new();
    for entry in vox_mcp_registry::TOOL_REGISTRY.iter() {
        set.insert(entry.name.to_string());
    }
    for t in vox_mcp_registry::SKILL_TOOLS {
        set.insert((*t).to_string());
    }
    for t in vox_mcp_registry::ORCHESTRATOR_TOOLS {
        set.insert((*t).to_string());
    }
    set
}

/// Flag skills that declare a `tool` not present in the MCP registry (SSOT drift).
pub fn validate_ssot(manifests: &[SkillManifest]) -> Vec<Candidate> {
    let known = known_tool_names();
    let mut out = Vec::new();
    for m in manifests {
        for tool in &m.tools {
            if !known.contains(tool) {
                out.push(Candidate {
                    kind: CandidateKind::SsotDrift,
                    members: vec![format!("{}->{}", m.id, tool)],
                    score: 1.0,
                    suggested_action: format!(
                        "Skill '{}' declares tool '{}' which is not in the MCP registry — fix the manifest or register the tool",
                        m.id, tool
                    ),
                    draft_frontmatter: None,
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_plugin_types::skill_manifest::SkillCategory;

    fn manifest(id: &str, name: &str, desc: &str) -> SkillManifest {
        SkillManifest::new(id, name, "0.1.0", "test", desc, SkillCategory::Unknown)
    }

    #[test]
    fn dedup_flags_near_identical_skills() {
        let opts = DiscoverOptions {
            shingle_k: 2,
            ..DiscoverOptions::default()
        };
        let manifests = vec![
            manifest(
                "a.fmt",
                "format vox",
                "Formats vox source files with the standard style",
            ),
            manifest(
                "b.fmt",
                "format vox",
                "Formats vox source files with the standard style",
            ),
            manifest(
                "c.git",
                "git status",
                "Shows the working tree status using git",
            ),
        ];
        let cands = dedup_skills(&manifests, &opts);
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].members.len(), 2);
        assert!(cands[0].score >= 0.9, "near-identical skills score high, got {}", cands[0].score);
    }

    #[test]
    fn validate_ssot_flags_unknown_tool() {
        let mut m = manifest("x.bad", "bad skill", "declares a phantom tool");
        m.tools = vec!["vox_totally_made_up_tool".to_string()];
        let cands = validate_ssot(&[m]);
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].kind, CandidateKind::SsotDrift);
        assert!(cands[0].members[0].contains("vox_totally_made_up_tool"));
    }

    #[test]
    fn validate_ssot_accepts_known_tool() {
        let mut m = manifest("x.good", "good skill", "declares a real tool");
        m.tools = vec!["vox_skill_list".to_string()];
        assert!(validate_ssot(&[m]).is_empty());
    }
}
