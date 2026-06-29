//! Coverage classification: for each node of a given `kind`, decide whether it
//! is surfaced by a caller, an orphan backend (no callers), or a dead-end
//! (target flagged `"missing": true`, i.e. a `dangling` edge points at nothing).
//!
//! Honesty firewall: this is a STRUCTURAL classification. It reports what the
//! graph says (callers / missing flags); it makes no judgment about whether a
//! surface "should" exist.
//!
//! `CliOnly` marks a `cli-command` node that no `surface:` node reaches (no GUI
//! path) — even when it joins to a `cmd:`/`tool:` impl. CLI-union scoring is now
//! live: passing `kind = "cli-command"` classifies the ingested clap tree as
//! `Surfaced` (a surface reaches it) or `CliOnly` (honest not-in-GUI).

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
    /// `cli-command` node present in the CLI catalog but reached by no
    /// `surface:` node — honest "not in the GUI". Produced for `kind =
    /// "cli-command"`; a name-match join to an impl is not itself a GUI path.
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

/// True when a `surface:` node reaches the cli node `id` — directly (a
/// `surface:` edge targets it) or via a joined impl (the cli node points
/// outbound to an impl that a `surface:` node also targets).
fn surface_reaches_cli(links: &[Value], id: &str) -> bool {
    // Direct: surface:* -> id.
    let direct = links.iter().any(|l| {
        str_field(l, "target") == Some(id)
            && str_field(l, "source").is_some_and(|s| s.starts_with("surface:"))
    });
    if direct {
        return true;
    }
    // Indirect: id -> impl, and surface:* -> impl (same impl node).
    for join in links.iter().filter(|l| str_field(l, "source") == Some(id)) {
        let Some(impl_id) = str_field(join, "target") else {
            continue;
        };
        let surfaced_impl = links.iter().any(|l| {
            str_field(l, "target") == Some(impl_id)
                && str_field(l, "source").is_some_and(|s| s.starts_with("surface:"))
        });
        if surfaced_impl {
            return true;
        }
    }
    false
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
        } else if kind == "cli-command" {
            // CLI-union scoring: a `cli:` node is honestly Surfaced only when a
            // `surface:` node reaches it — directly, or via a joined `cmd:`/`tool:`
            // impl that a surface also reaches. A name-match join to an impl is NOT
            // by itself a GUI path; absent any surface, the node is CliOnly.
            if surface_reaches_cli(links, id) {
                CoverageStatus::Surfaced
            } else {
                CoverageStatus::CliOnly
            }
        } else {
            let has_caller = links.iter().any(|l| str_field(l, "target") == Some(id));
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
    use super::{CoverageStatus, compute_coverage};
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
        let f = |id: &str| {
            r.entries
                .iter()
                .find(|e| e.id == id)
                .unwrap()
                .status
                .clone()
        };
        assert_eq!(f("cmd:wired"), CoverageStatus::Surfaced);
        assert_eq!(f("cmd:orphan"), CoverageStatus::OrphanBackend);
        assert_eq!(f("cmd:gone"), CoverageStatus::DeadEnd);
    }
}
