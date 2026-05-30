//! Writes draft artifacts to the staging dir. NEVER writes into the build tree.

use crate::route::RemediationDecision;
use std::path::Path;

/// Write one decision's drafted artifact into `staging_root`. Returns the path
/// written, or None if the decision has no artifact (form == None or unverified).
pub fn write_artifact(
    staging_root: &Path,
    decision: &RemediationDecision,
) -> std::io::Result<Option<std::path::PathBuf>> {
    let Some(artifact) = &decision.drafted_artifact else {
        return Ok(None);
    };
    if !decision.verified {
        return Ok(None);
    }
    // Filename only from staging_path; force it under staging_root; force .proposed.
    let filename = Path::new(&artifact.staging_path)
        .file_name()
        .ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "bad staging_path")
        })?;
    let dest = staging_root.join("artifacts").join(filename);
    assert!(
        dest.to_string_lossy().ends_with(".proposed"),
        "artifact must be .proposed"
    );
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&dest, &artifact.body)?;
    Ok(Some(dest))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route::{ArtifactForm, DraftedArtifact};

    fn decision(form: ArtifactForm, verified: bool) -> RemediationDecision {
        RemediationDecision {
            cluster_id: "c1".into(),
            member_commit_shas: vec![],
            member_count: 0,
            total_member_tokens: 0,
            artifact_form: form,
            confidence: 0.9,
            synthesized_fix_summary: "s".into(),
            drafted_artifact: if matches!(form, ArtifactForm::None) {
                None
            } else {
                Some(DraftedArtifact {
                    form,
                    staging_path: format!("c1.{}", form.staging_extension()),
                    body: "body".into(),
                    form_rationale: "r".into(),
                    authoring_model_vox_capable: false,
                })
            },
            verified,
            refutation_note: "n".into(),
        }
    }

    #[test]
    fn writes_proposed_file_under_staging() {
        let tmp = tempfile::tempdir().unwrap();
        let p = write_artifact(tmp.path(), &decision(ArtifactForm::CiGate, true))
            .unwrap()
            .unwrap();
        assert!(p.starts_with(tmp.path()));
        assert!(p.to_string_lossy().ends_with(".proposed"));
        assert!(p.components().any(|c| c.as_os_str() == "artifacts"));
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "body");
    }

    #[test]
    fn no_write_for_unverified_or_none() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(
            write_artifact(tmp.path(), &decision(ArtifactForm::CiGate, false))
                .unwrap()
                .is_none()
        );
        assert!(
            write_artifact(tmp.path(), &decision(ArtifactForm::None, true))
                .unwrap()
                .is_none()
        );
    }
}
