//! Normalize adapter remote status strings into `external_submission_jobs.status` values.

use serde::Serialize;

/// Result of mapping a venue-specific remote status to job queue semantics.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ScholarlyRemoteStatusMap {
    /// Lowercased trimmed remote status input.
    pub normalized_remote: String,
    /// Target job `status` (`succeeded`, `failed`, or unchanged sentinel).
    pub job_status: String,
    /// Whether this mapping represents a terminal remote outcome.
    pub terminal: bool,
    /// Stable machine-readable reason for logs and sync JSON.
    pub reason_code: &'static str,
    /// When true, caller should keep the existing job row status (unknown / in-flight remote).
    pub preserve_prior_job_status: bool,
    /// When true, remote string was not recognized adapter-specifically (quarantine for observability).
    pub quarantined_unknown: bool,
}

fn global_terminal_map(r: &str) -> Option<(&'static str, &'static str, bool)> {
    match r {
        "done" | "published" | "accepted" | "completed" => {
            Some(("succeeded", "global_terminal_ok", true))
        }
        "rejected" | "failed" | "error" | "deleted" | "cancelled" | "canceled" => {
            Some(("failed", "global_terminal_bad", true))
        }
        _ => None,
    }
}

fn map_zenodo(r: &str) -> Option<ScholarlyRemoteStatusMap> {
    match r {
        "published" => Some(ScholarlyRemoteStatusMap {
            normalized_remote: r.to_string(),
            job_status: "succeeded".into(),
            terminal: true,
            reason_code: "zenodo_published",
            preserve_prior_job_status: false,
            quarantined_unknown: false,
        }),
        "draft" | "inprogress" | "newversion" | "upload" | "expired" | "embargoed" => {
            Some(ScholarlyRemoteStatusMap {
                normalized_remote: r.to_string(),
                job_status: String::new(),
                terminal: false,
                reason_code: "zenodo_non_terminal",
                preserve_prior_job_status: true,
                quarantined_unknown: false,
            })
        }
        _ => None,
    }
}

fn map_openreview(r: &str) -> Option<ScholarlyRemoteStatusMap> {
    match r {
        // Common note / submission lifecycle labels (lowercased).
        "accepted" | "decision posted" | "published" => Some(ScholarlyRemoteStatusMap {
            normalized_remote: r.to_string(),
            job_status: "succeeded".into(),
            terminal: true,
            reason_code: "openreview_terminal_ok",
            preserve_prior_job_status: false,
            quarantined_unknown: false,
        }),
        "rejected" | "withdrawn" | "deleted" => Some(ScholarlyRemoteStatusMap {
            normalized_remote: r.to_string(),
            job_status: "failed".into(),
            terminal: true,
            reason_code: "openreview_terminal_bad",
            preserve_prior_job_status: false,
            quarantined_unknown: false,
        }),
        "submitted" | "under review" | "pending" | "invite" | "active" | "revision requested"
        | "expired" => Some(ScholarlyRemoteStatusMap {
            normalized_remote: r.to_string(),
            job_status: String::new(),
            terminal: false,
            reason_code: "openreview_non_terminal",
            preserve_prior_job_status: true,
            quarantined_unknown: false,
        }),
        _ => None,
    }
}

/// Map remote adapter status to job row status, preserving prior job status when remote is non-terminal or unknown.
#[must_use]
pub fn map_scholarly_remote_to_job_status(
    adapter: &str,
    remote_status: &str,
    prior_job_status: &str,
) -> ScholarlyRemoteStatusMap {
    let normalized_remote = remote_status.trim().to_ascii_lowercase();
    let r = normalized_remote.as_str();
    let adapter_norm = adapter.trim().to_ascii_lowercase();

    if let Some(m) = match adapter_norm.as_str() {
        "zenodo" => map_zenodo(r),
        "openreview" => map_openreview(r),
        _ => None,
    } {
        return m;
    }

    if let Some((st, reason, term)) = global_terminal_map(r) {
        return ScholarlyRemoteStatusMap {
            normalized_remote: r.to_string(),
            job_status: st.to_string(),
            terminal: term,
            reason_code: reason,
            preserve_prior_job_status: false,
            quarantined_unknown: false,
        };
    }

    ScholarlyRemoteStatusMap {
        normalized_remote: r.to_string(),
        job_status: prior_job_status.to_string(),
        terminal: false,
        reason_code: "unknown_remote_preserve_job",
        preserve_prior_job_status: true,
        quarantined_unknown: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zenodo_published_terminal() {
        let m = map_scholarly_remote_to_job_status("zenodo", "published", "running");
        assert_eq!(m.job_status, "succeeded");
        assert!(m.terminal);
        assert!(!m.preserve_prior_job_status);
    }

    #[test]
    fn zenodo_draft_preserves_job() {
        let m = map_scholarly_remote_to_job_status("zenodo", "draft", "running");
        assert!(m.preserve_prior_job_status);
        assert!(!m.terminal);
    }

    #[test]
    fn openreview_withdrawn_fails() {
        let m = map_scholarly_remote_to_job_status("openreview", "withdrawn", "running");
        assert_eq!(m.job_status, "failed");
        assert!(m.terminal);
    }

    #[test]
    fn unknown_quarantined() {
        let m = map_scholarly_remote_to_job_status("zenodo", "not_a_real_state_xyz", "queued");
        assert!(m.quarantined_unknown);
        assert!(m.preserve_prior_job_status);
        assert_eq!(m.job_status, "queued");
    }

    #[test]
    fn global_done_maps() {
        let m = map_scholarly_remote_to_job_status("local_ledger", "done", "running");
        assert_eq!(m.job_status, "succeeded");
    }
}
