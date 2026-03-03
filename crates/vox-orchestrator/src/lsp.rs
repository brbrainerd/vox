use std::path::Path;
use tower_lsp_server::ls_types::{Diagnostic, DiagnosticSeverity, Position, Range};

use crate::locks::LockKind;
use crate::orchestrator::Orchestrator;

/// Exposes orchestrator file-locking and ownership status as LSP diagnostics.
pub struct OrchestratorDiagnosticProvider;

impl OrchestratorDiagnosticProvider {
    /// Generate an informational diagnostic if a file is currently locked by an agent.
    pub fn get_diagnostics(orchestrator: &Orchestrator, path: &Path) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Convert path to canonical to match lock map
        let canonical_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

        if let Some((agent_id, kind)) = orchestrator.lock_manager().holder(&canonical_path) {
            let message = match kind {
                LockKind::Exclusive => {
                    format!("File is currently exclusively locked by agent {}", agent_id)
                }
                LockKind::SharedRead => {
                    format!("File has shared read lock by agent {}", agent_id)
                }
            };

            let range = Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 0,
                },
            };

            diagnostics.push(Diagnostic {
                range,
                severity: Some(DiagnosticSeverity::INFORMATION),
                code: None,
                code_description: None,
                source: Some("vox-orchestrator".to_string()),
                message,
                related_information: None,
                tags: None,
                data: None,
            });
        }

        // Also indicate affinity owner if any
        if let Some(agent_id) = orchestrator.affinity_map().lookup(&canonical_path) {
            let range = Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 0,
                },
            };

            diagnostics.push(Diagnostic {
                range,
                severity: Some(DiagnosticSeverity::HINT),
                code: None,
                code_description: None,
                source: Some("vox-orchestrator".to_string()),
                message: format!("Agent {} owns this file's affinity", agent_id),
                related_information: None,
                tags: None,
                data: None,
            });
        }

        diagnostics
    }
}
