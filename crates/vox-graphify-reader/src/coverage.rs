//! Coverage classification: for each node of a given `kind`, decide whether it
//! is surfaced by a caller, an orphan backend (no callers), or a dead-end
//! (target flagged `"missing": true`, i.e. a `dangling` edge points at nothing).
//!
//! Honesty firewall: this is a STRUCTURAL classification. It reports what the
//! graph says (callers / missing flags); it makes no judgment about whether a
//! surface "should" exist.
//!
//! `CliOnly` is reserved: it would mark a command node that also appears in the
//! ingested clap/command-catalog set but has no GUI caller. CLI-union scoring is
//! deferred until the command-catalog adapter is ingested (see plan D3/F1); the
//! enum arm exists so consumers can match exhaustively, but `compute_coverage`
//! never produces it today.

use serde::Serialize;
use serde_json::Value;

/// Coverage status for a single backend node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageStatus {
    /// No caller edge targets this node.
    OrphanBackend,
    /// Node is flagged `"missing": true` — a dangling edge points at nothing.
    DeadEnd,
    /// At least one caller edge targets this node.
    Surfaced,
    /// Reserved: command present in the CLI catalog but with no GUI caller.
    /// Not currently produced (CLI-union scoring deferred).
    CliOnly,
}

/// One coverage entry per node of the requested `kind`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CoverageEntry {
    pub id: String,
    pub label: String,
    pub status: CoverageStatus,
}

/// Result of [`compute_coverage`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CoverageReport {
    pub entries: Vec<CoverageEntry>,
}

fn nodes(graph: &Value) -> &[Value] {
    graph
        .get("nodes")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn links(graph: &Value) -> &[Value] {
    graph
        .get("links")
        .or_else(|| graph.get("edges"))
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn str_field<'a>(node: &'a Value, key: &str) -> Option<&'a str> {
    node.get(key).and_then(Value::as_str)
}

/// Classify every node whose `"kind"` equals `kind`.
///
/// For each such node: collect caller edges whose `target` is the node id; if the
/// node carries `"missing": true` → [`CoverageStatus::DeadEnd`]; else if any
/// caller targets it → [`CoverageStatus::Surfaced`]; else
/// [`CoverageStatus::OrphanBackend`].
pub fn compute_coverage(graph: &Value, kind: &str) -> CoverageReport {
    let links = links(graph);
    let mut entries = Vec::new();

    for node in nodes(graph) {
        if str_field(node, "kind") != Some(kind) {
            continue;
        }
        let Some(id) = str_field(node, "id") else {
            continue;
        };
        let label = str_field(node, "label").unwrap_or(id).to_string();

        let status = if node.get("missing").and_then(Value::as_bool) == Some(true) {
            CoverageStatus::DeadEnd
        } else {
            let has_caller = links
                .iter()
                .any(|l| str_field(l, "target") == Some(id));
            if has_caller {
                CoverageStatus::Surfaced
            } else {
                CoverageStatus::OrphanBackend
            }
        };

        entries.push(CoverageEntry {
            id: id.to_string(),
            label,
            status,
        });
    }

    CoverageReport { entries }
}

#[cfg(test)]
mod tests {
    use super::{compute_coverage, CoverageStatus};
    use serde_json::json;

    #[test]
    fn classifies_surfaced_orphan_deadend() {
        let g = json!({"nodes":[
            {"id":"cmd:wired","label":"wired","kind":"command","community":"c_0"},
            {"id":"cmd:orphan","label":"orphan","kind":"command","community":"c_0"},
            {"id":"cmd:gone","label":"gone","kind":"command","community":"c_0","missing":true},
            {"id":"S::go","label":"go","kind":"fn","community":"c_0"}],
          "links":[
            {"source":"S::go","target":"cmd:wired","confidence":"declared"},
            {"source":"S::go","target":"cmd:gone","confidence":"dangling"}]});
        let r = compute_coverage(&g, "command");
        let f = |id: &str| r.entries.iter().find(|e| e.id == id).unwrap().status.clone();
        assert_eq!(f("cmd:wired"), CoverageStatus::Surfaced);
        assert_eq!(f("cmd:orphan"), CoverageStatus::OrphanBackend);
        assert_eq!(f("cmd:gone"), CoverageStatus::DeadEnd);
    }
}
