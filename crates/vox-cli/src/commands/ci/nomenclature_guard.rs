//! `vox ci nomenclature-guard` — T189-T196, T198-T200
//!
//! Enforces the canonical English-first naming policy for workspace crates.
//! Fails CI when:
//! - A new `crates/vox-<latin>` directory appears that is not in the allowlist.
//! - A crate directory uses a pure Latin root that should have been canonical English.
//!
//! ## Latin Structural Denylist
//! These roots are recognized Latin words that should NOT be top-level crate names.
//! The canonical English equivalent must be used instead (or the crate must appear in the allowlist).
//!
//! ## Allowlist
//! Historical crates already established in the workspace are explicitly permitted.
//! New additions must go through the policy proposal template.
//!
//! See: `docs/src/architecture/english_core_migration_ledger.md`

use anyhow::{Result, anyhow};
use serde::Serialize;
use std::path::Path;

/// Latin root words that are structurally denied as primary crate directory names.
/// Each entry is `(latin_root, canonical_english_replacement)`.
const LATIN_STRUCTURAL_DENYLIST: &[(&str, &str)] = &[
    ("dei", "orchestrator"),
    ("ars", "skills"),
    ("fabrica", "forge"),
    ("codex", "database"),
    ("clavis", "secrets"),
    ("oratio", "speech"),
    ("populi", "ml"),
    ("ludus", "gamification"),
    ("schola", "tutorial"),
    ("mens", "ml"), // mens overlaps with ml/populi domain
];

/// These historical crates ARE allowed despite using Latin names (grandfathered).
/// Do not add new entries here without a policy proposal.
const HISTORICAL_ALLOWLIST: &[&str] = &[
    "vox-dei",              // grandfathered; migration complete 2026-05-08
    "vox-openclaw-runtime", // grandfathered — ARS runtime extracted from vox-skills::ars_shim; canonical replacement for retired vox-ars
    "vox-secrets", // canonical secret manager — name IS its Latin identity (policy exception)
    "vox-orchestrator", // canonical English — permitted
    "vox-skills",  // canonical English — permitted
    "vox-gamify",  // grandfathered — being migrated to vox-gamification
    "vox-oratio",  // grandfathered — being migrated to vox-speech
    "vox-populi",  // grandfathered — being migrated to vox-ml
    "vox-schola",  // grandfathered — being migrated to vox-tutorial
    "vox-codex-api", // grandfathered — database abstraction layer
    "vox-ml-cli",  // grandfathered — ML subsystem
    "vox-mens-eval", // grandfathered — Mn-T11 eval harness crate; pending english-core naming convergence
    "vox-dei-shim", // grandfathered — A-12 wedge crate extracted from vox-orchestrator; retains `dei` prefix during migration phase
    "vox-populi-types", // grandfathered — shared types crate for the vox-populi ecosystem; naming cohesion during migration to vox-ml
];

#[derive(Serialize)]
pub struct NomenclatureViolation {
    pub crate_dir: String,
    pub latin_root: String,
    pub canonical_english: String,
    pub message: String,
}

/// Run the nomenclature guard: scan `crates/*` for Latin structural violations.
pub(crate) fn run(repo_root: &Path, json: bool) -> Result<()> {
    let crates_dir = repo_root.join("crates");
    if !crates_dir.is_dir() {
        return Err(anyhow!(
            "missing crates/ directory at {}",
            repo_root.display()
        ));
    }

    let mut violations: Vec<NomenclatureViolation> = Vec::new();

    let entries = std::fs::read_dir(&crates_dir).map_err(|e| anyhow!("read_dir crates/: {e}"))?;

    for entry in entries {
        let entry = entry.map_err(|e| anyhow!("read crates/ entry: {e}"))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let dir_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        // Only check `vox-*` prefixed crate directories
        let Some(crate_suffix) = dir_name.strip_prefix("vox-") else {
            continue;
        };

        // Skip if in the historical allowlist
        if HISTORICAL_ALLOWLIST.contains(&dir_name.as_str()) {
            continue;
        }

        // Check if the suffix (or its first segment for compound names) matches a Latin denylist entry
        let first_segment = crate_suffix.split('-').next().unwrap_or(crate_suffix);
        for (latin_root, canonical_english) in LATIN_STRUCTURAL_DENYLIST {
            if first_segment == *latin_root {
                violations.push(NomenclatureViolation {
                    crate_dir: dir_name.clone(),
                    latin_root: latin_root.to_string(),
                    canonical_english: canonical_english.to_string(),
                    message: format!(
                        "T189: crate `{dir_name}` uses Latin root `{latin_root}` as structural identifier; \
                         use canonical English `vox-{canonical_english}` instead (or add to historical allowlist with policy justification)"
                    ),
                });
                break;
            }
        }
    }

    if json {
        let out = serde_json::to_string_pretty(&violations)
            .map_err(|e| anyhow!("serialize violations: {e}"))?;
        println!("{out}");
        if !violations.is_empty() {
            return Err(anyhow!(
                "nomenclature-guard: {} violation(s) found",
                violations.len()
            ));
        }
        return Ok(());
    }

    if violations.is_empty() {
        println!("nomenclature-guard OK (no Latin structural crate violations)");
        return Ok(());
    }

    for v in &violations {
        eprintln!("error: {}", v.message);
    }
    Err(anyhow!(
        "nomenclature-guard: {} violation(s) — see errors above. To add a grandfathered exception, add to HISTORICAL_ALLOWLIST in crates/vox-cli/src/commands/ci/nomenclature_guard.rs with policy justification.",
        violations.len()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_latin_roots_in_denylist() {
        let roots: Vec<&str> = LATIN_STRUCTURAL_DENYLIST.iter().map(|(l, _)| *l).collect();
        assert!(roots.contains(&"dei"), "dei must be denied");
        assert!(roots.contains(&"ars"), "ars must be denied");
        assert!(roots.contains(&"fabrica"), "fabrica must be denied");
    }

    #[test]
    fn historical_allowlist_contains_grandfathered_crates() {
        assert!(HISTORICAL_ALLOWLIST.contains(&"vox-dei"));
        assert!(HISTORICAL_ALLOWLIST.contains(&"vox-openclaw-runtime"));
        assert!(HISTORICAL_ALLOWLIST.contains(&"vox-secrets"));
    }

    #[test]
    fn nomenclature_guard_passes_on_real_repo() {
        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("repo root");
        // Should pass because vox-dei and vox-openclaw-runtime are in the historical allowlist
        run(repo_root, false).expect("nomenclature guard must pass on current repo");
    }
}
