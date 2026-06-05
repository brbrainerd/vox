//! Canonical `vox audit <thing>` JSON report shape.
//!
//! Mirrors the schema declared in
//! [`contracts/ci/vox-audit-contract.v1.yaml`](../../../../contracts/ci/vox-audit-contract.v1.yaml)
//! (`report_schema` block). Council-ratified 2026-05-15 (D21/D22 in
//! `docs/src/architecture/v1-llm-target-implementation-plan-2026.md` §8.1).
//!
//! Forward-compat: unknown fields are tolerated on parse via `#[serde(default)]`
//! where applicable; new fields are added without breaking the schema version.

use serde::{Deserialize, Serialize};

/// Output format for the `--json` / `--markdown` / `--html` flag set.
///
/// Stable: `Json` is the default; `Markdown` is the human-readable form;
/// `Html` is reserved for the dashboard surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum ReportFormat {
    #[default]
    Json,
    Markdown,
    Html,
}

impl ReportFormat {
    /// Parse a format string from a `--format` flag.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "json" => Ok(ReportFormat::Json),
            "markdown" | "md" => Ok(ReportFormat::Markdown),
            "html" => Ok(ReportFormat::Html),
            other => Err(format!(
                "unknown report format `{other}` (expected json | markdown | html)"
            )),
        }
    }
}


/// Canonical audit-subcommand exit code (mirrors contract §exit_codes).
///
/// Production code returns `i32` from `main`; this enum is the typed bridge.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitCode {
    /// Measurement complete; bar met (if threshold set) or no threshold set.
    Ok = 0,
    /// Measurement complete; bar not met.
    BarMissed = 1,
    /// Infrastructure error — corpus missing, panel unreachable, etc.
    /// Does NOT block CI per contract; runner returns 2 and logs telemetry.
    InfrastructureError = 2,
    /// Invalid arguments / malformed fixtures / schema violation.
    InvalidInput = 3,
}

impl ExitCode {
    pub fn as_i32(self) -> i32 {
        self as i32
    }
}

/// One panel-member descriptor for the `llm_panel` array.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelMember {
    pub id: String,
    pub version: String,
}

/// Reproducibility-knobs block (`temperature`, `seed`, `attempts_per_fixture`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reproducibility {
    pub temperature: f64,
    pub seed: u64,
    pub attempts_per_fixture: u32,
}

impl Reproducibility {
    /// Canonical defaults for v1.0 measurement runs (ratified by D6/§1.3 P2.8).
    pub fn canonical() -> Self {
        Self {
            temperature: 0.0,
            seed: 42,
            attempts_per_fixture: 5,
        }
    }
}

/// Per-LLM aggregated result block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerLlmResult {
    pub id: String,
    pub pass_rate: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub median_cost_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unreachable_count: Option<u32>,
}

/// Overall results block.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Results {
    /// Aggregate pass rate across the entire run.
    pub overall_pass_rate: f64,
    /// Median of per-LLM pass rates (used by CR-L1/L2/L4 scoring rule).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub median_pass_rate: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub per_llm: Vec<PerLlmResult>,
}

/// Threshold block — present when a CR-L bar applies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Threshold {
    pub target: f64,
    pub met: bool,
}

/// Delta-vs-baseline block — populated when `--baseline=<path>` is supplied.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaVsBaseline {
    pub baseline_hash: String,
    pub absolute: f64,
    pub relative_pct: f64,
}

/// The canonical audit report.
///
/// Every `vox audit <thing>` subcommand returns exactly this shape; CI consumes
/// `contracts/reports/<thing>/<date>.json` files matching this schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditReport {
    /// The audit subcommand name (e.g. `humaneval-vox`, `retirement`).
    pub thing: String,
    pub schema_version: u32,
    pub measured_at: String,
    pub corpus_hash: String,
    pub corpus_size: u32,
    pub llm_panel: Vec<PanelMember>,
    pub reproducibility: Reproducibility,
    pub results: Results,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold: Option<Threshold>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta_vs_baseline: Option<DeltaVsBaseline>,
    /// `true` when the report is emitted on exit-1/2/3 (partial / infra-error /
    /// invalid-input). Consumers treat partial reports as advisory.
    #[serde(default, skip_serializing_if = "is_false")]
    pub incomplete: bool,
    /// Optional human-readable note (used by infra-error reports to explain
    /// why measurement could not run).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

fn is_false(value: &bool) -> bool {
    !value
}

impl AuditReport {
    /// Construct a complete, bar-met report.
    pub fn complete(
        thing: impl Into<String>,
        corpus_hash: impl Into<String>,
        corpus_size: u32,
        results: Results,
    ) -> Self {
        Self {
            thing: thing.into(),
            schema_version: 1,
            measured_at: now_rfc3339(),
            corpus_hash: corpus_hash.into(),
            corpus_size,
            llm_panel: Vec::new(),
            reproducibility: Reproducibility::canonical(),
            results,
            threshold: None,
            delta_vs_baseline: None,
            incomplete: false,
            note: None,
        }
    }

    /// Construct an infrastructure-error report (exit code 2).
    ///
    /// Used by stub subcommands while their corpora are not yet authored
    /// (per contract: exit 2 logs telemetry, does not block CI).
    pub fn infra_error(thing: impl Into<String>, note: impl Into<String>) -> Self {
        Self {
            thing: thing.into(),
            schema_version: 1,
            measured_at: now_rfc3339(),
            corpus_hash: empty_blake3_hash(),
            corpus_size: 0,
            llm_panel: Vec::new(),
            reproducibility: Reproducibility::canonical(),
            results: Results::default(),
            threshold: None,
            delta_vs_baseline: None,
            incomplete: true,
            note: Some(note.into()),
        }
    }

    /// Render the report in the requested format.
    pub fn render(&self, fmt: ReportFormat) -> Result<String, serde_json::Error> {
        match fmt {
            ReportFormat::Json => serde_json::to_string_pretty(self),
            ReportFormat::Markdown => Ok(self.render_markdown()),
            ReportFormat::Html => Ok(self.render_html()),
        }
    }

    fn render_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("# vox audit {} report\n\n", self.thing));
        out.push_str(&format!("- measured_at: `{}`\n", self.measured_at));
        out.push_str(&format!("- corpus_hash: `{}`\n", self.corpus_hash));
        out.push_str(&format!("- corpus_size: {}\n", self.corpus_size));
        if self.incomplete {
            out.push_str("- incomplete: true\n");
            if let Some(note) = &self.note {
                out.push_str(&format!("- note: {note}\n"));
            }
        } else {
            out.push_str(&format!(
                "- overall_pass_rate: {:.4}\n",
                self.results.overall_pass_rate
            ));
            if let Some(t) = &self.threshold {
                out.push_str(&format!(
                    "- threshold target={:.4} met={}\n",
                    t.target, t.met
                ));
            }
        }
        out
    }

    fn render_html(&self) -> String {
        format!(
            "<section class=\"vox-audit-report\" data-thing=\"{thing}\">\n  \
             <pre>{json}</pre>\n\
             </section>",
            thing = self.thing,
            json = serde_json::to_string_pretty(self).unwrap_or_else(|_| String::new()),
        )
    }

    /// Compute the canonical report path under `contracts/reports/<thing>/<YYYY-MM-DD>.json`.
    pub fn canonical_report_path(&self) -> std::path::PathBuf {
        std::path::PathBuf::from("contracts")
            .join("reports")
            .join(&self.thing)
            .join(format!("{}.json", today_yyyymmdd()))
    }

    /// Atomically write the JSON report to the supplied path.
    pub fn write_json_atomic(&self, path: &std::path::Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("json.tmp");
        let json = serde_json::to_string_pretty(self)
            .map_err(|err| std::io::Error::other(err.to_string()))?;
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn today_yyyymmdd() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

/// Sentinel "empty corpus" hash used by infra-error reports.
fn empty_blake3_hash() -> String {
    // `blake3::hash(b"")` is deterministic across runs.
    format!("blake3:{}", blake3::hash(b"").to_hex())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_format_parse_roundtrips() {
        assert_eq!(ReportFormat::parse("json").unwrap(), ReportFormat::Json);
        assert_eq!(ReportFormat::parse("md").unwrap(), ReportFormat::Markdown);
        assert_eq!(
            ReportFormat::parse("markdown").unwrap(),
            ReportFormat::Markdown
        );
        assert_eq!(ReportFormat::parse("html").unwrap(), ReportFormat::Html);
        assert!(ReportFormat::parse("xml").is_err());
    }

    #[test]
    fn infra_error_report_round_trips_through_json() {
        let report = AuditReport::infra_error("humaneval-vox", "corpus is stub");
        let json = serde_json::to_string(&report).expect("serializes");
        let back: AuditReport = serde_json::from_str(&json).expect("parses");
        assert_eq!(back.thing, "humaneval-vox");
        assert!(back.incomplete);
        assert_eq!(back.note.as_deref(), Some("corpus is stub"));
        assert_eq!(back.corpus_size, 0);
    }

    #[test]
    fn complete_report_omits_incomplete_field_in_json() {
        let report = AuditReport::complete(
            "retirement",
            "blake3:abc",
            16,
            Results {
                overall_pass_rate: 1.0,
                median_pass_rate: None,
                per_llm: Vec::new(),
            },
        );
        let json = serde_json::to_string(&report).expect("serializes");
        assert!(
            !json.contains("\"incomplete\""),
            "complete report should not serialize the `incomplete` field (default false)"
        );
    }

    #[test]
    fn canonical_path_format() {
        let report = AuditReport::infra_error("plan-fidelity", "stub");
        let path = report.canonical_report_path();
        assert!(path.starts_with("contracts/reports/plan-fidelity"));
        assert!(
            path.to_string_lossy().ends_with(".json"),
            "path should end with .json"
        );
    }

    #[test]
    fn atomic_write_creates_directory_and_file() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("subdir").join("report.json");
        let report = AuditReport::infra_error("test", "stub");
        report.write_json_atomic(&path).expect("write");
        let raw = std::fs::read_to_string(&path).expect("read");
        let back: AuditReport = serde_json::from_str(&raw).expect("parse");
        assert_eq!(back.thing, "test");
        assert!(back.incomplete);
    }

    #[test]
    fn render_markdown_includes_thing_name() {
        let report = AuditReport::infra_error("retirement", "stub");
        let md = report.render(ReportFormat::Markdown).expect("renders");
        assert!(md.contains("# vox audit retirement report"));
    }

    #[test]
    fn empty_blake3_sentinel_is_stable() {
        // Tests that the empty-corpus marker doesn't drift across releases.
        let h = empty_blake3_hash();
        assert!(h.starts_with("blake3:"));
        // BLAKE3 of empty input is deterministic; the prefix is stable.
        assert_eq!(h, format!("blake3:{}", blake3::hash(b"").to_hex()));
    }
}
