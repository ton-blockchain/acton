use crate::language::{
    DefinitionRequest, FeatureSet, LanguagePlugin, ParseRequest, ParsedDocument, WorkspaceLanguage,
};
use crate::logging;
use crate::{DocumentSnapshot, DocumentUri, LanguageId, Location, Profiler, Range, TextIndex};
use anyhow::Context;
use include_dir::{Dir, include_dir};
use std::any::Any;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, RwLock};
use tolk_resolver::{
    FileDb, FileId, ProjectIndex, ProjectSource, ProjectSourceProvider, Resolved, Span, SymbolId,
};
use tolk_ty::{InferenceResult, TypeDb, TypeInterner, infer};
use tree_sitter::Tree;

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
}

impl WorkspaceLanguage for TolkLanguage {
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
            }
        }
        if remove_file {
            state.files.remove(&path);
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

    fn definition(&self, document: &DocumentSnapshot, position: crate::Position) -> Vec<Location> {
        let snapshot = {
            let state = self.state.read().expect("Tolk workspace lock poisoned");
            state.latest_snapshot.clone()
        };
        let Some(snapshot) = snapshot else {
            return Vec::new();
        };
        let path = logical_path_for_uri(document.uri());
        let Some(file_id) = snapshot.project_index.get_file_by_path(&path) else {
            return Vec::new();
        };
        let offset = document
            .text_index()
            .position_to_offset(document.text(), position);

        if let Some(name_use) = snapshot.project_index.find_use(file_id, offset)
            && !matches!(name_use.resolved, Resolved::Unresolved)
        {
            return snapshot.location_for_resolved(&name_use.resolved);
        }

        if let Some(symbol) = snapshot.project_index.find_symbol_at(file_id, offset) {
            return snapshot.location_for_span(symbol.id.file_id, symbol.name_span);
        }

        if let Some(resolve_index) = snapshot.project_index.get_resolved_uses(file_id)
            && let Some(local) = resolve_index.find_local_at(offset)
        {
            return snapshot.location_for_span(local.id.file_id, local.def_span);
        }

        let Some(file_info) = snapshot.file_db.get_by_id(file_id) else {
            return Vec::new();
        };
        let Some(symbol) = file_info.find_symbol_at(offset) else {
            return Vec::new();
        };
        snapshot
            .inferred_resolved_at(file_id, symbol.id, offset)
            .map_or_else(Vec::new, |resolved| {
                snapshot.location_for_resolved(&resolved)
            })
    }
}

impl TolkResolveSnapshot {
    fn inferred_resolved_at(
        &self,
        file_id: FileId,
        symbol_id: SymbolId,
        offset: usize,
    ) -> Option<Resolved> {
        let inference = self.all_body_types.get(&file_id)?.get(&symbol_id)?;
        if let Some(resolved) = inference.resolve(Span::from_offset(offset)) {
            return Some(resolved.resolved.clone());
        }
        inference
            .resolved_refs
            .iter()
            .find(|name_use| name_use.span.contains(offset))
            .map(|resolved| resolved.resolved.clone())
    }

    fn location_for_resolved(&self, resolved: &Resolved) -> Vec<Location> {
        match resolved {
            Resolved::Global(symbol_id) => self
                .project_index
                .resolve_symbol(*symbol_id)
                .map_or_else(Vec::new, |symbol| {
                    self.location_for_span(symbol.id.file_id, symbol.name_span)
                }),
            Resolved::Local(local_id) => self
                .project_index
                .get_resolved_uses(local_id.file_id)
                .and_then(|resolve_index| resolve_index.find_local(*local_id))
                .map_or_else(Vec::new, |local| {
                    self.location_for_span(local.id.file_id, local.def_span)
                }),
            Resolved::Unresolved => Vec::new(),
        }
    }
}

#[derive(Debug, Default)]
struct TolkWorkspaceState {
    files: BTreeMap<PathBuf, TolkWorkspaceFile>,
    roots: BTreeSet<PathBuf>,
    generation: u64,
    latest_snapshot: Option<Arc<TolkResolveSnapshot>>,
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
            self.latest_snapshot = None;
            return Ok(());
        }

        let provider = SnapshotSourceProvider {
            files: self.files.clone(),
        };
        let stdlib_path = PathBuf::from(TOLK_STDLIB_PATH);
        let file_db = Arc::new(FileDb::new(stdlib_path.clone(), None));
        let mut roots = self.roots.iter().cloned().collect::<Vec<_>>();
        roots.sort();
        let root = roots.remove(0);
        let index_started_at = profiler.start();
        let project_index = ProjectIndex::builder(&file_db, root)
            .with_additional_roots(roots)
            .with_stdlib(stdlib_path)
            .build_with_provider(&provider);
        profiler.finish("tolk.snapshot.index", index_started_at);
        let mut project_index = project_index?;

        let resolve_started_at = profiler.start();
        tolk_resolver::resolve(&file_db, &mut project_index);
        profiler.finish("tolk.resolve", resolve_started_at);

        let type_inference_started_at = profiler.start();
        let all_body_types = infer_body_types(&file_db, &project_index, &self.roots);
        profiler.finish("tolk.type_inference", type_inference_started_at);

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
            all_body_types,
            path_to_uri,
        }));
        profiler.finish("tolk.snapshot.materialize", materialize_started_at);
        tracing::debug!(
            target: logging::TOLK_TARGET,
            operation = "tolk.snapshot.rebuilt",
            generation = self.generation,
            root_count = self.roots.len(),
            file_count = self.files.len(),
            "rebuilt Tolk resolve snapshot"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
struct TolkWorkspaceFile {
    base_uri: Option<DocumentUri>,
    base_text: Option<Arc<str>>,
    open: Option<TolkOpenFile>,
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
    path_to_uri: BTreeMap<PathBuf, DocumentUri>,
}

impl TolkResolveSnapshot {
    fn location_for_span(&self, file_id: FileId, span: Span) -> Vec<Location> {
        let Some(file) = self.file_db.get_by_id(file_id) else {
            return Vec::new();
        };
        let uri = self
            .path_to_uri
            .get(file.path())
            .cloned()
            .unwrap_or_else(|| fallback_uri_for_path(file.path()));
        let source = file.source().source.as_ref();
        let range = range_for_span(source, span);
        vec![Location::new(uri, range)]
    }
}

#[derive(Clone, Debug)]
struct SnapshotSourceProvider {
    files: BTreeMap<PathBuf, TolkWorkspaceFile>,
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
        Ok(embedded_stdlib_source(&path))
    }
}

fn infer_body_types(
    file_db: &FileDb,
    project_index: &ProjectIndex,
    roots: &BTreeSet<PathBuf>,
) -> HashMap<FileId, HashMap<SymbolId, InferenceResult>> {
    let mut file_ids = BTreeSet::new();
    for root in roots {
        let Some(root_id) = project_index.get_file_by_path(root) else {
            continue;
        };
        file_ids.extend(project_index.reachable_files(root_id));
    }

    let mut interner = TypeInterner::new();
    let mut type_db = TypeDb::new(&mut interner, file_db, project_index);
    let mut all_body_types = HashMap::new();

    for file_id in file_ids {
        let Some(file_info) = file_db.get_by_id(file_id) else {
            continue;
        };
        let mut body_types = HashMap::new();
        for decl in file_info.source().top_levels() {
            let Some(index_decl) = file_info.find_declaration(&decl) else {
                continue;
            };
            let result = infer(&mut type_db, file_id, index_decl.id, &decl);
            body_types.insert(index_decl.id, result);
        }
        all_body_types.insert(file_id, body_types);
    }

    all_body_types
}

fn embedded_stdlib_source(path: &Path) -> Option<ProjectSource> {
    let relative_path = path.strip_prefix(Path::new(TOLK_STDLIB_PATH)).ok()?;
    let relative_path = relative_path.to_string_lossy();
    let file = TOLK_STDLIB_DIR.get_file(relative_path.as_ref())?;
    file.contents_utf8()
        .map(|content| ProjectSource::Text(Arc::from(content)))
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
