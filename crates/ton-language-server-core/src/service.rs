use crate::language::{
    DefinitionRequest, LanguagePlugin, ParseRequest, ParsedDocument, PluginContext,
};
use crate::logging;
use crate::profiling::Profiler;
use crate::types::{DocumentSnapshot, DocumentUri, LanguageId, Location, Position, TextEdit};
use std::collections::HashMap;
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
        self.documents.insert(
            document.uri().clone(),
            DocumentState {
                document,
                parsed: Arc::from(parsed),
            },
        );
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
        self.documents.insert(
            uri.clone(),
            DocumentState {
                document,
                parsed: Arc::from(parsed),
            },
        );
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
            self.documents.insert(
                uri.clone(),
                DocumentState {
                    document: DocumentSnapshot::new(uri.clone(), language_id, version, text),
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
        self.documents.insert(
            uri.clone(),
            DocumentState {
                document,
                parsed: Arc::from(parsed),
            },
        );
        Ok(())
    }

    pub fn close_document(&mut self, uri: &DocumentUri) {
        let removed = self.documents.remove(uri).is_some();
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

    #[must_use]
    pub const fn profiler(&self) -> &Profiler {
        &self.profiler
    }
}
