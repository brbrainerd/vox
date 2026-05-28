//! In-process tap on the SCIENTIA [`ResearchEvent`] broadcast for mesh / publication hooks.
//!
//! MCP tools attach a [`tokio::sync::broadcast`] sender; hosts spawn this subscriber from
//! [`ServerState::spawn_scientia_research_mesh_background_jobs`] without requiring Codex.
//! When `news-publish` is enabled and [`ScientiaMeshSubscriberOptions::publisher_mesh_intake_enabled`]
//! is true (from [`OrchestratorConfig::research_mesh_intake_writer_active`](vox_orchestrator::config::OrchestratorConfig::research_mesh_intake_writer_active)),
//! finding and publication events are mirrored into `vox-publisher` mesh intake files under the repo root.

use std::path::PathBuf;

use tokio::sync::broadcast;
use vox_research_events::ResearchEvent;

/// Configuration for optional `vox-publisher` on-disk mesh intake.
#[derive(Clone, Debug)]
pub struct ScientiaMeshSubscriberOptions {
    pub repo_root: PathBuf,
    /// When true and the binary is built with `news-publish`, write intake JSON under the repo.
    /// Set from `OrchestratorConfig::research_mesh_intake_writer_active()`.
    pub publisher_mesh_intake_enabled: bool,
}

fn trace_event(event: &ResearchEvent) {
    match event {
        ResearchEvent::FindingCandidateProposed {
            finding_id,
            session_id,
            ..
        } => {
            tracing::info!(
                target: "vox_orchestrator::scientia_mesh",
                finding_id = %finding_id,
                session_id = %session_id,
                "scientia_mesh_finding_candidate_observed"
            );
        }
        ResearchEvent::PublicationSucceeded {
            manifest_id, doi, ..
        } => {
            tracing::info!(
                target: "vox_orchestrator::scientia_mesh",
                manifest_id = %manifest_id,
                doi = ?doi,
                "scientia_mesh_publication_succeeded"
            );
        }
        ResearchEvent::PublicationFailed { manifest_id, error } => {
            tracing::warn!(
                target: "vox_orchestrator::scientia_mesh",
                manifest_id = %manifest_id,
                error = %error,
                "scientia_mesh_publication_failed"
            );
        }
        _ => {}
    }
}

fn apply_publisher_mesh_intake(repo_root: &std::path::Path, event: &ResearchEvent, enabled: bool) {
    if !enabled {
        return;
    }
    #[cfg(not(feature = "news-publish"))]
    {
        let _ = (repo_root, event);
        return;
    }
    #[cfg(feature = "news-publish")]
    {
        match event {
            ResearchEvent::FindingCandidateProposed {
                finding_id,
                session_id,
                claim_ids,
                worthiness_score,
                ..
            } => {
                if let Err(e) = vox_publisher::research_mesh::record_finding_candidate_proposed(
                    repo_root,
                    finding_id.as_str(),
                    session_id.as_str(),
                    claim_ids.as_slice(),
                    *worthiness_score,
                ) {
                    tracing::warn!(
                        target: "vox_orchestrator::scientia_mesh",
                        error = %e,
                        finding_id = %finding_id,
                        "research_mesh_intake_finding_failed"
                    );
                } else {
                    tracing::debug!(
                        target: "vox_orchestrator::scientia_mesh",
                        finding_id = %finding_id,
                        "research_mesh_intake_finding_written"
                    );
                }
            }
            ResearchEvent::PublicationSucceeded {
                manifest_id,
                doi,
                nanopub_uris,
            } => {
                if let Err(e) = vox_publisher::research_mesh::record_publication_succeeded(
                    repo_root,
                    manifest_id.as_str(),
                    doi.as_deref(),
                    nanopub_uris.as_slice(),
                ) {
                    tracing::warn!(
                        target: "vox_orchestrator::scientia_mesh",
                        error = %e,
                        manifest_id = %manifest_id,
                        "research_mesh_intake_publication_success_failed"
                    );
                } else {
                    tracing::debug!(
                        target: "vox_orchestrator::scientia_mesh",
                        manifest_id = %manifest_id,
                        "research_mesh_intake_publication_success_written"
                    );
                }
            }
            ResearchEvent::PublicationFailed { manifest_id, error } => {
                if let Err(e) = vox_publisher::research_mesh::record_publication_failed(
                    repo_root,
                    manifest_id.as_str(),
                    error.as_str(),
                ) {
                    tracing::warn!(
                        target: "vox_orchestrator::scientia_mesh",
                        error = %e,
                        manifest_id = %manifest_id,
                        "research_mesh_intake_publication_failed_write_failed"
                    );
                } else {
                    tracing::debug!(
                        target: "vox_orchestrator::scientia_mesh",
                        manifest_id = %manifest_id,
                        "research_mesh_intake_publication_failed_written"
                    );
                }
            }
            _ => {}
        }
    }
}

/// Background loop: observe high-signal events (finding candidates, publication outcomes).
///
/// Call once per process after the broadcast sender is created (for example from MCP `ServerState`).
pub fn spawn_scientia_mesh_research_event_subscriber(
    mut rx: broadcast::Receiver<ResearchEvent>,
    options: Option<ScientiaMeshSubscriberOptions>,
) {
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(evt) => {
                    trace_event(&evt);
                    if let Some(ref opts) = options {
                        apply_publisher_mesh_intake(
                            opts.repo_root.as_path(),
                            &evt,
                            opts.publisher_mesh_intake_enabled,
                        );
                    }
                }
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    tracing::debug!(
                        target: "vox_orchestrator::scientia_mesh",
                        skipped,
                        "scientia_mesh_research_events_lagged"
                    );
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}
