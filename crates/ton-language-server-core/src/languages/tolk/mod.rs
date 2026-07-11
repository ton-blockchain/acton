use self::incremental_analysis::{DeclarationChanges, collect_declaration_stamps, imports_changed};
use crate::language::{
    CodeActionRequest, CompletionRequest, DefinitionRequest, DocumentHighlightRequest,
    DocumentSymbolRequest, FeatureSet, FileRenameRequest, FoldingRangeRequest, FormattingRequest,
    HoverRequest, InlayHintRequest, LanguagePlugin, ParseRequest, ParsedDocument,
    PrepareRenameRequest, ReferenceRequest, RenameRequest, SemanticTokensRequest,
    SignatureHelpRequest, TypeAtPositionRequest, TypeDefinitionRequest, WorkspaceLanguage,
    WorkspaceSymbolRequest,
};
use crate::logging;
use crate::types::{normalize_logical_path, normalize_path};
use crate::{
    DocumentSnapshot, DocumentUri, LanguageId, Location, Profiler, Range, TextEdit, TextIndex,
    WorkspaceConfig,
};
use anyhow::Context;
use include_dir::{Dir, include_dir};
use rustc_hash::FxHashMap;
use serde::Deserialize;
use std::any::Any;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tolk_analysis::FileUseFacts;
use tolk_resolver::{FileDb, FileId, ProjectIndex, ProjectSource, ProjectSourceProvider, Span};
use tolk_ty::{FileBodyTypes, TypeDb, TypeDbCache, TypeInterner, WorkspaceBodyTypes, infer};
use tree_sitter::Tree;

mod code_actions;
mod completion;
mod definition;
mod document_highlights;
mod document_symbols;
mod file_info;
mod file_rename;
mod folding;
mod hover;
mod import_edits;
mod incremental_analysis;
mod inlay_hints;
mod references;
mod rename;
mod resolution;
mod semantic_tokens;
mod signature_help;
mod syntax;
mod type_at_position;
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
            type_at_position: true,
            formatting: true,
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
        let profile = request.profiler.span("tolk.parse");
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
        drop(profile);
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
        let profile = request.context.profiler.span("tolk.definition.resolve");
        let locations = self
            .engine
            .definition(request.context.document, request.position);
        drop(profile);
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
        let profile = request.context.profiler.span("tolk.references.resolve");
        let locations = self.engine.references(
            request.context.document,
            request.position,
            request.include_declaration,
        );
        drop(profile);

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
        let profile = request.context.profiler.span("tolk.semantic_tokens");
        let tokens = self.engine.semantic_tokens(request.context.document);
        drop(profile);
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
        let profile = request.context.profiler.span("tolk.inlay_hints");
        let hints = self
            .engine
            .inlay_hints(request.context.document, request.range);
        drop(profile);
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
        let parsed = request.context.parsed.as_tolk()?;
        let _profile = request.context.profiler.span("tolk.folding_ranges");
        let ranges = folding::folding_ranges(
            request.context.document,
            parsed.source_file.tree.root_node(),
        );
        Ok(ranges)
    }

    fn hover(&self, request: HoverRequest<'_>) -> anyhow::Result<Option<crate::Hover>> {
        let _profile = request.context.profiler.span("tolk.hover");
        let hover = self
            .engine
            .hover(request.context.document, request.position);
        Ok(hover)
    }

    fn document_symbols(
        &self,
        request: DocumentSymbolRequest<'_>,
    ) -> anyhow::Result<Vec<crate::DocumentSymbol>> {
        let _profile = request.context.profiler.span("tolk.document_symbols");
        let symbols = self.engine.document_symbols(request.context.document);
        Ok(symbols)
    }

    fn signature_help(
        &self,
        request: SignatureHelpRequest<'_>,
    ) -> anyhow::Result<Option<crate::SignatureHelp>> {
        let _profile = request.context.profiler.span("tolk.signature_help");
        let signature_help = self
            .engine
            .signature_help(request.context.document, request.position);
        Ok(signature_help)
    }

    fn prepare_rename(
        &self,
        request: PrepareRenameRequest<'_>,
    ) -> anyhow::Result<Option<crate::PrepareRename>> {
        let _profile = request.context.profiler.span("tolk.rename.prepare");
        self.engine
            .prepare_rename(request.context.document, request.position)
    }

    fn rename(&self, request: RenameRequest<'_>) -> anyhow::Result<Option<crate::WorkspaceEdit>> {
        let _profile = request.context.profiler.span("tolk.rename");
        self.engine
            .rename(request.context.document, request.position, request.new_name)
    }

    fn type_definition(&self, request: TypeDefinitionRequest<'_>) -> anyhow::Result<Vec<Location>> {
        let _profile = request.context.profiler.span("tolk.type_definition");
        let locations = self
            .engine
            .type_definition(request.context.document, request.position);
        Ok(locations)
    }

    fn document_highlights(
        &self,
        request: DocumentHighlightRequest<'_>,
    ) -> anyhow::Result<Vec<crate::DocumentHighlight>> {
        let _profile = request.context.profiler.span("tolk.document_highlights");
        let highlights = self
            .engine
            .document_highlights(request.context.document, request.position)
            .unwrap_or_default();
        Ok(highlights)
    }

    fn workspace_symbols(
        &self,
        request: WorkspaceSymbolRequest<'_>,
    ) -> anyhow::Result<Vec<crate::WorkspaceSymbol>> {
        let _profile = request.profiler.span("tolk.workspace_symbols");
        let symbols = self.engine.workspace_symbols(request.query);
        Ok(symbols)
    }

    fn code_actions(
        &self,
        request: CodeActionRequest<'_>,
    ) -> anyhow::Result<Vec<crate::CodeAction>> {
        let _profile = request.context.profiler.span("tolk.code_actions");
        let actions = self
            .engine
            .code_actions(request.context.document, request.range);
        Ok(actions)
    }

    fn will_rename_files(
        &self,
        request: FileRenameRequest<'_>,
    ) -> anyhow::Result<Option<crate::WorkspaceEdit>> {
        let _profile = request.profiler.span("tolk.files.rename.prepare");
        let edit = self.engine.will_rename_files(request.files);
        Ok(edit)
    }

    fn did_rename_files(&self, files: &[crate::FileRename]) -> anyhow::Result<()> {
        self.engine.did_rename_files(files)
    }

    fn completion(&self, request: CompletionRequest<'_>) -> anyhow::Result<crate::CompletionList> {
        let parsed = request.context.parsed.as_tolk()?;
        let mut profile = request.context.profiler.span("tolk.completion");
        let completion = self.engine.completion(
            request.context.document,
            &parsed.source_file,
            request.position,
            profile.profiler(),
        )?;
        drop(profile);
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

    fn type_at_position(
        &self,
        request: TypeAtPositionRequest<'_>,
    ) -> anyhow::Result<Option<crate::TypeAtPosition>> {
        let _profile = request.context.profiler.span("tolk.type_at_position");
        let result = self
            .engine
            .type_at_position(request.context.document, request.position);
        Ok(result)
    }

    fn formatting(&self, request: FormattingRequest<'_>) -> anyhow::Result<Vec<TextEdit>> {
        let _profile = request.context.profiler.span("tolk.formatting");
        let edits = self
            .engine
            .formatting(request.context.document, request.range)?;
        Ok(edits)
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
        let parsed = parsed.as_tolk()?;
        self.engine.open_document(document, parsed, profiler)
    }

    fn did_change(
        &self,
        document: &DocumentSnapshot,
        parsed: &dyn ParsedDocument,
        profiler: &mut Profiler,
    ) -> anyhow::Result<()> {
        let parsed = parsed.as_tolk()?;
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

trait ParsedDocumentExt {
    fn as_tolk(&self) -> anyhow::Result<&TolkParsedDocument>;
}

impl ParsedDocumentExt for dyn ParsedDocument + '_ {
    fn as_tolk(&self) -> anyhow::Result<&TolkParsedDocument> {
        self.as_any()
            .downcast_ref::<TolkParsedDocument>()
            .context("Tolk parsed document has an unexpected type")
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
        let path = uri.logical_path();
        let mut state = self.state.write().expect("Tolk workspace lock poisoned");
        let file = state.files.entry(path).or_default();
        file.base_uri = Some(uri);
        file.base_text = Some(text);
        file.dirty = true;
        let mut profiler = Profiler::disabled();
        state.rebuild_snapshot(&mut profiler)
    }

    fn remove_source_file(&self, uri: &DocumentUri) -> anyhow::Result<()> {
        let path = uri.logical_path();
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
        let affects_analysis = state.project_config.affects_analysis(&project_config);
        state.project_config = project_config;
        if !affects_analysis {
            return Ok(());
        }
        state.invalidate_project_config();
        let mut profiler = Profiler::disabled();
        state.rebuild_snapshot(&mut profiler)
    }

    fn formatting(
        &self,
        document: &DocumentSnapshot,
        range: Option<Range>,
    ) -> anyhow::Result<Vec<TextEdit>> {
        let (format_width, separate_import_groups) = {
            let state = self.state.read().expect("Tolk workspace lock poisoned");
            (
                state.project_config.format_width,
                state.project_config.separate_import_groups,
            )
        };

        let range = range.map(|range| {
            let start = document
                .text_index()
                .position_to_point(document.text(), range.start);
            let end = document
                .text_index()
                .position_to_point(document.text(), range.end);
            tolk_fmt::FormatRange {
                start: tolk_fmt::FormatPosition {
                    line: start.row,
                    character: start.column,
                },
                end: tolk_fmt::FormatPosition {
                    line: end.row,
                    character: end.column,
                },
            }
        });
        let formatted = tolk_fmt::format_source(
            document.text(),
            tolk_fmt::FormatOptions {
                width: format_width,
                separate_import_groups,
                range,
            },
        )?;
        if formatted == document.text() {
            return Ok(Vec::new());
        }

        let end = document
            .text_index()
            .offset_to_position(document.text(), document.text().len());
        Ok(vec![TextEdit::new(
            Range::new(crate::Position::new(0, 0), end),
            formatted,
        )])
    }

    fn open_document(
        &self,
        document: &DocumentSnapshot,
        parsed: &TolkParsedDocument,
        profiler: &mut Profiler,
    ) -> anyhow::Result<()> {
        let path = document.uri().logical_path();
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
        let path = uri.logical_path();
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
struct TolkAnalysisState {
    type_interner: Arc<TypeInterner>,
    type_db_cache: Arc<TypeDbCache>,
    all_body_types: Arc<WorkspaceBodyTypes>,
    declaration_stamps: FxHashMap<FileId, incremental_analysis::FileDeclarationStamps>,
}

impl Default for TolkAnalysisState {
    fn default() -> Self {
        Self {
            type_interner: Arc::new(TypeInterner::new()),
            type_db_cache: Arc::new(TypeDbCache::default()),
            all_body_types: Arc::new(WorkspaceBodyTypes::default()),
            declaration_stamps: FxHashMap::default(),
        }
    }
}

#[derive(Debug)]
struct TolkWorkspaceState {
    file_db: Arc<FileDb>,
    analysis: TolkAnalysisState,
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
            analysis: TolkAnalysisState::default(),
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

        let previous_project_index = self
            .latest_snapshot
            .as_ref()
            .map(|snapshot| snapshot.project_index.clone());
        let changed_file_ids = self.process_dirty_files(profiler)?;
        let file_db = self.file_db.clone();
        let project_config = self.project_config.clone();

        let index_started_at = profiler.start();
        let project_index = previous_project_index
            .as_deref()
            .and_then(|previous| previous.with_updated_files(&file_db, &changed_file_ids));

        let mut project_index = if let Some(project_index) = project_index {
            profiler.increment("tolk.snapshot.index.incremental");
            project_index
        } else {
            let full_index_started_at = profiler.start();
            let provider = SnapshotSourceProvider {
                files: &self.files,
                use_embedded_stdlib: project_config.use_embedded_stdlib,
            };
            let mut roots = self.roots.iter().cloned().collect::<Vec<_>>();
            roots.sort();
            let root = roots.remove(0);
            roots.extend(
                self.files
                    .iter()
                    .filter(|(path, file)| **path != root && file.active_source().is_some())
                    .map(|(path, _)| path.clone()),
            );

            if project_config.use_embedded_stdlib {
                let mut stdlib_roots = BTreeSet::new();
                collect_embedded_stdlib_paths(&TOLK_STDLIB_DIR, &mut stdlib_roots);
                roots.extend(stdlib_roots);
            }

            roots.sort();
            roots.dedup();

            let project_index = ProjectIndex::builder(&file_db, root)
                .with_additional_roots(roots)
                .with_stdlib(project_config.stdlib_path.clone())
                .with_mappings(&project_config.import_mappings)
                .build_with_provider(&provider)?;
            profiler.finish("tolk.snapshot.index.full", full_index_started_at);
            project_index
        };
        profiler.finish("tolk.snapshot.index", index_started_at);

        let resolve_started_at = profiler.start();
        let reused_files = previous_project_index.as_deref().map_or(0, |previous| {
            project_index.reuse_resolved_uses_from(previous, &changed_file_ids)
        });
        let mut files_to_resolve = project_index
            .files()
            .keys()
            .filter(|file_id| !project_index.resolved_uses().contains_key(file_id))
            .copied()
            .collect::<Vec<_>>();
        files_to_resolve.sort_unstable();

        for _ in 0..reused_files {
            profiler.increment("tolk.resolve.reused_file");
        }
        for _ in 0..files_to_resolve.len() {
            profiler.increment("tolk.resolve.file");
        }

        tolk_resolver::resolve_files(&file_db, &mut project_index, files_to_resolve);
        profiler.finish("tolk.resolve", resolve_started_at);

        // Release the engine's old snapshot before mutating copy-on-write analysis state.
        // Concurrent requests can still retain their own immutable snapshot safely.
        self.latest_snapshot = None;
        infer_incremental_workspace_body_types(
            &file_db,
            &project_index,
            previous_project_index.as_deref(),
            &mut self.analysis,
            &changed_file_ids,
            profiler,
        );

        self.generation += 1;
        let materialize_started_at = profiler.start();
        let file_uris = project_index
            .files()
            .iter()
            .filter_map(|(&file_id, file)| {
                let uri = if project_config.use_embedded_stdlib
                    && file.path.starts_with(&project_config.stdlib_path)
                {
                    DocumentUri::from(format!("file://{}", file.path.display()))
                } else {
                    self.files.get(&file.path)?.active_uri()?
                };
                Some((file_id, uri))
            })
            .collect();
        self.latest_snapshot = Some(Arc::new(TolkResolveSnapshot {
            generation: self.generation,
            file_db,
            project_index: Arc::new(project_index),
            all_body_types: self.analysis.all_body_types.clone(),
            body_types_override: None,
            use_facts: RwLock::new(FxHashMap::default()),
            type_interner: self.analysis.type_interner.clone(),
            type_db_cache: self.analysis.type_db_cache.clone(),
            file_uris,
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
        self.analysis = TolkAnalysisState::default();
        self.latest_snapshot = None;
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
    format_width: usize,
    separate_import_groups: bool,
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
            format_width: 100,
            separate_import_groups: false,
        }
    }
}

impl TolkProjectConfig {
    fn affects_analysis(&self, other: &Self) -> bool {
        self.project_root != other.project_root
            || self.stdlib_path != other.stdlib_path
            || self.use_embedded_stdlib != other.use_embedded_stdlib
            || self.import_mappings != other.import_mappings
            || self.contract_ids != other.contract_ids
            || self.wallet_names != other.wallet_names
    }

    fn from_workspace_config(config: &WorkspaceConfig) -> anyhow::Result<Self> {
        let manifest = toml::from_str::<ActonManifest>(config.manifest_text().as_ref())
            .with_context(|| {
                let uri = config
                    .manifest_uri()
                    .map_or("Acton.toml", DocumentUri::as_str);
                format!("failed to parse {uri}")
            })?;
        let project_root = config.root_uri().logical_path();
        let stdlib_path = config.tolk_stdlib_root_uri().map_or_else(
            || PathBuf::from(TOLK_STDLIB_PATH),
            DocumentUri::logical_path,
        );
        Ok(Self {
            import_mappings: normalize_import_mappings(manifest.import_mappings, &project_root),
            contract_ids: manifest.contracts.keys().cloned().collect(),
            wallet_names: manifest.wallets.keys().cloned().collect(),
            format_width: manifest.fmt.width.unwrap_or(100),
            separate_import_groups: manifest.fmt.separate_import_groups.unwrap_or(false),
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
    #[serde(default)]
    fmt: FormatterManifest,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct FormatterManifest {
    width: Option<usize>,
    separate_import_groups: Option<bool>,
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
    all_body_types: Arc<WorkspaceBodyTypes>,
    body_types_override: Option<(FileId, Arc<FileBodyTypes>)>,
    use_facts: RwLock<FxHashMap<FileId, Arc<FileUseFacts>>>,
    type_interner: Arc<TypeInterner>,
    type_db_cache: Arc<TypeDbCache>,
    file_uris: BTreeMap<FileId, DocumentUri>,
}

#[derive(Debug)]
struct SnapshotSourceProvider<'a> {
    files: &'a BTreeMap<PathBuf, TolkWorkspaceFile>,
    use_embedded_stdlib: bool,
}

impl ProjectSourceProvider for SnapshotSourceProvider<'_> {
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
    previous_project_index: Option<&ProjectIndex>,
    analysis: &mut TolkAnalysisState,
    changed_file_ids: &BTreeSet<FileId>,
    profiler: &mut Profiler,
) {
    let TolkAnalysisState {
        type_interner,
        type_db_cache,
        all_body_types,
        declaration_stamps,
    } = analysis;
    let type_interner = Arc::make_mut(type_interner);
    let type_db_cache = Arc::make_mut(type_db_cache);
    let all_body_types = Arc::make_mut(all_body_types);

    let workspace_file_ids = project_index
        .workspace_files()
        .into_iter()
        .map(|file| file.id)
        .collect::<BTreeSet<_>>();
    all_body_types.retain(|file_id, _| workspace_file_ids.contains(file_id));
    declaration_stamps.retain(|file_id, _| workspace_file_ids.contains(file_id));

    let invalidation_started_at = profiler.start();
    let mut current_stamps = FxHashMap::default();
    let mut declaration_changes = FxHashMap::default();
    let mut signature_changed_file_ids = BTreeSet::new();

    if changed_file_ids
        .iter()
        .any(|file_id| project_index.get_file_index(*file_id).is_none())
    {
        signature_changed_file_ids.extend(workspace_file_ids.iter().copied());
    }

    for &file_id in &workspace_file_ids {
        if !changed_file_ids.contains(&file_id)
            && declaration_stamps.contains_key(&file_id)
            && all_body_types.contains_key(&file_id)
        {
            continue;
        }

        let Some(file_info) = file_db.get_by_id(file_id) else {
            signature_changed_file_ids.extend(workspace_file_ids.iter().copied());
            continue;
        };
        let stamps = collect_declaration_stamps(&file_info);
        let changes = DeclarationChanges::between(&stamps, declaration_stamps.get(&file_id));

        if changes.signature_changed
            || imports_changed(project_index, previous_project_index, file_id)
            || !all_body_types.contains_key(&file_id)
        {
            signature_changed_file_ids.insert(file_id);
        }

        current_stamps.insert(file_id, stamps);
        declaration_changes.insert(file_id, changes);
    }

    let full_inference_file_ids = affected_workspace_file_ids(
        project_index,
        &workspace_file_ids,
        all_body_types,
        &signature_changed_file_ids,
    );

    for (&file_id, changes) in &mut declaration_changes {
        if full_inference_file_ids.contains(&file_id) {
            continue;
        }

        let Some(body_types) = all_body_types.get_mut(&file_id) else {
            signature_changed_file_ids.insert(file_id);
            continue;
        };
        let Some(stamps) = current_stamps.get(&file_id) else {
            continue;
        };

        body_types.retain(|symbol_id, _| stamps.contains_key(symbol_id));
        for (&symbol_id, &delta) in &changes.relocated {
            let Some(inference) = body_types.remove(&symbol_id) else {
                changes.changed.insert(symbol_id);
                continue;
            };
            let Some(inference) = inference.shifted(file_id, delta) else {
                changes.changed.insert(symbol_id);
                continue;
            };

            body_types.insert(symbol_id, inference);
            profiler.increment("tolk.type_inference.relocated_declaration");
        }

        let reused = stamps
            .len()
            .saturating_sub(changes.changed.len() + changes.relocated.len());
        for _ in 0..reused {
            profiler.increment("tolk.type_inference.reused_declaration");
        }
    }
    profiler.finish("tolk.invalidation", invalidation_started_at);

    for (file_id, stamps) in current_stamps {
        declaration_stamps.insert(file_id, stamps);
    }

    let cached_type_db: &TypeDbCache = type_db_cache;
    let previous_inferred_signatures = declaration_changes
        .iter()
        .filter(|(file_id, _)| !full_inference_file_ids.contains(file_id))
        .flat_map(|(&file_id, changes)| {
            changes
                .potential_signature_changes
                .iter()
                .map(move |&symbol_id| {
                    (
                        symbol_id,
                        (file_id, cached_type_db.top_level_type(symbol_id)),
                    )
                })
        })
        .collect::<FxHashMap<_, _>>();

    let signature_started_at = profiler.start();
    let mut type_db = TypeDb::new_with_cache(
        type_interner,
        file_db,
        project_index,
        std::mem::take(type_db_cache),
        full_inference_file_ids.iter().copied(),
    );
    profiler.finish("tolk.type_signature", signature_started_at);
    for _ in type_db.refreshed_files() {
        profiler.increment("tolk.type_signature.file");
    }

    let body_inference_started_at = profiler.start();
    infer_entire_files(
        file_db,
        &mut type_db,
        all_body_types,
        &full_inference_file_ids,
        profiler,
    );

    for (file_id, changes) in declaration_changes {
        if full_inference_file_ids.contains(&file_id) || changes.changed.is_empty() {
            continue;
        }

        let Some(file_info) = file_db.get_by_id(file_id) else {
            continue;
        };
        let Some(body_types) = all_body_types.get_mut(&file_id) else {
            continue;
        };
        let mut inferred_any = false;

        for declaration in file_info.source().top_levels() {
            let Some(symbol) = file_info.find_declaration(&declaration) else {
                continue;
            };
            if !changes.changed.contains(&symbol.id) {
                continue;
            }

            let inference = infer(&mut type_db, file_id, symbol.id, &declaration);
            body_types.insert(symbol.id, inference);
            inferred_any = true;
            profiler.increment("tolk.type_inference.declaration");
        }

        if inferred_any {
            profiler.increment("tolk.type_inference.file");
        }
    }

    let changed_inferred_signature_file_ids = previous_inferred_signatures
        .into_iter()
        .filter_map(|(symbol_id, (file_id, previous_ty))| {
            (type_db.top_level_types.get(&symbol_id).copied() != previous_ty).then_some(file_id)
        })
        .collect::<BTreeSet<_>>();

    if !changed_inferred_signature_file_ids.is_empty() {
        profiler.increment("tolk.type_inference.signature_fallback");
        let fallback_file_ids = affected_workspace_file_ids(
            project_index,
            &workspace_file_ids,
            all_body_types,
            &changed_inferred_signature_file_ids,
        );
        let cache = type_db.into_cache();
        let fallback_signature_started_at = profiler.start();
        type_db = TypeDb::new_with_cache(
            type_interner,
            file_db,
            project_index,
            cache,
            fallback_file_ids.iter().copied(),
        );
        profiler.finish(
            "tolk.type_signature.fallback",
            fallback_signature_started_at,
        );
        for _ in type_db.refreshed_files() {
            profiler.increment("tolk.type_signature.file");
        }

        infer_entire_files(
            file_db,
            &mut type_db,
            all_body_types,
            &fallback_file_ids,
            profiler,
        );
    }
    profiler.finish("tolk.type_inference", body_inference_started_at);

    *type_db_cache = type_db.into_cache();
}

fn infer_entire_files(
    file_db: &FileDb,
    type_db: &mut TypeDb<'_>,
    all_body_types: &mut WorkspaceBodyTypes,
    file_ids: &BTreeSet<FileId>,
    profiler: &mut Profiler,
) {
    for &file_id in file_ids {
        let Some(file_info) = file_db.get_by_id(file_id) else {
            continue;
        };
        let mut body_types = FileBodyTypes::default();

        for declaration in file_info.source().top_levels() {
            let Some(symbol) = file_info.find_declaration(&declaration) else {
                continue;
            };
            let inference = infer(type_db, file_id, symbol.id, &declaration);
            body_types.insert(symbol.id, inference);
            profiler.increment("tolk.type_inference.declaration");
        }

        profiler.increment("tolk.type_inference.file");
        all_body_types.insert(file_id, body_types);
    }
}

fn affected_workspace_file_ids(
    project_index: &ProjectIndex,
    workspace_file_ids: &BTreeSet<FileId>,
    all_body_types: &WorkspaceBodyTypes,
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
