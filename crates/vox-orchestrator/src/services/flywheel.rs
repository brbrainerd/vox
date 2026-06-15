use crate::Orchestrator;
use crate::process_util::quiet_tokio_command;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::interval;
use vox_corpus::flywheel::{FlywheelConfig, FlywheelSignal, FlywheelState};

/// Background service that monitors corpus diversity and triggers training.
pub struct FlywheelMonitor {
    orch: Arc<Orchestrator>,
    states: HashMap<String, FlywheelState>,
}

impl FlywheelMonitor {
    pub fn new(orch: Arc<Orchestrator>) -> Self {
        let mut states = HashMap::new();

        // Load default config
        let base_config = FlywheelConfig {
            sample_floor: 500, // Reduced from 1000 to match flywheel.yaml defaults
            min_ast_diversity: 0.40,
            auto_train: true,
            ..Default::default()
        };

        // Initialize tracked domains
        // Ideally we'd load this from domain-profiles.yaml, but for now we'll
        // hardcode the three core domains requested.
        let domains = vec!["vox-lang", "rust-expert", "agents"];
        for domain in domains {
            states.insert(domain.to_string(), FlywheelState::new(base_config.clone()));
        }

        Self { orch, states }
    }

    pub async fn spawn(self) {
        let mut tick = interval(vox_config::timeouts::D_300S); // Check every 5 minutes
        let orch = self.orch.clone();
        let training_in_progress = Arc::new(tokio::sync::Mutex::new(false));
        let mut states = self.states;

        tokio::spawn(async move {
            loop {
                tick.tick().await;

                let domains: Vec<String> = states.keys().cloned().collect();

                for domain in domains {
                    // 1. Gather current metrics from Codex for this specific domain
                    let (current_samples, current_diversity) = {
                        if let Some(db) = orch.db() {
                            let session_id = format!("corpus_diversity_check:{}", domain);

                            // Query diversity
                            let diversity_metrics = db
                                .list_research_metrics_by_type("ast_diversity", &session_id, 1)
                                .await
                                .ok();
                            let diversity = diversity_metrics
                                .and_then(|m| m.first().and_then(|(_, val, _)| *val))
                                .unwrap_or(0.0);

                            // Query sample count (recorded by our updated diversity-check)
                            let count_metrics = db
                                .list_research_metrics_by_type(
                                    "corpus_sample_count",
                                    &session_id,
                                    1,
                                )
                                .await
                                .ok();
                            let count = count_metrics
                                .and_then(|m| m.first().and_then(|(_, val, _)| *val))
                                .unwrap_or(0.0) as usize;

                            (count, diversity)
                        } else {
                            (0, 0.0)
                        }
                    };

                    let flywheel = states.get_mut(&domain).unwrap();

                    // 2. Evaluate
                    match flywheel.check(current_samples, current_diversity) {
                        FlywheelSignal::Ready { ast_diversity } => {
                            tracing::info!(domain = %domain, current_samples, ast_diversity, "Flywheel: Training gate PASSED. Ready for wave.");

                            // 3. Trigger training if auto_train is true and not already running
                            if flywheel.config.auto_train {
                                let training_in_progress = training_in_progress.clone();
                                let in_progress = training_in_progress.lock().await;
                                if !*in_progress {
                                    tracing::info!(domain = %domain, "Flywheel: Auto-dispatching autonomous training wave...");
                                    drop(in_progress);

                                    let training_flag = training_in_progress.clone();
                                    let domain_to_train = domain.clone();

                                    tokio::spawn(async move {
                                        {
                                            let mut g = training_flag.lock().await;
                                            *g = true;
                                        }
                                        let res =
                                            trigger_autonomous_training(&domain_to_train).await;
                                        if let Err(e) = res {
                                            tracing::error!(domain = %domain_to_train, error = %e, "Flywheel: Autonomous training wave FAILED");
                                        } else {
                                            tracing::info!(domain = %domain_to_train, "Flywheel: Autonomous training wave COMPLETED successfully");
                                        }
                                        let mut g = training_flag.lock().await;
                                        *g = false;
                                    });
                                } else {
                                    tracing::debug!(domain = %domain, "Flywheel: Training already in progress, skipping trigger.");
                                }
                            }
                        }
                        FlywheelSignal::Pending { new_samples } => {
                            tracing::debug!(domain = %domain, new_samples, "Flywheel: Accumulating samples...");
                        }
                        _ => {
                            tracing::debug!(domain = %domain, "Flywheel: Idle (waiting for diversity signal/samples)");
                        }
                    }
                }
            }
        });
    }
}

async fn trigger_autonomous_training(domain: &str) -> anyhow::Result<()> {
    tracing::info!(domain = %domain, "Starting autonomous training subprocess...");

    let status = quiet_tokio_command("pwsh")
        .arg("-NonInteractive")
        .arg("-NoProfile")
        .arg("-File")
        .arg("scripts/mens-full-pipeline.ps1")
        .arg("-Domain")
        .arg(domain)
        .status()
        .await?;

    if !status.success() {
        anyhow::bail!(
            "Autonomous training subprocess for domain '{}' exited with code {:?}",
            domain,
            status.code()
        );
    }
    Ok(())
}
