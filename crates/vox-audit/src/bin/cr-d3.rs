//! CR-D3 CLI subcommand help+example coverage sweep.
//!
//! Per `docs/superpowers/specs/2026-05-21-v1-honest-completion-plan.md` §5.9
//! and v1-release-criteria CR-D3: "100% of `vox-cli` subcommands must have
//! machine-readable help and associated `.vox` example scripts in the
//! training corpus."
//!
//! What this sweep does:
//!
//!   1. Run `vox commands --format json` (which exists today and exposes the
//!      clap-derived help tree) to enumerate every subcommand path.
//!   2. For each subcommand, search the training corpus (`examples/` and
//!      `scripts/`) for any `.vox` file whose first-50-line header mentions
//!      `vox <subcommand>` as the canonical invocation.
//!   3. Emit `contracts/reports/arch/cr-d3/<UTC>.json` listing
//!      `subcommands_without_example`. Exits non-zero if the list is
//!      non-empty.
//!
//! Heuristic note: matching "first 50 lines mention `vox X`" undercounts —
//! authors may reference a subcommand without using the literal invocation
//! string. That's the honest direction to err: false-negatives surface real
//! gaps; false-positives would hide them.

use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let workspace = vox_audit::workspace_root();

    // 1. Pull machine-readable subcommand surface from clap.
    let subcommands = match enumerate_subcommands(&workspace) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("CR-D3: failed to enumerate subcommands: {e}");
            std::process::exit(2);
        }
    };
    eprintln!("CR-D3: {} subcommand paths enumerated", subcommands.len());

    // 2. Walk the corpus once, build a set of mentioned subcommand strings.
    let corpus_roots = [workspace.join("examples"), workspace.join("scripts")];
    let mentioned = scan_corpus_for_mentions(&corpus_roots, &subcommands);

    // 3. Categorize subcommands.
    let mut with_example: Vec<String> = Vec::new();
    let mut without_example: Vec<String> = Vec::new();
    for sub in &subcommands {
        if mentioned.contains(sub) {
            with_example.push(sub.clone());
        } else {
            without_example.push(sub.clone());
        }
    }

    let total = subcommands.len() as u32;
    let covered = with_example.len() as u32;
    let coverage_pct = if total == 0 {
        0.0
    } else {
        100.0 * f64::from(covered) / f64::from(total)
    };
    let met = without_example.is_empty();

    eprintln!(
        "CR-D3 coverage: {covered}/{total} subcommands have at least one .vox example ({coverage_pct:.1}%)"
    );
    if !met {
        eprintln!(
            "CR-D3: {} subcommand(s) without an example:",
            without_example.len()
        );
        for sub in without_example.iter().take(20) {
            eprintln!("  - {sub}");
        }
        if without_example.len() > 20 {
            eprintln!("  … +{} more", without_example.len() - 20);
        }
    }

    let artifact = json!({
        "schema_version": 1,
        "criterion": "CR-D3",
        "measured_at": chrono::Utc::now().to_rfc3339(),
        "corpus_roots": corpus_roots.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
        "total_subcommands": total,
        "subcommands_with_example": with_example,
        "subcommands_without_example": without_example,
        "coverage_pct": coverage_pct,
        "threshold": {
            "target_coverage_pct": 100.0,
            "met": met,
        },
    });
    let body = serde_json::to_string_pretty(&artifact).expect("serialize");
    let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let out_dir = workspace
        .join("contracts")
        .join("reports")
        .join("arch")
        .join("cr-d3");
    std::fs::create_dir_all(&out_dir).expect("create cr-d3 dir");
    let out_path = out_dir.join(format!("{date}.json"));
    std::fs::write(&out_path, body).expect("write artifact");
    eprintln!("artifact: {}", out_path.display());

    if !met {
        std::process::exit(1);
    }
}

/// Run `cargo run -q -p vox-cli -- commands --format json` and parse the
/// nested command tree into a flat set of dotted paths
/// (e.g. `["ci", "ci.lint", "ci.lint.run"]`).
fn enumerate_subcommands(workspace: &std::path::Path) -> Result<Vec<String>, String> {
    let output = Command::new("cargo")
        .args([
            "run", "-q", "-p", "vox-cli", "--", "commands", "--format", "json",
        ])
        .current_dir(workspace)
        .output()
        .map_err(|e| format!("spawn cargo: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("vox commands failed: {stderr}"));
    }
    let body = String::from_utf8_lossy(&output.stdout);
    // Trim potential cargo-run preamble noise. Find the first '{'.
    let first_brace = body.find('{').ok_or("no JSON in output")?;
    let json: Value =
        serde_json::from_str(&body[first_brace..]).map_err(|e| format!("parse json: {e}"))?;
    let entries = json["entries"]
        .as_array()
        .ok_or("missing `entries` array")?;
    let mut out: Vec<String> = Vec::new();
    for e in entries {
        let Some(path) = e["path"].as_array() else {
            continue;
        };
        let dotted: Vec<String> = path
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();
        if dotted.is_empty() {
            continue;
        }
        out.push(dotted.join("."));
    }
    out.sort();
    out.dedup();
    Ok(out)
}

/// Walk every `.vox` file under `corpus_roots` and collect the set of
/// subcommand paths each file references via `vox <subcommand>`.
fn scan_corpus_for_mentions(corpus_roots: &[PathBuf], subcommands: &[String]) -> BTreeSet<String> {
    // Precompute the per-subcommand search needles. For path "ci.lint.run"
    // we look for "vox ci lint run" (clap's actual invocation form).
    let mut needles: Vec<(String, String)> = subcommands
        .iter()
        .map(|p| (p.clone(), format!("vox {}", p.replace('.', " "))))
        .collect();
    needles.sort_by(|a, b| b.1.len().cmp(&a.1.len())); // longest first

    let mut mentioned: BTreeSet<String> = BTreeSet::new();
    for root in corpus_roots {
        if !root.is_dir() {
            continue;
        }
        for entry in walkdir::WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_map(|r| r.ok())
        {
            let p = entry.path();
            if !(p.is_file() && p.extension().is_some_and(|x| x == "vox")) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(p) else {
                continue;
            };
            let header: String = text.lines().take(50).collect::<Vec<_>>().join("\n");
            for (path, needle) in &needles {
                if header.contains(needle) {
                    mentioned.insert(path.clone());
                }
            }
        }
    }
    mentioned
}
