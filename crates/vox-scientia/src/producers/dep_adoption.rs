//! Dependency-adoption signal producer.
//!
//! Parses the workspace `Cargo.toml`'s `[workspace.dependencies]` table and
//! counts how many in-tree crates declare each dep as `{ workspace = true }`.
//! A workspace dep adopted by ≥ [`MIN_CONSUMERS`] crates indicates a
//! load-bearing infrastructure choice worth publishing about — e.g., a new
//! retrieval backend, a new serialization scheme, a new policy enforcer.
//!
//! Pure text-parsing — does not load Cargo, so the producer stays fast and
//! has no transitive-dep weight.

use async_trait::async_trait;
use sha3::{Digest, Sha3_256};
use vox_research_events::ResearchEvent;

use super::heuristics::date_slug;
use super::producer::{Producer, ProducerContext};

const PRODUCER_NAME: &str = "dep_adoption";
/// Minimum number of in-tree crates that must declare a dep before the
/// producer emits a candidate for it.
pub const MIN_CONSUMERS: usize = 3;

pub struct DepAdoptionProducer;

impl DepAdoptionProducer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DepAdoptionProducer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Producer for DepAdoptionProducer {
    fn name(&self) -> &'static str {
        PRODUCER_NAME
    }

    async fn observe(&self, ctx: &ProducerContext) -> Vec<ResearchEvent> {
        scan(&ctx.repo_root, ctx.now_ms, &ctx.session_id)
    }
}

/// Extract workspace-dep names from the root `Cargo.toml`'s
/// `[workspace.dependencies]` section. Returns lowercased crate names.
///
/// Public so tests can validate the parser independently.
pub fn extract_workspace_dep_names(cargo_toml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_section = false;
    for line in cargo_toml.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('[') {
            in_section = trimmed.starts_with("[workspace.dependencies]");
            continue;
        }
        if !in_section {
            continue;
        }
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        // Lines look like `name = "1.0"` or `name = { path = "..." }`.
        if let Some((name, _)) = trimmed.split_once('=') {
            let name = name.trim();
            if !name.is_empty() && !name.starts_with('#') {
                out.push(name.to_ascii_lowercase());
            }
        }
    }
    out
}

/// Count how many in-tree crates declare each `dep_name` as a workspace
/// dependency in their `Cargo.toml`. Match pattern: a line starting with
/// `<dep_name>` (or `<dep_name>.workspace`) followed by `=` and containing
/// `workspace = true` or `workspace.workspace`.
pub fn count_dep_consumers(
    crates_root: &std::path::Path,
    dep_names: &[String],
) -> std::collections::HashMap<String, usize> {
    let mut counts: std::collections::HashMap<String, usize> = dep_names
        .iter()
        .map(|n| (n.clone(), 0usize))
        .collect();
    let Ok(entries) = std::fs::read_dir(crates_root) else {
        return counts;
    };
    for entry in entries.flatten() {
        let crate_dir = entry.path();
        let cargo = crate_dir.join("Cargo.toml");
        if !cargo.is_file() {
            continue;
        }
        let Ok(contents) = std::fs::read_to_string(&cargo) else {
            continue;
        };
        for line in contents.lines() {
            let trimmed = line.trim_start();
            // `<name> = { workspace = true ... }` or
            // `<name>.workspace = true`.
            if !trimmed.contains("workspace") || trimmed.starts_with('#') {
                continue;
            }
            for name in dep_names {
                let prefix_a = format!("{name} ");
                let prefix_b = format!("{name}.workspace");
                let prefix_c = format!("{name}=");
                if trimmed.to_ascii_lowercase().starts_with(&prefix_a)
                    || trimmed.to_ascii_lowercase().starts_with(&prefix_b)
                    || trimmed.to_ascii_lowercase().starts_with(&prefix_c)
                {
                    *counts.entry(name.clone()).or_insert(0) += 1;
                    break;
                }
            }
        }
    }
    counts
}

fn scan(
    repo_root: &std::path::Path,
    now_ms: i64,
    session_id: &str,
) -> Vec<ResearchEvent> {
    let workspace_cargo = repo_root.join("Cargo.toml");
    let Ok(cargo_toml) = std::fs::read_to_string(&workspace_cargo) else {
        return Vec::new();
    };
    let dep_names = extract_workspace_dep_names(&cargo_toml);
    if dep_names.is_empty() {
        return Vec::new();
    }
    let crates_root = repo_root.join("crates");
    let counts = count_dep_consumers(&crates_root, &dep_names);

    let slug = date_slug(now_ms);
    let mut out = Vec::new();
    let mut sorted: Vec<(String, usize)> = counts.into_iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    for (dep, count) in sorted {
        if count < MIN_CONSUMERS {
            continue;
        }
        let mut h = Sha3_256::new();
        h.update(PRODUCER_NAME.as_bytes());
        h.update(b"::");
        h.update(dep.as_bytes());
        h.update(count.to_le_bytes());
        let digest = h.finalize();
        let sha8: String = digest.iter().take(4).map(|b| format!("{b:02x}")).collect();
        let finding_id = format!("algimp-{slug}-dep-{sha8}");
        let score = ((count as f64) / 20.0).min(1.0);
        out.push(ResearchEvent::FindingCandidateProposed {
            finding_id,
            claim_ids: vec![],
            worthiness_score: score,
            session_id: session_id.to_string(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_dep_names_handles_canonical_workspace_block() {
        let toml = r#"
[workspace]
members = ["crates/*"]

[workspace.dependencies]
serde = { version = "1" }
tokio = { version = "1", features = ["full"] }
# commented = "0.1"
anyhow = "1"

[profile.dev]
opt-level = 0
"#;
        let names = extract_workspace_dep_names(toml);
        assert_eq!(names, vec!["serde", "tokio", "anyhow"]);
    }

    #[test]
    fn extract_dep_names_lowercases_for_case_insensitive_matching() {
        let toml = r#"
[workspace.dependencies]
Foo = "1"
BAR = "1"
"#;
        let names = extract_workspace_dep_names(toml);
        assert_eq!(names, vec!["foo", "bar"]);
    }

    #[test]
    fn extract_dep_names_outside_section_returns_empty() {
        let toml = r#"
[package]
name = "demo"
"#;
        assert!(extract_workspace_dep_names(toml).is_empty());
    }

    fn write_crate_with_dep(
        crates_root: &std::path::Path,
        crate_name: &str,
        dep_line: &str,
    ) {
        let crate_dir = crates_root.join(crate_name);
        std::fs::create_dir_all(&crate_dir).unwrap();
        std::fs::write(
            crate_dir.join("Cargo.toml"),
            format!(
                "[package]\nname = \"{crate_name}\"\nversion = \"0.1.0\"\n\n[dependencies]\n{dep_line}\n"
            ),
        )
        .unwrap();
    }

    #[test]
    fn count_dep_consumers_matches_workspace_true_form() {
        let tmp = tempfile::tempdir().unwrap();
        write_crate_with_dep(tmp.path(), "a", "serde = { workspace = true }");
        write_crate_with_dep(tmp.path(), "b", "serde = { workspace = true }");
        write_crate_with_dep(tmp.path(), "c", "tokio = { workspace = true }");
        let counts = count_dep_consumers(tmp.path(), &["serde".into(), "tokio".into()]);
        assert_eq!(counts["serde"], 2);
        assert_eq!(counts["tokio"], 1);
    }

    #[test]
    fn count_dep_consumers_matches_dotted_workspace_form() {
        let tmp = tempfile::tempdir().unwrap();
        write_crate_with_dep(tmp.path(), "a", "serde.workspace = true");
        let counts = count_dep_consumers(tmp.path(), &["serde".into()]);
        assert_eq!(counts["serde"], 1);
    }

    #[test]
    fn scan_below_threshold_produces_no_candidates() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[workspace.dependencies]\nserde = \"1\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(tmp.path().join("crates")).unwrap();
        write_crate_with_dep(&tmp.path().join("crates"), "a", "serde = { workspace = true }");
        let out = scan(tmp.path(), 1_747_000_000_000, "s");
        assert!(out.is_empty()); // only 1 consumer, threshold is 3
    }

    #[test]
    fn scan_at_threshold_emits_algimp_candidate() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[workspace.dependencies]\nserde = \"1\"\n",
        )
        .unwrap();
        let crates_root = tmp.path().join("crates");
        std::fs::create_dir_all(&crates_root).unwrap();
        for name in ["a", "b", "c"] {
            write_crate_with_dep(&crates_root, name, "serde = { workspace = true }");
        }
        let out = scan(tmp.path(), 1_747_000_000_000, "s");
        assert_eq!(out.len(), 1);
        match &out[0] {
            ResearchEvent::FindingCandidateProposed { finding_id, .. } => {
                assert!(finding_id.starts_with("algimp-"));
                assert!(finding_id.contains("-dep-"));
            }
            _ => panic!("unexpected variant"),
        }
    }

    #[test]
    fn missing_workspace_cargo_toml_yields_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let out = scan(tmp.path(), 1_747_000_000_000, "s");
        assert!(out.is_empty());
    }
}
