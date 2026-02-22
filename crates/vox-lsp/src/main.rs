use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use tracing::info;

use vox_lexer::cursor::lex;
use vox_parser::parser::parse;
use vox_typeck::diagnostics::Severity;
use vox_typeck::typecheck_module;

#[derive(Debug)]
struct Backend {
    client: Client,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        info!("Vox LSP initializing...");
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Vox LSP initialized!")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
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
        if let Some(text) = params.text {
            self.validate_document(params.text_document.uri, text).await;
        } else {
            // If text is not provided on save, we might need to read from disk or rely on did_change state.
            // For now, assume did_change handles the live state.
            // But to be robust, let's just log "Saved".
            self.client
                .log_message(
                    MessageType::INFO,
                    format!("Saved {}", params.text_document.uri),
                )
                .await;
        }
    }
}

impl Backend {
    async fn validate_document(&self, uri: Url, text: String) {
        let mut diagnostics = Vec::new();

        // 1. Lex
        let tokens = lex(&text);

        // 2. Parse errors are now handled to position them properly
        match parse(tokens) {
            Ok(module) => {
                // 3. Type Check
                let type_errors = typecheck_module(&module);

                for err in type_errors {
                    let start = index_to_pos(&text, err.span.start);
                    let end = index_to_pos(&text, err.span.end);

                    diagnostics.push(Diagnostic {
                        range: Range { start, end },
                        severity: Some(match err.severity {
                            Severity::Error => DiagnosticSeverity::ERROR,
                            Severity::Warning => DiagnosticSeverity::WARNING,
                        }),
                        code: None,
                        code_description: None,
                        source: Some("vox-lsp".to_string()),
                        message: err.message,
                        related_information: None,
                        tags: None,
                        data: None,
                    });
                }
            }
            Err(parse_errors) => {
                // Convert ParseError to Diagnostic
                for err in parse_errors {
                    let start = index_to_pos(&text, err.span.start);
                    let end = index_to_pos(&text, err.span.end);
                    diagnostics.push(Diagnostic {
                        range: Range { start, end },
                        severity: Some(DiagnosticSeverity::ERROR),
                        code: None,
                        code_description: None,
                        message: err.to_string(),
                        source: Some("vox-lsp".to_string()),
                        ..Default::default()
                    });
                }
            }
        }

        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }
}

/// Convert byte index to LSP Position (line, character)
fn index_to_pos(text: &str, index: usize) -> Position {
    let mut line = 0;
    let mut col = 0;

    // Iterate over chars until we reach the byte index
    for (byte_idx, c) in text.char_indices() {
        if byte_idx >= index {
            break;
        }
        if c == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1; // Note: simplified column count (Unicode support needed for production)
        }
    }
    Position {
        line,
        character: col,
    }
}

#[tokio::main]
async fn main() {
    // Initialize logging
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .try_init();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend { client });
    Server::new(stdin, stdout, socket).serve(service).await;
}
