use dashmap::DashMap;
use lsp_types::*;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use tolk_resolver::file_db::FileDb;
use tolk_resolver::file_index::FileId;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::Url;
use tower_lsp::{Client, LanguageServer};

pub mod profiling;
pub mod utils;

use crate::AnalysisResult;
use crate::languages::tolk::semantic_tokens;
#[cfg(feature = "profiling")]
pub use profiling::ProfilingContext;

pub struct Backend {
    pub client: Client,
    pub file_db: Arc<FileDb>,
    pub project_root: PathBuf,
    pub mappings: Option<BTreeMap<String, String>>,
    pub documents: DashMap<Url, String>,
    pub analysis: DashMap<Url, Arc<AnalysisResult>>,
    pub file_urls: DashMap<FileId, Url>,
    #[cfg(feature = "profiling")]
    pub profiling: Arc<ProfilingContext>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> LspResult<InitializeResult> {
        let now = std::time::Instant::now();
        log::info!("Request: initialize");
        let res = Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::INCREMENTAL,
                )),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                inlay_hint_provider: Some(OneOf::Left(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions(
                        SemanticTokensRegistrationOptions {
                            text_document_registration_options: TextDocumentRegistrationOptions {
                                document_selector: Some(vec![DocumentFilter {
                                    language: Some("tolk".to_string()),
                                    scheme: Some("file".to_string()),
                                    pattern: None,
                                }]),
                            },
                            semantic_tokens_options: SemanticTokensOptions {
                                work_done_progress_options: WorkDoneProgressOptions {
                                    work_done_progress: None,
                                },
                                range: Some(false),
                                full: Some(SemanticTokensFullOptions::Bool(true)),
                                legend: SemanticTokensLegend {
                                    token_types: semantic_tokens::TOKEN_TYPES.to_vec(),
                                    token_modifiers: semantic_tokens::TOKEN_MODIFIERS.to_vec(),
                                },
                            },
                            static_registration_options: StaticRegistrationOptions { id: None },
                        },
                    ),
                ),
                ..Default::default()
            },
            ..Default::default()
        });
        log::info!("Response: initialize took {:?}", now.elapsed());
        res
    }

    async fn initialized(&self, _: InitializedParams) {
        let now = std::time::Instant::now();
        log::info!("Notification: initialized");
        self.client
            .log_message(MessageType::INFO, "Tolk Language Server initialized")
            .await;
        log::info!("Notification: initialized took {:?}", now.elapsed());
    }

    async fn shutdown(&self) -> LspResult<()> {
        let now = std::time::Instant::now();
        log::info!("Request: shutdown");
        let res = Ok(());
        log::info!("Response: shutdown took {:?}", now.elapsed());
        res
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let now = std::time::Instant::now();
        log::info!("Notification: did_open for {}", params.text_document.uri);
        self.update_document(&params.text_document.uri, params.text_document.text);
        self.analyze(params.text_document.uri).await;
        log::info!("Notification: did_open took {:?}", now.elapsed());
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        self.handle_did_change(params).await;
    }

    async fn did_save(&self, _params: DidSaveTextDocumentParams) {}

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> LspResult<Option<GotoDefinitionResponse>> {
        self.handle_goto_definition(params).await
    }

    async fn references(&self, params: ReferenceParams) -> LspResult<Option<Vec<Location>>> {
        self.handle_references(params).await
    }

    async fn inlay_hint(&self, params: InlayHintParams) -> LspResult<Option<Vec<InlayHint>>> {
        self.handle_inlay_hint(params).await
    }

    async fn code_action(&self, params: CodeActionParams) -> LspResult<Option<CodeActionResponse>> {
        self.handle_code_action(params).await
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> LspResult<Option<Vec<SymbolInformation>>> {
        self.handle_symbol(params).await
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> LspResult<Option<SemanticTokensResult>> {
        self.handle_semantic_tokens_full(params).await
    }
}

impl Backend {
    pub fn get_file_url(&self, file_info: &tolk_resolver::file_db::FileInfo) -> Option<Url> {
        use crate::backend::utils::FileInfoExt;
        let url = self
            .file_urls
            .entry(file_info.id())
            .or_insert_with(|| file_info.url().expect("Failed to get URL for file"));
        Some(url.clone())
    }
}
