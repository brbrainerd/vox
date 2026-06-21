//! B8.1 — Pre-flight readiness check from the data-sufficiency spike report.
//!
//! Reads `mens/data-sufficiency-spike-b2_5.json` (or an override path) and
//! determines which spokes are ready to train and which are blocked.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tracing::warn;

/// Readiness data for a single training spoke, as recorded in the B2.5
/// data-sufficiency spike report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpokeReadiness {
    /// Estimated training row count.
    pub rows: usize,
    /// Diversity score (0.0–1.0, higher is better).
    pub diversity: f64,
    /// Either `"PROCEED"` or `"BLOCKED"` (from `decision` field in JSON).
    pub status: String,
}

/// Top-level structure of `mens/data-sufficiency-spike-b2_5.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSufficiencySpike {
    /// Per-spoke readiness keyed by spoke name.
    pub spokes: HashMap<String, SpokeReadiness>,
    /// Whether cloud spend has been authorised by a human (gate flag).
    pub cloud_spend_authorized: bool,
}

/// The result of running `pre_flight_check`.
#[derive(Debug, Clone)]
pub struct PreFlightResult {
    pub ready: Vec<String>,
    pub blocked: Vec<String>,
    pub can_spend: bool,
}

// ---------------------------------------------------------------------------
// Public helpers
// ---------------------------------------------------------------------------

/// Load a `DataSufficiencySpike` from the JSON file at `path`.
///
/// The JSON shape mirrors `mens/data-sufficiency-spike-b2_5.json`.  Each spoke
/// entry exposes `rows`, `diversity`, and `decision`; we map `decision` →
/// `status` here so callers work with normalised `"PROCEED"` / `"BLOCKED"`.
pub fn load_spike_report(path: &Path) -> Result<DataSufficiencySpike> {
    let content = vox_bounded_fs::read_utf8_path_capped(path)
        .with_context(|| format!("reading spike report at {}", path.display()))?;

    // The on-disk JSON uses `decision` (values: "proceed" / "blocked") rather
    // than a pre-normalised `status` field.  We deserialise into an intermediate
    // representation and normalise.
    #[derive(Deserialize)]
    struct RawSpoke {
        rows: usize,
        diversity: f64,
        decision: String,
    }

    #[derive(Deserialize)]
    struct RawSpike {
        spokes: HashMap<String, RawSpoke>,
        cloud_spend_authorized: bool,
    }

    let raw: RawSpike = serde_json::from_str(&content)
        .with_context(|| format!("parsing spike report at {}", path.display()))?;

    let spokes = raw
        .spokes
        .into_iter()
        .map(|(name, s)| {
            let status = if s.decision.eq_ignore_ascii_case("proceed") {
                "PROCEED".to_string()
            } else {
                "BLOCKED".to_string()
            };
            (
                name,
                SpokeReadiness {
                    rows: s.rows,
                    diversity: s.diversity,
                    status,
                },
            )
        })
        .collect();

    Ok(DataSufficiencySpike {
        spokes,
        cloud_spend_authorized: raw.cloud_spend_authorized,
    })
}

/// Return the names of spokes whose `status` is `"PROCEED"`.
pub fn ready_spokes(spike: &DataSufficiencySpike) -> Vec<String> {
    let mut names: Vec<String> = spike
        .spokes
        .iter()
        .filter(|(_, s)| s.status == "PROCEED")
        .map(|(name, _)| name.clone())
        .collect();
    names.sort(); // deterministic order
    names
}

/// Run the complete pre-flight check.
///
/// * Reads the spike report from `spike_path`.
/// * Emits a `warn!` for each blocked spoke.
/// * Returns `Err` if there are no ready spokes (nothing to train).
pub fn pre_flight_check(spike_path: &Path) -> Result<PreFlightResult> {
    let spike = load_spike_report(spike_path)?;

    let ready = ready_spokes(&spike);
    let mut blocked: Vec<String> = spike
        .spokes
        .keys()
        .filter(|k| !ready.contains(*k))
        .cloned()
        .collect();
    blocked.sort();

    for name in &blocked {
        warn!(spoke = %name, "spoke is BLOCKED — skipping in pre-flight");
    }

    if ready.is_empty() {
        anyhow::bail!("No spokes are ready to train — all spokes are BLOCKED");
    }

    Ok(PreFlightResult {
        ready,
        blocked,
        can_spend: spike.cloud_spend_authorized,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// Path to the real spike report that ships in the repo.
    fn spike_path() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            // crates/vox-ml-cli  →  workspace root
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("mens/data-sufficiency-spike-b2_5.json")
    }

    #[test]
    fn real_spike_vox_lang_and_rust_are_ready() {
        let p = spike_path();
        let spike = load_spike_report(&p).expect("should load real spike");
        let ready = ready_spokes(&spike);
        assert!(
            ready.contains(&"vox-lang".to_string()),
            "vox-lang should be PROCEED; got ready={ready:?}"
        );
        assert!(
            ready.contains(&"rust".to_string()),
            "rust should be PROCEED; got ready={ready:?}"
        );
    }

    #[test]
    fn real_spike_tool_selection_and_arg_gen_blocked() {
        let p = spike_path();
        let spike = load_spike_report(&p).expect("should load real spike");
        let ready = ready_spokes(&spike);
        assert!(
            !ready.contains(&"tool-selection".to_string()),
            "tool-selection should be BLOCKED"
        );
        assert!(
            !ready.contains(&"argument-generation".to_string()),
            "argument-generation should be BLOCKED"
        );
    }

    #[test]
    fn pre_flight_check_returns_correct_sets() {
        let p = spike_path();
        let result = pre_flight_check(&p).expect("pre_flight_check should succeed");
        assert!(result.ready.contains(&"vox-lang".to_string()));
        assert!(result.ready.contains(&"rust".to_string()));
        assert!(result.blocked.contains(&"tool-selection".to_string()));
        assert!(result.blocked.contains(&"argument-generation".to_string()));
        // The on-disk spike has cloud_spend_authorized: false
        assert!(!result.can_spend);
    }

    #[test]
    fn pre_flight_check_err_when_all_blocked() {
        // Build an in-memory spike where every spoke is blocked, write to a
        // temp file, and confirm pre_flight_check returns Err.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("spike.json");
        let json = serde_json::json!({
            "spokes": {
                "a": { "rows": 100, "diversity": 0.3, "decision": "blocked" },
                "b": { "rows": 200, "diversity": 0.2, "decision": "blocked" }
            },
            "cloud_spend_authorized": false
        });
        std::fs::write(&path, serde_json::to_string(&json).unwrap()).unwrap();
        let err = pre_flight_check(&path).unwrap_err();
        assert!(
            err.to_string().contains("BLOCKED"),
            "error should mention BLOCKED; got: {err}"
        );
    }

    #[test]
    fn ready_spokes_empty_returns_empty_vec() {
        let spike = DataSufficiencySpike {
            spokes: HashMap::new(),
            cloud_spend_authorized: false,
        };
        assert!(ready_spokes(&spike).is_empty());
    }

    #[test]
    fn load_spike_report_rejects_invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.json");
        std::fs::write(&path, "not json at all").unwrap();
        assert!(load_spike_report(&path).is_err());
    }
}
