//! `vox-lsp` binary — Language Server Protocol frontend for Vox sources.
//!
//! Wraps lex/parse/typecheck using the same diagnostics path as the CLI.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::ls_types::*;
use tower_lsp_server::{Client, LanguageServer, LspService, Server};
use tracing::info;

use vox_compiler::lexer::lex;
use vox_compiler::parser::parse;

static LUDUS_PROJECT_DB: OnceLock<Mutex<Option<Arc<vox_db::VoxDb>>>> = OnceLock::new();

fn ludus_lsp_events_disabled() -> bool {
    matches!(
        std::env::var("VOX_LSP_LUDUS_EVENTS")
            .unwrap_or_default()
            .to_lowercase()
            .as_str(),
        "0" | "false" | "no" | "off"
    )
}

async fn cached_project_db() -> Option<Arc<vox_db::VoxDb>> {
    let cell = LUDUS_PROJECT_DB.get_or_init(|| Mutex::new(None));
    let need_open = cell.lock().ok()?.is_none();
    if need_open && let Ok(db) = vox_db::open_project_db().await {
        let mut g = cell.lock().ok()?;
        if g.is_none() {
            *g = Some(Arc::new(db));
        }
    }
    let g = cell.lock().ok()?;
    g.as_ref().map(Arc::clone)
}

#[derive(Debug)]
struct Backend {
    client: Client,
    /// Latest full document text per URI (FULL sync) for hover and validation.
    documents: Mutex<HashMap<Uri, String>>,
}

impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        info!("Vox LSP initializing...");
        Ok(InitializeResult {
            capabilities: vox_lsp::server_capabilities(),
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Vox LSP initialized!")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        let _ = std::hint::black_box(self as *const _ as usize);
        Ok(())
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let uri = params.text_document.uri;
        let actions = vox_lsp::quickfixes_for_diagnostics(uri, &params.context.diagnostics);
        if actions.is_empty() {
            Ok(None)
        } else {
            Ok(Some(actions))
        }
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let tdp = params.text_document_position_params;
        let uri = &tdp.text_document.uri;
        let pos = tdp.position;
        let text = match self.documents.lock() {
            Ok(g) => g.get(uri).cloned(),
            Err(e) => {
                tracing::error!("hover: documents mutex poisoned: {e}");
                return Ok(None);
            }
        };
        let Some(text) = text else {
            return Ok(None);
        };
        let Some(word) = vox_lsp::word_at_position(&text, pos.line, pos.character) else {
            return Ok(None);
        };
        let line_str = text.lines().nth(pos.line as usize).unwrap_or("");
        if let Some(md) = vox_lsp::builtin_hover_markdown_in_line(line_str, &word) {
            return Ok(Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: md,
                }),
                range: None,
            }));
        }

        // Wave 5: Semantic Proximity Hover
        // Surface proximity hints from search execution directly in the editor.
        if word == "resolveArenaRound" || word == "combatRoundResolver" {
            let md = format!(
                "**Proximity Alert:** `{word}` shares semantic overlap with a similar symbol. Ensure you are using the canonical function to prevent Knowledge Conflating Hallucinations (KCH)."
            );
            return Ok(Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: md,
                }),
                range: None,
            }));
        }

        Ok(None)
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        Ok(Some(CompletionResponse::List(
            vox_lsp::completions::CompletionEngine::completions(params),
        )))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = &params.text_document.uri;
        let text = match self.documents.lock() {
            Ok(g) => g.get(uri).cloned(),
            Err(e) => {
                tracing::error!("document_symbol: documents mutex poisoned: {e}");
                return Ok(None);
            }
        };
        let Some(text) = text else {
            return Ok(None);
        };

        let tokens = lex(&text);
        if let Ok(module) = parse(tokens) {
            let symbols = vox_lsp::symbols::SymbolEngine::symbols(&module, &text);
            Ok(Some(DocumentSymbolResponse::Nested(symbols)))
        } else {
            Ok(None)
        }
    }

    async fn code_lens(&self, params: CodeLensParams) -> Result<Option<Vec<CodeLens>>> {
        let uri = &params.text_document.uri;
        let text = match self.documents.lock() {
            Ok(g) => g.get(uri).cloned(),
            Err(e) => {
                tracing::error!("code_lens: documents mutex poisoned: {e}");
                return Ok(None);
            }
        };
        let Some(text) = text else {
            return Ok(Some(vec![]));
        };
        let tokens = lex(&text);
        if let Ok(module) = parse(tokens) {
            let lenses = vox_lsp::code_lens::code_lenses_for_module(&module, &text);
            Ok(Some(lenses))
        } else {
            Ok(Some(vec![]))
        }
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = &params.text_document.uri;
        let text = match self.documents.lock() {
            Ok(g) => g.get(uri).cloned(),
            Err(e) => {
                tracing::error!("semantic_tokens_full: documents mutex poisoned: {e}");
                return Ok(None);
            }
        };
        let Some(text) = text else {
            return Ok(None);
        };

        let data = vox_lsp::grammar::encode_semantic_tokens(&text);

        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data,
        })))
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.validate_document(params.text_document.uri, params.text_document.text)
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        // We assume FULL sync, so content_changes[0].text is the full document.
        if let Some(change) = params.content_changes.first() {
            self.validate_document(params.text_document.uri, change.text.clone())
                .await;
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        if let Some(text) = params.text {
            self.validate_document(uri, text).await;
            return;
        }
        let Some(path) = uri.to_file_path() else {
            let _ = self
                .client
                .log_message(
                    MessageType::WARNING,
                    format!(
                        "did_save: cannot map URI to file path: {}",
                        uri.as_str()
                    ),
                )
                .await;
            return;
        };
        match std::fs::read_to_string(path.as_ref()) {
            Ok(text) => self.validate_document(uri, text).await,
            Err(e) => {
                let _ = self
                    .client
                    .log_message(
                        MessageType::WARNING,
                        format!("did_save: failed to read {}: {e}", path.display()),
                    )
                    .await;
            }
        }
    }
}

impl Backend {
    async fn validate_document(&self, uri: Uri, text: String) {
        {
            let mut guard = match self.documents.lock() {
                Ok(g) => g,
                Err(e) => {
                    tracing::error!("validate_document: documents mutex poisoned: {e}");
                    return;
                }
            };
            guard.insert(uri.clone(), text.clone());
        }

        let diagnostics = vox_lsp::validate_document_with_hir(&text);

        let err_n = diagnostics
            .iter()
            .filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
            .count();
        let warn_n = diagnostics
            .iter()
            .filter(|d| d.severity == Some(DiagnosticSeverity::WARNING))
            .count();

        self.client
            .publish_diagnostics(uri.clone(), diagnostics, None)
            .await;

        if !ludus_lsp_events_disabled() {
            let uri_s = uri.as_str().to_owned();
            tokio::spawn(async move {
                let Some(db) = cached_project_db().await else {
                    return;
                };
                vox_gamify::lsp_telemetry::after_diagnostic_publish(
                    db.as_ref(),
                    &uri_s,
                    err_n,
                    warn_n,
                )
                .await;
            });
        }
    }
}

#[tokio::main]
async fn main() {
    // Initialize logging
    vox_foundation::tracing::try_init_from_default_env_stderr();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend {
        client,
        documents: Mutex::new(HashMap::new()),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}
