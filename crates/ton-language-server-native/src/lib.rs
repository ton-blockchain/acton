use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;
use tokio::net::TcpListener;
use ton_language_server_core::languages::fift::FiftLanguage;
use ton_language_server_core::languages::tasm::{STACK_EFFECT_CODE_LENS_COMMAND, TasmLanguage};
use ton_language_server_core::languages::tlb::TlbLanguage;
use ton_language_server_core::languages::tolk::{LANGUAGE_ID as TOLK_LANGUAGE_ID, TolkLanguage};
use ton_language_server_core::{
    CodeLens, DocumentUri, FoldingRange, Hover, InlayHint, InlayHintKind, LanguageId,
    LanguageService, LanguageServiceConfig, Location, Position, Range,
    SEMANTIC_TOKEN_MODIFIER_NAMES, SEMANTIC_TOKEN_TYPE_NAMES, SemanticToken, SemanticTokens,
    TextEdit, TextIndex, WorkspaceConfig,
};
use tower_lsp::jsonrpc;
use tower_lsp::lsp_types as lsp;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Level, Metadata, Subscriber};

pub use ton_language_server_core::LogLevel;

const TASM_SPEC_JSON: &str = include_str!("../../tasm-core/spec/tvm-specification.json");

#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub project_root: PathBuf,
    pub tolk_stdlib_root: Option<PathBuf>,
    pub logging: Option<NativeLoggingConfig>,
    pub enable_profiling: bool,
}

impl ServerConfig {
    #[must_use]
    pub fn new(project_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
            tolk_stdlib_root: None,
            logging: None,
            enable_profiling: cfg!(feature = "profiling"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct NativeLoggingConfig {
    pub path: PathBuf,
    pub level: LogLevel,
}

impl NativeLoggingConfig {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>, level: LogLevel) -> Self {
        Self {
            path: path.into(),
            level,
        }
    }
}

pub async fn serve_stdio(config: ServerConfig) -> anyhow::Result<()> {
    install_logging(config.logging.as_ref())?;
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) =
        LspService::new(|client| NativeLanguageServer::new(client, config.clone()));
    Server::new(stdin, stdout, socket).serve(service).await;
    Ok(())
}

pub async fn serve_tcp(config: ServerConfig, port: u16) -> anyhow::Result<()> {
    install_logging(config.logging.as_ref())?;
    let listener = TcpListener::bind(("127.0.0.1", port)).await?;
    tracing::info!(
        target: "ton_language_server_native",
        operation = "server.listen",
        port,
        "language server is listening"
    );
    let (stream, _) = listener.accept().await?;
    let (reader, writer) = tokio::io::split(stream);
    let (service, socket) =
        LspService::new(|client| NativeLanguageServer::new(client, config.clone()));
    Server::new(reader, writer, socket).serve(service).await;
    Ok(())
}

pub struct NativeLanguageServer {
    client: Client,
    service: Mutex<LanguageService>,
    project_root: PathBuf,
    root_uri: DocumentUri,
    manifest_uri: Option<DocumentUri>,
    tolk_stdlib_root_uri: Option<DocumentUri>,
    documents: Mutex<HashMap<String, OpenDocument>>,
}

impl NativeLanguageServer {
    #[must_use]
    pub fn new(client: Client, config: ServerConfig) -> Self {
        let project_root = canonicalize_project_root(config.project_root);
        let root_uri = DocumentUri::from(file_uri_string(&project_root));
        let manifest_path = project_root.join("Acton.toml");
        let manifest_uri = Some(DocumentUri::from(file_uri_string(&manifest_path)));
        let tolk_stdlib_root = config.tolk_stdlib_root.map(canonicalize_project_root);
        let tolk_stdlib_root_uri = tolk_stdlib_root
            .as_ref()
            .map(|path| DocumentUri::from(file_uri_string(path)));
        let mut service = native_language_service(config.enable_profiling);

        if let Err(error) = apply_initial_workspace_config(
            &mut service,
            &root_uri,
            manifest_uri.as_ref(),
            tolk_stdlib_root_uri.as_ref(),
        ) {
            tracing::warn!(
                target: "ton_language_server_native",
                operation = "workspace.config.init",
                error = %error,
                "failed to apply initial Acton.toml"
            );
        }
        if let Err(error) =
            prime_workspace_sources(&mut service, &project_root, tolk_stdlib_root.as_deref())
        {
            let tolk_stdlib_root_log = tolk_stdlib_root
                .as_deref()
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_default();
            tracing::warn!(
                target: "ton_language_server_native",
                operation = "workspace.scan",
                project_root = project_root.to_string_lossy().as_ref(),
                tolk_stdlib_root = tolk_stdlib_root_log.as_str(),
                error = %error,
                "failed to scan workspace sources"
            );
        }

        Self {
            client,
            service: Mutex::new(service),
            project_root,
            root_uri,
            manifest_uri,
            tolk_stdlib_root_uri,
            documents: Mutex::new(HashMap::new()),
        }
    }

    async fn report_error(&self, operation: &'static str, error: impl ToString) {
        let message = format!("{operation}: {}", error.to_string());
        tracing::warn!(
            target: "ton_language_server_native",
            operation,
            error = %message,
            "language server operation failed"
        );
        self.client
            .log_message(lsp::MessageType::ERROR, message)
            .await;
    }

    fn with_service<T>(
        &self,
        f: impl FnOnce(&mut LanguageService) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        let mut service = self
            .service
            .lock()
            .map_err(|_| anyhow::anyhow!("language service lock poisoned"))?;
        f(&mut service)
    }

    fn apply_workspace_config_text(&self, text: String) -> anyhow::Result<()> {
        let root_uri = self.root_uri.clone();
        let manifest_uri = self.manifest_uri.clone();
        let tolk_stdlib_root_uri = self.tolk_stdlib_root_uri.clone();
        self.with_service(|service| {
            service.set_workspace_config(
                LanguageId::from(TOLK_LANGUAGE_ID),
                workspace_config(root_uri, manifest_uri, tolk_stdlib_root_uri, text),
            )
        })
    }

    fn reload_tolk_file(&self, uri: &lsp::Url) -> anyhow::Result<()> {
        let path = uri
            .to_file_path()
            .map_err(|()| anyhow::anyhow!("cannot convert uri to file path: {uri}"))?;
        if !is_tolk_path(&path) {
            return Ok(());
        }
        let text = fs::read_to_string(&path)?;
        let uri = DocumentUri::from(uri.to_string());
        self.with_service(|service| {
            service.add_source_file(LanguageId::from(TOLK_LANGUAGE_ID), uri, text)
        })
    }

    fn remove_tolk_file(&self, uri: &lsp::Url) -> anyhow::Result<()> {
        let path = uri
            .to_file_path()
            .map_err(|()| anyhow::anyhow!("cannot convert uri to file path: {uri}"))?;
        if !is_tolk_path(&path) {
            return Ok(());
        }
        let uri = DocumentUri::from(uri.to_string());
        self.with_service(|service| {
            service.remove_source_file(LanguageId::from(TOLK_LANGUAGE_ID), &uri)
        })
    }

    fn prepare_document_change(
        &self,
        uri: &lsp::Url,
        changes: &[lsp::TextDocumentContentChangeEvent],
    ) -> anyhow::Result<(OpenDocumentKind, String, AppliedChanges)> {
        let uri_string = uri.to_string();
        let mut documents = self
            .documents
            .lock()
            .map_err(|_| anyhow::anyhow!("document map lock poisoned"))?;
        let Some(document) = documents.get_mut(&uri_string) else {
            anyhow::bail!("document is not open: {uri}");
        };
        let applied = apply_lsp_changes_to_text(&mut document.text, changes)?;
        let kind = document.kind.clone();
        let text = document.text.clone();
        drop(documents);
        Ok((kind, text, applied))
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for NativeLanguageServer {
    async fn initialize(
        &self,
        _params: lsp::InitializeParams,
    ) -> jsonrpc::Result<lsp::InitializeResult> {
        Ok(lsp::InitializeResult {
            capabilities: lsp::ServerCapabilities {
                position_encoding: Some(lsp::PositionEncodingKind::UTF16),
                text_document_sync: Some(lsp::TextDocumentSyncCapability::Options(
                    lsp::TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(lsp::TextDocumentSyncKind::INCREMENTAL),
                        will_save: None,
                        will_save_wait_until: None,
                        save: Some(lsp::TextDocumentSyncSaveOptions::SaveOptions(
                            lsp::SaveOptions {
                                include_text: Some(true),
                            },
                        )),
                    },
                )),
                hover_provider: Some(lsp::HoverProviderCapability::Simple(true)),
                definition_provider: Some(lsp::OneOf::Left(true)),
                references_provider: Some(lsp::OneOf::Left(true)),
                inlay_hint_provider: Some(lsp::OneOf::Left(true)),
                semantic_tokens_provider: Some(lsp::SemanticTokensServerCapabilities::from(
                    lsp::SemanticTokensOptions {
                        work_done_progress_options: lsp::WorkDoneProgressOptions {
                            work_done_progress: None,
                        },
                        legend: semantic_tokens_legend_to_lsp(),
                        range: Some(false),
                        full: Some(lsp::SemanticTokensFullOptions::Bool(true)),
                    },
                )),
                code_lens_provider: Some(lsp::CodeLensOptions {
                    resolve_provider: Some(false),
                }),
                folding_range_provider: Some(lsp::FoldingRangeProviderCapability::Simple(true)),
                execute_command_provider: Some(lsp::ExecuteCommandOptions {
                    commands: execute_commands(),
                    work_done_progress_options: lsp::WorkDoneProgressOptions {
                        work_done_progress: None,
                    },
                }),
                ..lsp::ServerCapabilities::default()
            },
            server_info: Some(lsp::ServerInfo {
                name: "Acton Language Server".to_owned(),
                version: None,
            }),
        })
    }

    async fn initialized(&self, _: lsp::InitializedParams) {
        self.client
            .log_message(
                lsp::MessageType::INFO,
                format!(
                    "Acton language server started for {}",
                    self.project_root.display()
                ),
            )
            .await;
    }

    async fn shutdown(&self) -> jsonrpc::Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: lsp::DidOpenTextDocumentParams) {
        let item = params.text_document;
        let uri = item.uri;
        let uri_string = uri.to_string();
        if is_acton_manifest_uri(&uri) {
            if let Err(error) = self.apply_workspace_config_text(item.text.clone()) {
                self.report_error("workspace.config.open", error).await;
            }
            if let Ok(mut documents) = self.documents.lock() {
                documents.insert(
                    uri_string,
                    OpenDocument {
                        kind: OpenDocumentKind::ActonManifest,
                        text: item.text,
                    },
                );
            }
            return;
        }

        let kind = language_id_for_document(&item.language_id, &uri)
            .map_or(OpenDocumentKind::Unsupported, |language_id| {
                OpenDocumentKind::Language { language_id }
            });

        if let OpenDocumentKind::Language { language_id } = &kind {
            let result = self.with_service(|service| {
                service.open_document(
                    DocumentUri::from(uri_string.clone()),
                    language_id.clone(),
                    item.version,
                    item.text.clone(),
                )
            });
            if let Err(error) = result {
                self.report_error("document.open", error).await;
            }
        }

        if let Ok(mut documents) = self.documents.lock() {
            documents.insert(
                uri_string,
                OpenDocument {
                    kind,
                    text: item.text,
                },
            );
        }
    }

    async fn did_change(&self, params: lsp::DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let uri_string = uri.to_string();
        let version = params.text_document.version;
        let change = match self.prepare_document_change(&uri, &params.content_changes) {
            Ok(change) => change,
            Err(error) => {
                self.report_error("document.change", error).await;
                return;
            }
        };

        match change {
            (OpenDocumentKind::ActonManifest, text, _) => {
                if let Err(error) = self.apply_workspace_config_text(text) {
                    self.report_error("workspace.config.change", error).await;
                }
            }
            (
                OpenDocumentKind::Language { language_id: _ },
                full_text,
                AppliedChanges::FullText,
            ) => {
                let result = self.with_service(|service| {
                    service.change_document(&DocumentUri::from(uri_string), version, full_text)
                });
                if let Err(error) = result {
                    self.report_error("document.change", error).await;
                }
            }
            (
                OpenDocumentKind::Language { language_id: _ },
                _,
                AppliedChanges::Incremental(edits),
            ) => {
                let result = self.with_service(|service| {
                    service.edit_document(&DocumentUri::from(uri_string), version, edits)
                });
                if let Err(error) = result {
                    self.report_error("document.edit", error).await;
                }
            }
            (OpenDocumentKind::Unsupported, _, _) => {}
        }
    }

    async fn did_save(&self, params: lsp::DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        if is_acton_manifest_uri(&uri) {
            let text = params.text.unwrap_or_else(|| {
                uri.to_file_path()
                    .ok()
                    .and_then(|path| fs::read_to_string(path).ok())
                    .unwrap_or_default()
            });
            if let Err(error) = self.apply_workspace_config_text(text) {
                self.report_error("workspace.config.save", error).await;
            }
        } else if let Err(error) = self.reload_tolk_file(&uri) {
            self.report_error("source_file.reload", error).await;
        }
    }

    async fn did_close(&self, params: lsp::DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        let uri_string = uri.to_string();
        let document = self
            .documents
            .lock()
            .ok()
            .and_then(|mut documents| documents.remove(&uri_string));

        if matches!(
            document,
            Some(OpenDocument {
                kind: OpenDocumentKind::Language { .. },
                ..
            })
        ) {
            self.with_service(|service| {
                service.close_document(&DocumentUri::from(uri_string));
                Ok(())
            })
            .unwrap_or_else(|error| {
                tracing::warn!(
                    target: "ton_language_server_native",
                    operation = "document.close",
                    error = %error,
                    "failed to close document"
                );
            });
        }
    }

    async fn goto_definition(
        &self,
        params: lsp::GotoDefinitionParams,
    ) -> jsonrpc::Result<Option<lsp::GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = position_from_lsp(params.text_document_position_params.position);
        let locations = self
            .with_service(|service| {
                service.definition(&DocumentUri::from(uri.to_string()), position)
            })
            .map_err(rpc_error)?;
        Ok(locations_to_definition_response(locations))
    }

    async fn references(
        &self,
        params: lsp::ReferenceParams,
    ) -> jsonrpc::Result<Option<Vec<lsp::Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let position = position_from_lsp(params.text_document_position.position);
        let locations = self
            .with_service(|service| {
                service.references(&DocumentUri::from(uri.to_string()), position, false)
            })
            .map_err(rpc_error)?;
        Ok(Some(locations.iter().filter_map(location_to_lsp).collect()))
    }

    async fn hover(&self, params: lsp::HoverParams) -> jsonrpc::Result<Option<lsp::Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = position_from_lsp(params.text_document_position_params.position);
        let hover = self
            .with_service(|service| service.hover(&DocumentUri::from(uri.to_string()), position))
            .map_err(rpc_error)?;
        Ok(hover.map(hover_to_lsp))
    }

    async fn semantic_tokens_full(
        &self,
        params: lsp::SemanticTokensParams,
    ) -> jsonrpc::Result<Option<lsp::SemanticTokensResult>> {
        let uri = params.text_document.uri;
        let tokens = self
            .with_service(|service| service.semantic_tokens(&DocumentUri::from(uri.to_string())))
            .map_err(rpc_error)?;
        Ok(Some(lsp::SemanticTokensResult::Tokens(
            semantic_tokens_to_lsp(tokens),
        )))
    }

    async fn inlay_hint(
        &self,
        params: lsp::InlayHintParams,
    ) -> jsonrpc::Result<Option<Vec<lsp::InlayHint>>> {
        let uri = params.text_document.uri;
        let range = range_from_lsp(params.range);
        let hints = self
            .with_service(|service| service.inlay_hints(&DocumentUri::from(uri.to_string()), range))
            .map_err(rpc_error)?;
        Ok(Some(hints.into_iter().map(inlay_hint_to_lsp).collect()))
    }

    async fn code_lens(
        &self,
        params: lsp::CodeLensParams,
    ) -> jsonrpc::Result<Option<Vec<lsp::CodeLens>>> {
        let uri = params.text_document.uri;
        let lenses = self
            .with_service(|service| service.code_lens(&DocumentUri::from(uri.to_string())))
            .map_err(rpc_error)?;
        Ok(Some(lenses.into_iter().map(code_lens_to_lsp).collect()))
    }

    async fn folding_range(
        &self,
        params: lsp::FoldingRangeParams,
    ) -> jsonrpc::Result<Option<Vec<lsp::FoldingRange>>> {
        let uri = params.text_document.uri;
        let ranges = self
            .with_service(|service| service.folding_ranges(&DocumentUri::from(uri.to_string())))
            .map_err(rpc_error)?;
        Ok(Some(ranges.into_iter().map(folding_range_to_lsp).collect()))
    }

    async fn did_change_watched_files(&self, params: lsp::DidChangeWatchedFilesParams) {
        for change in params.changes {
            let result = if is_acton_manifest_uri(&change.uri) {
                let text = if change.typ == lsp::FileChangeType::DELETED {
                    String::new()
                } else {
                    change
                        .uri
                        .to_file_path()
                        .ok()
                        .and_then(|path| fs::read_to_string(path).ok())
                        .unwrap_or_default()
                };
                self.apply_workspace_config_text(text)
            } else if change.typ == lsp::FileChangeType::DELETED {
                self.remove_tolk_file(&change.uri)
            } else {
                self.reload_tolk_file(&change.uri)
            };

            if let Err(error) = result {
                self.report_error("workspace.files.change", error).await;
            }
        }
    }

    async fn execute_command(
        &self,
        _params: lsp::ExecuteCommandParams,
    ) -> jsonrpc::Result<Option<Value>> {
        Ok(None)
    }
}

#[derive(Clone, Debug)]
struct OpenDocument {
    kind: OpenDocumentKind,
    text: String,
}

#[derive(Clone, Debug)]
enum OpenDocumentKind {
    Language { language_id: LanguageId },
    ActonManifest,
    Unsupported,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum AppliedChanges {
    Incremental(Vec<TextEdit>),
    FullText,
}

fn native_language_service(enable_profiling: bool) -> LanguageService {
    let mut service = LanguageService::new(LanguageServiceConfig { enable_profiling });

    service.register_language(TlbLanguage::new());

    match TasmLanguage::with_spec_json(TASM_SPEC_JSON) {
        Ok(language) => service.register_language(language),
        Err(error) => {
            tracing::warn!(
                target: "ton_language_server_native",
                operation = "language.register",
                language_id = "tasm",
                error = %error,
                "failed to load bundled TASM specification"
            );
            service.register_language(TasmLanguage::new());
        }
    }

    service.register_language(FiftLanguage::new());
    service.register_language(TolkLanguage::new());

    service
}

fn execute_commands() -> Vec<String> {
    vec![STACK_EFFECT_CODE_LENS_COMMAND.to_owned()]
}

fn apply_initial_workspace_config(
    service: &mut LanguageService,
    root_uri: &DocumentUri,
    manifest_uri: Option<&DocumentUri>,
    tolk_stdlib_root_uri: Option<&DocumentUri>,
) -> anyhow::Result<()> {
    let manifest_text = manifest_uri
        .and_then(|uri| lsp::Url::parse(uri.as_str()).ok())
        .and_then(|uri| uri.to_file_path().ok())
        .and_then(|path| fs::read_to_string(path).ok())
        .unwrap_or_default();
    service.set_workspace_config(
        LanguageId::from(TOLK_LANGUAGE_ID),
        workspace_config(
            root_uri.clone(),
            manifest_uri.cloned(),
            tolk_stdlib_root_uri.cloned(),
            manifest_text,
        ),
    )?;

    Ok(())
}

fn prime_workspace_sources(
    service: &mut LanguageService,
    project_root: &Path,
    tolk_stdlib_root: Option<&Path>,
) -> anyhow::Result<()> {
    if let Some(tolk_stdlib_root) = tolk_stdlib_root {
        prime_tolk_sources(service, tolk_stdlib_root, &[])?;
    }

    let excluded_roots = tolk_stdlib_root
        .into_iter()
        .map(Path::to_path_buf)
        .collect::<Vec<_>>();
    prime_tolk_sources(service, project_root, &excluded_roots)
}

fn prime_tolk_sources(
    service: &mut LanguageService,
    root: &Path,
    excluded_roots: &[PathBuf],
) -> anyhow::Result<()> {
    visit_tolk_sources(root, excluded_roots, &mut |path| {
        let text = fs::read_to_string(path)?;
        service.add_source_file(
            LanguageId::from(TOLK_LANGUAGE_ID),
            DocumentUri::from(file_uri_string(path)),
            text,
        )
    })?;
    Ok(())
}

fn visit_tolk_sources(
    dir: &Path,
    excluded_roots: &[PathBuf],
    visit: &mut impl FnMut(&Path) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };

    for entry in entries {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let path = entry.path();
        if file_type.is_dir() {
            if !is_excluded_workspace_dir(&path) && !is_excluded_source_root(&path, excluded_roots)
            {
                visit_tolk_sources(&path, excluded_roots, visit)?;
            }
        } else if file_type.is_file() && is_tolk_path(&path) {
            visit(&path)?;
        }
    }
    Ok(())
}

fn workspace_config(
    root_uri: DocumentUri,
    manifest_uri: Option<DocumentUri>,
    tolk_stdlib_root_uri: Option<DocumentUri>,
    manifest_text: impl Into<std::sync::Arc<str>>,
) -> WorkspaceConfig {
    let config = WorkspaceConfig::new(root_uri, manifest_uri, manifest_text);
    match tolk_stdlib_root_uri {
        Some(uri) => config.with_tolk_stdlib_root_uri(uri),
        None => config,
    }
}

fn language_id_for_document(language_id: &str, uri: &lsp::Url) -> Option<LanguageId> {
    match language_id {
        "tolk" | "tasm" | "fift" | "tlb" => Some(LanguageId::from(language_id.to_owned())),
        _ => language_id_for_path(&uri.to_file_path().ok()?),
    }
}

fn language_id_for_path(path: &Path) -> Option<LanguageId> {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("tolk") => Some(LanguageId::from("tolk")),
        Some("tasm") => Some(LanguageId::from("tasm")),
        Some("fif" | "fift") => Some(LanguageId::from("fift")),
        Some("tlb") => Some(LanguageId::from("tlb")),
        _ => None,
    }
}

fn apply_lsp_changes_to_text(
    text: &mut String,
    changes: &[lsp::TextDocumentContentChangeEvent],
) -> anyhow::Result<AppliedChanges> {
    let mut edits = Vec::new();
    let mut has_full_text_change = false;

    for change in changes {
        let Some(range) = change.range else {
            text.clone_from(&change.text);
            edits.clear();
            has_full_text_change = true;
            continue;
        };

        let range = range_from_lsp(range);
        let index = TextIndex::new(text);
        let start = index.position_to_offset(text, range.start);
        let end = index.position_to_offset(text, range.end);
        if start > end {
            anyhow::bail!(
                "change range start {}:{} is after end {}:{}",
                range.start.line,
                range.start.character,
                range.end.line,
                range.end.character
            );
        }

        text.replace_range(start..end, &change.text);
        if !has_full_text_change {
            edits.push(TextEdit::new(range, change.text.clone()));
        }
    }

    if has_full_text_change {
        Ok(AppliedChanges::FullText)
    } else {
        Ok(AppliedChanges::Incremental(edits))
    }
}

fn locations_to_definition_response(
    locations: Vec<Location>,
) -> Option<lsp::GotoDefinitionResponse> {
    match locations.as_slice() {
        [] => None,
        [location] => location_to_lsp(location).map(lsp::GotoDefinitionResponse::Scalar),
        _ => Some(lsp::GotoDefinitionResponse::Array(
            locations.iter().filter_map(location_to_lsp).collect(),
        )),
    }
}

fn location_to_lsp(location: &Location) -> Option<lsp::Location> {
    Some(lsp::Location {
        uri: lsp::Url::parse(location.uri.as_str()).ok()?,
        range: range_to_lsp(location.range),
    })
}

fn hover_to_lsp(hover: Hover) -> lsp::Hover {
    lsp::Hover {
        contents: lsp::HoverContents::Markup(lsp::MarkupContent {
            kind: lsp::MarkupKind::Markdown,
            value: hover.contents,
        }),
        range: hover.range.map(range_to_lsp),
    }
}

fn semantic_tokens_legend_to_lsp() -> lsp::SemanticTokensLegend {
    lsp::SemanticTokensLegend {
        token_types: SEMANTIC_TOKEN_TYPE_NAMES
            .iter()
            .copied()
            .map(lsp::SemanticTokenType::from)
            .collect(),
        token_modifiers: SEMANTIC_TOKEN_MODIFIER_NAMES
            .iter()
            .copied()
            .map(lsp::SemanticTokenModifier::from)
            .collect(),
    }
}

fn semantic_tokens_to_lsp(tokens: SemanticTokens) -> lsp::SemanticTokens {
    lsp::SemanticTokens {
        result_id: tokens.result_id,
        data: tokens.data.into_iter().map(semantic_token_to_lsp).collect(),
    }
}

fn inlay_hint_to_lsp(hint: InlayHint) -> lsp::InlayHint {
    lsp::InlayHint {
        position: position_to_lsp(hint.position),
        label: lsp::InlayHintLabel::String(hint.label),
        kind: hint.kind.map(|kind| match kind {
            InlayHintKind::Type => lsp::InlayHintKind::TYPE,
            InlayHintKind::Parameter => lsp::InlayHintKind::PARAMETER,
        }),
        text_edits: None,
        tooltip: hint.tooltip.map(lsp::InlayHintTooltip::String),
        padding_left: Some(hint.padding_left),
        padding_right: Some(hint.padding_right),
        data: None,
    }
}

const fn semantic_token_to_lsp(token: SemanticToken) -> lsp::SemanticToken {
    lsp::SemanticToken {
        delta_line: token.delta_line,
        delta_start: token.delta_start,
        length: token.length,
        token_type: token.token_type,
        token_modifiers_bitset: token.token_modifiers_bitset,
    }
}

fn code_lens_to_lsp(lens: CodeLens) -> lsp::CodeLens {
    lsp::CodeLens {
        range: range_to_lsp(lens.range),
        command: lens.command.map(|command| lsp::Command {
            title: command.title,
            command: command.command,
            arguments: (!command.arguments.is_empty()).then(|| {
                command
                    .arguments
                    .into_iter()
                    .map(Value::String)
                    .collect::<Vec<_>>()
            }),
        }),
        data: None,
    }
}

const fn folding_range_to_lsp(range: FoldingRange) -> lsp::FoldingRange {
    lsp::FoldingRange {
        start_line: range.start_line,
        start_character: range.start_character,
        end_line: range.end_line,
        end_character: range.end_character,
        kind: None,
        collapsed_text: None,
    }
}

const fn position_from_lsp(position: lsp::Position) -> Position {
    Position::new(position.line, position.character)
}

const fn range_from_lsp(range: lsp::Range) -> Range {
    Range::new(position_from_lsp(range.start), position_from_lsp(range.end))
}

const fn position_to_lsp(position: Position) -> lsp::Position {
    lsp::Position {
        line: position.line,
        character: position.character,
    }
}

const fn range_to_lsp(range: Range) -> lsp::Range {
    lsp::Range {
        start: position_to_lsp(range.start),
        end: position_to_lsp(range.end),
    }
}

fn rpc_error(error: impl ToString) -> jsonrpc::Error {
    let mut rpc_error = jsonrpc::Error::internal_error();
    rpc_error.message = error.to_string().into();
    rpc_error
}

fn is_acton_manifest_uri(uri: &lsp::Url) -> bool {
    uri.to_file_path()
        .ok()
        .and_then(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(str::to_owned)
        })
        .is_some_and(|name| name.eq_ignore_ascii_case("Acton.toml"))
}

fn is_tolk_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension == "tolk")
}

fn is_excluded_workspace_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| matches!(name, ".git" | "target" | "node_modules" | ".direnv"))
}

fn is_excluded_source_root(path: &Path, excluded_roots: &[PathBuf]) -> bool {
    excluded_roots
        .iter()
        .any(|root| path == root || path.starts_with(root))
}

fn canonicalize_project_root(project_root: PathBuf) -> PathBuf {
    dunce::canonicalize(&project_root).unwrap_or(project_root)
}

fn file_uri_string(path: &Path) -> String {
    lsp::Url::from_file_path(path).map_or_else(
        |()| format!("file://{}", path.display()),
        |uri| uri.to_string(),
    )
}

fn install_logging(config: Option<&NativeLoggingConfig>) -> anyhow::Result<()> {
    let Some(config) = config else {
        return Ok(());
    };
    if let Some(parent) = config.path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&config.path)?;
    let _ = tracing::subscriber::set_global_default(FileSubscriber::new(file, config.level));
    Ok(())
}

struct FileSubscriber {
    writer: Mutex<File>,
    level: LogLevel,
    next_span_id: AtomicU64,
}

impl FileSubscriber {
    const fn new(file: File, level: LogLevel) -> Self {
        Self {
            writer: Mutex::new(file),
            level,
            next_span_id: AtomicU64::new(1),
        }
    }
}

impl Subscriber for FileSubscriber {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        level_enabled(self.level, *metadata.level())
    }

    fn new_span(&self, _span: &Attributes<'_>) -> Id {
        Id::from_u64(self.next_span_id.fetch_add(1, Ordering::Relaxed))
    }

    fn record(&self, _span: &Id, _values: &Record<'_>) {}

    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

    fn event(&self, event: &Event<'_>) {
        if !self.enabled(event.metadata()) {
            return;
        }
        let mut visitor = EventVisitor::default();
        event.record(&mut visitor);
        let Ok(mut writer) = self.writer.lock() else {
            return;
        };
        let _ = writeln!(
            writer,
            "[{}][{}] {}{}",
            event.metadata().level(),
            event.metadata().target(),
            visitor.message(),
            visitor.render_fields()
        );
    }

    fn enter(&self, _span: &Id) {}

    fn exit(&self, _span: &Id) {}
}

#[derive(Default)]
struct EventVisitor {
    message: Option<String>,
    fields: Vec<(String, String)>,
}

impl EventVisitor {
    fn message(&self) -> &str {
        self.message.as_deref().unwrap_or("")
    }

    fn render_fields(&self) -> String {
        if self.fields.is_empty() {
            return String::new();
        }
        let fields = self
            .fields
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join(" ");
        format!(" {fields}")
    }
}

impl Visit for EventVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let value = format!("{value:?}");
        if field.name() == "message" {
            self.message = Some(trim_debug_string(&value).to_owned());
        } else {
            self.fields.push((field.name().to_owned(), value));
        }
    }
}

fn trim_debug_string(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value)
}

const fn level_enabled(configured: LogLevel, level: Level) -> bool {
    match configured {
        LogLevel::Off => false,
        LogLevel::Error => level_rank(level) <= 1,
        LogLevel::Warn => level_rank(level) <= 2,
        LogLevel::Info => level_rank(level) <= 3,
        LogLevel::Debug => level_rank(level) <= 4,
        LogLevel::Trace => level_rank(level) <= 5,
    }
}

const fn level_rank(level: Level) -> u8 {
    if matches!(level, Level::ERROR) {
        1
    } else if matches!(level, Level::WARN) {
        2
    } else if matches!(level, Level::INFO) {
        3
    } else if matches!(level, Level::DEBUG) {
        4
    } else {
        5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_language_from_path() -> anyhow::Result<()> {
        let fift = lsp::Url::parse("file:///workspace/main.fif")?;
        let tlb = lsp::Url::parse("file:///workspace/schema.tlb")?;
        let acton_toml = lsp::Url::parse("file:///workspace/Acton.toml")?;

        assert_eq!(
            language_id_for_document("plaintext", &fift).map(|id| id.to_string()),
            Some("fift".to_owned())
        );
        assert_eq!(
            language_id_for_document("plaintext", &tlb).map(|id| id.to_string()),
            Some("tlb".to_owned())
        );
        assert!(is_acton_manifest_uri(&acton_toml));

        Ok(())
    }

    #[test]
    fn applies_incremental_changes_with_utf16_positions() -> anyhow::Result<()> {
        let mut text = "a💎c".to_owned();
        let change = lsp::TextDocumentContentChangeEvent {
            range: Some(lsp::Range {
                start: lsp::Position {
                    line: 0,
                    character: 3,
                },
                end: lsp::Position {
                    line: 0,
                    character: 4,
                },
            }),
            range_length: None,
            text: "d".to_owned(),
        };

        let applied = apply_lsp_changes_to_text(&mut text, &[change])?;

        assert_eq!(text, "a💎d");
        assert_eq!(
            applied,
            AppliedChanges::Incremental(vec![TextEdit::new(
                Range::new(Position::new(0, 3), Position::new(0, 4)),
                "d"
            )])
        );

        Ok(())
    }

    #[test]
    fn primes_tolk_workspace_sources_for_imports() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let lib_path = dir.path().join("lib.tolk");
        let main_path = dir.path().join("main.tolk");
        fs::write(&lib_path, "fun helper(): int { return 1; }\n")?;
        let main_source = "import \"lib\"\nfun main(): int { return helper(); }\n";
        fs::write(&main_path, main_source)?;

        let mut service = native_language_service(false);
        prime_workspace_sources(&mut service, dir.path(), None)?;
        let main_uri = DocumentUri::from(file_uri_string(&main_path));
        service.open_document(
            main_uri.clone(),
            LanguageId::from(TOLK_LANGUAGE_ID),
            1,
            main_source,
        )?;

        let locations = service.definition(&main_uri, Position::new(1, 25))?;

        assert_eq!(locations.len(), 1);
        assert_eq!(
            locations[0].uri,
            DocumentUri::from(file_uri_string(&lib_path))
        );

        Ok(())
    }

    #[test]
    fn primes_external_tolk_stdlib_root_for_imports() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let stdlib_dir = dir.path().join(".acton").join("tolk-stdlib");
        fs::create_dir_all(&stdlib_dir)?;
        let stdlib_common_path = stdlib_dir.join("common.tolk");
        let main_path = dir.path().join("main.tolk");
        fs::write(
            &stdlib_common_path,
            "fun stdlibHelper(): int { return 1; }\n",
        )?;
        let main_source = "import \"@stdlib/common\"\nfun main(): int { return stdlibHelper(); }\n";
        fs::write(&main_path, main_source)?;

        let root_uri = DocumentUri::from(file_uri_string(dir.path()));
        let stdlib_uri = DocumentUri::from(file_uri_string(&stdlib_dir));
        let mut service = native_language_service(false);
        service.set_workspace_config(
            LanguageId::from(TOLK_LANGUAGE_ID),
            workspace_config(root_uri, None, Some(stdlib_uri), ""),
        )?;
        prime_workspace_sources(&mut service, dir.path(), Some(&stdlib_dir))?;
        let main_uri = DocumentUri::from(file_uri_string(&main_path));
        service.open_document(
            main_uri.clone(),
            LanguageId::from(TOLK_LANGUAGE_ID),
            1,
            main_source,
        )?;

        let locations = service.definition(&main_uri, Position::new(1, 25))?;

        assert_eq!(locations.len(), 1);
        assert_eq!(
            locations[0].uri,
            DocumentUri::from(file_uri_string(&stdlib_common_path))
        );

        Ok(())
    }
}
