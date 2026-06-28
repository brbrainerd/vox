//! T9: `compute_coverage` over the unified `cli-command` node-set.
//!
//! A `cli:` node is honestly `CliOnly` when no `surface:` node reaches it (no
//! GUI path) — even if it joins to a `cmd:`/`tool:` impl. It is `Surfaced` only
//! when its joined impl is itself reached by a `surface:` node.

use serde_json::json;
use vox_graph_reader::coverage::{CoverageStatus, compute_coverage};

#[test]
fn cli_only_command_classified_cli_only() {
    let graph = json!({
      "nodes": [
        { "id": "cli:db:vacuum", "label": "vacuum", "kind": "cli-command" },
        { "id": "cli:ci:lint",   "label": "lint",   "kind": "cli-command" },
        { "id": "cmd:lint",      "label": "lint",   "kind": "command" },
        { "id": "surface:develop:ci", "label": "ci", "kind": "surface" }
      ],
      "links": [
        { "source": "cli:ci:lint", "target": "cmd:lint" },
        { "source": "surface:develop:ci", "target": "cmd:lint" }
      ]
    });

    let report = compute_coverage(&graph, "cli-command");

    let vacuum = report
        .entries
        .iter()
        .find(|e| e.id == "cli:db:vacuum")
        .expect("cli:db:vacuum entry");
    assert_eq!(
        vacuum.status,
        CoverageStatus::CliOnly,
        "surface-less cli node must be CliOnly"
    );

    let lint = report
        .entries
        .iter()
        .find(|e| e.id == "cli:ci:lint")
        .expect("cli:ci:lint entry");
    assert_eq!(
        lint.status,
        CoverageStatus::Surfaced,
        "cli node whose impl a surface reaches must be Surfaced"
    );
}
