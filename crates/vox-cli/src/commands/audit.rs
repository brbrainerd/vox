//! `vox audit` — umbrella command that iterates quality gates defined in
//! `contracts/ci/check-targets.v1.yaml`.
//!
//! # Usage
//!
//! ```text
//! vox audit                         # run all checks
//! vox audit --category lint         # run only lint checks
//! vox audit --list                  # print check IDs without running
//! vox audit --dry-run               # print commands without executing
//! ```
//!
//! The manifest at `contracts/ci/check-targets.v1.yaml` is the SSOT for every
//! quality gate.  See that file for per-check metadata: `blocking`, `runs_on`,
//! `rust_only`, and `command`.

use anyhow::{Context, Result, bail};
use clap::Args;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Command;

// ---------------------------------------------------------------------------
// Manifest types
// ---------------------------------------------------------------------------

/// Top-level structure of `contracts/ci/check-targets.v1.yaml`.
#[derive(Debug, Deserialize)]
pub struct CheckManifest {
    pub schema_version: u32,
    pub checks: Vec<CheckEntry>,
}

/// One quality-gate entry in the manifest.
#[derive(Debug, Deserialize, Clone)]
pub struct CheckEntry {
    pub id: String,
    pub description: String,
    pub category: String,
    pub blocking: bool,
    pub runs_on: Vec<String>,
    #[serde(default)]
    pub rust_only: bool,
    pub command: Vec<String>,
    /// When `true` this check is skipped by `--quick`.
    #[serde(default)]
    pub quick_skip: bool,
}

// ---------------------------------------------------------------------------
// CLI args
// ---------------------------------------------------------------------------

/// Run quality-gate checks defined in `contracts/ci/check-targets.v1.yaml`,
/// OR run a CR-L gate from the `vox-audit` umbrella registry.
///
/// Two dispatch modes, mutually exclusive:
///   • Default — iterates `check-targets.v1.yaml` and runs each entry.
///   • `--gate <NAME>` / `--list-gates` — dispatches to
///     `vox_audit::registry()` for the v1.0 LLM-target CR-L gates (CR-L0..L8).
#[derive(Args, Debug, Clone)]
pub struct AuditArgs {
    /// Filter to a specific category (lint, test, audit, doc, arch).
    /// Ignored when `--gate` is set.
    #[arg(long, value_name = "CATEGORY")]
    pub category: Option<String>,

    /// Print all matching check IDs and exit without running anything.
    /// Ignored when `--gate` is set.
    #[arg(long)]
    pub list: bool,

    /// Print each command without executing it.
    #[arg(long)]
    pub dry_run: bool,

    /// Skip checks marked `quick_skip: true` in the manifest.
    /// Ignored when `--gate` is set.
    #[arg(long)]
    pub quick: bool,

    /// CR-L gate to run from the `vox-audit` registry (e.g. `retirement`,
    /// `corpus-feedback`, `all`). When set, the check-targets dispatch path
    /// is skipped and we route through `vox_audit::registry()` instead. This
    /// aligns with `contracts/ci/vox-audit-contract.v1.yaml` §cli_surface
    /// ("`vox audit <thing>`" is the user-facing shape).
    #[arg(long, value_name = "GATE", conflicts_with_all = ["list", "category", "quick"])]
    pub gate: Option<String>,

    /// List every registered CR-L gate with its description, then exit.
    /// Mutually exclusive with the check-targets `--list`.
    #[arg(long, conflicts_with = "list")]
    pub list_gates: bool,

    /// Output format for CR-L gate reports (`json`/`markdown`/`html`).
    /// Only meaningful with `--gate`. Defaults to `json`.
    #[arg(long, value_name = "FORMAT", default_value = "json")]
    pub format: String,

    /// Override the corpus / contract path for the targeted gate.
    /// Only meaningful with `--gate`.
    #[arg(long, value_name = "PATH")]
    pub corpus: Option<PathBuf>,

    /// Override the bar from the corpus manifest (e.g. `--threshold 0.8`).
    /// Only meaningful with `--gate`.
    #[arg(long)]
    pub threshold: Option<f64>,

    /// Suppress writing the canonical `contracts/reports/<thing>/<date>.json`.
    /// Only meaningful with `--gate`.
    #[arg(long)]
    pub no_canonical_report: bool,
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Entry point called from the CLI dispatch.
pub fn run(args: &AuditArgs) -> Result<()> {
    // CR-L gate routing takes precedence over check-targets dispatch.
    if args.list_gates {
        // Honor --format so dashboards / tooling can consume structured JSON.
        // Default (no --format) defaults to "json" via clap, which gives
        // structured output for the most-common programmatic consumer; the
        // text-style listing remains available via `--format markdown` etc.
        let format = vox_audit::report::ReportFormat::parse(&args.format)
            .map_err(|msg| anyhow::anyhow!(msg))?;
        return run_list_cr_l_gates_with_format(&format);
    }
    if let Some(gate_name) = args.gate.as_deref() {
        return run_cr_l_gate(gate_name, args);
    }

    let root = vox_repository::resolve_repo_root_for_ci();
    let manifest = load_manifest(&root)?;
    let checks = filter_checks(&manifest.checks, args);

    if args.list {
        for check in &checks {
            println!("{}: {} [{}]", check.id, check.description, check.category);
        }
        return Ok(());
    }

    for check in &checks {
        println!("==> {}: {}", check.id, check.description);
        if args.dry_run {
            println!("    DRY-RUN: {}", check.command.join(" "));
            continue;
        }
        run_check(check, &root)?;
        println!("    OK");
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// CR-L gate dispatch (B1, ratified 2026-05-15).
// ---------------------------------------------------------------------------

/// Render every registered CR-L gate.
///
/// Honors `--format`:
/// - `json` (default for programmatic consumers like the dashboard): emits a
///   stable JSON array of `GateDescriptor` records;
/// - `markdown`/`md`: emits a Markdown bullet list;
/// - default text: the original human-readable two-line-per-gate format.
fn run_list_cr_l_gates_with_format(format: &vox_audit::report::ReportFormat) -> Result<()> {
    use vox_audit::report::ReportFormat;
    let registry = vox_audit::registry();
    let descriptors: Vec<GateDescriptor> = registry
        .iter()
        .map(|sub| {
            let gate = sub.gate();
            GateDescriptor {
                name: gate.thing_name().to_string(),
                gate: format!("{gate:?}"),
                block_ga: gate.block_ga(),
                cost_metered: gate.cost_metered(),
                description: sub.description().to_string(),
            }
        })
        .collect();

    match format {
        ReportFormat::Json => {
            // A12: wrap in {schema_version, gates: [...]} envelope. Matches
            // the contract-wide JSON envelope convention so dashboards can
            // branch on schema versions.
            let envelope = GateListingEnvelope {
                schema_version: 1,
                gates: &descriptors,
            };
            let json = serde_json::to_string_pretty(&envelope)
                .map_err(|err| anyhow::anyhow!("serialize gate list: {err}"))?;
            println!("{json}");
        }
        ReportFormat::Markdown => {
            println!("# Registered CR-L gates ({})\n", descriptors.len());
            for d in &descriptors {
                println!(
                    "- **{name}** — `block_ga={block}`, `cost_metered={cost}`\n  > {desc}",
                    name = d.name,
                    block = d.block_ga,
                    cost = d.cost_metered,
                    desc = d.description,
                );
            }
        }
        ReportFormat::Html => {
            println!(
                "<table class=\"vox-audit-gates\"><thead><tr>\
                 <th>name</th><th>block_ga</th><th>cost_metered</th><th>description</th>\
                 </tr></thead><tbody>"
            );
            for d in &descriptors {
                println!(
                    "<tr><td>{name}</td><td>{block}</td><td>{cost}</td><td>{desc}</td></tr>",
                    name = html_escape(&d.name),
                    block = d.block_ga,
                    cost = d.cost_metered,
                    desc = html_escape(&d.description),
                );
            }
            println!("</tbody></table>");
        }
    }
    Ok(())
}

/// Stable JSON shape for `--list-gates --format json` consumers (dashboards,
/// other tooling). Field names are intentionally `snake_case` to match the
/// repo-wide telemetry/contract conventions.
#[derive(Debug, serde::Serialize)]
struct GateDescriptor {
    name: String,
    gate: String,
    block_ga: bool,
    cost_metered: bool,
    description: String,
}

/// Envelope shape (A12, ratified 2026-05-15) that wraps the array of
/// [`GateDescriptor`]s. Matches the contract-wide `{schema_version, ...}`
/// convention used by `AuditReport` and other CR-L surfaces, so dashboard
/// consumers can branch on schema versions across future revisions.
#[derive(Debug, serde::Serialize)]
struct GateListingEnvelope<'a> {
    schema_version: u32,
    gates: &'a [GateDescriptor],
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Resolve `gate_name` to a `CrlGate` and dispatch via the umbrella registry.
/// `gate_name == "all"` runs every gate and aggregates the exit code per
/// `contracts/ci/vox-audit-contract.v1.yaml` §umbrella.
fn run_cr_l_gate(gate_name: &str, args: &AuditArgs) -> Result<()> {
    let common = build_common_args(args)?;

    if gate_name == "all" {
        let outcomes = vox_audit::run_all(&common);
        for outcome in &outcomes {
            render_outcome(&outcome.report, &common.format)?;
        }
        let exit_code = vox_audit::aggregate_exit_code(&outcomes);
        std::process::exit(exit_code.as_i32());
    }

    let Some(gate) = vox_audit::gate_from_name(gate_name) else {
        bail!(
            "unknown CR-L gate `{gate_name}`. Use `vox audit --list-gates` to see registered gates."
        );
    };
    let outcome = vox_audit::run_gate(gate, &common);
    render_outcome(&outcome.report, &common.format)?;
    std::process::exit(outcome.exit_code.as_i32());
}

fn render_outcome(
    report: &vox_audit::report::AuditReport,
    format: &vox_audit::report::ReportFormat,
) -> Result<()> {
    let text = report
        .render(*format)
        .map_err(|err| anyhow::anyhow!("render audit report: {err}"))?;
    println!("{text}");
    Ok(())
}

fn build_common_args(args: &AuditArgs) -> Result<vox_audit::CommonArgs> {
    let format = vox_audit::report::ReportFormat::parse(&args.format)
        .map_err(|msg| anyhow::anyhow!(msg))?;
    Ok(vox_audit::CommonArgs {
        format,
        baseline: None,
        threshold: args.threshold,
        corpus: args.corpus.clone(),
        llm_panel: None,
        dry_run: args.dry_run,
        write_canonical_report: !args.no_canonical_report,
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Load and parse the manifest from `<root>/contracts/ci/check-targets.v1.yaml`.
fn load_manifest(root: &Path) -> Result<CheckManifest> {
    let path = root
        .join("contracts")
        .join("ci")
        .join("check-targets.v1.yaml");
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("read manifest at {}", path.display()))?;
    let manifest: CheckManifest =
        serde_yaml::from_str(&content).with_context(|| "parse check-targets.v1.yaml")?;
    if manifest.schema_version != 1 {
        bail!(
            "unsupported manifest schema_version {}; expected 1",
            manifest.schema_version
        );
    }
    Ok(manifest)
}

/// Return the subset of checks that match the supplied CLI filters.
fn filter_checks<'a>(checks: &'a [CheckEntry], args: &AuditArgs) -> Vec<&'a CheckEntry> {
    checks
        .iter()
        .filter(|c| {
            if let Some(ref cat) = args.category {
                if c.category != *cat {
                    return false;
                }
            }
            if args.quick && c.quick_skip {
                return false;
            }
            true
        })
        .collect()
}

/// Execute a single check; bail on non-zero exit.
fn run_check(check: &CheckEntry, root: &Path) -> Result<()> {
    let (program, rest) = check
        .command
        .split_first()
        .context("check command is empty")?;

    let status = Command::new(program)
        .args(rest)
        .current_dir(root)
        .status()
        .with_context(|| format!("spawn command for check `{}`", check.id))?;

    if !status.success() {
        bail!(
            "check `{}` failed with exit code {:?}",
            check.id,
            status.code()
        );
    }
    Ok(())
}

/// Walk parent directories from `start` until a `Cargo.toml` is found.
/// Returns `None` if the filesystem root is reached without finding one.
///
/// This is a lightweight fallback for environments where
/// `vox_repository::resolve_repo_root_for_ci` is unavailable.
#[allow(dead_code)]
fn find_cargo_root(start: &Path) -> Option<PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        if dir.join("Cargo.toml").exists() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// The manifest file must be present and parse without error.
    #[test]
    fn manifest_parses_from_repo_root() {
        // Walk from the manifest file's known location relative to this source
        // file: crates/vox-cli/src/commands/audit.rs → repo root is 5 levels up.
        let src = std::path::Path::new(file!());
        // file!() gives a relative path from repo root; convert to absolute.
        let abs = std::env::current_dir()
            .expect("cwd")
            .join(src)
            .canonicalize()
            .unwrap_or_else(|_| std::path::PathBuf::from(file!()));

        // Walk upward to find Cargo.toml (workspace root).
        let mut root = abs.as_path();
        let manifest_path = loop {
            let candidate = root.join("contracts/ci/check-targets.v1.yaml");
            if candidate.exists() {
                break candidate;
            }
            match root.parent() {
                Some(p) => root = p,
                None => {
                    // If we can't find it (e.g. in a detached worktree without the
                    // contracts dir), skip the test rather than fail.
                    eprintln!("skip: contracts/ci/check-targets.v1.yaml not found from {abs:?}");
                    return;
                }
            }
        };

        let content = std::fs::read_to_string(&manifest_path).expect("read check-targets.v1.yaml");
        let manifest: CheckManifest =
            serde_yaml::from_str(&content).expect("parse check-targets.v1.yaml");
        assert_eq!(manifest.schema_version, 1, "schema_version must be 1");
        assert!(
            !manifest.checks.is_empty(),
            "manifest must list at least one check"
        );
        // All checks must have non-empty ids and commands.
        for check in &manifest.checks {
            assert!(!check.id.is_empty(), "check id must not be empty");
            assert!(
                !check.command.is_empty(),
                "check `{}` command must not be empty",
                check.id
            );
        }
    }

    /// `filter_checks` by category must return only matching entries.
    #[test]
    fn filter_by_category() {
        let checks = vec![
            CheckEntry {
                id: "fmt".into(),
                description: "fmt".into(),
                category: "lint".into(),
                blocking: true,
                runs_on: vec!["ci".into()],
                rust_only: true,
                command: vec!["cargo".into(), "fmt".into()],
                quick_skip: false,
            },
            CheckEntry {
                id: "arch-check".into(),
                description: "arch".into(),
                category: "arch".into(),
                blocking: true,
                runs_on: vec!["ci".into()],
                rust_only: true,
                command: vec!["cargo".into(), "run".into()],
                quick_skip: false,
            },
        ];

        let args = AuditArgs {
            category: Some("lint".into()),
            list: false,
            dry_run: false,
            quick: false,
            gate: None,
            list_gates: false,
            format: "json".into(),
            corpus: None,
            threshold: None,
            no_canonical_report: false,
        };
        let filtered = filter_checks(&checks, &args);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "fmt");
    }

    // -----------------------------------------------------------------------
    // CR-L gate bridge (B1) — verify routing into `vox_audit::registry()`.
    // -----------------------------------------------------------------------

    fn args_for_gate(gate: &str) -> AuditArgs {
        AuditArgs {
            category: None,
            list: false,
            dry_run: false,
            quick: false,
            gate: Some(gate.into()),
            list_gates: false,
            format: "json".into(),
            corpus: None,
            threshold: None,
            no_canonical_report: true,
        }
    }

    #[test]
    fn build_common_args_round_trips_format_and_corpus() {
        let args = AuditArgs {
            corpus: Some(std::path::PathBuf::from("/tmp/contracts")),
            threshold: Some(0.5),
            ..args_for_gate("retirement")
        };
        let common = build_common_args(&args).expect("build common args");
        assert_eq!(common.threshold, Some(0.5));
        assert_eq!(
            common.corpus.as_deref(),
            Some(std::path::Path::new("/tmp/contracts"))
        );
        assert!(
            !common.write_canonical_report,
            "no_canonical_report: true should disable writing"
        );
    }

    #[test]
    fn build_common_args_rejects_bad_format_string() {
        let args = AuditArgs {
            format: "xml".into(),
            ..args_for_gate("retirement")
        };
        assert!(build_common_args(&args).is_err());
    }

    #[test]
    fn gate_descriptor_serializes_to_stable_json_shape() {
        // The dashboard and other tooling consume this JSON; lock the
        // field names so a rename here is a contract change.
        let d = GateDescriptor {
            name: "retirement".into(),
            gate: "L6Retirement".into(),
            block_ga: true,
            cost_metered: false,
            description: "CR-L6 description".into(),
        };
        let json = serde_json::to_string(&d).expect("ser");
        assert!(json.contains("\"name\":\"retirement\""));
        assert!(json.contains("\"gate\":\"L6Retirement\""));
        assert!(json.contains("\"block_ga\":true"));
        assert!(json.contains("\"cost_metered\":false"));
        assert!(json.contains("\"description\":\"CR-L6 description\""));
    }

    #[test]
    fn gate_listing_envelope_serializes_with_schema_version() {
        // A12: dashboards consume the envelope, not a bare array. The
        // top-level shape must match `{schema_version: 1, gates: [...]}`.
        let descriptors = vec![GateDescriptor {
            name: "retirement".into(),
            gate: "L6Retirement".into(),
            block_ga: true,
            cost_metered: false,
            description: "x".into(),
        }];
        let envelope = GateListingEnvelope {
            schema_version: 1,
            gates: &descriptors,
        };
        let json = serde_json::to_string(&envelope).expect("ser");
        assert!(
            json.starts_with("{\"schema_version\":1"),
            "envelope must start with schema_version field; got {json}"
        );
        assert!(json.contains("\"gates\":["));
        assert!(json.contains("\"name\":\"retirement\""));
    }

    #[test]
    fn html_escape_handles_special_chars() {
        assert_eq!(
            html_escape("<a href=\"x\">&amp;</a>"),
            "&lt;a href=&quot;x&quot;&gt;&amp;amp;&lt;/a&gt;"
        );
    }

    #[test]
    fn gate_from_name_resolves_known_gates_via_vox_audit() {
        // Smoke-check that the bridge depends on the same registry as the
        // umbrella binary — every thing_name must round-trip.
        for gate in vox_audit::CrlGate::all() {
            let name = gate.thing_name();
            assert_eq!(vox_audit::gate_from_name(name), Some(gate));
        }
        assert_eq!(vox_audit::gate_from_name("does-not-exist"), None);
    }

    /// `filter_checks` with `--quick` skips `quick_skip: true` entries.
    #[test]
    fn filter_quick_skip() {
        let checks = vec![
            CheckEntry {
                id: "slow".into(),
                description: "slow check".into(),
                category: "test".into(),
                blocking: false,
                runs_on: vec!["ci".into()],
                rust_only: true,
                command: vec!["cargo".into(), "test".into()],
                quick_skip: true,
            },
            CheckEntry {
                id: "fast".into(),
                description: "fast check".into(),
                category: "lint".into(),
                blocking: true,
                runs_on: vec!["ci".into()],
                rust_only: false,
                command: vec!["cargo".into(), "fmt".into()],
                quick_skip: false,
            },
        ];

        let args = AuditArgs {
            category: None,
            list: false,
            dry_run: false,
            quick: true,
            gate: None,
            list_gates: false,
            format: "json".into(),
            corpus: None,
            threshold: None,
            no_canonical_report: false,
        };
        let filtered = filter_checks(&checks, &args);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "fast");
    }
}
