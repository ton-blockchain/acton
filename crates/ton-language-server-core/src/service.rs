use crate::language::{
    CodeActionRequest, CodeLensRequest, CompletionRequest, DefinitionRequest,
    DocumentHighlightRequest, DocumentSymbolRequest, FileRenameRequest, FoldingRangeRequest,
    HoverRequest, InlayHintRequest, LanguagePlugin, ParseRequest, ParsedDocument, PluginContext,
    PrepareRenameRequest, ReferenceRequest, RenameRequest, SemanticTokensRequest,
    SignatureHelpRequest, TypeAtPositionRequest, TypeDefinitionRequest, WorkspaceSymbolRequest,
};
use crate::logging;
use crate::profiling::Profiler;
use crate::semantic_tokens::SemanticTokens;
use crate::types::{
    CodeAction, CodeLens, DocumentEdits, DocumentHighlight, DocumentSnapshot, DocumentSymbol,
    DocumentUri, FileRename, FoldingRange, Hover, InlayHint, LanguageId, Location, Position,
    PrepareRename, Range, SignatureHelp, TextEdit, WorkspaceConfig, WorkspaceEdit, WorkspaceSymbol,
};
use crate::{CompletionList, CompletionTrigger, TypeAtPosition};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

#[derive(Clone, Debug, Default)]
pub struct LanguageServiceConfig {
    pub enable_profiling: bool,
}

struct DocumentState {
    document: DocumentSnapshot,
    parsed: Arc<dyn ParsedDocument>,
}

pub struct LanguageService {
    plugins: HashMap<LanguageId, Box<dyn LanguagePlugin>>,
    documents: HashMap<DocumentUri, DocumentState>,
    profiler: Profiler,
}

impl LanguageService {
    #[must_use]
    pub fn new(config: LanguageServiceConfig) -> Self {
        let profiler = if config.enable_profiling {
            Profiler::enabled()
        } else {
            Profiler::disabled()
        };
        Self {
            plugins: HashMap::new(),
            documents: HashMap::new(),
            profiler,
        }
    }

    pub fn register_language(&mut self, plugin: impl LanguagePlugin + 'static) {
        let language_id = plugin.language_id();
        let replaced = self
            .plugins
            .insert(language_id.clone(), Box::new(plugin))
            .is_some();
        tracing::info!(
            target: logging::SERVICE_TARGET,
            operation = "language.register",
            language_id = language_id.as_str(),
            replaced,
            "registered language plugin"
        );
    }

    pub fn add_source_file(
        &mut self,
        language_id: impl Into<LanguageId>,
        uri: impl Into<DocumentUri>,
        text: impl Into<Arc<str>>,
    ) -> anyhow::Result<()> {
        let language_id = language_id.into();
        let uri = uri.into();
        let text = text.into();
        let text_len = text.len();
        tracing::info!(
            target: logging::SERVICE_TARGET,
            operation = "source_file.add",
            uri = uri.as_str(),
            language_id = language_id.as_str(),
            text_len,
            "adding provider-backed source file"
        );

        let Some(plugin) = self.plugins.get(&language_id) else {
            tracing::warn!(
                target: logging::SERVICE_TARGET,
                operation = "source_file.add",
                uri = uri.as_str(),
                language_id = language_id.as_str(),
                "unsupported language"
            );
            anyhow::bail!("unsupported language '{language_id}'");
        };
        let Some(workspace) = plugin.workspace() else {
            tracing::warn!(
                target: logging::SERVICE_TARGET,
                operation = "source_file.add",
                uri = uri.as_str(),
                language_id = language_id.as_str(),
                "language has no workspace provider"
            );
            anyhow::bail!("language '{language_id}' does not support workspace source files");
        };

        let result = workspace.add_source_file(uri.clone(), text);
        match &result {
            Ok(()) => {
                self.profiler.increment("source_file.add");
                tracing::info!(
                    target: logging::SERVICE_TARGET,
                    operation = "source_file.add",
                    uri = uri.as_str(),
                    language_id = language_id.as_str(),
                    text_len,
                    "added provider-backed source file"
                );
            }
            Err(error) => {
                tracing::warn!(
                    target: logging::SERVICE_TARGET,
                    operation = "source_file.add",
                    uri = uri.as_str(),
                    language_id = language_id.as_str(),
                    error = %error,
                    "failed to add provider-backed source file"
                );
            }
        }
        result
    }

    pub fn remove_source_file(
        &mut self,
        language_id: impl Into<LanguageId>,
        uri: &DocumentUri,
    ) -> anyhow::Result<()> {
        let language_id = language_id.into();
        tracing::info!(
            target: logging::SERVICE_TARGET,
            operation = "source_file.remove",
            uri = uri.as_str(),
            language_id = language_id.as_str(),
            "removing provider-backed source file"
        );

        let Some(plugin) = self.plugins.get(&language_id) else {
            tracing::warn!(
                target: logging::SERVICE_TARGET,
                operation = "source_file.remove",
                uri = uri.as_str(),
                language_id = language_id.as_str(),
                "unsupported language"
            );
            anyhow::bail!("unsupported language '{language_id}'");
        };
        let Some(workspace) = plugin.workspace() else {
            tracing::warn!(
                target: logging::SERVICE_TARGET,
                operation = "source_file.remove",
                uri = uri.as_str(),
                language_id = language_id.as_str(),
                "language has no workspace provider"
            );
            anyhow::bail!("language '{language_id}' does not support workspace source files");
        };

        let result = workspace.remove_source_file(uri);
        match &result {
            Ok(()) => {
                self.profiler.increment("source_file.remove");
                tracing::info!(
                    target: logging::SERVICE_TARGET,
                    operation = "source_file.remove",
                    uri = uri.as_str(),
                    language_id = language_id.as_str(),
                    "removed provider-backed source file"
                );
            }
            Err(error) => {
                tracing::warn!(
                    target: logging::SERVICE_TARGET,
                    operation = "source_file.remove",
                    uri = uri.as_str(),
                    language_id = language_id.as_str(),
                    error = %error,
                    "failed to remove provider-backed source file"
                );
            }
        }
        result
    }

    pub fn set_workspace_config(
        &mut self,
        language_id: impl Into<LanguageId>,
        config: WorkspaceConfig,
    ) -> anyhow::Result<()> {
        let language_id = language_id.into();
        tracing::info!(
            target: logging::SERVICE_TARGET,
            operation = "workspace.config.set",
            root_uri = config.root_uri().as_str(),
            manifest_uri = config.manifest_uri().map(DocumentUri::as_str),
            language_id = language_id.as_str(),
            text_len = config.manifest_text().len(),
            "setting workspace configuration"
        );

        let Some(plugin) = self.plugins.get(&language_id) else {
            tracing::warn!(
                target: logging::SERVICE_TARGET,
                operation = "workspace.config.set",
                root_uri = config.root_uri().as_str(),
                manifest_uri = config.manifest_uri().map(DocumentUri::as_str),
                language_id = language_id.as_str(),
                "unsupported language"
            );
            anyhow::bail!("unsupported language '{language_id}'");
        };
        let Some(workspace) = plugin.workspace() else {
            tracing::warn!(
                target: logging::SERVICE_TARGET,
                operation = "workspace.config.set",
                root_uri = config.root_uri().as_str(),
                manifest_uri = config.manifest_uri().map(DocumentUri::as_str),
                language_id = language_id.as_str(),
                "language has no workspace provider"
            );
            anyhow::bail!("language '{language_id}' does not support workspace configuration");
        };

        let result = workspace.set_workspace_config(config);
        match &result {
            Ok(()) => {
                self.profiler.increment("workspace.config.set");
                tracing::info!(
                    target: logging::SERVICE_TARGET,
                    operation = "workspace.config.set",
                    language_id = language_id.as_str(),
                    "workspace configuration updated"
                );
            }
            Err(error) => {
                tracing::warn!(
                    target: logging::SERVICE_TARGET,
                    operation = "workspace.config.set",
                    language_id = language_id.as_str(),
                    error = %error,
                    "failed to set workspace configuration"
                );
            }
        }
        result
    }

    pub fn open_document(
        &mut self,
        uri: impl Into<DocumentUri>,
        language_id: impl Into<LanguageId>,
        version: i32,
        text: impl Into<Arc<str>>,
    ) -> anyhow::Result<()> {
        let document = DocumentSnapshot::new(uri.into(), language_id.into(), version, text);
        tracing::info!(
            target: logging::SERVICE_TARGET,
            operation = "document.open",
            uri = document.uri().as_str(),
            language_id = document.language_id().as_str(),
            version = document.version(),
            text_len = document.text().len(),
            "opening document"
        );

        let Some(plugin) = self.plugins.get(document.language_id()) else {
            tracing::warn!(
                target: logging::SERVICE_TARGET,
                operation = "document.open",
                uri = document.uri().as_str(),
                language_id = document.language_id().as_str(),
                version = document.version(),
                "unsupported language"
            );
            anyhow::bail!("unsupported language '{}'", document.language_id());
        };
        let parsed = match plugin.parse(ParseRequest {
            document: &document,
            old_tree: None,
            profiler: &mut self.profiler,
        }) {
            Ok(parsed) => parsed,
            Err(error) => {
                tracing::warn!(
                    target: logging::SERVICE_TARGET,
                    operation = "document.open",
                    uri = document.uri().as_str(),
                    language_id = document.language_id().as_str(),
                    version = document.version(),
                    error = %error,
                    "failed to parse opened document"
                );
                return Err(error);
            }
        };
        let parsed: Arc<dyn ParsedDocument> = Arc::from(parsed);
        if let Some(workspace) = plugin.workspace() {
            workspace.did_open(&document, parsed.as_ref(), &mut self.profiler)?;
        }
        self.profiler.increment("document.open");
        tracing::info!(
            target: logging::SERVICE_TARGET,
            operation = "document.open",
            uri = document.uri().as_str(),
            language_id = document.language_id().as_str(),
            version = document.version(),
            incremental = false,
            "document opened"
        );
        self.documents
            .insert(document.uri().clone(), DocumentState { document, parsed });
        Ok(())
    }

    pub fn change_document(
        &mut self,
        uri: &DocumentUri,
        version: i32,
        text: impl Into<Arc<str>>,
    ) -> anyhow::Result<()> {
        let Some(old_state) = self.documents.get(uri) else {
            tracing::warn!(
                target: logging::SERVICE_TARGET,
                operation = "document.change",
                uri = uri.as_str(),
                version,
                "change for unopened document"
            );
            anyhow::bail!("document not open: {uri}");
        };
        let document = DocumentSnapshot::new(
            uri.clone(),
            old_state.document.language_id().clone(),
            version,
            text,
        );
        tracing::info!(
            target: logging::SERVICE_TARGET,
            operation = "document.change",
            uri = document.uri().as_str(),
            language_id = document.language_id().as_str(),
            version = document.version(),
            text_len = document.text().len(),
            incremental = false,
            "changing document with full text replacement"
        );

        let Some(plugin) = self.plugins.get(document.language_id()) else {
            tracing::warn!(
                target: logging::SERVICE_TARGET,
                operation = "document.change",
                uri = document.uri().as_str(),
                language_id = document.language_id().as_str(),
                version = document.version(),
                "unsupported language"
            );
            anyhow::bail!("unsupported language '{}'", document.language_id());
        };
        let parsed = match plugin.parse(ParseRequest {
            document: &document,
            // Full-text changes do not carry a tree-sitter InputEdit yet.
            old_tree: None,
            profiler: &mut self.profiler,
        }) {
            Ok(parsed) => parsed,
            Err(error) => {
                tracing::warn!(
                    target: logging::SERVICE_TARGET,
                    operation = "document.change",
                    uri = document.uri().as_str(),
                    language_id = document.language_id().as_str(),
                    version = document.version(),
                    error = %error,
                    "failed to parse changed document"
                );
                return Err(error);
            }
        };
        let parsed: Arc<dyn ParsedDocument> = Arc::from(parsed);
        if let Some(workspace) = plugin.workspace() {
            workspace.did_change(&document, parsed.as_ref(), &mut self.profiler)?;
        }
        self.profiler.increment("document.change");
        tracing::info!(
            target: logging::SERVICE_TARGET,
            operation = "document.change",
            uri = document.uri().as_str(),
            language_id = document.language_id().as_str(),
            version = document.version(),
            incremental = false,
            "document changed"
        );
        self.documents
            .insert(uri.clone(), DocumentState { document, parsed });
        Ok(())
    }

    pub fn edit_document(
        &mut self,
        uri: &DocumentUri,
        version: i32,
        edits: impl IntoIterator<Item = TextEdit>,
    ) -> anyhow::Result<()> {
        let edits = edits.into_iter().collect::<Vec<_>>();
        let edit_count = edits.len();
        let (language_id, mut text, old_tree, old_parsed) = {
            let Some(old_state) = self.documents.get(uri) else {
                tracing::warn!(
                    target: logging::SERVICE_TARGET,
                    operation = "document.edit",
                    uri = uri.as_str(),
                    version,
                    edit_count,
                    "edit for unopened document"
                );
                anyhow::bail!("document not open: {uri}");
            };
            (
                old_state.document.language_id().clone(),
                old_state.document.text().to_owned(),
                old_state.parsed.tree().clone(),
                old_state.parsed.clone(),
            )
        };

        tracing::info!(
            target: logging::SERVICE_TARGET,
            operation = "document.edit",
            uri = uri.as_str(),
            language_id = language_id.as_str(),
            version,
            edit_count,
            incremental = true,
            "editing document"
        );

        if edits.is_empty() {
            let document = DocumentSnapshot::new(uri.clone(), language_id, version, text);
            let Some(plugin) = self.plugins.get(document.language_id()) else {
                tracing::warn!(
                    target: logging::SERVICE_TARGET,
                    operation = "document.edit",
                    uri = document.uri().as_str(),
                    language_id = document.language_id().as_str(),
                    version = document.version(),
                    edit_count = 0,
                    "unsupported language"
                );
                anyhow::bail!("unsupported language '{}'", document.language_id());
            };
            if let Some(workspace) = plugin.workspace() {
                workspace.did_change(&document, old_parsed.as_ref(), &mut self.profiler)?;
            }
            self.documents.insert(
                uri.clone(),
                DocumentState {
                    document,
                    parsed: old_parsed,
                },
            );
            tracing::debug!(
                target: logging::SERVICE_TARGET,
                operation = "document.edit",
                uri = uri.as_str(),
                version,
                edit_count = 0,
                "document edit skipped"
            );
            return Ok(());
        }

        let mut edited_tree = old_tree;
        let mut text_index = crate::TextIndex::new(&text);
        for (edit_index, edit) in edits.into_iter().enumerate() {
            tracing::trace!(
                target: logging::SERVICE_TARGET,
                operation = "document.edit.apply",
                uri = uri.as_str(),
                version,
                edit_index,
                start_line = edit.range.start.line,
                start_character = edit.range.start.character,
                end_line = edit.range.end.line,
                end_character = edit.range.end.character,
                new_text_len = edit.new_text.len(),
                "applying document edit"
            );
            let input_edit = text_index.apply_edit(&mut text, edit.range, &edit.new_text)?;
            edited_tree.edit(&input_edit);
            text_index = crate::TextIndex::new(&text);
        }

        let document = DocumentSnapshot::new(uri.clone(), language_id, version, text);
        let Some(plugin) = self.plugins.get(document.language_id()) else {
            tracing::warn!(
                target: logging::SERVICE_TARGET,
                operation = "document.edit",
                uri = document.uri().as_str(),
                language_id = document.language_id().as_str(),
                version = document.version(),
                edit_count,
                "unsupported language"
            );
            anyhow::bail!("unsupported language '{}'", document.language_id());
        };
        let parsed = match plugin.parse(ParseRequest {
            document: &document,
            old_tree: Some(&edited_tree),
            profiler: &mut self.profiler,
        }) {
            Ok(parsed) => parsed,
            Err(error) => {
                tracing::warn!(
                    target: logging::SERVICE_TARGET,
                    operation = "document.edit",
                    uri = document.uri().as_str(),
                    language_id = document.language_id().as_str(),
                    version = document.version(),
                    edit_count,
                    error = %error,
                    "failed to parse edited document"
                );
                return Err(error);
            }
        };
        let parsed: Arc<dyn ParsedDocument> = Arc::from(parsed);
        if let Some(workspace) = plugin.workspace() {
            workspace.did_change(&document, parsed.as_ref(), &mut self.profiler)?;
        }
        self.profiler.increment("document.edit");
        tracing::info!(
            target: logging::SERVICE_TARGET,
            operation = "document.edit",
            uri = document.uri().as_str(),
            language_id = document.language_id().as_str(),
            version = document.version(),
            edit_count,
            text_len = document.text().len(),
            incremental = true,
            "document edited"
        );
        self.documents
            .insert(uri.clone(), DocumentState { document, parsed });
        Ok(())
    }

    pub fn close_document(&mut self, uri: &DocumentUri) {
        let removed = self.documents.remove(uri);
        if let Some(state) = &removed
            && let Some(plugin) = self.plugins.get(state.document.language_id())
            && let Some(workspace) = plugin.workspace()
        {
            workspace.did_close(uri);
        }
        let removed = removed.is_some();
        tracing::info!(
            target: logging::SERVICE_TARGET,
            operation = "document.close",
            uri = uri.as_str(),
            removed,
            "closed document"
        );
    }

    pub fn definition(
        &mut self,
        uri: &DocumentUri,
        position: Position,
    ) -> anyhow::Result<Vec<Location>> {
        let Some(state) = self.documents.get(uri) else {
            tracing::warn!(
                target: logging::SERVICE_TARGET,
                operation = "definition",
                uri = uri.as_str(),
                line = position.line,
                character = position.character,
                "definition request for unopened document"
            );
            anyhow::bail!("document not open: {uri}");
        };
        tracing::info!(
            target: logging::SERVICE_TARGET,
            operation = "definition",
            uri = state.document.uri().as_str(),
            language_id = state.document.language_id().as_str(),
            version = state.document.version(),
            line = position.line,
            character = position.character,
            "definition requested"
        );

        let Some(plugin) = self.plugins.get(state.document.language_id()) else {
            tracing::warn!(
                target: logging::SERVICE_TARGET,
                operation = "definition",
                uri = state.document.uri().as_str(),
                language_id = state.document.language_id().as_str(),
                version = state.document.version(),
                "unsupported language"
            );
            anyhow::bail!("unsupported language '{}'", state.document.language_id());
        };
        if !plugin.capabilities().definition {
            tracing::debug!(
                target: logging::SERVICE_TARGET,
                operation = "definition",
                uri = state.document.uri().as_str(),
                language_id = state.document.language_id().as_str(),
                version = state.document.version(),
                "definition unsupported by language"
            );
            return Ok(Vec::new());
        }

        let started_at = self.profiler.start();
        let result = plugin.definition(DefinitionRequest {
            context: PluginContext {
                document: &state.document,
                parsed: state.parsed.as_ref(),
                profiler: &mut self.profiler,
            },
            position,
        });
        self.profiler.finish("definition", started_at);
        match &result {
            Ok(locations) => {
                tracing::info!(
                    target: logging::SERVICE_TARGET,
                    operation = "definition",
                    uri = state.document.uri().as_str(),
                    language_id = state.document.language_id().as_str(),
                    version = state.document.version(),
                    line = position.line,
                    character = position.character,
                    result_count = locations.len(),
                    "definition completed"
                );
            }
            Err(error) => {
                tracing::warn!(
                    target: logging::SERVICE_TARGET,
                    operation = "definition",
                    uri = state.document.uri().as_str(),
                    language_id = state.document.language_id().as_str(),
                    version = state.document.version(),
                    line = position.line,
                    character = position.character,
                    error = %error,
                    "definition failed"
                );
            }
        }
        result
    }

    pub fn type_definition(
        &mut self,
        uri: &DocumentUri,
        position: Position,
    ) -> anyhow::Result<Vec<Location>> {
        let Some(state) = self.documents.get(uri) else {
            anyhow::bail!("document not open: {uri}");
        };
        let Some(plugin) = self.plugins.get(state.document.language_id()) else {
            anyhow::bail!("unsupported language '{}'", state.document.language_id());
        };
        if !plugin.capabilities().type_definition {
            return Ok(Vec::new());
        }

        let started_at = self.profiler.start();
        let result = plugin.type_definition(TypeDefinitionRequest {
            context: PluginContext {
                document: &state.document,
                parsed: state.parsed.as_ref(),
                profiler: &mut self.profiler,
            },
            position,
        });
        self.profiler.finish("type_definition", started_at);

        tracing::info!(
            target: logging::SERVICE_TARGET,
            operation = "type_definition",
            uri = state.document.uri().as_str(),
            language_id = state.document.language_id().as_str(),
            version = state.document.version(),
            line = position.line,
            character = position.character,
            result_count = result.as_ref().map_or(0, Vec::len),
            "type definition completed"
        );
        result
    }

    pub fn hover(
        &mut self,
        uri: &DocumentUri,
        position: Position,
    ) -> anyhow::Result<Option<Hover>> {
        let Some(state) = self.documents.get(uri) else {
            tracing::warn!(
                target: logging::SERVICE_TARGET,
                operation = "hover",
                uri = uri.as_str(),
                line = position.line,
                character = position.character,
                "hover request for unopened document"
            );
            anyhow::bail!("document not open: {uri}");
        };
        tracing::info!(
            target: logging::SERVICE_TARGET,
            operation = "hover",
            uri = state.document.uri().as_str(),
            language_id = state.document.language_id().as_str(),
            version = state.document.version(),
            line = position.line,
            character = position.character,
            "hover requested"
        );

        let Some(plugin) = self.plugins.get(state.document.language_id()) else {
            tracing::warn!(
                target: logging::SERVICE_TARGET,
                operation = "hover",
                uri = state.document.uri().as_str(),
                language_id = state.document.language_id().as_str(),
                version = state.document.version(),
                "unsupported language"
            );
            anyhow::bail!("unsupported language '{}'", state.document.language_id());
        };
        if !plugin.capabilities().hover {
            tracing::debug!(
                target: logging::SERVICE_TARGET,
                operation = "hover",
                uri = state.document.uri().as_str(),
                language_id = state.document.language_id().as_str(),
                version = state.document.version(),
                "hover unsupported by language"
            );
            return Ok(None);
        }

        let started_at = self.profiler.start();
        let result = plugin.hover(HoverRequest {
            context: PluginContext {
                document: &state.document,
                parsed: state.parsed.as_ref(),
                profiler: &mut self.profiler,
            },
            position,
        });
        self.profiler.finish("hover", started_at);
        match &result {
            Ok(hover) => {
                tracing::info!(
                    target: logging::SERVICE_TARGET,
                    operation = "hover",
                    uri = state.document.uri().as_str(),
                    language_id = state.document.language_id().as_str(),
                    version = state.document.version(),
                    line = position.line,
                    character = position.character,
                    has_result = hover.is_some(),
                    "hover completed"
                );
            }
            Err(error) => {
                tracing::warn!(
                    target: logging::SERVICE_TARGET,
                    operation = "hover",
                    uri = state.document.uri().as_str(),
                    language_id = state.document.language_id().as_str(),
                    version = state.document.version(),
                    line = position.line,
                    character = position.character,
                    error = %error,
                    "hover failed"
                );
            }
        }
        result
    }

    pub fn type_at_position(
        &mut self,
        uri: &DocumentUri,
        position: Position,
    ) -> anyhow::Result<Option<TypeAtPosition>> {
        let Some(state) = self.documents.get(uri) else {
            anyhow::bail!("document not open: {uri}");
        };
        let Some(plugin) = self.plugins.get(state.document.language_id()) else {
            anyhow::bail!("unsupported language '{}'", state.document.language_id());
        };
        if !plugin.capabilities().type_at_position {
            return Ok(None);
        }

        let started_at = self.profiler.start();
        let result = plugin.type_at_position(TypeAtPositionRequest {
            context: PluginContext {
                document: &state.document,
                parsed: state.parsed.as_ref(),
                profiler: &mut self.profiler,
            },
            position,
        });
        self.profiler.finish("type_at_position", started_at);
        result
    }

    pub fn references(
        &mut self,
        uri: &DocumentUri,
        position: Position,
        include_declaration: bool,
    ) -> anyhow::Result<Vec<Location>> {
        let Some(state) = self.documents.get(uri) else {
            tracing::warn!(
                target: logging::SERVICE_TARGET,
                operation = "references",
                uri = uri.as_str(),
                line = position.line,
                character = position.character,
                include_declaration,
                "references request for unopened document"
            );
            anyhow::bail!("document not open: {uri}");
        };
        tracing::info!(
            target: logging::SERVICE_TARGET,
            operation = "references",
            uri = state.document.uri().as_str(),
            language_id = state.document.language_id().as_str(),
            version = state.document.version(),
            line = position.line,
            character = position.character,
            include_declaration,
            "references requested"
        );

        let Some(plugin) = self.plugins.get(state.document.language_id()) else {
            tracing::warn!(
                target: logging::SERVICE_TARGET,
                operation = "references",
                uri = state.document.uri().as_str(),
                language_id = state.document.language_id().as_str(),
                version = state.document.version(),
                "unsupported language"
            );
            anyhow::bail!("unsupported language '{}'", state.document.language_id());
        };
        if !plugin.capabilities().references {
            tracing::debug!(
                target: logging::SERVICE_TARGET,
                operation = "references",
                uri = state.document.uri().as_str(),
                language_id = state.document.language_id().as_str(),
                version = state.document.version(),
                "references unsupported by language"
            );
            return Ok(Vec::new());
        }

        let started_at = self.profiler.start();
        let result = plugin.references(ReferenceRequest {
            context: PluginContext {
                document: &state.document,
                parsed: state.parsed.as_ref(),
                profiler: &mut self.profiler,
            },
            position,
            include_declaration,
        });
        self.profiler.finish("references", started_at);
        match &result {
            Ok(locations) => {
                tracing::info!(
                    target: logging::SERVICE_TARGET,
                    operation = "references",
                    uri = state.document.uri().as_str(),
                    language_id = state.document.language_id().as_str(),
                    version = state.document.version(),
                    line = position.line,
                    character = position.character,
                    include_declaration,
                    result_count = locations.len(),
                    "references completed"
                );
            }
            Err(error) => {
                tracing::warn!(
                    target: logging::SERVICE_TARGET,
                    operation = "references",
                    uri = state.document.uri().as_str(),
                    language_id = state.document.language_id().as_str(),
                    version = state.document.version(),
                    line = position.line,
                    character = position.character,
                    include_declaration,
                    error = %error,
                    "references failed"
                );
            }
        }
        result
    }

    pub fn document_highlights(
        &mut self,
        uri: &DocumentUri,
        position: Position,
    ) -> anyhow::Result<Vec<DocumentHighlight>> {
        let Some(state) = self.documents.get(uri) else {
            anyhow::bail!("document not open: {uri}");
        };
        let Some(plugin) = self.plugins.get(state.document.language_id()) else {
            anyhow::bail!("unsupported language '{}'", state.document.language_id());
        };
        if !plugin.capabilities().document_highlight {
            return Ok(Vec::new());
        }

        let started_at = self.profiler.start();
        let result = plugin.document_highlights(DocumentHighlightRequest {
            context: PluginContext {
                document: &state.document,
                parsed: state.parsed.as_ref(),
                profiler: &mut self.profiler,
            },
            position,
        });
        self.profiler.finish("document_highlights", started_at);

        tracing::info!(
            target: logging::SERVICE_TARGET,
            operation = "document_highlights",
            uri = state.document.uri().as_str(),
            language_id = state.document.language_id().as_str(),
            version = state.document.version(),
            line = position.line,
            character = position.character,
            result_count = result.as_ref().map_or(0, Vec::len),
            "document highlights completed"
        );
        result
    }

    pub fn completion(
        &mut self,
        uri: &DocumentUri,
        position: Position,
        trigger: CompletionTrigger,
    ) -> anyhow::Result<CompletionList> {
        let Some(state) = self.documents.get(uri) else {
            tracing::warn!(
                target: logging::SERVICE_TARGET,
                operation = "completion",
                uri = uri.as_str(),
                line = position.line,
                character = position.character,
                "completion request for unopened document"
            );
            anyhow::bail!("document not open: {uri}");
        };
        tracing::debug!(
            target: logging::SERVICE_TARGET,
            operation = "completion",
            uri = state.document.uri().as_str(),
            language_id = state.document.language_id().as_str(),
            version = state.document.version(),
            line = position.line,
            character = position.character,
            trigger_kind = ?trigger.kind,
            trigger_character = trigger.character.as_deref(),
            "completion requested"
        );

        let Some(plugin) = self.plugins.get(state.document.language_id()) else {
            anyhow::bail!("unsupported language '{}'", state.document.language_id());
        };
        if !plugin.capabilities().completion {
            return Ok(CompletionList::default());
        }

        let started_at = self.profiler.start();
        let result = plugin.completion(CompletionRequest {
            context: PluginContext {
                document: &state.document,
                parsed: state.parsed.as_ref(),
                profiler: &mut self.profiler,
            },
            position,
            trigger,
        });
        self.profiler.finish("completion", started_at);
        match &result {
            Ok(completion) => tracing::debug!(
                target: logging::SERVICE_TARGET,
                operation = "completion",
                uri = state.document.uri().as_str(),
                language_id = state.document.language_id().as_str(),
                version = state.document.version(),
                line = position.line,
                character = position.character,
                result_count = completion.items.len(),
                is_incomplete = completion.is_incomplete,
                "completion completed"
            ),
            Err(error) => tracing::warn!(
                target: logging::SERVICE_TARGET,
                operation = "completion",
                uri = state.document.uri().as_str(),
                language_id = state.document.language_id().as_str(),
                version = state.document.version(),
                line = position.line,
                character = position.character,
                error = %error,
                "completion failed"
            ),
        }
        result
    }

    pub fn semantic_tokens(&mut self, uri: &DocumentUri) -> anyhow::Result<SemanticTokens> {
        let Some(state) = self.documents.get(uri) else {
            tracing::warn!(
                target: logging::SERVICE_TARGET,
                operation = "semantic_tokens",
                uri = uri.as_str(),
                "semantic tokens request for unopened document"
            );
            anyhow::bail!("document not open: {uri}");
        };
        tracing::info!(
            target: logging::SERVICE_TARGET,
            operation = "semantic_tokens",
            uri = state.document.uri().as_str(),
            language_id = state.document.language_id().as_str(),
            version = state.document.version(),
            "semantic tokens requested"
        );

        let Some(plugin) = self.plugins.get(state.document.language_id()) else {
            tracing::warn!(
                target: logging::SERVICE_TARGET,
                operation = "semantic_tokens",
                uri = state.document.uri().as_str(),
                language_id = state.document.language_id().as_str(),
                version = state.document.version(),
                "unsupported language"
            );
            anyhow::bail!("unsupported language '{}'", state.document.language_id());
        };
        if !plugin.capabilities().semantic_tokens {
            tracing::debug!(
                target: logging::SERVICE_TARGET,
                operation = "semantic_tokens",
                uri = state.document.uri().as_str(),
                language_id = state.document.language_id().as_str(),
                version = state.document.version(),
                "semantic tokens unsupported by language"
            );
            return Ok(SemanticTokens::new(Vec::new()));
        }

        let started_at = self.profiler.start();
        let result = plugin.semantic_tokens(SemanticTokensRequest {
            context: PluginContext {
                document: &state.document,
                parsed: state.parsed.as_ref(),
                profiler: &mut self.profiler,
            },
        });
        self.profiler.finish("semantic_tokens", started_at);
        match &result {
            Ok(tokens) => {
                tracing::info!(
                    target: logging::SERVICE_TARGET,
                    operation = "semantic_tokens",
                    uri = state.document.uri().as_str(),
                    language_id = state.document.language_id().as_str(),
                    version = state.document.version(),
                    result_count = tokens.len(),
                    "semantic tokens completed"
                );
            }
            Err(error) => {
                tracing::warn!(
                    target: logging::SERVICE_TARGET,
                    operation = "semantic_tokens",
                    uri = state.document.uri().as_str(),
                    language_id = state.document.language_id().as_str(),
                    version = state.document.version(),
                    error = %error,
                    "semantic tokens failed"
                );
            }
        }
        result.map(SemanticTokens::new)
    }

    pub fn inlay_hints(
        &mut self,
        uri: &DocumentUri,
        range: Range,
    ) -> anyhow::Result<Vec<InlayHint>> {
        let Some(state) = self.documents.get(uri) else {
            tracing::warn!(
                target: logging::SERVICE_TARGET,
                operation = "inlay_hints",
                uri = uri.as_str(),
                "inlay hints request for unopened document"
            );
            anyhow::bail!("document not open: {uri}");
        };
        tracing::info!(
            target: logging::SERVICE_TARGET,
            operation = "inlay_hints",
            uri = state.document.uri().as_str(),
            language_id = state.document.language_id().as_str(),
            version = state.document.version(),
            start_line = range.start.line,
            start_character = range.start.character,
            end_line = range.end.line,
            end_character = range.end.character,
            "inlay hints requested"
        );

        let Some(plugin) = self.plugins.get(state.document.language_id()) else {
            tracing::warn!(
                target: logging::SERVICE_TARGET,
                operation = "inlay_hints",
                uri = state.document.uri().as_str(),
                language_id = state.document.language_id().as_str(),
                version = state.document.version(),
                "unsupported language"
            );
            anyhow::bail!("unsupported language '{}'", state.document.language_id());
        };
        if !plugin.capabilities().inlay_hints {
            tracing::debug!(
                target: logging::SERVICE_TARGET,
                operation = "inlay_hints",
                uri = state.document.uri().as_str(),
                language_id = state.document.language_id().as_str(),
                version = state.document.version(),
                "inlay hints unsupported by language"
            );
            return Ok(Vec::new());
        }

        let started_at = self.profiler.start();
        let result = plugin.inlay_hints(InlayHintRequest {
            context: PluginContext {
                document: &state.document,
                parsed: state.parsed.as_ref(),
                profiler: &mut self.profiler,
            },
            range,
        });
        self.profiler.finish("inlay_hints", started_at);
        match &result {
            Ok(hints) => {
                tracing::info!(
                    target: logging::SERVICE_TARGET,
                    operation = "inlay_hints",
                    uri = state.document.uri().as_str(),
                    language_id = state.document.language_id().as_str(),
                    version = state.document.version(),
                    result_count = hints.len(),
                    "inlay hints completed"
                );
            }
            Err(error) => {
                tracing::warn!(
                    target: logging::SERVICE_TARGET,
                    operation = "inlay_hints",
                    uri = state.document.uri().as_str(),
                    language_id = state.document.language_id().as_str(),
                    version = state.document.version(),
                    error = %error,
                    "inlay hints failed"
                );
            }
        }
        result
    }

    pub fn code_lens(&mut self, uri: &DocumentUri) -> anyhow::Result<Vec<CodeLens>> {
        let Some(state) = self.documents.get(uri) else {
            tracing::warn!(
                target: logging::SERVICE_TARGET,
                operation = "code_lens",
                uri = uri.as_str(),
                "code lens request for unopened document"
            );
            anyhow::bail!("document not open: {uri}");
        };
        tracing::info!(
            target: logging::SERVICE_TARGET,
            operation = "code_lens",
            uri = state.document.uri().as_str(),
            language_id = state.document.language_id().as_str(),
            version = state.document.version(),
            "code lens requested"
        );

        let Some(plugin) = self.plugins.get(state.document.language_id()) else {
            tracing::warn!(
                target: logging::SERVICE_TARGET,
                operation = "code_lens",
                uri = state.document.uri().as_str(),
                language_id = state.document.language_id().as_str(),
                version = state.document.version(),
                "unsupported language"
            );
            anyhow::bail!("unsupported language '{}'", state.document.language_id());
        };
        if !plugin.capabilities().code_lens {
            tracing::debug!(
                target: logging::SERVICE_TARGET,
                operation = "code_lens",
                uri = state.document.uri().as_str(),
                language_id = state.document.language_id().as_str(),
                version = state.document.version(),
                "code lens unsupported by language"
            );
            return Ok(Vec::new());
        }

        let started_at = self.profiler.start();
        let result = plugin.code_lens(CodeLensRequest {
            context: PluginContext {
                document: &state.document,
                parsed: state.parsed.as_ref(),
                profiler: &mut self.profiler,
            },
        });
        self.profiler.finish("code_lens", started_at);
        match &result {
            Ok(lenses) => {
                tracing::info!(
                    target: logging::SERVICE_TARGET,
                    operation = "code_lens",
                    uri = state.document.uri().as_str(),
                    language_id = state.document.language_id().as_str(),
                    version = state.document.version(),
                    result_count = lenses.len(),
                    "code lens completed"
                );
            }
            Err(error) => {
                tracing::warn!(
                    target: logging::SERVICE_TARGET,
                    operation = "code_lens",
                    uri = state.document.uri().as_str(),
                    language_id = state.document.language_id().as_str(),
                    version = state.document.version(),
                    error = %error,
                    "code lens failed"
                );
            }
        }
        result
    }

    pub fn folding_ranges(&mut self, uri: &DocumentUri) -> anyhow::Result<Vec<FoldingRange>> {
        let Some(state) = self.documents.get(uri) else {
            tracing::warn!(
                target: logging::SERVICE_TARGET,
                operation = "folding_ranges",
                uri = uri.as_str(),
                "folding range request for unopened document"
            );
            anyhow::bail!("document not open: {uri}");
        };
        tracing::info!(
            target: logging::SERVICE_TARGET,
            operation = "folding_ranges",
            uri = state.document.uri().as_str(),
            language_id = state.document.language_id().as_str(),
            version = state.document.version(),
            "folding ranges requested"
        );

        let Some(plugin) = self.plugins.get(state.document.language_id()) else {
            tracing::warn!(
                target: logging::SERVICE_TARGET,
                operation = "folding_ranges",
                uri = state.document.uri().as_str(),
                language_id = state.document.language_id().as_str(),
                version = state.document.version(),
                "unsupported language"
            );
            anyhow::bail!("unsupported language '{}'", state.document.language_id());
        };
        if !plugin.capabilities().folding_ranges {
            tracing::debug!(
                target: logging::SERVICE_TARGET,
                operation = "folding_ranges",
                uri = state.document.uri().as_str(),
                language_id = state.document.language_id().as_str(),
                version = state.document.version(),
                "folding ranges unsupported by language"
            );
            return Ok(Vec::new());
        }

        let started_at = self.profiler.start();
        let result = plugin.folding_ranges(FoldingRangeRequest {
            context: PluginContext {
                document: &state.document,
                parsed: state.parsed.as_ref(),
                profiler: &mut self.profiler,
            },
        });
        self.profiler.finish("folding_ranges", started_at);
        match &result {
            Ok(ranges) => {
                tracing::info!(
                    target: logging::SERVICE_TARGET,
                    operation = "folding_ranges",
                    uri = state.document.uri().as_str(),
                    language_id = state.document.language_id().as_str(),
                    version = state.document.version(),
                    result_count = ranges.len(),
                    "folding ranges completed"
                );
            }
            Err(error) => {
                tracing::warn!(
                    target: logging::SERVICE_TARGET,
                    operation = "folding_ranges",
                    uri = state.document.uri().as_str(),
                    language_id = state.document.language_id().as_str(),
                    version = state.document.version(),
                    error = %error,
                    "folding ranges failed"
                );
            }
        }
        result
    }

    pub fn document_symbols(&mut self, uri: &DocumentUri) -> anyhow::Result<Vec<DocumentSymbol>> {
        let Some(state) = self.documents.get(uri) else {
            anyhow::bail!("document not open: {uri}");
        };
        let Some(plugin) = self.plugins.get(state.document.language_id()) else {
            anyhow::bail!("unsupported language '{}'", state.document.language_id());
        };
        if !plugin.capabilities().document_symbols {
            return Ok(Vec::new());
        }

        let started_at = self.profiler.start();
        let result = plugin.document_symbols(DocumentSymbolRequest {
            context: PluginContext {
                document: &state.document,
                parsed: state.parsed.as_ref(),
                profiler: &mut self.profiler,
            },
        });
        self.profiler.finish("document_symbols", started_at);
        result
    }

    pub fn workspace_symbols(&mut self, query: &str) -> anyhow::Result<Vec<WorkspaceSymbol>> {
        let started_at = self.profiler.start();
        let mut symbols = Vec::new();

        for plugin in self.plugins.values() {
            if !plugin.capabilities().workspace_symbols {
                continue;
            }
            symbols.extend(plugin.workspace_symbols(WorkspaceSymbolRequest {
                query,
                profiler: &mut self.profiler,
            })?);
        }

        symbols.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then(left.location.uri.as_str().cmp(right.location.uri.as_str()))
                .then(left.location.range.start.cmp(&right.location.range.start))
        });
        self.profiler.finish("workspace_symbols", started_at);
        Ok(symbols)
    }

    pub fn code_actions(
        &mut self,
        uri: &DocumentUri,
        range: Range,
    ) -> anyhow::Result<Vec<CodeAction>> {
        let Some(state) = self.documents.get(uri) else {
            anyhow::bail!("document not open: {uri}");
        };
        let Some(plugin) = self.plugins.get(state.document.language_id()) else {
            anyhow::bail!("unsupported language '{}'", state.document.language_id());
        };
        if !plugin.capabilities().code_actions {
            return Ok(Vec::new());
        }

        let started_at = self.profiler.start();
        let result = plugin.code_actions(CodeActionRequest {
            context: PluginContext {
                document: &state.document,
                parsed: state.parsed.as_ref(),
                profiler: &mut self.profiler,
            },
            range,
        });
        self.profiler.finish("code_actions", started_at);
        result
    }

    pub fn will_rename_files(
        &mut self,
        files: &[FileRename],
    ) -> anyhow::Result<Option<WorkspaceEdit>> {
        let started_at = self.profiler.start();
        let mut documents = BTreeMap::<String, DocumentEdits>::new();

        for plugin in self.plugins.values() {
            if !plugin.capabilities().file_rename {
                continue;
            }
            let Some(edit) = plugin.will_rename_files(FileRenameRequest {
                files,
                profiler: &mut self.profiler,
            })?
            else {
                continue;
            };
            for document in edit.documents {
                documents
                    .entry(document.uri.as_str().to_owned())
                    .or_insert_with(|| DocumentEdits::new(document.uri, Vec::new()))
                    .edits
                    .extend(document.edits);
            }
        }

        self.profiler.finish("files.rename.prepare", started_at);
        let documents = documents.into_values().collect::<Vec<_>>();
        Ok((!documents.is_empty()).then(|| WorkspaceEdit::new(documents)))
    }

    pub fn did_rename_files(&mut self, files: &[FileRename]) -> anyhow::Result<()> {
        for plugin in self.plugins.values() {
            if plugin.capabilities().file_rename {
                plugin.did_rename_files(files)?;
            }
        }
        for rename in files {
            let Some(state) = self.documents.remove(&rename.old_uri) else {
                continue;
            };
            let document = DocumentSnapshot::new(
                rename.new_uri.clone(),
                state.document.language_id().clone(),
                state.document.version(),
                Arc::<str>::from(state.document.text()),
            );
            self.documents.insert(
                rename.new_uri.clone(),
                DocumentState {
                    document,
                    parsed: state.parsed,
                },
            );
        }
        self.profiler.increment("files.rename");
        Ok(())
    }

    pub fn signature_help(
        &mut self,
        uri: &DocumentUri,
        position: Position,
    ) -> anyhow::Result<Option<SignatureHelp>> {
        let Some(state) = self.documents.get(uri) else {
            tracing::warn!(
                target: logging::SERVICE_TARGET,
                operation = "signature_help",
                uri = uri.as_str(),
                line = position.line,
                character = position.character,
                "signature help request for unopened document"
            );
            anyhow::bail!("document not open: {uri}");
        };

        let Some(plugin) = self.plugins.get(state.document.language_id()) else {
            anyhow::bail!("unsupported language '{}'", state.document.language_id());
        };
        if !plugin.capabilities().signature_help {
            return Ok(None);
        }

        let started_at = self.profiler.start();
        let result = plugin.signature_help(SignatureHelpRequest {
            context: PluginContext {
                document: &state.document,
                parsed: state.parsed.as_ref(),
                profiler: &mut self.profiler,
            },
            position,
        });
        self.profiler.finish("signature_help", started_at);

        match &result {
            Ok(signature_help) => {
                tracing::info!(
                    target: logging::SERVICE_TARGET,
                    operation = "signature_help",
                    uri = state.document.uri().as_str(),
                    language_id = state.document.language_id().as_str(),
                    version = state.document.version(),
                    line = position.line,
                    character = position.character,
                    has_result = signature_help.is_some(),
                    "signature help completed"
                );
            }
            Err(error) => {
                tracing::warn!(
                    target: logging::SERVICE_TARGET,
                    operation = "signature_help",
                    uri = state.document.uri().as_str(),
                    language_id = state.document.language_id().as_str(),
                    version = state.document.version(),
                    line = position.line,
                    character = position.character,
                    error = %error,
                    "signature help failed"
                );
            }
        }

        result
    }

    pub fn prepare_rename(
        &mut self,
        uri: &DocumentUri,
        position: Position,
    ) -> anyhow::Result<Option<PrepareRename>> {
        let Some(state) = self.documents.get(uri) else {
            anyhow::bail!("document not open: {uri}");
        };
        let Some(plugin) = self.plugins.get(state.document.language_id()) else {
            anyhow::bail!("unsupported language '{}'", state.document.language_id());
        };
        if !plugin.capabilities().rename {
            return Ok(None);
        }

        let started_at = self.profiler.start();
        let result = plugin.prepare_rename(PrepareRenameRequest {
            context: PluginContext {
                document: &state.document,
                parsed: state.parsed.as_ref(),
                profiler: &mut self.profiler,
            },
            position,
        });
        self.profiler.finish("rename.prepare", started_at);

        tracing::debug!(
            target: logging::SERVICE_TARGET,
            operation = "rename.prepare",
            uri = state.document.uri().as_str(),
            language_id = state.document.language_id().as_str(),
            version = state.document.version(),
            line = position.line,
            character = position.character,
            has_result = result.as_ref().is_ok_and(Option::is_some),
            "prepare rename completed"
        );
        result
    }

    pub fn rename(
        &mut self,
        uri: &DocumentUri,
        position: Position,
        new_name: &str,
    ) -> anyhow::Result<Option<WorkspaceEdit>> {
        let Some(state) = self.documents.get(uri) else {
            anyhow::bail!("document not open: {uri}");
        };
        let Some(plugin) = self.plugins.get(state.document.language_id()) else {
            anyhow::bail!("unsupported language '{}'", state.document.language_id());
        };
        if !plugin.capabilities().rename {
            return Ok(None);
        }

        let started_at = self.profiler.start();
        let result = plugin.rename(RenameRequest {
            context: PluginContext {
                document: &state.document,
                parsed: state.parsed.as_ref(),
                profiler: &mut self.profiler,
            },
            position,
            new_name,
        });
        self.profiler.finish("rename", started_at);

        tracing::info!(
            target: logging::SERVICE_TARGET,
            operation = "rename",
            uri = state.document.uri().as_str(),
            language_id = state.document.language_id().as_str(),
            version = state.document.version(),
            line = position.line,
            character = position.character,
            new_name,
            document_count = result
                .as_ref()
                .ok()
                .and_then(Option::as_ref)
                .map_or(0, |edit| edit.documents.len()),
            "rename completed"
        );
        result
    }

    #[must_use]
    pub const fn profiler(&self) -> &Profiler {
        &self.profiler
    }
}
