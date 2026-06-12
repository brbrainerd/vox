use super::SweepRule;
use crate::features::ExtractedFeatures;
use std::collections::HashMap;
use vox_code_audit::rules::{Finding, FindingConfidence, Severity};

pub struct BodyHashRule {
    pub threshold: usize,
    pub min_lines: u32,
}

impl Default for BodyHashRule {
    fn default() -> Self {
        Self {
            threshold: 2,
            min_lines: 5,
        }
    }
}

impl SweepRule for BodyHashRule {
    fn id(&self) -> &'static str {
        "sweep/duplicate-body"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }

    fn sweep(
        &self,
        files: &[ExtractedFeatures],
        ctx: &crate::rules::WorkspaceContext,
    ) -> Vec<Finding> {
        let mut index: HashMap<u64, Vec<(std::path::PathBuf, String, usize)>> = HashMap::new();
        for f in files {
            for def in &f.fn_definitions {
                let sig = f
                    .body_signatures
                    .iter()
                    .find(|b| b.parent_fn.as_deref() == Some(&def.name));
                if let Some(sig) = sig
                    && sig.line_count < self.min_lines
                {
                    continue;
                }
                index.entry(def.body_hash).or_default().push((
                    f.file.clone(),
                    def.name.clone(),
                    def.loc.line,
                ));
            }
        }
        index
            .into_iter()
            .filter(|(_, locs)| locs.len() >= self.threshold)
            // Filter clusters confined to declared sibling crates (layers.toml `sibling_of`).
            // Replaces the previously hardcoded vendor-split allowlist.
            .filter(|(_, locs)| !is_tolerated_duplication(locs, &ctx.layers))
            .map(|(_, locs)| {
                let names: Vec<_> = locs.iter().map(|(_, n, _)| n.as_str()).collect();
                Finding {
                    rule_id: self.id().to_string(),
                    rule_name: "Duplicate Function Body".into(),
                    severity: self.severity(),
                    file: locs[0].0.clone(),
                    line: locs[0].2,
                    column: 0,
                    message: format!(
                        "Functions {:?} have identical bodies — extract a shared helper",
                        names
                    ),
                    suggestion: Some("Extract to a shared module".into()),
                    context: locs[1..]
                        .iter()
                        .map(|(p, n, l)| format!("{}:{} ({})", p.display(), l, n))
                        .collect::<Vec<_>>()
                        .join(", "),
                    confidence: Some(FindingConfidence::High),
                    evidence: None,
                    diagnostic_id: None,
                    alternatives: vec![],
                    rationale: None,
                }
            })
            .collect()
    }
}

/// Returns true when every crate in the duplicate set belongs to a declared
/// sibling cluster (per `layers.toml` `sibling_of`). The previous hardcoded
/// list (`vox-plugin-mens-candle-cuda` / `…-metal`, `vox-speech` / `vox-plugin-
/// oratio`, …) now lives in the manifest, so renames and new vendor splits
/// only require a layers.toml edit.
fn is_tolerated_duplication(
    locs: &[(std::path::PathBuf, String, usize)],
    layers: &crate::layers_manifest::LayersManifest,
) -> bool {
    fn crate_of(p: &std::path::Path) -> Option<String> {
        let mut comps = p.components().peekable();
        while let Some(c) = comps.next() {
            if c.as_os_str() == "crates" {
                return comps
                    .next()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned());
            }
        }
        None
    }

    let crates: std::collections::HashSet<String> =
        locs.iter().filter_map(|(p, _, _)| crate_of(p)).collect();

    // Single crate → not a "cross-crate" duplication concern this rule cares about;
    // a within-crate duplicate is still a real finding and should not be tolerated.
    if crates.len() < 2 {
        return false;
    }
    layers.all_in_one_sibling_cluster(&crates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::*;
    use crate::layers_manifest::LayersManifest;
    use crate::rules::WorkspaceContext;
    use std::path::PathBuf;
    use vox_code_audit::rules::Language;

    fn empty_ctx() -> WorkspaceContext {
        WorkspaceContext {
            workspace_version: "0.6.0".into(), // drift-allow(version-string): test fixture
            workspace_root: PathBuf::from("."),
            layers: LayersManifest::default(),
        }
    }

    fn make_fn(crate_relative_path: &str, name: &str, hash: u64) -> ExtractedFeatures {
        let mut f = ExtractedFeatures::new(PathBuf::from(crate_relative_path), Language::Rust);
        f.crate_name = crate::extractor::crate_name_from_path(&f.file);
        f.fn_definitions.push(FnDef {
            name: name.into(),
            body_hash: hash,
            sig_hash: hash,
            loc: Loc::default(),
        });
        f.body_signatures.push(BodySignature {
            hash,
            line_count: 10,
            parent_fn: Some(name.into()),
            loc: Loc::default(),
        });
        f
    }

    #[test]
    fn finds_duplicate_fn_bodies_across_unrelated_crates() {
        let files = vec![
            make_fn("crates/alpha/src/lib.rs", "shared", 42),
            make_fn("crates/beta/src/lib.rs", "shared", 42),
        ];
        let rule = BodyHashRule::default();
        let findings = rule.sweep(&files, &empty_ctx());
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn within_single_crate_is_still_flagged() {
        // Two copies inside one crate is the original code-smell — should flag
        // even when no sibling clusters are declared.
        let files = vec![
            make_fn("crates/alpha/src/a.rs", "shared", 42),
            make_fn("crates/alpha/src/b.rs", "shared", 42),
        ];
        let rule = BodyHashRule::default();
        assert_eq!(rule.sweep(&files, &empty_ctx()).len(), 1);
    }

    #[test]
    fn declared_siblings_are_tolerated() {
        let layers = LayersManifest::load_from_file(std::path::Path::new("/tmp/none.toml"));
        // Manually compose a manifest by parsing inline TOML to keep the test hermetic.
        let layers = {
            let _ = layers;
            // Reach into the parser through the public load_from_file path used by
            // tests of LayersManifest — that test confirms parse() behavior.
            // Here we just build via parse to keep this test focused on body_hash.
            crate::layers_manifest::LayersManifest::default()
        };
        let _ = layers;

        // Use a manifest that declares cuda ↔ metal as siblings.
        let toml = r#"
[crates.vox-plugin-mens-candle-cuda]
sibling_of = ["vox-plugin-mens-candle-metal"]

[crates.vox-plugin-mens-candle-metal]
"#;
        // Round-trip through the same loader path the engine uses.
        let dir = std::env::temp_dir().join("vox-drift-test-layers");
        let arch_dir = dir.join("docs/src/architecture");
        std::fs::create_dir_all(&arch_dir).unwrap();
        std::fs::write(arch_dir.join("layers.toml"), toml).unwrap();
        let layers = LayersManifest::load(&dir);
        std::fs::remove_dir_all(&dir).ok();

        let ctx = WorkspaceContext {
            workspace_version: "0.6.0".into(), // drift-allow(version-string): test fixture
            workspace_root: PathBuf::from("."),
            layers,
        };

        let files = vec![
            make_fn("crates/vox-plugin-mens-candle-cuda/src/x.rs", "shared", 42),
            make_fn("crates/vox-plugin-mens-candle-metal/src/x.rs", "shared", 42),
        ];
        let rule = BodyHashRule::default();
        assert!(rule.sweep(&files, &ctx).is_empty());
    }
}
