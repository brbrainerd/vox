use crate::orchestrator::{Orchestrator, OrchestratorError};
use crate::types::AgentId;
use std::path::PathBuf;

impl Orchestrator {
    pub async fn apply_vox_healing(
        &self,
        agent_id: AgentId,
        task_desc: &str,
        write_files: &[PathBuf],
    ) -> Result<(), OrchestratorError> {
        #[cfg(feature = "runtime")]
        {
            let vox_files: Vec<PathBuf> = write_files
                .iter()
                .filter(|p| p.extension().is_some_and(|ext| ext == "vox"))
                .cloned()
                .collect();

            for vox_file in vox_files {
                if let Ok(original_source) = vox_bounded_fs::read_utf8_path_capped(&vox_file) {
                    let loop_ = vox_populi::mens::healing::HealingLoop::new(
                        3,
                        |src| {
                            let diagnostics = vox_lsp::validate_document(src);
                            let errors: Vec<_> = diagnostics
                                .into_iter()
                                .filter(|d| {
                                    matches!(
                                        d.severity,
                                        Some(tower_lsp_server::ls_types::DiagnosticSeverity::ERROR)
                                    )
                                })
                                .map(|d| d.message)
                                .collect();
                            vox_populi::mens::healing::HealResult {
                                ok: errors.is_empty(),
                                diagnostics: errors,
                            }
                        },
                        |src, errs| {
                            tokio::task::block_in_place(|| {
                                tokio::runtime::Handle::current().block_on(async {
                                    let prompt = format!(
                                        "Fix these Vox compiler errors:\n{}\n\nCode:\n```vox\n{}\n```\n\nReturn only the fixed code, no extra text.",
                                        errs.join("\n"), src
                                    );
                                    let client = vox_gamify::ai::FreeAiClient::auto_discover().await;
                                    let raw = client.generate(&prompt).await.unwrap_or_default();

                                    let mut cleaned = raw;
                                    if cleaned.starts_with("```vox") {
                                        cleaned = cleaned.trim_start_matches("```vox").trim_end_matches("```").trim().to_string();
                                    } else if cleaned.starts_with("```") {
                                        cleaned = cleaned.trim_start_matches("```").trim_end_matches("```").trim().to_string();
                                    }
                                    Ok(cleaned)
                                })
                            })
                        },
                    );
                    if let vox_populi::mens::healing::HealOutcome::Success { source, .. } =
                        loop_.heal(task_desc, &original_source).await
                    {
                        let _ = std::fs::write(&vox_file, &source);
                        self.event_bus()
                            .emit(crate::events::AgentEventKind::AutoHealApplied {
                                agent_id,
                                path: vox_file,
                                description: "Automated AST healing fixed compiler errors."
                                    .to_string(),
                                new_source: source,
                            });
                    }
                }
            }
        }
        Ok(())
    }
}
