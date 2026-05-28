use crate::planning::policy::default_policy_for_goal;
use crate::planning::{PlanNode, PlanStatus};
use crate::types::FileAffinity;

fn infer_file_manifest_from_clause(clause: &str) -> Vec<FileAffinity> {
    let lower = clause.to_ascii_lowercase();
    let write_hint = [
        "fix ",
        "edit ",
        "change ",
        "add ",
        "implement ",
        "update ",
        "refactor ",
        "write ",
        "create ",
        "delete ",
        "remove ",
        "patch ",
    ]
    .iter()
    .any(|k| lower.contains(k));

    let exts = [
        ".rs", ".toml", ".md", ".yaml", ".yml", ".json", ".vox", ".tsx", ".ts", ".js",
    ];
    let mut seen = std::collections::HashSet::<String>::new();
    let mut out = Vec::new();

    for raw_word in clause.split_whitespace() {
        let w = raw_word.trim_matches(|c: char| {
            c == '`'
                || c == '"'
                || c == '\''
                || c == '('
                || c == ')'
                || c == ','
                || c == ';'
                || c == ':'
        });
        if w.len() < 3 {
            continue;
        }
        let norm = w.replace('\\', "/");
        if !norm.contains('/') {
            continue;
        }
        let ext_ok = exts.iter().any(|e| norm.ends_with(e));
        let prefix_ok = norm.starts_with("crates/")
            || norm.starts_with("src/")
            || norm.starts_with("docs/")
            || norm.starts_with("examples/");
        if !ext_ok && !prefix_ok {
            continue;
        }
        if !seen.insert(norm.clone()) {
            continue;
        }
        if write_hint {
            out.push(FileAffinity::write(&norm));
        } else {
            out.push(FileAffinity::read(&norm));
        }
    }
    out
}

fn infer_verification_manifest(goal: &str) -> Vec<FileAffinity> {
    let lower = goal.to_ascii_lowercase();
    let mut files = Vec::new();
    if lower.contains("contract") || lower.contains("schema") {
        files.push(FileAffinity::read("contracts/index.yaml"));
    }
    if lower.contains("doc") || lower.contains("ssot") || lower.contains("readme") {
        files.push(FileAffinity::read("docs/src/reference"));
    }
    if lower.contains("test") || lower.contains("verify") || lower.contains("regression") {
        files.push(FileAffinity::read("Cargo.toml"));
    }
    files
}

fn split_goal_clauses(goal: &str) -> Vec<String> {
    let mut pieces: Vec<String> = Vec::new();
    let normalized = goal.replace('\r', "");
    for block in normalized.split('\n') {
        let line = block.trim();
        if line.is_empty() {
            continue;
        }
        // Numbered list lines: "1. Do thing"
        let rest = if let Some((head, tail)) = line.split_once('.') {
            let is_ordered =
                head.trim().chars().all(|c| c.is_ascii_digit()) && !head.trim().is_empty();
            if is_ordered { tail.trim() } else { line }
        } else {
            line
        };
        for segment in rest.split(" and ") {
            let s = segment.trim().trim_end_matches('.').trim();
            if s.is_empty() {
                continue;
            }
            for sub in s.split(';') {
                let t = sub.trim().trim_end_matches('.').trim();
                if t.is_empty() {
                    continue;
                }
                for part in t.split(" then ") {
                    let p = part.trim().trim_end_matches('.').trim();
                    if !p.is_empty() {
                        pieces.push(p.to_string());
                    }
                }
            }
        }
    }
    pieces
}

/// When clause splitting yields one very long paragraph, break it into sequential steps so native
/// synthesis does not collapse complex goals into a single node.
fn burst_long_monolithic_clause(text: &str) -> Vec<String> {
    let t = text.trim();
    if t.is_empty() {
        return vec![];
    }
    let words = t.split_whitespace().count();
    if words <= 36 {
        return vec![t.to_string()];
    }

    let semi_split: Vec<String> = t
        .split(';')
        .map(|s| s.trim().trim_end_matches('.').trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if semi_split.len() >= 2 {
        return semi_split;
    }

    let core = semi_split
        .into_iter()
        .next()
        .unwrap_or_else(|| t.to_string());

    let mut sentences: Vec<String> = Vec::new();
    for raw in core.split(". ") {
        let u = raw.trim();
        if u.split_whitespace().count() < 5 {
            continue;
        }
        let mut piece = u.to_string();
        if !piece.ends_with('.') {
            piece.push('.');
        }
        sentences.push(piece);
    }
    if sentences.len() >= 2 {
        return sentences;
    }

    let basis = sentences.into_iter().next().unwrap_or(core);
    let w: Vec<&str> = basis.split_whitespace().collect();
    const STRIDE: usize = 16;
    if w.len() <= STRIDE {
        return vec![basis];
    }
    w.chunks(STRIDE)
        .map(|c| c.join(" "))
        .filter(|s| !s.is_empty())
        .collect()
}

pub fn parse_llm_plan_response(raw: &str) -> Result<Vec<PlanNode>, String> {
    if let Some(start) = raw.find("<execute>") {
        if let Some(end) = raw[start + 9..].find("</execute>") {
            let json_str = raw[start + 9..start + 9 + end].trim();
            if let Ok(nodes) = serde_json::from_str::<Vec<PlanNode>>(json_str) {
                if !nodes.is_empty() {
                    return Ok(nodes);
                }
            }
        }
    }
    let stripped = raw
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    if let Ok(nodes) = serde_json::from_str::<Vec<PlanNode>>(stripped) {
        if !nodes.is_empty() {
            return Ok(nodes);
        }
    }
    Err("Failed to parse LLM plan response".to_string())
}

pub fn build_goal_user_prompt(goal: &str, depth: &str) -> String {
    format!(
        "GOAL: {}\nPLANNING DEPTH: {}\n\n{}",
        goal,
        depth,
        include_str!("prompts/synthesize_nodes_v2.txt")
    )
}

pub async fn synthesize_plan_nodes_with_llm<F, Fut>(
    goal: &str,
    depth: &str,
    llm_fn: F,
) -> Vec<PlanNode>
where
    F: Fn(&str, &str) -> Fut,
    Fut: std::future::Future<Output = Result<String, String>>,
{
    // Note: PLANNER_SYSTEM_PROMPT is in crate::planning::prompts
    use crate::planning::prompts::PLANNER_SYSTEM_PROMPT;
    let user_prompt = build_goal_user_prompt(goal, depth);
    match llm_fn(PLANNER_SYSTEM_PROMPT, &user_prompt).await {
        Ok(raw) => parse_llm_plan_response(&raw).unwrap_or_else(|_| synthesize_plan_nodes(goal)),
        Err(_) => synthesize_plan_nodes(goal),
    }
}

pub fn synthesize_plan_nodes(goal: &str) -> Vec<PlanNode> {
    if let Ok(nodes) = parse_llm_plan_response(goal) {
        return nodes;
    }

    let mut parts = split_goal_clauses(goal);
    if parts.is_empty() {
        let trimmed = goal.trim();
        if trimmed.is_empty() {
            return vec![];
        }
        parts.push(trimmed.to_string());
    }
    if parts.len() == 1 {
        parts = burst_long_monolithic_clause(&parts[0]);
    }

    let mut nodes = Vec::with_capacity(parts.len());
    for (idx, part) in parts.iter().enumerate() {
        let node_id = format!("n{}", idx + 1);
        let depends_on = if idx == 0 {
            vec![]
        } else {
            vec![format!("n{}", idx)]
        };
        let mut policy = default_policy_for_goal(part);
        let inferred = infer_file_manifest_from_clause(part);
        if !inferred.is_empty() {
            policy.file_manifest = inferred;
        }
        nodes.push(PlanNode {
            node_id,
            description: part.clone(),
            depends_on,
            status: PlanStatus::Pending,
            execution_policy: policy,
            workflow_invocation: None,
        });
    }
    let needs_verify_node = {
        let g = goal.to_ascii_lowercase();
        !g.contains("verify")
            && !g.contains("validation")
            && !g.contains("regression")
            && !g.contains("test")
    };
    if needs_verify_node {
        let mut policy = default_policy_for_goal("verify reconstruction outputs");
        let verify_manifest = infer_verification_manifest(goal);
        if !verify_manifest.is_empty() {
            policy.file_manifest = verify_manifest;
        }
        let last_dep = nodes.last().map(|n| n.node_id.clone());
        nodes.push(PlanNode {
            node_id: format!("n{}", nodes.len() + 1),
            description: "Run verification stack (compile, targeted tests, contract/doc checks) and report gaps."
                .to_string(),
            depends_on: last_dep.into_iter().collect(),
            status: PlanStatus::Pending,
            execution_policy: policy,
            workflow_invocation: Some("verification_stack".to_string()),
        });
    }
    nodes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_semicolon_and_then() {
        let g = "run fmt; run tests then deploy";
        let n = synthesize_plan_nodes(g);
        assert!(n.len() >= 3, "expected multiple clauses, got {}", n.len());
        assert_eq!(n[0].depends_on.len(), 0);
        assert_eq!(n[1].depends_on, vec!["n1".to_string()]);
    }

    #[test]
    fn numbered_lines_become_chain() {
        let g = "1. First\n2. Second\n3. Third";
        let n = synthesize_plan_nodes(g);
        assert!(n.len() >= 3);
        assert_eq!(n[2].depends_on, vec!["n2".to_string()]);
    }

    #[test]
    fn clause_paths_become_file_manifest() {
        let g = "fix compiler error in crates/foo/src/lib.rs";
        let n = synthesize_plan_nodes(g);
        assert!(!n.is_empty());
        assert_eq!(n[0].execution_policy.file_manifest.len(), 1);
        assert!(
            n[0].execution_policy.file_manifest[0]
                .path
                .to_string_lossy()
                .contains("crates/foo/src/lib.rs")
        );
    }

    #[test]
    fn synthesize_appends_verification_stack_node_when_missing() {
        let n = synthesize_plan_nodes("implement parser recovery logic");
        let last = n.last().expect("at least one node");
        assert_eq!(
            last.workflow_invocation.as_deref(),
            Some("verification_stack")
        );
        assert!(
            last.description
                .to_ascii_lowercase()
                .contains("verification")
        );
    }

    #[test]
    fn synthesize_omits_extra_verification_node_when_goal_includes_tests() {
        let n = synthesize_plan_nodes("run tests and verify parser behavior");
        assert!(
            n.iter()
                .filter(|node| node.workflow_invocation.as_deref() == Some("verification_stack"))
                .count()
                == 0
        );
    }

    #[test]
    fn long_monolithic_line_splits_into_chain() {
        let mut words: Vec<&str> = Vec::new();
        for i in 0..50 {
            words.push(if i % 10 == 0 { "then" } else { "step" });
        }
        let g = words.join(" ");
        let n = synthesize_plan_nodes(&g);
        assert!(
            n.len() >= 3,
            "expected burst into multiple nodes, got {}",
            n.len()
        );
    }
}
