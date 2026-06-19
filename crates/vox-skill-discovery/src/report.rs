//! Render candidates for human or machine consumption. Advisory only.

use crate::candidate::Candidate;

/// Human-readable terminal report.
pub fn render_terminal(candidates: &[Candidate]) -> String {
    if candidates.is_empty() {
        return "No discovery candidates found.".to_string();
    }
    let mut out = format!("Found {} candidate(s):\n", candidates.len());
    for (i, c) in candidates.iter().enumerate() {
        out.push_str(&format!(
            "\n[{}] {:?} (score {:.2})\n    action: {}\n    members:\n",
            i + 1,
            c.kind,
            c.score,
            c.suggested_action
        ));
        for m in &c.members {
            out.push_str(&format!("      - {m}\n"));
        }
    }
    out
}

/// Machine-readable JSON report.
pub fn render_json(candidates: &[Candidate]) -> anyhow::Result<String> {
    Ok(serde_json::to_string_pretty(candidates)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::candidate::CandidateKind;

    fn sample() -> Vec<Candidate> {
        vec![Candidate {
            kind: CandidateKind::RepeatedCode,
            members: vec!["a.vox:1".into(), "b.vox:9".into()],
            score: 0.95,
            suggested_action: "Extract block".into(),
            draft_frontmatter: None,
        }]
    }

    #[test]
    fn terminal_lists_members() {
        let r = render_terminal(&sample());
        assert!(r.contains("a.vox:1"));
        assert!(r.contains("RepeatedCode"));
    }

    #[test]
    fn empty_terminal_is_clean() {
        assert!(render_terminal(&[]).contains("No discovery candidates"));
    }

    #[test]
    fn json_round_trips() {
        let j = render_json(&sample()).unwrap();
        assert!(j.contains("RepeatedCode"));
    }
}
