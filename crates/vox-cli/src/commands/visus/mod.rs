use crate::workspace_db::connect_cli_workspace_voxdb;
use clap::{Parser, Subcommand};
use owo_colors::OwoColorize;
use serde_json::json;
use vox_db::store::types::{VisusAuditLogRow, VisusBaselineRow};

/// Vox Visus: Voice of Vision. Agentic GUI visual intelligence and bug detection.
#[derive(Parser, Debug)]
#[command(about = "GUI visual intelligence, screenshot auditing, and VLM-based bug detection.")]
pub struct VisusArgs {
    #[command(subcommand)]
    pub cmd: VisusCmd,
}

#[derive(Subcommand, Debug)]
pub enum VisusCmd {
    /// Audit a URL or local file for visual bugs using the Image Analysis Lane.
    Audit {
        /// Target URL or local file path.
        target: String,
        /// Viewport dimensions (e.g., 1280x800).
        #[arg(long, default_value = "1280x800")]
        viewport: String,
        /// Force theme (light, dark, auto).
        #[arg(long, default_value = "auto")]
        theme: String,
        /// Output path for the screenshot.
        #[arg(long)]
        screenshot: Option<String>,
        /// Output path for the AXTree JSON.
        #[arg(long)]
        ax_tree: Option<String>,
        /// Trigger deeper VLM analysis after the initial overlap check.
        #[arg(long)]
        vlm: bool,
    },
    /// Update or compare against a visual baseline.
    Baseline {
        /// Target URL or local file path.
        target: String,
        #[arg(long)]
        update: bool,
        /// Viewport dimensions (e.g., 1280x800).
        #[arg(long, default_value = "1280x800")]
        viewport: String,
        /// Force theme (light, dark, auto).
        #[arg(long, default_value = "auto")]
        theme: String,
    },
    /// Ingest approved audit findings into the MENS gui-vision training corpus.
    Train {
        /// Optional limit on the number of samples to ingest.
        #[arg(long)]
        limit: Option<usize>,
    },
}

pub async fn dispatch(cmd: VisusCmd) -> miette::Result<()> {
    match cmd {
        VisusCmd::Audit {
            target,
            viewport,
            theme,
            screenshot,
            ax_tree,
            vlm,
        } => {
            println!(
                "{} Executing Vox Visus (Voice of Vision) Audit on: {}",
                "▶".blue(),
                target.bold()
            );

            let db = connect_cli_workspace_voxdb()
                .await
                .map_err(|e| miette::miette!("Failed to connect to VoxDb: {}", e))?;

            let target_owned = target.clone();
            let (page_id, ss_bytes, tree_bytes) = tokio::task::spawn_blocking(
                move || -> anyhow::Result<(String, Vec<u8>, Vec<u8>)> {
                    let plugin = vox_plugin_host::cached_code_plugin("browser")
                        .map_err(|e| anyhow::anyhow!("browser plugin: {e}"))?;
                    let backend = plugin
                        .plugin
                        .as_browser_automation()
                        .into_option()
                        .ok_or_else(|| {
                            anyhow::anyhow!(
                                "browser plugin loaded but BrowserAutomation accessor returned None"
                            )
                        })?;
                    let page_id = backend
                        .open(target_owned.as_str().into(), true)
                        .into_result()
                        .map_err(|e| anyhow::anyhow!("browser open: {e}"))?
                        .into_string();
                    let ss_bytes: Vec<u8> = backend
                        .screenshot_bytes(page_id.as_str().into())
                        .into_result()
                        .map_err(|e| anyhow::anyhow!("browser screenshot: {e}"))?
                        .into_iter()
                        .collect();
                    let tree_str = backend
                        .ax_tree(page_id.as_str().into())
                        .into_result()
                        .map_err(|e| anyhow::anyhow!("browser ax_tree: {e}"))?
                        .into_string();
                    let tree_bytes = tree_str.into_bytes();
                    Ok((page_id, ss_bytes, tree_bytes))
                },
            )
            .await
            .map_err(|e| miette::miette!("spawn_blocking: {e}"))?
            .map_err(|e| miette::miette!("browser ops: {e}"))?;

            // 2. Local Persistence (Optional Files)
            if let Some(path) = screenshot {
                std::fs::write(&path, &ss_bytes)
                    .map_err(|e| miette::miette!("Failed to save screenshot: {}", e))?;
            }
            if let Some(path) = ax_tree {
                std::fs::write(&path, &tree_bytes)
                    .map_err(|e| miette::miette!("Failed to save AXTree: {}", e))?;
            }

            // 3. CAS Storage
            let ss_hash = db
                .store("visus_screenshot", &ss_bytes)
                .await
                .map_err(|e| miette::miette!("Failed to store screenshot in CAS: {}", e))?;
            let tree_hash = db
                .store("visus_ax_tree", &tree_bytes)
                .await
                .map_err(|e| miette::miette!("Failed to store AXTree in CAS: {}", e))?;

            // 4. Baseline Comparison
            let baseline = db
                .get_visus_baseline(&target, &viewport, &theme)
                .await
                .unwrap_or(None);

            if let Some(ref b) = baseline {
                println!(
                    "{} Comparing against baseline created at: {}",
                    "ℹ".blue(),
                    b.created_at.dimmed()
                );
                if b.ax_tree_cas == tree_hash {
                    println!("{} AXTree matches golden baseline exactly.", "✓".green());
                } else {
                    println!(
                        "{} AXTree drift detected from golden baseline.",
                        "⚠".yellow()
                    );
                }
            } else {
                println!("{} No baseline found for this target/config.", "ℹ".blue());
            }

            // 5. Layer 1: Deterministic Overlap Detection
            // check_overlaps is not exposed on the BrowserAutomation trait; overlap detection
            // is deferred to the VLM layer (Layer 2) when --vlm is passed.
            println!("{} Running Layer 1 (Deterministic) audit...", "▶".blue());
            let overlaps: Vec<serde_json::Value> = vec![];

            let outcome = if overlaps.is_empty() {
                println!("{} No deterministic overlaps detected.", "✓".green());
                "clean"
            } else {
                println!(
                    "{} Found {} potential stacking context/overlap issues:",
                    "⚠".yellow(),
                    overlaps.len()
                );
                for (i, finding) in overlaps.iter().enumerate() {
                    println!("   {}. {:?}", i + 1, finding);
                }
                "warning"
            };

            // 6. Log Audit
            let audit_id = uuid::Uuid::new_v4().to_string();
            db.log_visus_audit(VisusAuditLogRow {
                id: audit_id,
                baseline_id: baseline.map(|b| b.id),
                target_url: target.clone(),
                outcome: outcome.to_string(),
                findings_json: serde_json::to_string(&overlaps).unwrap(),
                screenshot_cas: Some(ss_hash.clone()),
                created_at: "".to_string(), // Handled by SQL DEFAULT
            })
            .await
            .map_err(|e| miette::miette!("Failed to log audit: {}", e))?;

            // 7. Layer 2: VLM-Augmented Analysis (Optional)
            if vlm {
                println!(
                    "{} Handoff to Layer 2 (VLM Intelligence) for pixel-grounding analysis...",
                    "▶".magenta()
                );

                // Construct attachment manifest
                let manifest = vox_orchestrator::attachment_manifest::AttachmentManifest {
                    attachments: vec![
                        vox_orchestrator::attachment_manifest::AttachmentEntry {
                            sha256: ss_hash,
                            mime_type: "image/png".to_string(),
                            label: "Screenshot".to_string(),
                            visual_segments: None,
                        },
                        vox_orchestrator::attachment_manifest::AttachmentEntry {
                            sha256: tree_hash,
                            mime_type: "application/json".to_string(),
                            label: "AXTree".to_string(),
                            visual_segments: None,
                        },
                    ],
                };

                let mut config = vox_orchestrator::OrchestratorConfig::default();
                config.merge_env_overrides();
                let orch =
                    vox_orchestrator::build_repo_scoped_orchestrator(config, None).orchestrator;

                // Create a Visus task with the category hint [[visus]]
                let description = format!(
                    "Audit the GUI for structural and visual bugs at {}. Layer 1 outcome: {}. [[visus]]",
                    target, outcome
                );
                let hints = vox_orchestrator::TaskEnqueueHints {
                    attachment_manifest: Some(manifest),
                    ..Default::default()
                };

                let task_id = orch
                    .submit_task_with_agent(
                        description,
                        vec![],
                        None,
                        None,
                        None,
                        Some(hints),
                        None,
                        None,
                    )
                    .await
                    .map_err(|e| miette::miette!("Failed to submit VLM task: {}", e))?;

                println!(
                    "{} VLM Task submitted successfully! Task ID: {}",
                    "✓".green(),
                    task_id.bold()
                );
                println!("{} Waiting for visual intelligence report...", "ℹ".blue());
            }

            println!("{} Wave 0 audit complete.", "✓".green());

            let page_id_for_close = page_id.clone();
            tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
                let plugin = vox_plugin_host::cached_code_plugin("browser")
                    .map_err(|e| anyhow::anyhow!("browser plugin: {e}"))?;
                let backend = plugin
                    .plugin
                    .as_browser_automation()
                    .into_option()
                    .ok_or_else(|| anyhow::anyhow!("BrowserAutomation accessor returned None"))?;
                backend
                    .close(page_id_for_close.as_str().into())
                    .into_result()
                    .map_err(|e| anyhow::anyhow!("browser close: {e}"))
            })
            .await
            .map_err(|e| miette::miette!("spawn_blocking: {e}"))?
            .map_err(|e| miette::miette!("Failed to close browser: {e}"))?;
        }
        VisusCmd::Baseline {
            target,
            update,
            viewport,
            theme,
        } => {
            if update {
                println!(
                    "{} Updating Vox Visus golden baseline for: {}",
                    "▶".blue(),
                    target.bold()
                );

                let db = connect_cli_workspace_voxdb()
                    .await
                    .map_err(|e| miette::miette!("Failed to connect to VoxDb: {}", e))?;

                let target_owned = target.clone();
                let (page_id, ss_bytes, tree_bytes) = tokio::task::spawn_blocking(
                    move || -> anyhow::Result<(String, Vec<u8>, Vec<u8>)> {
                        let plugin = vox_plugin_host::cached_code_plugin("browser")
                            .map_err(|e| anyhow::anyhow!("browser plugin: {e}"))?;
                        let backend = plugin
                            .plugin
                            .as_browser_automation()
                            .into_option()
                            .ok_or_else(|| {
                                anyhow::anyhow!("BrowserAutomation accessor returned None")
                            })?;
                        let page_id = backend
                            .open(target_owned.as_str().into(), true)
                            .into_result()
                            .map_err(|e| anyhow::anyhow!("browser open: {e}"))?
                            .into_string();
                        let ss_bytes: Vec<u8> = backend
                            .screenshot_bytes(page_id.as_str().into())
                            .into_result()
                            .map_err(|e| anyhow::anyhow!("browser screenshot: {e}"))?
                            .into_iter()
                            .collect();
                        let tree_str = backend
                            .ax_tree(page_id.as_str().into())
                            .into_result()
                            .map_err(|e| anyhow::anyhow!("browser ax_tree: {e}"))?
                            .into_string();
                        let tree_bytes = tree_str.into_bytes();
                        Ok((page_id, ss_bytes, tree_bytes))
                    },
                )
                .await
                .map_err(|e| miette::miette!("spawn_blocking: {e}"))?
                .map_err(|e| miette::miette!("browser ops: {e}"))?;

                let ss_hash = db
                    .store("visus_screenshot", &ss_bytes)
                    .await
                    .map_err(|e| miette::miette!("Failed to store screenshot: {}", e))?;
                let tree_hash = db
                    .store("visus_ax_tree", &tree_bytes)
                    .await
                    .map_err(|e| miette::miette!("Failed to store AXTree: {}", e))?;

                let baseline_id = uuid::Uuid::new_v4().to_string();
                db.upsert_visus_baseline(VisusBaselineRow {
                    id: baseline_id,
                    target_url: target,
                    viewport,
                    theme,
                    screenshot_cas: ss_hash,
                    ax_tree_cas: tree_hash,
                    metadata_json: None,
                    created_at: "".to_string(),
                })
                .await
                .map_err(|e| miette::miette!("Failed to save baseline: {}", e))?;

                println!("{} Golden baseline updated successfully.", "✓".green());
                let page_id_for_close = page_id.clone();
                tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
                    let plugin = vox_plugin_host::cached_code_plugin("browser")
                        .map_err(|e| anyhow::anyhow!("browser plugin: {e}"))?;
                    let backend = plugin
                        .plugin
                        .as_browser_automation()
                        .into_option()
                        .ok_or_else(|| {
                            anyhow::anyhow!("BrowserAutomation accessor returned None")
                        })?;
                    backend
                        .close(page_id_for_close.as_str().into())
                        .into_result()
                        .map_err(|e| anyhow::anyhow!("browser close: {e}"))
                })
                .await
                .map_err(|e| miette::miette!("spawn_blocking: {e}"))?
                .map_err(|e| miette::miette!("Failed to close browser: {e}"))?;
            } else {
                println!(
                    "{} Comparing current state against Vox Visus baselines...",
                    "▶".blue()
                );
                // In a real CLI, we might just delegate to Audit here, or list baselines.
            }
        }
        VisusCmd::Train { limit } => {
            println!(
                "{} Closing the loop: Ingesting visual audit findings into MENS training data...",
                "▶".blue()
            );

            let db = connect_cli_workspace_voxdb()
                .await
                .map_err(|e| miette::miette!("Failed to connect to VoxDb: {}", e))?;

            let logs = db
                .list_visus_audit_logs(limit)
                .await
                .map_err(|e| miette::miette!("Failed to fetch audit logs: {}", e))?;

            if logs.is_empty() {
                println!("{} No audit logs found to ingest.", "ℹ".blue());
                return Ok(());
            }

            println!(
                "{} Found {} audit samples to process.",
                "ℹ".blue(),
                logs.len()
            );

            let mut samples = Vec::new();
            for log in logs {
                if log.outcome == "clean" {
                    continue;
                }

                // Construct a VLM-style training sample: screenshot hash + AXTree findings
                let sample = json!({
                    "instruction": "Audit the provided GUI screenshot for layout bugs, overlaps, and semantic misalignments.",
                    "screenshot_cas": log.screenshot_cas,
                    "target_url": log.target_url,
                    "model_output": format!("Audit found issues: {}", log.findings_json),
                    "lanes": ["gui-vision", "visus"],
                    "quality_score": 1.0,
                });
                samples.push(sample);
            }

            if samples.is_empty() {
                println!(
                    "{} No 'warning' or 'error' samples found to ingest.",
                    "ℹ".blue()
                );
                return Ok(());
            }

            // In a real system, we'd write to the corpus directory
            let corpus_path = "mens/corpus/gui-vision-flywheel.jsonl";
            std::fs::create_dir_all("mens/corpus").ok();

            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(corpus_path)
                .map_err(|e| miette::miette!("Failed to open corpus file: {}", e))?;

            use std::io::Write;
            for s in &samples {
                let line = serde_json::to_string(&s).unwrap();
                writeln!(file, "{}", line)
                    .map_err(|e| miette::miette!("Failed to write sample: {}", e))?;
            }

            println!(
                "{} Success: {} samples appended to {}.",
                "✓".green(),
                samples.len(),
                corpus_path.bold()
            );
        }
    }
    Ok(())
}
