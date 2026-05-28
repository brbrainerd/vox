//! `vox scientia` — Vox Scientia facade over Codex research and capability-map tools.
//!
//! Delegates to `super::db_cli::DbCli` so `vox db …` remains the implementation SSOT.

pub use vox_cli_core::scientia::ScientiaCmd;

/// Dispatch `vox scientia` to the shared `vox db` handlers.
pub async fn run(cmd: ScientiaCmd) -> anyhow::Result<()> {
    use super::ci::repo_root;
    use super::db_cli::{self, DbCli, DbCliCore, DbCliPublication};
    use super::scientia_ledger_contract;

    match cmd {
        ScientiaCmd::FindingCandidateValidate { json } => {
            let root = repo_root();
            let path = if json.is_absolute() {
                json
            } else {
                std::env::current_dir()?.join(json)
            };
            scientia_ledger_contract::validate_finding_candidate_file(&root, &path)?;
            Ok(())
        }
        ScientiaCmd::NoveltyEvidenceBundleValidate { json } => {
            let root = repo_root();
            let path = if json.is_absolute() {
                json
            } else {
                std::env::current_dir()?.join(json)
            };
            scientia_ledger_contract::validate_novelty_bundle_file(&root, &path)?;
            Ok(())
        }
        cmd => {
            let db_cmd = match cmd {
                ScientiaCmd::FindingCandidateValidate { .. }
                | ScientiaCmd::NoveltyEvidenceBundleValidate { .. } => unreachable!(
                    "finding-candidate-validate and novelty-evidence-bundle-validate handled above"
                ),
                ScientiaCmd::CapabilityList => DbCli::Core(DbCliCore::CapabilityList),
                ScientiaCmd::ResearchList {
                    vendor,
                    topic,
                    limit,
                } => DbCli::Core(DbCliCore::ResearchList {
                    vendor,
                    topic,
                    limit,
                }),
                ScientiaCmd::ResearchMapList {
                    vendor,
                    topic,
                    limit,
                } => DbCli::Core(DbCliCore::ResearchMapList {
                    vendor,
                    topic,
                    limit,
                }),
                ScientiaCmd::RetrievalStatus => DbCli::Core(DbCliCore::RetrievalStatus),
                ScientiaCmd::MirrorSearchCorpus {
                    root,
                    source_uri_prefix,
                } => DbCli::Core(DbCliCore::MirrorSearchCorpus {
                    root,
                    source_uri_prefix,
                }),
                ScientiaCmd::ResearchRefresh { vendor, dry_run } => {
                    DbCli::Core(DbCliCore::ResearchRefresh { vendor, dry_run })
                }
                ScientiaCmd::PublicationPrepare {
                    body,
                    preflight,
                    preflight_profile,
                    discovery_intake_gate,
                } => DbCli::Publication(DbCliPublication::PublicationPrepare {
                    content_type: "scientia".to_string(),
                    body,
                    preflight,
                    preflight_profile,
                    discovery_intake_gate,
                }),
                ScientiaCmd::PublicationPrepareValidated {
                    body,
                    preflight_profile,
                    discovery_intake_gate,
                } => DbCli::Publication(DbCliPublication::PublicationPrepareValidated {
                    content_type: "scientia".to_string(),
                    body,
                    preflight_profile,
                    discovery_intake_gate,
                }),
                ScientiaCmd::PublicationPreflight {
                    publication_id,
                    profile,
                    with_worthiness,
                } => DbCli::Publication(DbCliPublication::PublicationPreflight {
                    publication_id,
                    profile,
                    with_worthiness,
                }),
                ScientiaCmd::PublicationZenodoMetadata { publication_id } => {
                    DbCli::Publication(DbCliPublication::PublicationZenodoMetadata {
                        publication_id,
                    })
                }
                ScientiaCmd::PublicationOpenreviewProfile { publication_id } => {
                    DbCli::Publication(DbCliPublication::PublicationOpenreviewProfile {
                        publication_id,
                    })
                }
                ScientiaCmd::PublicationScholarlyStagingExport {
                    publication_id,
                    output_dir,
                    venue,
                } => DbCli::Publication(DbCliPublication::PublicationScholarlyStagingExport {
                    publication_id,
                    output_dir,
                    venue,
                }),
                ScientiaCmd::PublicationWorthinessEvaluate {
                    contract_yaml,
                    metrics_json,
                } => DbCli::Publication(DbCliPublication::PublicationWorthinessEvaluate {
                    contract_yaml,
                    metrics_json,
                }),
                ScientiaCmd::PublicationApprove {
                    publication_id,
                    approver,
                } => DbCli::Publication(DbCliPublication::PublicationApprove {
                    publication_id,
                    approver,
                }),
                ScientiaCmd::PublicationSubmitLocal {
                    publication_id,
                    adapter,
                } => DbCli::Publication(DbCliPublication::PublicationSubmitLocal {
                    publication_id,
                    adapter,
                }),
                ScientiaCmd::PublicationStatus {
                    publication_id,
                    with_worthiness,
                } => DbCli::Publication(DbCliPublication::PublicationStatus {
                    publication_id,
                    with_worthiness,
                }),
                ScientiaCmd::PublicationDiscoveryScan { state, limit } => {
                    DbCli::Publication(DbCliPublication::PublicationDiscoveryScan {
                        content_type: Some("scientia".to_string()),
                        state,
                        limit,
                    })
                }
                ScientiaCmd::PublicationDiscoveryPublishRss {
                    feed_path,
                    limit,
                    json,
                } => DbCli::Publication(DbCliPublication::PublicationDiscoveryPublishRss {
                    // Always scope to `scientia` content type from the `vox scientia` surface.
                    content_type: Some("scientia".to_string()),
                    feed_path,
                    limit,
                    json,
                }),
                ScientiaCmd::PublicationDiscoveryExplain { publication_id } => {
                    DbCli::Publication(DbCliPublication::PublicationDiscoveryExplain {
                        publication_id,
                    })
                }
                ScientiaCmd::PublicationTransformPreview { publication_id } => {
                    DbCli::Publication(DbCliPublication::PublicationTransformPreview {
                        publication_id,
                    })
                }
                ScientiaCmd::PublicationNoveltyFetch {
                    publication_id,
                    offline,
                    persist_metadata,
                } => DbCli::Publication(DbCliPublication::PublicationNoveltyFetch {
                    publication_id,
                    offline,
                    persist_metadata,
                }),
                ScientiaCmd::PublicationDecisionExplain {
                    publication_id,
                    live_prior_art,
                    offline,
                } => DbCli::Publication(DbCliPublication::PublicationDecisionExplain {
                    publication_id,
                    live_prior_art,
                    offline,
                }),
                ScientiaCmd::PublicationNoveltyHappyPath {
                    publication_id,
                    offline,
                } => DbCli::Publication(DbCliPublication::PublicationNoveltyHappyPath {
                    publication_id,
                    offline,
                }),
                ScientiaCmd::PublicationScholarlyRemoteStatus {
                    publication_id,
                    external_submission_id,
                } => DbCli::Publication(DbCliPublication::PublicationScholarlyRemoteStatus {
                    publication_id,
                    external_submission_id,
                }),
                ScientiaCmd::PublicationScholarlyRemoteStatusSyncAll { publication_id } => {
                    DbCli::Publication(DbCliPublication::PublicationScholarlyRemoteStatusSyncAll {
                        publication_id,
                    })
                }
                ScientiaCmd::PublicationScholarlyRemoteStatusSyncBatch {
                    limit,
                    iterations,
                    interval_secs,
                    max_runtime_secs,
                    jitter_secs,
                } => DbCli::Publication(
                    DbCliPublication::PublicationScholarlyRemoteStatusSyncBatch {
                        limit,
                        iterations,
                        interval_secs,
                        max_runtime_secs,
                        jitter_secs,
                    },
                ),
                ScientiaCmd::PublicationArxivHandoffRecord {
                    publication_id,
                    stage,
                    operator,
                    note,
                    arxiv_id,
                } => DbCli::Publication(DbCliPublication::PublicationArxivHandoffRecord {
                    publication_id,
                    stage,
                    operator,
                    note,
                    arxiv_id,
                }),
                ScientiaCmd::PublicationExternalJobsDue { limit } => {
                    DbCli::Publication(DbCliPublication::PublicationExternalJobsDue { limit })
                }
                ScientiaCmd::PublicationExternalJobsDeadLetter { limit } => {
                    DbCli::Publication(DbCliPublication::PublicationExternalJobsDeadLetter {
                        limit,
                    })
                }
                ScientiaCmd::PublicationExternalJobsReplay { job_id } => {
                    DbCli::Publication(DbCliPublication::PublicationExternalJobsReplay { job_id })
                }
                ScientiaCmd::PublicationExternalJobsTick {
                    limit,
                    lock_ttl_ms,
                    lock_owner,
                    iterations,
                    interval_secs,
                    max_runtime_secs,
                    jitter_secs,
                } => DbCli::Publication(DbCliPublication::PublicationExternalJobsTick {
                    limit,
                    lock_ttl_ms,
                    lock_owner,
                    iterations,
                    interval_secs,
                    max_runtime_secs,
                    jitter_secs,
                }),
                ScientiaCmd::PublicationScholarlyPipelineRun {
                    publication_id,
                    preflight_profile,
                    dry_run,
                    staging_output_dir,
                    venue,
                    adapter,
                    json,
                } => DbCli::Publication(DbCliPublication::PublicationScholarlyPipelineRun {
                    publication_id,
                    preflight_profile,
                    dry_run,
                    staging_output_dir,
                    venue,
                    adapter,
                    json,
                }),
                ScientiaCmd::PublicationExternalPipelineMetrics { since_hours } => {
                    DbCli::Publication(DbCliPublication::PublicationExternalPipelineMetrics {
                        since_hours,
                    })
                }
                ScientiaCmd::IngestTick { feed_id, limit } => {
                    DbCli::Publication(DbCliPublication::IngestTick { feed_id, limit })
                }
                ScientiaCmd::FeedSourceAdd {
                    id,
                    url,
                    kind,
                    interval_ms,
                } => DbCli::Publication(DbCliPublication::FeedSourceAdd {
                    id,
                    url,
                    kind,
                    interval_ms,
                }),
                ScientiaCmd::FeedSourceList => DbCli::Publication(DbCliPublication::FeedSourceList),
                ScientiaCmd::Diagnose { live } => {
                    return diagnose_adapters(live).await;
                }
                ScientiaCmd::Scout {
                    commit_window,
                    days_window,
                    output,
                    candidate_class,
                } => {
                    return super::scout::run(
                        commit_window,
                        days_window,
                        output,
                        candidate_class,
                    )
                    .await;
                }
                ScientiaCmd::PublicationReplayExecute {
                    main_entity,
                    stage_dir,
                } => {
                    return super::scientia_phase_handlers::replay_execute(
                        &main_entity,
                        &stage_dir,
                    )
                    .await;
                }
                ScientiaCmd::PublicationManuscriptDraft { scaffold } => {
                    return super::scientia_phase_handlers::manuscript_draft(&scaffold);
                }
                ScientiaCmd::PublicationCriticGateCheck { inputs } => {
                    return super::scientia_phase_handlers::critic_gate_check(&inputs);
                }
                ScientiaCmd::PublicationCriticApprove {
                    publication_id,
                    critic_id,
                    inputs,
                } => {
                    return super::scientia_phase_handlers::critic_approve(
                        &publication_id,
                        &critic_id,
                        &inputs,
                    )
                    .await;
                }
                ScientiaCmd::PublicationExtractClaims { publication_id } => {
                    return super::scientia_phase_handlers::publication_extract_claims(
                        &publication_id,
                    )
                    .await;
                }
                ScientiaCmd::PublicationRenderLatex { scaffold, output } => {
                    return super::scientia_phase_handlers::render_latex_handler(
                        &scaffold,
                        output.as_deref(),
                    );
                }
                ScientiaCmd::PublicationArxivBundle {
                    scaffold,
                    figures_dir,
                    output,
                } => {
                    return super::scientia_phase_handlers::arxiv_bundle_handler(
                        &scaffold,
                        &figures_dir,
                        &output,
                    );
                }
                ScientiaCmd::PublicationVenueRecommend {
                    candidate_class,
                    yaml_config,
                } => {
                    return super::scientia_phase_handlers::venue_recommend(
                        &candidate_class,
                        yaml_config.as_deref(),
                    );
                }
                ScientiaCmd::PublicationFindingPageRender { page } => {
                    return super::scientia_phase_handlers::finding_page_render(&page);
                }
                ScientiaCmd::PublicationDashboardSnapshot { inputs } => {
                    return super::scientia_phase_handlers::dashboard_snapshot(&inputs);
                }
            };
            db_cli::run(db_cmd).await
        }
    }
}

async fn diagnose_adapters(live: bool) -> anyhow::Result<()> {
    use vox_publisher::PublisherConfig;
    use vox_publisher::adapter_health::report_health;

    let site = vox_publisher::contract::NewsSiteConfig {
        base_url: "https://vox.foundation".to_string(),
        rss_feed_path: std::path::PathBuf::from("feed.xml"),
    };

    let cfg = PublisherConfig::from_operator_environment(false, None, site);
    let report = report_health(&cfg, live).await?;

    println!("Vox Scientia Publication Pipeline Health Report");
    println!("===============================================");
    println!();

    // Simple table-like output without requiring comfy-table if not present
    println!(
        "{:<20} {:<10} {:<15} Heartbeat",
        "Channel", "Feature", "Credentials"
    );
    println!("{:-<20} {:-<10} {:-<15} {:-<15}", "", "", "", "");

    for adapter in report.adapters {
        let feature = if adapter.feature_enabled {
            "ENABLED"
        } else {
            "DISABLED"
        };
        let creds = if adapter.credentials_present {
            "PRESENT"
        } else {
            "MISSING"
        };
        let heartbeat = match adapter.heartbeat_status {
            Some(s) => format!("{:?}", s),
            None => "N/A".to_string(),
        };

        println!(
            "{:<20} {:<10} {:<15} {}",
            adapter.name, feature, creds, heartbeat
        );
        if let Some(msg) = adapter.diagnostic_message {
            println!("  ! {}", msg);
        }
    }

    Ok(())
}
