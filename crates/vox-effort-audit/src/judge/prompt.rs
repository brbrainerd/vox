//! Prompt construction for the per-commit judge.

use crate::shape::ShapeFeatures;
use crate::walk::CommitRecord;
use vox_actor_runtime::llm::LlmChatMessage;

pub fn build_messages(rec: &CommitRecord, shape: &ShapeFeatures) -> Vec<LlmChatMessage> {
    let system = include_str!("prompt_system.md");
    let user = format!(
"COMMIT_SHA: {sha}
COMMIT_MESSAGE:
{msg}

SHAPE_FEATURES (locally computed, trust as ground truth):
- additions: {add}
- deletions: {del}
- files_changed: {fc}
- commit_kind_from_message: {kind:?}
- mechanical_sweep_score: {ms:.2}
- is_lockfile_only: {ll}
- is_generated_only: {gen}
- is_doc_only: {doc}

UNIFIED_DIFF (possibly truncated; see `[TRUNCATED]` marker):
```
{diff}
```

Return a single JSON object matching the schema. Be concise.",
        sha = rec.sha,
        msg = rec.message,
        add = rec.additions,
        del = rec.deletions,
        fc = rec.files.len(),
        kind = shape.commit_kind_from_message,
        ms = shape.mechanical_sweep_score,
        ll = shape.is_lockfile_only,
        gen = shape.is_generated_only,
        doc = shape.is_doc_only,
        diff = if rec.diff_truncated {
            format!("[TRUNCATED — only file list shown]\n{}",
                rec.files.iter().map(|f| format!("- {} (+{}/-{})", f.path, f.additions, f.deletions)).collect::<Vec<_>>().join("\n"))
        } else {
            rec.unified_diff_text.clone()
        },
    );
    vec![
        LlmChatMessage {
            role: "system".into(),
            content: system.into(),
        },
        LlmChatMessage {
            role: "user".into(),
            content: user,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shape::CommitKind;
    use std::collections::HashMap;

    fn fake_rec() -> CommitRecord {
        crate::walk::CommitRecord {
            sha: "abc123".into(),
            parent_sha: None,
            commit_ts: chrono::Utc::now(),
            message: "refactor: foo".into(),
            author_email_sha256: "z".into(),
            files: vec![],
            additions: 10,
            deletions: 5,
            unified_diff_text: "diff body".into(),
            diff_truncated: false,
        }
    }
    fn fake_shape() -> ShapeFeatures {
        ShapeFeatures {
            additions: 10,
            deletions: 5,
            files_changed: 2,
            file_extension_histogram: HashMap::new(),
            mechanical_sweep_score: 0.85,
            is_lockfile_only: false,
            is_generated_only: false,
            is_doc_only: false,
            commit_kind_from_message: CommitKind::Refactor,
        }
    }

    #[test]
    fn includes_shape_features_in_user_prompt() {
        let m = build_messages(&fake_rec(), &fake_shape());
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].role, "system");
        assert!(m[1].content.contains("mechanical_sweep_score: 0.85"));
        assert!(m[1].content.contains("abc123"));
    }

    #[test]
    fn truncation_marker_when_truncated() {
        let mut r = fake_rec();
        r.diff_truncated = true;
        let m = build_messages(&r, &fake_shape());
        assert!(m[1].content.contains("[TRUNCATED"));
    }
}
