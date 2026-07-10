use crate::language::{
    CodeActionRequest, CompletionRequest, DefinitionRequest, DocumentHighlightRequest,
    DocumentSymbolRequest, FeatureSet, FileRenameRequest, FoldingRangeRequest, HoverRequest,
    InlayHintRequest, LanguagePlugin, ParseRequest, ParsedDocument, PrepareRenameRequest,
    ReferenceRequest, RenameRequest, SemanticTokensRequest, SignatureHelpRequest,
    TypeDefinitionRequest, WorkspaceLanguage, WorkspaceSymbolRequest,
};
use crate::logging;
use crate::{
    DocumentSnapshot, DocumentUri, LanguageId, Location, Profiler, Range, TextIndex,
    WorkspaceConfig,
};
use anyhow::Context;
use include_dir::{Dir, include_dir};
use serde::Deserialize;
use std::any::Any;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, RwLock};
use tolk_resolver::{
    FileDb, FileId, ProjectIndex, ProjectSource, ProjectSourceProvider, Span, SymbolId,
};
use tolk_ty::{InferenceResult, TypeDb, TypeDbCache, TypeInterner, infer};
use tree_sitter::Tree;

mod code_actions;
mod completion;
mod definition;
mod document_highlights;
mod document_symbols;
mod file_rename;
mod folding;
mod hover;
mod import_edits;
mod inlay_hints;
mod references;
mod rename;
mod resolution;
mod semantic_tokens;
mod signature_help;
mod type_definition;
mod workspace_symbols;

pub const LANGUAGE_ID: &str = "tolk";
const TOLK_STDLIB_PATH: &str = "/__tolk_stdlib__";

static TOLK_STDLIB_DIR: Dir<'static> =
    include_dir!("$CARGO_MANIFEST_DIR/../tolk-compiler/assets/tolk-stdlib");

#[derive(Clone, Debug)]
pub struct TolkLanguage {
    engine: Arc<TolkWorkspaceEngine>,
}

impl TolkLanguage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            engine: Arc::new(TolkWorkspaceEngine::new()),
        }
    }

    /// Adds a provider-backed source file. Open documents with the same logical
    /// path override this content until they are closed.
    pub fn add_source_file(
        &self,
        uri: impl Into<DocumentUri>,
        text: impl Into<Arc<str>>,
    ) -> anyhow::Result<()> {
        self.engine.add_source_file(uri.into(), text.into())
    }
}

impl Default for TolkLanguage {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguagePlugin for TolkLanguage {
    fn language_id(&self) -> LanguageId {
        LanguageId::from(LANGUAGE_ID)
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["tolk"]
    }

    fn capabilities(&self) -> FeatureSet {
        FeatureSet {
            definition: true,
            references: true,
            completion: true,
            semantic_tokens: true,
            inlay_hints: true,
            folding_ranges: true,
            hover: true,
            document_symbols: true,
            signature_help: true,
            rename: true,
            type_definition: true,
            document_highlight: true,
            workspace_symbols: true,
            code_actions: true,
            file_rename: true,
            ..FeatureSet::default()
        }
    }

    fn workspace(&self) -> Option<&dyn WorkspaceLanguage> {
        Some(self)
    }

    fn parse(&self, request: ParseRequest<'_>) -> anyhow::Result<Box<dyn ParsedDocument>> {
        tracing::debug!(
            target: logging::TOLK_TARGET,
            operation = "tolk.parse",
            uri = request.document.uri().as_str(),
            version = request.document.version(),
            incremental = request.old_tree.is_some(),
            text_len = request.document.text().len(),
            "parsing Tolk document"
        );
        let parse_started_at = request.profiler.start();
        let source_file =
            match tolk_syntax::parse_with_old_tree(request.document.text(), request.old_tree) {
                Ok(source_file) => source_file,
                Err(error) => {
                    tracing::debug!(
                        target: logging::TOLK_TARGET,
                        operation = "tolk.parse",
                        uri = request.document.uri().as_str(),
                        version = request.document.version(),
                        incremental = request.old_tree.is_some(),
                        error = %error,
                        "Tolk parse failed"
                    );
                    return Err(error);
                }
            };
        request.profiler.finish("tolk.parse", parse_started_at);
        tracing::debug!(
            target: logging::TOLK_TARGET,
            operation = "tolk.parse",
            uri = request.document.uri().as_str(),
            version = request.document.version(),
            incremental = request.old_tree.is_some(),
            has_error = source_file.tree.root_node().has_error(),
            "parsed Tolk document"
        );

        Ok(Box::new(TolkParsedDocument { source_file }))
    }

    fn definition(&self, request: DefinitionRequest<'_>) -> anyhow::Result<Vec<Location>> {
        let _parsed = request
            .context
            .parsed
            .as_any()
            .downcast_ref::<TolkParsedDocument>()
            .context("Tolk parsed document has an unexpected type")?;
        let started_at = request.context.profiler.start();
        let locations = self
            .engine
            .definition(request.context.document, request.position);
        request
            .context
            .profiler
            .finish("tolk.definition.resolve", started_at);
        tracing::debug!(
            target: logging::TOLK_TARGET,
            operation = "tolk.definition.resolve",
            uri = request.context.document.uri().as_str(),
            version = request.context.document.version(),
            line = request.position.line,
            character = request.position.character,
            result_count = locations.len(),
            "resolved Tolk definition"
        );
        Ok(locations)
    }

    fn references(&self, request: ReferenceRequest<'_>) -> anyhow::Result<Vec<Location>> {
        let _parsed = request
            .context
            .parsed
            .as_any()
            .downcast_ref::<TolkParsedDocument>()
            .context("Tolk parsed document has an unexpected type")?;
        let started_at = request.context.profiler.start();
        let locations = self.engine.references(
            request.context.document,
            request.position,
            request.include_declaration,
        );
        request
            .context
            .profiler
            .finish("tolk.references.resolve", started_at);
        tracing::debug!(
            target: logging::TOLK_TARGET,
            operation = "tolk.references.resolve",
            uri = request.context.document.uri().as_str(),
            version = request.context.document.version(),
            line = request.position.line,
            character = request.position.character,
            include_declaration = request.include_declaration,
            result_count = locations.len(),
            "resolved Tolk references"
        );
        Ok(locations)
    }

    fn semantic_tokens(
        &self,
        request: SemanticTokensRequest<'_>,
    ) -> anyhow::Result<Vec<crate::SemanticToken>> {
        let _parsed = request
            .context
            .parsed
            .as_any()
            .downcast_ref::<TolkParsedDocument>()
            .context("Tolk parsed document has an unexpected type")?;
        let started_at = request.context.profiler.start();
        let tokens = self.engine.semantic_tokens(request.context.document);
        request
            .context
            .profiler
            .finish("tolk.semantic_tokens", started_at);
        tracing::debug!(
            target: logging::TOLK_TARGET,
            operation = "tolk.semantic_tokens",
            uri = request.context.document.uri().as_str(),
            version = request.context.document.version(),
            result_count = tokens.len(),
            "resolved Tolk semantic tokens"
        );
        Ok(tokens)
    }

    fn inlay_hints(&self, request: InlayHintRequest<'_>) -> anyhow::Result<Vec<crate::InlayHint>> {
        let _parsed = request
            .context
            .parsed
            .as_any()
            .downcast_ref::<TolkParsedDocument>()
            .context("Tolk parsed document has an unexpected type")?;
        let started_at = request.context.profiler.start();
        let hints = self
            .engine
            .inlay_hints(request.context.document, request.range);
        request
            .context
            .profiler
            .finish("tolk.inlay_hints", started_at);
        tracing::debug!(
            target: logging::TOLK_TARGET,
            operation = "tolk.inlay_hints",
            uri = request.context.document.uri().as_str(),
            version = request.context.document.version(),
            result_count = hints.len(),
            "resolved Tolk inlay hints"
        );
        Ok(hints)
    }

    fn folding_ranges(
        &self,
        request: FoldingRangeRequest<'_>,
    ) -> anyhow::Result<Vec<crate::FoldingRange>> {
        let parsed = request
            .context
            .parsed
            .as_any()
            .downcast_ref::<TolkParsedDocument>()
            .context("Tolk parsed document has an unexpected type")?;
        let started_at = request.context.profiler.start();
        let ranges = folding::folding_ranges(
            request.context.document,
            parsed.source_file.tree.root_node(),
        );
        request
            .context
            .profiler
            .finish("tolk.folding_ranges", started_at);
        Ok(ranges)
    }

    fn hover(&self, request: HoverRequest<'_>) -> anyhow::Result<Option<crate::Hover>> {
        let _parsed = request
            .context
            .parsed
            .as_any()
            .downcast_ref::<TolkParsedDocument>()
            .context("Tolk parsed document has an unexpected type")?;
        let started_at = request.context.profiler.start();
        let hover = self
            .engine
            .hover(request.context.document, request.position);
        request.context.profiler.finish("tolk.hover", started_at);
        Ok(hover)
    }

    fn document_symbols(
        &self,
        request: DocumentSymbolRequest<'_>,
    ) -> anyhow::Result<Vec<crate::DocumentSymbol>> {
        let _parsed = request
            .context
            .parsed
            .as_any()
            .downcast_ref::<TolkParsedDocument>()
            .context("Tolk parsed document has an unexpected type")?;
        let started_at = request.context.profiler.start();
        let symbols = self.engine.document_symbols(request.context.document);
        request
            .context
            .profiler
            .finish("tolk.document_symbols", started_at);
        Ok(symbols)
    }

    fn signature_help(
        &self,
        request: SignatureHelpRequest<'_>,
    ) -> anyhow::Result<Option<crate::SignatureHelp>> {
        let _parsed = request
            .context
            .parsed
            .as_any()
            .downcast_ref::<TolkParsedDocument>()
            .context("Tolk parsed document has an unexpected type")?;
        let started_at = request.context.profiler.start();
        let signature_help = self
            .engine
            .signature_help(request.context.document, request.position);
        request
            .context
            .profiler
            .finish("tolk.signature_help", started_at);
        Ok(signature_help)
    }

    fn prepare_rename(
        &self,
        request: PrepareRenameRequest<'_>,
    ) -> anyhow::Result<Option<crate::PrepareRename>> {
        let _parsed = request
            .context
            .parsed
            .as_any()
            .downcast_ref::<TolkParsedDocument>()
            .context("Tolk parsed document has an unexpected type")?;
        let started_at = request.context.profiler.start();
        let result = self
            .engine
            .prepare_rename(request.context.document, request.position);
        request
            .context
            .profiler
            .finish("tolk.rename.prepare", started_at);
        result
    }

    fn rename(&self, request: RenameRequest<'_>) -> anyhow::Result<Option<crate::WorkspaceEdit>> {
        let _parsed = request
            .context
            .parsed
            .as_any()
            .downcast_ref::<TolkParsedDocument>()
            .context("Tolk parsed document has an unexpected type")?;
        let started_at = request.context.profiler.start();
        let result =
            self.engine
                .rename(request.context.document, request.position, request.new_name);
        request.context.profiler.finish("tolk.rename", started_at);
        result
    }

    fn type_definition(&self, request: TypeDefinitionRequest<'_>) -> anyhow::Result<Vec<Location>> {
        let _parsed = request
            .context
            .parsed
            .as_any()
            .downcast_ref::<TolkParsedDocument>()
            .context("Tolk parsed document has an unexpected type")?;
        let started_at = request.context.profiler.start();
        let locations = self
            .engine
            .type_definition(request.context.document, request.position);
        request
            .context
            .profiler
            .finish("tolk.type_definition", started_at);
        Ok(locations)
    }

    fn document_highlights(
        &self,
        request: DocumentHighlightRequest<'_>,
    ) -> anyhow::Result<Vec<crate::DocumentHighlight>> {
        let _parsed = request
            .context
            .parsed
            .as_any()
            .downcast_ref::<TolkParsedDocument>()
            .context("Tolk parsed document has an unexpected type")?;
        let started_at = request.context.profiler.start();
        let highlights = self
            .engine
            .document_highlights(request.context.document, request.position);
        request
            .context
            .profiler
            .finish("tolk.document_highlights", started_at);
        Ok(highlights)
    }

    fn workspace_symbols(
        &self,
        request: WorkspaceSymbolRequest<'_>,
    ) -> anyhow::Result<Vec<crate::WorkspaceSymbol>> {
        let started_at = request.profiler.start();
        let symbols = self.engine.workspace_symbols(request.query);
        request
            .profiler
            .finish("tolk.workspace_symbols", started_at);
        Ok(symbols)
    }

    fn code_actions(
        &self,
        request: CodeActionRequest<'_>,
    ) -> anyhow::Result<Vec<crate::CodeAction>> {
        let _parsed = request
            .context
            .parsed
            .as_any()
            .downcast_ref::<TolkParsedDocument>()
            .context("Tolk parsed document has an unexpected type")?;
        let started_at = request.context.profiler.start();
        let actions = self
            .engine
            .code_actions(request.context.document, request.range);
        request
            .context
            .profiler
            .finish("tolk.code_actions", started_at);
        Ok(actions)
    }

    fn will_rename_files(
        &self,
        request: FileRenameRequest<'_>,
    ) -> anyhow::Result<Option<crate::WorkspaceEdit>> {
        let started_at = request.profiler.start();
        let edit = self.engine.will_rename_files(request.files);
        request
            .profiler
            .finish("tolk.files.rename.prepare", started_at);
        Ok(edit)
    }

    fn did_rename_files(&self, files: &[crate::FileRename]) -> anyhow::Result<()> {
        self.engine.did_rename_files(files)
    }

    fn completion(&self, request: CompletionRequest<'_>) -> anyhow::Result<crate::CompletionList> {
        let started_at = request.context.profiler.start();
        let completion = self
            .engine
            .completion(request.context.document, request.position)?;
        request
            .context
            .profiler
            .finish("tolk.completion", started_at);
        tracing::debug!(
            target: logging::TOLK_TARGET,
            operation = "tolk.completion",
            uri = request.context.document.uri().as_str(),
            version = request.context.document.version(),
            line = request.position.line,
            character = request.position.character,
            result_count = completion.items.len(),
            "resolved Tolk completion"
        );
        Ok(completion)
    }
}

impl WorkspaceLanguage for TolkLanguage {
    fn add_source_file(&self, uri: DocumentUri, text: Arc<str>) -> anyhow::Result<()> {
        TolkLanguage::add_source_file(self, uri, text)
    }

    fn remove_source_file(&self, uri: &DocumentUri) -> anyhow::Result<()> {
        self.engine.remove_source_file(uri)
    }

    fn set_workspace_config(&self, config: WorkspaceConfig) -> anyhow::Result<()> {
        self.engine.set_workspace_config(config)
    }

    fn did_open(
        &self,
        document: &DocumentSnapshot,
        parsed: &dyn ParsedDocument,
        profiler: &mut Profiler,
    ) -> anyhow::Result<()> {
        let parsed = parsed
            .as_any()
            .downcast_ref::<TolkParsedDocument>()
            .context("Tolk parsed document has an unexpected type")?;
        self.engine.open_document(document, parsed, profiler)
    }

    fn did_change(
        &self,
        document: &DocumentSnapshot,
        parsed: &dyn ParsedDocument,
        profiler: &mut Profiler,
    ) -> anyhow::Result<()> {
        let parsed = parsed
            .as_any()
            .downcast_ref::<TolkParsedDocument>()
            .context("Tolk parsed document has an unexpected type")?;
        self.engine.open_document(document, parsed, profiler)
    }

    fn did_close(&self, uri: &DocumentUri) {
        self.engine.close_document(uri);
    }
}

#[derive(Debug)]
pub struct TolkParsedDocument {
    source_file: tolk_syntax::SourceFile,
}

impl ParsedDocument for TolkParsedDocument {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn tree(&self) -> &Tree {
        &self.source_file.tree
    }
}

#[derive(Debug)]
struct TolkWorkspaceEngine {
    state: RwLock<TolkWorkspaceState>,
}

impl TolkWorkspaceEngine {
    fn new() -> Self {
        Self {
            state: RwLock::new(TolkWorkspaceState::default()),
        }
    }

    fn add_source_file(&self, uri: DocumentUri, text: Arc<str>) -> anyhow::Result<()> {
        let path = logical_path_for_uri(&uri);
        let mut state = self.state.write().expect("Tolk workspace lock poisoned");
        let file = state.files.entry(path).or_default();
        file.base_uri = Some(uri);
        file.base_text = Some(text);
        file.dirty = true;
        let mut profiler = Profiler::disabled();
        state.rebuild_snapshot(&mut profiler)
    }

    fn remove_source_file(&self, uri: &DocumentUri) -> anyhow::Result<()> {
        let path = logical_path_for_uri(uri);
        let mut state = self.state.write().expect("Tolk workspace lock poisoned");
        let mut remove_file = false;
        if let Some(file) = state.files.get_mut(&path) {
            file.base_uri = None;
            file.base_text = None;
            if file.open.is_some() {
                file.dirty = true;
            } else {
                remove_file = true;
            }
        }
        if remove_file {
            state.files.remove(&path);
            state.file_db.remove_path(&path);
        }
        let mut profiler = Profiler::disabled();
        state.rebuild_snapshot(&mut profiler)
    }

    fn set_workspace_config(&self, config: WorkspaceConfig) -> anyhow::Result<()> {
        let project_config = TolkProjectConfig::from_workspace_config(&config)?;
        let mut state = self.state.write().expect("Tolk workspace lock poisoned");
        if state.project_config == project_config {
            return Ok(());
        }
        state.project_config = project_config;
        state.invalidate_project_config();
        let mut profiler = Profiler::disabled();
        state.rebuild_snapshot(&mut profiler)
    }

    fn open_document(
        &self,
        document: &DocumentSnapshot,
        parsed: &TolkParsedDocument,
        profiler: &mut Profiler,
    ) -> anyhow::Result<()> {
        let path = logical_path_for_uri(document.uri());
        let mut state = self.state.write().expect("Tolk workspace lock poisoned");
        state.roots.insert(path.clone());
        let file = state.files.entry(path).or_default();
        file.open = Some(TolkOpenFile {
            uri: document.uri().clone(),
            source_file: parsed.source_file.clone(),
        });
        file.dirty = true;
        state.rebuild_snapshot(profiler)
    }

    fn close_document(&self, uri: &DocumentUri) {
        let path = logical_path_for_uri(uri);
        let mut state = self.state.write().expect("Tolk workspace lock poisoned");
        state.roots.remove(&path);
        let mut remove_file = false;
        if let Some(file) = state.files.get_mut(&path) {
            file.open = None;
            if file.base_text.is_none() {
                remove_file = true;
            } else {
                file.dirty = true;
            }
        }
        if remove_file {
            state.files.remove(&path);
            state.file_db.remove_path(&path);
        }
        let mut profiler = Profiler::disabled();
        if let Err(error) = state.rebuild_snapshot(&mut profiler) {
            tracing::warn!(
                target: logging::TOLK_TARGET,
                operation = "tolk.snapshot.rebuild",
                uri = uri.as_str(),
                error = %error,
                "failed to rebuild Tolk snapshot after close"
            );
        }
    }
}

#[derive(Debug)]
struct TolkWorkspaceState {
    file_db: Arc<FileDb>,
    type_interner: TypeInterner,
    type_db_cache: TypeDbCache,
    all_body_types: HashMap<FileId, HashMap<SymbolId, InferenceResult>>,
    project_config: TolkProjectConfig,
    files: BTreeMap<PathBuf, TolkWorkspaceFile>,
    roots: BTreeSet<PathBuf>,
    generation: u64,
    latest_snapshot: Option<Arc<TolkResolveSnapshot>>,
}

impl Default for TolkWorkspaceState {
    fn default() -> Self {
        Self {
            file_db: Arc::new(FileDb::new(PathBuf::from(TOLK_STDLIB_PATH), None)),
            type_interner: TypeInterner::new(),
            type_db_cache: TypeDbCache::default(),
            all_body_types: HashMap::new(),
            project_config: TolkProjectConfig::default(),
            files: BTreeMap::new(),
            roots: BTreeSet::new(),
            generation: 0,
            latest_snapshot: None,
        }
    }
}

impl TolkWorkspaceState {
    fn rebuild_snapshot(&mut self, profiler: &mut Profiler) -> anyhow::Result<()> {
        let started_at = profiler.start();
        let result = self.rebuild_snapshot_inner(profiler);
        profiler.finish("tolk.snapshot.rebuild", started_at);
        result
    }

    fn rebuild_snapshot_inner(&mut self, profiler: &mut Profiler) -> anyhow::Result<()> {
        if self.roots.is_empty() {
            self.process_dirty_files(profiler)?;
            self.latest_snapshot = None;
            return Ok(());
        }

        let provider = SnapshotSourceProvider {
            files: self.files.clone(),
            use_embedded_stdlib: self.project_config.use_embedded_stdlib,
        };
        let changed_file_ids = self.process_dirty_files(profiler)?;
        let file_db = self.file_db.clone();
        let mut roots = self.roots.iter().cloned().collect::<Vec<_>>();
        roots.sort();
        let root = roots.remove(0);
        roots.extend(
            provider
                .files
                .iter()
                .filter(|(path, file)| **path != root && file.active_source().is_some())
                .map(|(path, _)| path.clone()),
        );
        let project_config = self.project_config.clone();
        if project_config.use_embedded_stdlib {
            let mut stdlib_roots = BTreeSet::new();
            collect_embedded_stdlib_paths(&TOLK_STDLIB_DIR, &mut stdlib_roots);
            roots.extend(stdlib_roots);
        }
        roots.sort();
        roots.dedup();
        let index_started_at = profiler.start();
        let project_index = ProjectIndex::builder(&file_db, root)
            .with_additional_roots(roots)
            .with_stdlib(project_config.stdlib_path.clone())
            .with_mappings(&project_config.import_mappings)
            .build_with_provider(&provider);
        profiler.finish("tolk.snapshot.index", index_started_at);
        let mut project_index = project_index?;

        let resolve_started_at = profiler.start();
        tolk_resolver::resolve(&file_db, &mut project_index);
        profiler.finish("tolk.resolve", resolve_started_at);

        infer_incremental_workspace_body_types(
            &file_db,
            &project_index,
            &mut self.type_interner,
            &mut self.type_db_cache,
            &mut self.all_body_types,
            &changed_file_ids,
            profiler,
        );

        self.generation += 1;
        let materialize_started_at = profiler.start();
        let path_to_uri = provider
            .files
            .iter()
            .filter_map(|(path, file)| file.active_uri().map(|uri| (path.clone(), uri)))
            .collect();
        self.latest_snapshot = Some(Arc::new(TolkResolveSnapshot {
            generation: self.generation,
            file_db,
            project_index: Arc::new(project_index),
            all_body_types: self.all_body_types.clone(),
            type_interner: self.type_interner.clone(),
            type_db_cache: self.type_db_cache.clone(),
            path_to_uri,
        }));
        profiler.finish("tolk.snapshot.materialize", materialize_started_at);
        tracing::debug!(
            target: logging::TOLK_TARGET,
            operation = "tolk.snapshot.rebuilt",
            generation = self.generation,
            root_count = self.roots.len(),
            file_count = self.files.len(),
            project_root = project_config.project_root.to_string_lossy().as_ref(),
            stdlib_root = project_config.stdlib_path.to_string_lossy().as_ref(),
            embedded_stdlib = project_config.use_embedded_stdlib,
            import_mapping_count = project_config
                .import_mappings
                .as_ref()
                .map_or(0, BTreeMap::len),
            "rebuilt Tolk resolve snapshot"
        );
        Ok(())
    }

    fn invalidate_project_config(&mut self) {
        self.file_db = Arc::new(FileDb::new(self.project_config.stdlib_path.clone(), None));
        for file in self.files.values_mut() {
            file.dirty = true;
        }
        self.type_db_cache = TypeDbCache::default();
        self.all_body_types.clear();
    }

    fn process_dirty_files(&mut self, profiler: &mut Profiler) -> anyhow::Result<BTreeSet<FileId>> {
        let started_at = profiler.start();
        let dirty_paths = self
            .files
            .iter()
            .filter_map(|(path, file)| file.dirty.then_some(path.clone()))
            .collect::<Vec<_>>();
        let mut changed_file_ids = BTreeSet::new();
        for path in dirty_paths {
            let Some(file) = self.files.get_mut(&path) else {
                continue;
            };
            let Some(source) = file.active_source() else {
                if let Some(info) = self.file_db.get_by_path(&path) {
                    changed_file_ids.insert(info.id());
                }
                self.file_db.remove_path(&path);
                file.dirty = false;
                continue;
            };
            let info = match source {
                ProjectSource::Parsed(source_file) => {
                    self.file_db.process_source_file(path, source_file)
                }
                ProjectSource::Text(text) => self.file_db.process_content(path, &text)?,
            };
            changed_file_ids.insert(info.id());
            file.dirty = false;
        }
        profiler.finish("tolk.snapshot.update_files", started_at);
        Ok(changed_file_ids)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TolkProjectConfig {
    project_root: PathBuf,
    stdlib_path: PathBuf,
    use_embedded_stdlib: bool,
    import_mappings: Option<BTreeMap<String, String>>,
    contract_ids: Vec<String>,
    wallet_names: Vec<String>,
}

impl Default for TolkProjectConfig {
    fn default() -> Self {
        Self {
            project_root: PathBuf::from("/"),
            stdlib_path: PathBuf::from(TOLK_STDLIB_PATH),
            use_embedded_stdlib: true,
            import_mappings: None,
            contract_ids: Vec::new(),
            wallet_names: Vec::new(),
        }
    }
}

impl TolkProjectConfig {
    fn from_workspace_config(config: &WorkspaceConfig) -> anyhow::Result<Self> {
        let manifest = toml::from_str::<ActonManifest>(config.manifest_text().as_ref())
            .with_context(|| {
                let uri = config
                    .manifest_uri()
                    .map_or("Acton.toml", DocumentUri::as_str);
                format!("failed to parse {uri}")
            })?;
        let project_root = logical_path_for_uri(config.root_uri());
        let stdlib_path = config
            .tolk_stdlib_root_uri()
            .map_or_else(|| PathBuf::from(TOLK_STDLIB_PATH), logical_path_for_uri);
        Ok(Self {
            import_mappings: normalize_import_mappings(manifest.import_mappings, &project_root),
            contract_ids: manifest.contracts.keys().cloned().collect(),
            wallet_names: manifest.wallets.keys().cloned().collect(),
            project_root,
            stdlib_path,
            use_embedded_stdlib: config.tolk_stdlib_root_uri().is_none(),
        })
    }
}

#[derive(Debug, Default, Deserialize)]
struct ActonManifest {
    #[serde(default, rename = "import-mappings")]
    import_mappings: Option<BTreeMap<String, String>>,
    #[serde(default)]
    contracts: BTreeMap<String, toml::Value>,
    #[serde(default)]
    wallets: BTreeMap<String, toml::Value>,
}

fn normalize_import_mappings(
    mappings: Option<BTreeMap<String, String>>,
    project_root: &Path,
) -> Option<BTreeMap<String, String>> {
    let mappings = mappings?;
    Some(
        mappings
            .into_iter()
            .map(|(key, value)| {
                let value_path = Path::new(&value);
                let normalized_value = if value_path.is_absolute() {
                    normalize_path(value_path)
                } else {
                    normalize_path(&project_root.join(value_path))
                };
                (key, normalized_value.to_string_lossy().to_string())
            })
            .collect(),
    )
}

#[derive(Clone, Debug, Default)]
struct TolkWorkspaceFile {
    base_uri: Option<DocumentUri>,
    base_text: Option<Arc<str>>,
    open: Option<TolkOpenFile>,
    dirty: bool,
}

impl TolkWorkspaceFile {
    fn active_uri(&self) -> Option<DocumentUri> {
        self.open
            .as_ref()
            .map(|file| file.uri.clone())
            .or_else(|| self.base_uri.clone())
    }

    fn active_source(&self) -> Option<ProjectSource> {
        self.open
            .as_ref()
            .map(|file| ProjectSource::Parsed(file.source_file.clone()))
            .or_else(|| {
                self.base_text
                    .as_ref()
                    .map(|text| ProjectSource::Text(text.clone()))
            })
    }
}

#[derive(Clone, Debug)]
struct TolkOpenFile {
    uri: DocumentUri,
    source_file: tolk_syntax::SourceFile,
}

#[derive(Debug)]
struct TolkResolveSnapshot {
    #[allow(dead_code)]
    generation: u64,
    file_db: Arc<FileDb>,
    project_index: Arc<ProjectIndex>,
    all_body_types: HashMap<FileId, HashMap<SymbolId, InferenceResult>>,
    type_interner: TypeInterner,
    type_db_cache: TypeDbCache,
    path_to_uri: BTreeMap<PathBuf, DocumentUri>,
}

#[derive(Clone, Debug)]
struct SnapshotSourceProvider {
    files: BTreeMap<PathBuf, TolkWorkspaceFile>,
    use_embedded_stdlib: bool,
}

impl ProjectSourceProvider for SnapshotSourceProvider {
    fn canonicalize(&self, path: &Path) -> anyhow::Result<PathBuf> {
        Ok(normalize_logical_path(path))
    }

    fn source(&self, path: &Path) -> anyhow::Result<Option<ProjectSource>> {
        let path = normalize_logical_path(path);
        if let Some(source) = self
            .files
            .get(&path)
            .and_then(TolkWorkspaceFile::active_source)
        {
            return Ok(Some(source));
        }
        if !self.use_embedded_stdlib {
            return Ok(None);
        }
        Ok(embedded_stdlib_source(&path))
    }
}

fn infer_incremental_workspace_body_types(
    file_db: &FileDb,
    project_index: &ProjectIndex,
    type_interner: &mut TypeInterner,
    type_db_cache: &mut TypeDbCache,
    all_body_types: &mut HashMap<FileId, HashMap<SymbolId, InferenceResult>>,
    changed_file_ids: &BTreeSet<FileId>,
    profiler: &mut Profiler,
) {
    let workspace_file_ids = project_index
        .workspace_files()
        .into_iter()
        .map(|file| file.id)
        .collect::<BTreeSet<_>>();
    all_body_types.retain(|file_id, _| workspace_file_ids.contains(file_id));

    let affected_file_ids = affected_workspace_file_ids(
        project_index,
        &workspace_file_ids,
        all_body_types,
        changed_file_ids,
    );

    let signature_started_at = profiler.start();
    let mut type_db = TypeDb::new_with_cache(
        type_interner,
        file_db,
        project_index,
        std::mem::take(type_db_cache),
        affected_file_ids.iter().copied(),
    );
    profiler.finish("tolk.type_signature", signature_started_at);
    for _ in type_db.refreshed_files() {
        profiler.increment("tolk.type_signature.file");
    }

    let body_inference_started_at = profiler.start();
    for file_id in affected_file_ids {
        let Some(file_info) = file_db.get_by_id(file_id) else {
            continue;
        };
        let mut body_types = HashMap::new();
        for decl in file_info.source().top_levels() {
            let Some(index_decl) = file_info.find_declaration(&decl) else {
                continue;
            };
            let inference = infer(&mut type_db, file_id, index_decl.id, &decl);
            body_types.insert(index_decl.id, inference);
        }
        profiler.increment("tolk.type_inference.file");
        all_body_types.insert(file_id, body_types);
    }
    profiler.finish("tolk.type_inference", body_inference_started_at);
    *type_db_cache = type_db.into_cache();
}

fn affected_workspace_file_ids(
    project_index: &ProjectIndex,
    workspace_file_ids: &BTreeSet<FileId>,
    all_body_types: &HashMap<FileId, HashMap<SymbolId, InferenceResult>>,
    changed_file_ids: &BTreeSet<FileId>,
) -> BTreeSet<FileId> {
    if all_body_types.is_empty() {
        return workspace_file_ids.clone();
    }

    let mut affected = workspace_file_ids
        .iter()
        .filter(|file_id| !all_body_types.contains_key(file_id))
        .copied()
        .collect::<BTreeSet<_>>();

    let mut queue = Vec::new();
    for file_id in changed_file_ids {
        if project_index.get_file_index(*file_id).is_none() {
            return workspace_file_ids.clone();
        }
        queue.push(*file_id);
    }

    while let Some(file_id) = queue.pop() {
        for dependent in project_index.direct_dependents(file_id) {
            if affected.insert(dependent) {
                queue.push(dependent);
            }
        }
    }

    affected
        .into_iter()
        .filter(|file_id| workspace_file_ids.contains(file_id))
        .collect()
}

fn embedded_stdlib_source(path: &Path) -> Option<ProjectSource> {
    let relative_path = path.strip_prefix(Path::new(TOLK_STDLIB_PATH)).ok()?;
    let relative_path = relative_path.to_string_lossy();
    let file = TOLK_STDLIB_DIR.get_file(relative_path.as_ref())?;
    file.contents_utf8()
        .map(|content| ProjectSource::Text(Arc::from(content)))
}

fn collect_embedded_stdlib_paths(dir: &Dir<'_>, paths: &mut BTreeSet<PathBuf>) {
    for file in dir.files() {
        paths.insert(PathBuf::from(TOLK_STDLIB_PATH).join(file.path()));
    }
    for dir in dir.dirs() {
        collect_embedded_stdlib_paths(dir, paths);
    }
}

fn range_for_span(source: &str, span: Span) -> Range {
    TextIndex::new(source).range_for_offsets(source, span.start(), span.end())
}

fn fallback_uri_for_path(path: &Path) -> DocumentUri {
    if path.to_string_lossy().starts_with('/') {
        DocumentUri::from(format!("file://{}", path.display()))
    } else {
        DocumentUri::from(path.display().to_string())
    }
}

fn logical_path_for_uri(uri: &DocumentUri) -> PathBuf {
    let raw = uri.as_str();
    let path = if let Some(file_path) = raw.strip_prefix("file://") {
        PathBuf::from(format!("/{}", file_path.trim_start_matches('/')))
    } else if let Some((_, rest)) = raw.split_once("://") {
        PathBuf::from(format!("/{}", rest.trim_start_matches('/')))
    } else {
        PathBuf::from(raw)
    };
    normalize_logical_path(&path)
}

fn normalize_logical_path(path: &Path) -> PathBuf {
    let normalized = normalize_path(path);
    if normalized.is_absolute() {
        return normalized;
    }
    if normalized.as_os_str().is_empty() || normalized == Path::new(".") {
        return PathBuf::from("/");
    }
    Path::new("/").join(normalized)
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(Path::new("/")),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    if normalized.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        normalized
    }
}
