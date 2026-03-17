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
use crate::backend::utils::{SourceLanguage, detect_language};
// use crate::completion::data::CompletionItemDataEnvelope;
use crate::languages::engine::edits::apply_lsp_changes;
use crate::languages::engine::registry::SelfContainedLanguageRegistry;
use crate::languages::tasm;
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
    pub registry: SelfContainedLanguageRegistry,
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
                folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(true),
                    trigger_characters: Some(vec![
                        ".".to_string(),
                        "\"".to_string(),
                        "'".to_string(),
                    ]),
                    ..Default::default()
                }),
                code_lens_provider: Some(CodeLensOptions {
                    resolve_provider: Some(false),
                }),
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec![tasm::code_lenses::STACK_EFFECT_CODE_LENS_COMMAND.to_string()],
                    work_done_progress_options: WorkDoneProgressOptions {
                        work_done_progress: None,
                    },
                }),
                inlay_hint_provider: Some(OneOf::Left(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions(
                        SemanticTokensRegistrationOptions {
                            text_document_registration_options: TextDocumentRegistrationOptions {
                                document_selector: Some(vec![
                                    DocumentFilter {
                                        language: Some("tolk".to_string()),
                                        scheme: Some("file".to_string()),
                                        pattern: None,
                                    },
                                    DocumentFilter {
                                        language: Some("fift".to_string()),
                                        scheme: Some("file".to_string()),
                                        pattern: None,
                                    },
                                    DocumentFilter {
                                        language: Some("tlb".to_string()),
                                        scheme: Some("file".to_string()),
                                        pattern: None,
                                    },
                                ]),
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
        let uri = params.text_document.uri;
        let version = params.text_document.version;
        let text = params.text_document.text;
        let language = detect_language(&uri);

        log::info!("Notification: did_open for {}", uri);

        if language.is_self_contained() {
            if let Err(error) = self.registry.did_open(language, &uri, version, &text) {
                log::warn!("Failed to initialize self-contained cache for {uri}: {error}");
            }
        } else {
            self.update_document(&uri, text.clone());
        }

        if language == SourceLanguage::Tolk {
            self.analyze(uri).await;
        }
        log::info!("Notification: did_open took {:?}", now.elapsed());
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let language = detect_language(&params.text_document.uri);
        if language == SourceLanguage::Tolk {
            self.handle_did_change(params).await;
            return;
        }

        if language.is_self_contained() {
            self.handle_self_contained_did_change(language, params);
            return;
        }

        self.handle_text_only_did_change(params);
    }

    async fn did_save(&self, _params: DidSaveTextDocumentParams) {}

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        let language = detect_language(&uri);

        if language.is_self_contained() {
            self.registry.did_close(language, &uri);
        }
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> LspResult<Option<GotoDefinitionResponse>> {
        match detect_language(&params.text_document_position_params.text_document.uri) {
            SourceLanguage::Tolk => self.handle_goto_definition(params).await,
            SourceLanguage::Fift => Ok(self.handle_fift_goto_definition(params).await),
            SourceLanguage::Tlb => Ok(self.handle_tlb_goto_definition(params).await),
            SourceLanguage::Tasm | SourceLanguage::Toml | SourceLanguage::Unknown => Ok(None),
        }
    }

    async fn references(&self, params: ReferenceParams) -> LspResult<Option<Vec<Location>>> {
        match detect_language(&params.text_document_position.text_document.uri) {
            SourceLanguage::Tolk => self.handle_references(params).await,
            SourceLanguage::Fift => Ok(self.handle_fift_references(params).await),
            SourceLanguage::Tlb => Ok(self.handle_tlb_references(params).await),
            SourceLanguage::Tasm | SourceLanguage::Toml | SourceLanguage::Unknown => Ok(None),
        }
    }

    async fn inlay_hint(&self, params: InlayHintParams) -> LspResult<Option<Vec<InlayHint>>> {
        match detect_language(&params.text_document.uri) {
            SourceLanguage::Tolk => self.handle_inlay_hint(params).await,
            SourceLanguage::Tasm
            | SourceLanguage::Fift
            | SourceLanguage::Tlb
            | SourceLanguage::Toml
            | SourceLanguage::Unknown => Ok(None),
        }
    }

    async fn code_action(&self, params: CodeActionParams) -> LspResult<Option<CodeActionResponse>> {
        match detect_language(&params.text_document.uri) {
            SourceLanguage::Tolk => self.handle_code_action(params).await,
            SourceLanguage::Tasm
            | SourceLanguage::Fift
            | SourceLanguage::Tlb
            | SourceLanguage::Toml
            | SourceLanguage::Unknown => Ok(None),
        }
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
        match detect_language(&params.text_document.uri) {
            SourceLanguage::Tolk => self.handle_semantic_tokens_full(params).await,
            SourceLanguage::Fift => Ok(self.handle_fift_semantic_tokens_full(params).await),
            SourceLanguage::Tlb => Ok(self.handle_tlb_semantic_tokens_full(params).await),
            SourceLanguage::Tasm | SourceLanguage::Toml | SourceLanguage::Unknown => Ok(None),
        }
    }

    async fn folding_range(
        &self,
        params: FoldingRangeParams,
    ) -> LspResult<Option<Vec<FoldingRange>>> {
        match detect_language(&params.text_document.uri) {
            SourceLanguage::Tasm => Ok(self.handle_tasm_folding_range(params).await),
            SourceLanguage::Fift => Ok(self.handle_fift_folding_range(params).await),
            SourceLanguage::Tolk
            | SourceLanguage::Tlb
            | SourceLanguage::Toml
            | SourceLanguage::Unknown => Ok(None),
        }
    }

    async fn hover(&self, params: HoverParams) -> LspResult<Option<Hover>> {
        match detect_language(&params.text_document_position_params.text_document.uri) {
            SourceLanguage::Tasm => Ok(self.handle_tasm_hover(params).await),
            SourceLanguage::Fift => Ok(self.handle_fift_hover(params).await),
            SourceLanguage::Toml => Ok(self.handle_toml_hover(params).await),
            SourceLanguage::Tolk | SourceLanguage::Tlb | SourceLanguage::Unknown => Ok(None),
        }
    }

    async fn completion(&self, params: CompletionParams) -> LspResult<Option<CompletionResponse>> {
        match detect_language(&params.text_document_position.text_document.uri) {
            SourceLanguage::Tlb => Ok(self.handle_tlb_completion(params).await),
            SourceLanguage::Toml => Ok(self.handle_toml_completion(params).await),
            SourceLanguage::Tolk
            | SourceLanguage::Tasm
            | SourceLanguage::Fift
            | SourceLanguage::Unknown => Ok(None),
        }
    }

    async fn completion_resolve(&self, item: CompletionItem) -> LspResult<CompletionItem> {
        // if let Some(data) = item.data.as_ref()
        //     && let Some(envelope) = CompletionItemDataEnvelope::from_json_value(data)
        // {
        //     log::debug!(
        //         "Completion resolve request for language={} provider={} id={}",
        //         envelope.language,
        //         envelope.provider,
        //         envelope.resolve_id
        //     );
        // }

        Ok(item)
    }

    async fn code_lens(&self, params: CodeLensParams) -> LspResult<Option<Vec<CodeLens>>> {
        match detect_language(&params.text_document.uri) {
            SourceLanguage::Tasm => Ok(self.handle_tasm_code_lens(params).await),
            SourceLanguage::Tolk
            | SourceLanguage::Fift
            | SourceLanguage::Tlb
            | SourceLanguage::Toml
            | SourceLanguage::Unknown => Ok(None),
        }
    }

    async fn execute_command(
        &self,
        params: ExecuteCommandParams,
    ) -> LspResult<Option<serde_json::Value>> {
        if params.command == tasm::code_lenses::STACK_EFFECT_CODE_LENS_COMMAND {
            return Ok(None);
        }

        log::warn!("Unknown execute command: {}", params.command);
        Ok(None)
    }
}

impl Backend {
    fn handle_self_contained_did_change(
        &self,
        language: SourceLanguage,
        params: DidChangeTextDocumentParams,
    ) {
        let uri = params.text_document.uri;
        let version = params.text_document.version;
        match self
            .registry
            .did_change(language, &uri, version, &params.content_changes)
        {
            Ok(Some(change)) => {
                if change.parse_failed {
                    log::debug!(
                        "Self-contained parse failed for {uri}; cached text/version advanced without snapshot"
                    );
                }
            }
            Ok(None) => {
                log::debug!(
                    "No self-contained cache entry for {uri} during did_change; skipping text fallback"
                );
            }
            Err(error) => {
                log::warn!("Failed to sync self-contained cache for {uri}: {error}");
            }
        }
    }

    fn handle_text_only_did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let current_text = self
            .documents
            .get(&uri)
            .map(|d| d.clone())
            .unwrap_or_default();
        let applied = apply_lsp_changes(&current_text, &params.content_changes);
        self.update_document(&uri, applied.text);
    }

    pub fn get_file_url(&self, file_info: &tolk_resolver::file_db::FileInfo) -> Option<Url> {
        use crate::backend::utils::FileInfoExt;
        let url = self
            .file_urls
            .entry(file_info.id())
            .or_insert_with(|| file_info.url().expect("Failed to get URL for file"));
        Some(url.clone())
    }
}
