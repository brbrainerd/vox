//! GA roll-up — `vox audit --gate all --strict-block-ga`.
//!
//! Runs every registered gate (foundation first per CR-F0), folds in the
//! standalone product binaries ([`product_binary_gates`]), and writes
//! `contracts/reports/_snapshot/<UTC>.json`. If any foundation gate is red,
//! every downstream gate is forced `blocked_by_foundation` and GA fails.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct GateRow {
    pub thing: String,
    pub tier: String,
    pub met: bool,
    pub blocked_by_foundation: bool,
    pub exit_code: i32,
    #[serde(default)]
    pub external_infra: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct GaSnapshot {
    pub schema_version: u32,
    pub measured_at: String,
    pub strict_block_ga: bool,
    pub foundation_red: bool,
    pub ga_met: bool,
    pub exit_code: i32,
    pub gates: Vec<GateRow>,
}

impl GaSnapshot {
    /// Build the snapshot from already-evaluated rows, applying CR-F0
    /// foundation-blocking and computing the GA verdict + exit code.
    pub fn from_rows(mut gates: Vec<GateRow>, strict_block_ga: bool) -> Self {
        let foundation_red = gates.iter().any(|g| g.tier == "foundation" && !g.met);
        if foundation_red {
            for g in gates.iter_mut() {
                if g.tier != "foundation" {
                    g.blocked_by_foundation = true;
                }
            }
        }
        // GA is met when every non-tooling gate is met (external_infra gates
        // included — their honest-red state must be cleared for GA) and no
        // foundation gate is red.
        let ga_met = !foundation_red && gates.iter().filter(|g| g.tier != "tooling").all(|g| g.met);
        let exit_code = if strict_block_ga && !ga_met { 1 } else { 0 };
        Self {
            schema_version: 1,
            measured_at: now_rfc3339(),
            strict_block_ga,
            foundation_red,
            ga_met,
            exit_code,
            gates,
        }
    }

    /// Write to `contracts/reports/_snapshot/<YYYY-MM-DD>.json` under the root.
    pub fn write_canonical(&self, root: &std::path::Path) -> std::io::Result<()> {
        let dir = root.join("contracts").join("reports").join("_snapshot");
        std::fs::create_dir_all(&dir)?;
        let date = today_yyyymmdd();
        let body =
            serde_json::to_string_pretty(self).map_err(|e| std::io::Error::other(e.to_string()))?;
        std::fs::write(dir.join(format!("{date}.json")), body)
    }
}

/// Descriptor for a standalone `bin/cr-*.rs` product gate.
pub struct ProductBin {
    pub bin: &'static str,
    pub thing: &'static str,
    pub external_infra: bool,
}

/// Descriptors for the standalone `bin/cr-*.rs` product gates. CR-P* are
/// external_infra (live deploy / soak). cr-p3/cr-e3 are not yet bins — they
/// land in Phase 6.
pub fn product_binary_descriptors() -> Vec<ProductBin> {
    vec![
        ProductBin { bin: "cr-a1", thing: "cr-a1", external_infra: false },
        ProductBin { bin: "cr-a2", thing: "cr-a2", external_infra: false },
        ProductBin { bin: "cr-a4", thing: "cr-a4", external_infra: false },
        ProductBin { bin: "cr-d3", thing: "cr-d3", external_infra: false },
        ProductBin { bin: "cr-e1", thing: "cr-e1", external_infra: false },
        ProductBin { bin: "cr-e2", thing: "cr-e2", external_infra: false },
        ProductBin { bin: "cr-p1", thing: "cr-p1", external_infra: true },
        ProductBin { bin: "cr-p2", thing: "cr-p2", external_infra: true },
    ]
}

/// Run each product binary by resolving its sibling path next to the current
/// executable; record a [`GateRow`]. A missing binary is recorded as a
/// non-met row with `exit_code -1` ("not measured") rather than a panic.
pub fn product_binary_gates(_args: &crate::CommonArgs) -> Vec<GateRow> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));
    product_binary_descriptors()
        .into_iter()
        .map(|d| {
            let (met, code) = match &exe_dir {
                Some(dir) => {
                    let mut path = dir.join(d.bin);
                    if cfg!(windows) {
                        path.set_extension("exe");
                    }
                    if path.exists() {
                        match std::process::Command::new(&path).output() {
                            Ok(o) => (o.status.success(), o.status.code().unwrap_or(-1)),
                            Err(_) => (false, -1),
                        }
                    } else {
                        (false, -1)
                    }
                }
                None => (false, -1),
            };
            GateRow {
                thing: d.thing.to_string(),
                tier: "product".to_string(),
                met,
                blocked_by_foundation: false,
                exit_code: code,
                external_infra: d.external_infra,
            }
        })
        .collect()
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}
fn today_yyyymmdd() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(thing: &str, tier: crate::Tier, met: bool) -> GateRow {
        GateRow {
            thing: thing.into(),
            tier: tier.as_str().into(),
            met,
            blocked_by_foundation: false,
            exit_code: if met { 0 } else { 1 },
            external_infra: false,
        }
    }

    #[test]
    fn red_foundation_blocks_all_downstream() {
        let rows = vec![
            row("behavioral-goldens", crate::Tier::Foundation, false),
            row("retirement", crate::Tier::Product, true),
        ];
        let snap = GaSnapshot::from_rows(rows, /* strict */ true);
        assert!(snap.foundation_red);
        let downstream = snap.gates.iter().find(|g| g.thing == "retirement").unwrap();
        assert!(
            downstream.blocked_by_foundation,
            "product row must be blocked when foundation is red"
        );
        assert!(!snap.ga_met);
        assert_ne!(snap.exit_code, 0);
    }

    #[test]
    fn all_green_passes_ga() {
        let rows = vec![
            row("behavioral-goldens", crate::Tier::Foundation, true),
            row("retirement", crate::Tier::Product, true),
        ];
        let snap = GaSnapshot::from_rows(rows, true);
        assert!(!snap.foundation_red);
        assert!(snap.ga_met);
        assert_eq!(snap.exit_code, 0);
    }

    #[test]
    fn external_infra_red_does_not_block_when_not_strict() {
        // A built-but-unrun external_infra gate is honest-red; non-strict GA
        // reports it but exits 0.
        let mut r = row("cr-p2", crate::Tier::Product, false);
        r.external_infra = true;
        let snap = GaSnapshot::from_rows(vec![r], /* strict */ false);
        assert!(!snap.ga_met);
        assert_eq!(snap.exit_code, 0, "non-strict run never fails the build");
    }

    #[test]
    fn product_binary_descriptors_cover_existing_bins() {
        let names: Vec<&str> = product_binary_descriptors().iter().map(|d| d.bin).collect();
        for expected in ["cr-a1", "cr-a2", "cr-a4", "cr-d3", "cr-e1", "cr-e2", "cr-p1", "cr-p2"] {
            assert!(names.contains(&expected), "missing descriptor for {expected}");
        }
        let p1 = product_binary_descriptors().into_iter().find(|d| d.bin == "cr-p1").unwrap();
        assert!(p1.external_infra);
        let a1 = product_binary_descriptors().into_iter().find(|d| d.bin == "cr-a1").unwrap();
        assert!(!a1.external_infra);
    }
}
