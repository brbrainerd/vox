//! Mines recurring tool-call procedures from captured operations. Pure: takes
//! `MinedOp` rows, returns advisory `Candidate`s. No DB / IO dependency.

use crate::candidate::{Candidate, CandidateKind, DraftFrontmatter};
use std::collections::BTreeSet;
use std::collections::HashMap;

/// One captured operation the miner reasons over. The caller (vox-cli) maps a
/// `vox-db` row into this; rows with no `session_id` are dropped before mining.
#[derive(Debug, Clone)]
pub struct MinedOp {
    pub ts_ms: i64,
    pub session_id: String,
    pub tool_name: String,
    /// Top-level arg keys, sorted + deduped (values are redacted/ignored).
    pub arg_keys: Vec<String>,
}

/// Tuning for sequence mining.
#[derive(Debug, Clone)]
pub struct OpMiningOptions {
    pub min_len: usize,
    pub max_len: usize,
    pub min_occurrences: usize,
    pub min_distinct_sessions: usize,
}

impl Default for OpMiningOptions {
    fn default() -> Self {
        Self {
            min_len: 2,
            max_len: 6,
            min_occurrences: 3,
            min_distinct_sessions: 2,
        }
    }
}

/// Extract sorted, deduped top-level keys from a JSON object string. Non-object
/// or unparseable input → empty (the op still participates by tool_name).
pub fn arg_keys(args_json: &str) -> Vec<String> {
    match serde_json::from_str::<serde_json::Value>(args_json) {
        Ok(serde_json::Value::Object(map)) => {
            let set: BTreeSet<String> = map.keys().cloned().collect();
            set.into_iter().collect()
        }
        _ => Vec::new(),
    }
}

/// Stable per-op identity: `tool_name(k1,k2,...)`.
fn op_key(op: &MinedOp) -> String {
    if op.arg_keys.is_empty() {
        op.tool_name.clone()
    } else {
        format!("{}({})", op.tool_name, op.arg_keys.join(","))
    }
}

/// Skill-name-safe slug from an n-gram of tool names (Agent Skills `name` rule:
/// lowercase alphanumeric + single hyphens, ≤64, no leading/trailing/double hyphen).
fn ngram_name(tools: &[String]) -> String {
    let raw = tools.join("-").to_ascii_lowercase();
    let mut s: String = raw
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    while s.contains("--") {
        s = s.replace("--", "-");
    }
    let s: String = s.trim_matches('-').chars().take(64).collect();
    let s = s.trim_end_matches('-').to_string();
    if s.is_empty() {
        "procedure".to_string()
    } else {
        s
    }
}

/// Mine recurring contiguous tool-call sequences into ranked candidates.
pub fn mine_repeated_operations(ops: &[MinedOp], opts: &OpMiningOptions) -> Vec<Candidate> {
    // Group by session, ordered by ts.
    let mut by_session: HashMap<&str, Vec<&MinedOp>> = HashMap::new();
    for o in ops {
        by_session.entry(o.session_id.as_str()).or_default().push(o);
    }
    for v in by_session.values_mut() {
        v.sort_by_key(|o| o.ts_ms);
    }

    struct Agg {
        count: usize,
        sessions: BTreeSet<String>,
        tools: Vec<String>,
        anchors: Vec<String>,
    }
    let mut agg: HashMap<String, Agg> = HashMap::new();

    for (sid, list) in &by_session {
        let keys: Vec<String> = list.iter().map(|o| op_key(o)).collect();
        let tools: Vec<String> = list.iter().map(|o| o.tool_name.clone()).collect();
        let n = keys.len();
        for len in opts.min_len..=opts.max_len {
            if len > n {
                break;
            }
            for start in 0..=(n - len) {
                let gram = keys[start..start + len].join(" -> ");
                let e = agg.entry(gram).or_insert_with(|| Agg {
                    count: 0,
                    sessions: BTreeSet::new(),
                    tools: tools[start..start + len].to_vec(),
                    anchors: Vec::new(),
                });
                e.count += 1;
                e.sessions.insert((*sid).to_string());
                if e.anchors.len() < 20 {
                    e.anchors.push(format!("session:{}@{}", sid, list[start].ts_ms));
                }
            }
        }
    }

    let mut out: Vec<Candidate> = agg
        .into_values()
        .filter(|a| {
            a.count >= opts.min_occurrences && a.sessions.len() >= opts.min_distinct_sessions
        })
        .map(|a| {
            let arrow = a.tools.join(" → ");
            let name = ngram_name(&a.tools);
            Candidate {
                kind: CandidateKind::RepeatedOperations,
                members: a.anchors,
                score: (a.count * a.tools.len()) as f32,
                suggested_action: "Save recurring procedure as a skill".to_string(),
                draft_frontmatter: Some(DraftFrontmatter {
                    name,
                    description: format!(
                        "Recurring procedure: {arrow} (seen {}× across {} sessions)",
                        a.count,
                        a.sessions.len()
                    ),
                    category: "workflow".to_string(),
                    tags: vec!["auto-discovered".to_string(), "operations".to_string()],
                }),
            }
        })
        .collect();
    // Highest score first; stable tiebreak by name for deterministic output.
    out.sort_by(|x, y| {
        y.score
            .partial_cmp(&x.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                let xn = x
                    .draft_frontmatter
                    .as_ref()
                    .map(|d| d.name.as_str())
                    .unwrap_or("");
                let yn = y
                    .draft_frontmatter
                    .as_ref()
                    .map(|d| d.name.as_str())
                    .unwrap_or("");
                xn.cmp(yn)
            })
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn op(session: &str, ts: i64, tool: &str, keys: &[&str]) -> MinedOp {
        MinedOp {
            ts_ms: ts,
            session_id: session.to_string(),
            tool_name: tool.to_string(),
            arg_keys: keys.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn seq(session: &str, base: i64, tools: &[&str]) -> Vec<MinedOp> {
        tools
            .iter()
            .enumerate()
            .map(|(i, t)| op(session, base + i as i64, t, &[]))
            .collect()
    }

    fn default_opts() -> OpMiningOptions {
        OpMiningOptions {
            min_len: 2,
            max_len: 6,
            min_occurrences: 3,
            min_distinct_sessions: 2,
        }
    }

    #[test]
    fn detects_sequence_recurring_across_sessions() {
        let mut ops = Vec::new();
        ops.extend(seq("s1", 0, &["a", "b", "c"]));
        ops.extend(seq("s1", 10, &["a", "b", "c"]));
        ops.extend(seq("s2", 0, &["a", "b", "c"]));
        let cands = mine_repeated_operations(&ops, &default_opts());
        let abc = cands
            .iter()
            .find(|c| c.draft_frontmatter.as_ref().map(|d| d.name.as_str()) == Some("a-b-c"));
        assert!(abc.is_some(), "expected a-b-c candidate, got {cands:?}");
        let abc = abc.unwrap();
        assert_eq!(abc.kind, CandidateKind::RepeatedOperations);
        assert!(
            abc.draft_frontmatter
                .as_ref()
                .unwrap()
                .description
                .contains("3×")
        );
    }

    #[test]
    fn excludes_sequence_confined_to_one_session() {
        let mut ops = Vec::new();
        ops.extend(seq("s1", 0, &["a", "b", "c"]));
        ops.extend(seq("s1", 10, &["a", "b", "c"]));
        ops.extend(seq("s1", 20, &["a", "b", "c"]));
        let cands = mine_repeated_operations(&ops, &default_opts());
        assert!(
            cands.is_empty(),
            "min_distinct_sessions=2 should exclude single-session, got {cands:?}"
        );
    }

    #[test]
    fn excludes_below_min_occurrences() {
        let mut ops = Vec::new();
        ops.extend(seq("s1", 0, &["a", "b", "c"]));
        ops.extend(seq("s2", 0, &["a", "b", "c"]));
        let cands = mine_repeated_operations(&ops, &default_opts());
        assert!(cands.is_empty());
    }

    #[test]
    fn arg_key_shape_distinguishes_ops() {
        let mut ops = Vec::new();
        for s in ["s1", "s2", "s3"] {
            ops.push(op(s, 0, "read", &["path"]));
            ops.push(op(s, 1, "write", &[]));
        }
        for s in ["s4", "s5"] {
            ops.push(op(s, 0, "read", &["path", "range"]));
            ops.push(op(s, 1, "write", &[]));
        }
        let cands = mine_repeated_operations(&ops, &default_opts());
        let rw: Vec<_> = cands
            .iter()
            .filter(|c| {
                c.draft_frontmatter.as_ref().map(|d| d.name.as_str()) == Some("read-write")
            })
            .collect();
        assert_eq!(rw.len(), 1, "arg-shapes must not merge; got {cands:?}");
        assert!(
            rw[0]
                .draft_frontmatter
                .as_ref()
                .unwrap()
                .description
                .contains("3×"),
            "expected 3× (not merged to 5×); got {:?}",
            rw[0].draft_frontmatter
        );
    }

    #[test]
    fn arg_keys_parses_sorts_dedups() {
        assert_eq!(arg_keys(r#"{"b":1,"a":2}"#), vec!["a".to_string(), "b".to_string()]);
        assert_eq!(arg_keys("not json"), Vec::<String>::new());
        assert_eq!(arg_keys("[1,2]"), Vec::<String>::new());
    }

    #[test]
    fn empty_and_single_op_sessions_yield_nothing() {
        assert!(mine_repeated_operations(&[], &default_opts()).is_empty());
        assert!(mine_repeated_operations(&seq("s1", 0, &["a"]), &default_opts()).is_empty());
    }
}
