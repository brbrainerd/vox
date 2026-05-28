//! LSP server capability advertisement (shared between the binary and integration tests).

use tower_lsp_server::ls_types::*;

/// Capabilities returned from `initialize` — keep in sync with [`crate`] request handlers.
#[must_use]
pub fn server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(
            TextDocumentSyncKind::FULL,
        )),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        semantic_tokens_provider: Some(
            SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions(
                SemanticTokensRegistrationOptions {
                    text_document_registration_options: TextDocumentRegistrationOptions {
                        document_selector: Some(vec![DocumentFilter {
                            language: Some("vox".to_string()),
                            scheme: Some("file".to_string()),
                            pattern: None,
                        }]),
                    },
                    semantic_tokens_options: SemanticTokensOptions {
                        work_done_progress_options: WorkDoneProgressOptions {
                            work_done_progress: None,
                        },
                        range: None,
                        full: Some(SemanticTokensFullOptions::Bool(true)),
                        legend: SemanticTokensLegend {
                            token_types: crate::grammar::SEMANTIC_TOKEN_TYPES.to_vec(),
                            token_modifiers: vec![
                                SemanticTokenModifier::DECLARATION,
                                SemanticTokenModifier::DEFINITION,
                                SemanticTokenModifier::READONLY,
                            ],
                        },
                    },
                    static_registration_options: StaticRegistrationOptions { id: None },
                },
            ),
        ),
        completion_provider: Some(CompletionOptions {
            trigger_characters: Some(vec!["@".to_string(), ".".to_string()]),
            ..Default::default()
        }),
        document_symbol_provider: Some(OneOf::Left(true)),
        code_lens_provider: Some(CodeLensOptions {
            resolve_provider: Some(false),
        }),
        code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
        ..Default::default()
    }
}
