//! Project-wide index of files, symbols, and resolved imports.
//!
//! This module provides the `ProjectIndex` struct which aggregates multiple
//! `FileIndex`es and tracks the relationships between files through imports.

use crate::file_db::FileDb;
use crate::file_index::{FileId, FileIndex, FileSource, Import, Symbol, SymbolId, SymbolKind};
use crate::resolve_index::{FileResolveIndex, NameUse};
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Source loaded by a project index builder.
#[derive(Debug, Clone)]
pub enum ProjectSource {
    /// Raw file text. The builder parses it before indexing.
    Text(Arc<str>),
    /// Already parsed source file. This avoids reparsing open editor buffers.
    Parsed(tolk_syntax::SourceFile),
}

/// Platform-neutral source provider for project indexing.
///
/// Native adapters can implement this over the filesystem. Browser adapters and
/// tests can implement it over in-memory files and open-document overlays.
pub trait ProjectSourceProvider {
    /// Returns a canonical logical path. This must not require a native
    /// filesystem; virtual workspaces can normalize path components instead.
    fn canonicalize(&self, path: &Path) -> anyhow::Result<PathBuf>;

    /// Loads source for the canonical path, or returns `None` when the file is
    /// not available.
    fn source(&self, path: &Path) -> anyhow::Result<Option<ProjectSource>>;
}

struct DiskProjectSourceProvider<'a> {
    file_db: &'a FileDb,
}

impl ProjectSourceProvider for DiskProjectSourceProvider<'_> {
    fn canonicalize(&self, path: &Path) -> anyhow::Result<PathBuf> {
        Ok(self.file_db.canonicalize(path)?)
    }

    fn source(&self, path: &Path) -> anyhow::Result<Option<ProjectSource>> {
        let content = std::fs::read_to_string(path)?;
        Ok(Some(ProjectSource::Text(Arc::from(content))))
    }
}

/// Represents an import where the target file has been resolved to a `FileId`.
#[derive(Debug, Clone)]
pub struct ResolvedImport {
    /// Original import information.
    import: Import,
    /// ID of the target file, if it could be resolved.
    target: Option<FileId>,
}

impl ResolvedImport {
    /// Returns AST node of this import.
    #[must_use]
    pub const fn import(&self) -> &Import {
        &self.import
    }

    /// Return `FileId` which is imported.
    #[must_use]
    pub const fn target(&self) -> Option<FileId> {
        self.target
    }
}

/// A project-level index containing information about all files and their relationships.
#[derive(Debug, Clone)]
pub struct ProjectIndex {
    /// Map from `FileId` to its corresponding `FileIndex`.
    pub(crate) files: FxHashMap<FileId, Arc<FileIndex>>,
    /// Map from `FileId` to a list of resolved imports in that file.
    pub(crate) imports: FxHashMap<FileId, Vec<ResolvedImport>>,
    /// Map from `FileId` to a list of file IDs that import this file.
    pub(crate) dependents: FxHashMap<FileId, Vec<FileId>>,
    /// Map from absolute file path to `FileId`.
    pub(crate) path_to_file_id: HashMap<PathBuf, FileId>,
    /// Path to the Tolk standard library, if provided.
    pub(crate) stdlib_path: Option<PathBuf>,
    /// Path mappings used to resolve `@alias/...` imports.
    pub(crate) mappings: FxHashMap<String, String>,
    /// Map from symbol name to all `SymbolId`s that declare it across the project.
    pub(crate) global_symbols: HashMap<Arc<str>, Vec<SymbolId>>,
    /// List of errors encountered during project indexing.
    pub(crate) errors: Vec<String>,
    /// Map from `FileId` to its name resolution index.
    pub resolved_uses: FxHashMap<FileId, Arc<FileResolveIndex>>,
}

impl ProjectIndex {
    /// Creates a new builder for `ProjectIndex`.
    pub fn builder(file_db: &'_ FileDb, root_path: PathBuf) -> ProjectIndexBuilder<'_> {
        ProjectIndexBuilder::new(file_db, root_path)
    }

    #[must_use]
    pub const fn files(&self) -> &FxHashMap<FileId, Arc<FileIndex>> {
        &self.files
    }

    #[must_use]
    pub fn workspace_files(&self) -> Vec<Arc<FileIndex>> {
        let mut files = self
            .files
            .values()
            .filter(|f| f.is_workspace_file())
            .cloned()
            .collect::<Vec<_>>();
        files.sort_unstable();
        files
    }

    #[must_use]
    pub const fn imports(&self) -> &FxHashMap<FileId, Vec<ResolvedImport>> {
        &self.imports
    }

    #[must_use]
    pub fn imports_of(&self, file_id: FileId) -> Option<Vec<ResolvedImport>> {
        self.imports.get(&file_id).cloned()
    }

    #[must_use]
    pub const fn path_to_file_id(&self) -> &HashMap<PathBuf, FileId> {
        &self.path_to_file_id
    }

    #[must_use]
    pub const fn resolved_uses(&self) -> &FxHashMap<FileId, Arc<FileResolveIndex>> {
        &self.resolved_uses
    }

    #[must_use]
    pub const fn dependents(&self) -> &FxHashMap<FileId, Vec<FileId>> {
        &self.dependents
    }

    /// Returns a list of all file IDs that directly import the given file.
    #[must_use]
    pub fn direct_dependents(&self, file_id: FileId) -> Vec<FileId> {
        let Some(file) = self.files.get(&file_id) else {
            // very unlikely and likely a bug
            return Vec::new();
        };
        let is_common = file.source_kind == FileSource::Stdlib
            && file.path.file_name().is_some_and(|n| n == "common.tolk");

        if is_common {
            // all files depend on common.tolk
            return self.files.keys().copied().collect();
        }

        let mut result = vec![file_id]; // include the file itself
        if let Some(dependents) = self.dependents.get(&file_id) {
            result.extend(dependents);
        }

        result
    }

    #[must_use]
    pub fn stdlib_path(&self) -> Option<&Path> {
        self.stdlib_path.as_deref()
    }

    #[must_use]
    pub const fn mappings(&self) -> &FxHashMap<String, String> {
        &self.mappings
    }

    #[must_use]
    pub fn errors(&self) -> &[String] {
        &self.errors
    }

    #[must_use]
    pub fn get_file_index(&self, file_id: FileId) -> Option<&Arc<FileIndex>> {
        self.files.get(&file_id)
    }

    #[must_use]
    pub fn get_resolved_uses(&self, file_id: FileId) -> Option<&Arc<FileResolveIndex>> {
        self.resolved_uses.get(&file_id)
    }

    #[must_use]
    pub const fn global_symbols(&self) -> &HashMap<Arc<str>, Vec<SymbolId>> {
        &self.global_symbols
    }

    /// Returns a list of all file IDs that are recursively reachable from the given file.
    #[must_use]
    pub fn reachable_files(&self, file_id: FileId) -> Vec<FileId> {
        // TODO: add common.tolk
        let mut queue = VecDeque::new();
        queue.push_back(file_id);

        let mut visited = FxHashSet::default();
        visited.insert(file_id);
        let mut result = vec![file_id];
        while let Some(file_id) = queue.pop_front() {
            let Some(imports) = self.imports.get(&file_id) else {
                continue;
            };

            for imported_file in imports.iter().filter_map(|import| import.target) {
                if visited.insert(imported_file) {
                    result.push(imported_file);
                    queue.push_back(imported_file);
                }
            }
        }

        result
    }

    /// Resolves a `SymbolId` to its corresponding `Symbol` declaration.
    #[must_use]
    pub fn resolve_symbol(&self, symbol_id: SymbolId) -> Option<&Symbol> {
        let file_index = self.files.get(&symbol_id.file_id)?;
        file_index.decls.iter().find_map(|d| {
            if d.id == symbol_id {
                return Some(d);
            }

            match &d.kind {
                SymbolKind::Struct { fields, .. } => fields.iter().find(|f| f.id == symbol_id),
                SymbolKind::Enum { members } => members.iter().find(|f| f.id == symbol_id),
                _ => None,
            }
        })
    }

    #[must_use]
    pub fn find_symbol_at(&self, file_id: FileId, offset: usize) -> Option<&Symbol> {
        let file = self.files().get(&file_id)?;
        file.decls.iter().find_map(|d| {
            if d.name_span.contains(offset) {
                return Some(d);
            }

            match &d.kind {
                SymbolKind::Struct { fields, .. } => {
                    fields.iter().find(|f| f.name_span.contains(offset))
                }
                SymbolKind::Enum { members } => {
                    members.iter().find(|f| f.name_span.contains(offset))
                }
                _ => None,
            }
        })
    }

    /// Finds a name usage at the specified byte offset in a file.
    #[must_use]
    pub fn find_use(&self, file_id: FileId, pos: usize) -> Option<&NameUse> {
        self.resolved_uses.get(&file_id)?.find_use(pos)
    }

    #[must_use]
    pub fn get_file_by_path(&self, path: &Path) -> Option<FileId> {
        self.path_to_file_id.get(path).copied()
    }

    fn resolve_imports_with_provider(
        index: &FileIndex,
        path_to_id: &HashMap<PathBuf, FileId>,
        provider: &dyn ProjectSourceProvider,
        stdlib_path: Option<&Path>,
        mappings: &FxHashMap<String, String>,
    ) -> (Vec<ResolvedImport>, Vec<String>) {
        let mut errors = vec![];
        let mut file_imports = Vec::with_capacity(index.imports.len());
        for import in &index.imports {
            let resolved = match Self::resolve_path_with_provider(
                &import.path,
                &index.path,
                provider,
                stdlib_path,
                mappings,
            ) {
                Ok(resolved) => resolved,
                Err(err) => {
                    errors.push(format!("{err:#?}"));
                    continue;
                }
            };
            let file_id = path_to_id.get(&resolved);
            file_imports.push(ResolvedImport {
                import: import.clone(),
                target: file_id.copied(),
            });
        }
        (file_imports, errors)
    }

    fn resolve_path_with_provider(
        import: &Arc<str>,
        file: &Path,
        provider: &dyn ProjectSourceProvider,
        stdlib_path: Option<&Path>,
        mappings: &FxHashMap<String, String>,
    ) -> anyhow::Result<PathBuf> {
        if let Some(relative_path) = import.strip_prefix("@stdlib/") {
            let Some(stdlib) = stdlib_path else {
                anyhow::bail!("Stdlib path not provided for @stdlib import: {import}");
            };
            let abs_path = stdlib.join(relative_path);
            let abs_path = Self::append_tolk_extension_if_needed(abs_path);
            return provider.canonicalize(&abs_path);
        }

        if import.starts_with('@') {
            let (prefix, suffix) = match import.find('/') {
                Some(pos) => (&import[..pos], &import[pos + 1..]),
                None => (import.as_ref(), ""),
            };

            let Some(target) = mappings.get(prefix) else {
                anyhow::bail!("Unknown path mapping '{prefix}'");
            };

            let abs_path = Path::new(target).join(suffix);
            let abs_path = Self::append_tolk_extension_if_needed(abs_path);
            return provider.canonicalize(&abs_path);
        }

        let Some(dir) = file.parent() else {
            anyhow::bail!("No parent directory found");
        };
        let abs_path = dir.join(import.as_ref());
        let abs_path = Self::append_tolk_extension_if_needed(abs_path);
        provider.canonicalize(&abs_path)
    }

    fn append_tolk_extension_if_needed(abs_path: PathBuf) -> PathBuf {
        if abs_path.extension().is_some_and(|ext| ext == "tolk") {
            return abs_path;
        }

        let mut abs_path = abs_path;
        abs_path.as_mut_os_string().push(".tolk");
        abs_path
    }
}

/// A builder for creating a `ProjectIndex`.
pub struct ProjectIndexBuilder<'a> {
    file_db: &'a FileDb,
    root_paths: Vec<PathBuf>,
    stdlib_path: Option<PathBuf>,
    mappings: FxHashMap<String, String>,
}

impl<'a> ProjectIndexBuilder<'a> {
    pub fn new(file_db: &'a FileDb, root_path: PathBuf) -> Self {
        Self {
            file_db,
            root_paths: vec![root_path],
            stdlib_path: None,
            mappings: FxHashMap::default(),
        }
    }

    #[must_use]
    pub fn with_stdlib(mut self, path: PathBuf) -> Self {
        self.stdlib_path = Some(path);
        self
    }

    #[must_use]
    pub fn with_additional_roots(mut self, paths: impl IntoIterator<Item = PathBuf>) -> Self {
        self.root_paths.extend(paths);
        self
    }

    /// Sets path mappings used to resolve `@alias/...` imports.
    ///
    /// Keys are normalized to include `@` prefix, matching compiler behavior.
    #[must_use]
    pub fn with_mappings(mut self, mappings: &Option<BTreeMap<String, String>>) -> Self {
        if let Some(mappings) = mappings {
            self.mappings = mappings
                .iter()
                .map(|(key, value)| {
                    if key.starts_with('@') {
                        (key.clone(), value.clone())
                    } else {
                        (format!("@{key}"), value.clone())
                    }
                })
                .collect();
        }

        self
    }

    /// Builds the `ProjectIndex` by recursively following imports from the root file.
    pub fn build(self) -> anyhow::Result<ProjectIndex> {
        let provider = DiskProjectSourceProvider {
            file_db: self.file_db,
        };
        self.build_with_provider(&provider)
    }

    /// Builds the `ProjectIndex` from a platform-neutral source provider.
    pub fn build_with_provider(
        self,
        provider: &dyn ProjectSourceProvider,
    ) -> anyhow::Result<ProjectIndex> {
        let mut errors = vec![];
        let root_paths = self
            .root_paths
            .iter()
            .map(|path| provider.canonicalize(path))
            .collect::<anyhow::Result<Vec<_>>>()?;
        let Some(first_root_path) = root_paths.first().cloned() else {
            anyhow::bail!("Cannot build project index without root files");
        };

        let first_root = match self.process_provider_path(provider, &first_root_path) {
            Ok(info) => info.index().clone(),
            Err(err) => {
                anyhow::bail!("Cannot process root file: {err}")
            }
        };

        let mut files = FxHashMap::default();
        let mut queue = VecDeque::new();

        let mut path_to_file_id = HashMap::new();
        queue.extend(
            first_root
                .imports
                .iter()
                .map(|import| (first_root.id, import.path.clone())),
        );
        path_to_file_id.insert(first_root.path.clone(), first_root.id);
        files.insert(first_root.id, first_root);

        for root_path in root_paths.into_iter().skip(1) {
            let index = match self.process_provider_path(provider, &root_path) {
                Ok(info) => info.index().clone(),
                Err(err) => {
                    errors.push(format!(
                        "Cannot process root file {}: {err}",
                        root_path.display()
                    ));
                    continue;
                }
            };
            if path_to_file_id.contains_key(&index.path) {
                continue;
            }
            let file_id = index.id;
            queue.extend(index.imports.iter().map(|el| (file_id, el.path.clone())));
            path_to_file_id.insert(index.path.clone(), file_id);
            files.insert(file_id, index);
        }

        // process common.tolk file if stdlib_path is provided
        if let Some(ref stdlib) = self.stdlib_path {
            let common_tolk = provider.canonicalize(&stdlib.join("common.tolk"))?;
            match self.process_provider_path(provider, &common_tolk) {
                Ok(info) => {
                    let index = info.index().clone();
                    let file_id = index.id;
                    path_to_file_id.insert(index.path.clone(), file_id);
                    files.insert(file_id, index);
                }
                Err(err) => {
                    errors.push(format!("Cannot process common.tolk file: {err}"));
                }
            }
        }

        while let Some((root_file_id, import)) = queue.pop_front() {
            let Some(root_file) = files.get(&root_file_id).map(|file| &file.path) else {
                continue;
            };
            let resolved = match ProjectIndex::resolve_path_with_provider(
                &import,
                root_file,
                provider,
                self.stdlib_path.as_deref(),
                &self.mappings,
            ) {
                Ok(resolved) => resolved,
                Err(err) => {
                    errors.push(format!("{err:#?}"));
                    continue;
                }
            };

            if path_to_file_id.contains_key(&resolved) {
                continue;
            }

            let index = match self.process_provider_path(provider, &resolved) {
                Ok(info) => info.index().clone(),
                Err(err) => {
                    errors.push(format!("{err:#?}"));
                    continue;
                }
            };
            let file_id = index.id;
            queue.extend(index.imports.iter().map(|el| (file_id, el.path.clone())));

            path_to_file_id.insert(resolved, file_id);
            files.insert(file_id, index);
        }

        let mut imports = FxHashMap::with_capacity_and_hasher(files.len(), Default::default());
        for (id, index) in &files {
            let (file_imports, file_errors) = ProjectIndex::resolve_imports_with_provider(
                index,
                &path_to_file_id,
                provider,
                self.stdlib_path.as_deref(),
                &self.mappings,
            );
            imports.insert(*id, file_imports);
            errors.extend(file_errors);
        }

        let mut dependents: FxHashMap<FileId, Vec<FileId>> =
            FxHashMap::with_capacity_and_hasher(files.len(), Default::default());
        for (id, file_imports) in &imports {
            for import in file_imports {
                if let Some(target_id) = import.target {
                    dependents.entry(target_id).or_default().push(*id);
                }
            }
        }

        let mut global_symbols = HashMap::new();
        for file in files.values() {
            for decl in &file.decls {
                Self::add_symbol_to_global_index(&mut global_symbols, decl);
            }
        }

        Ok(ProjectIndex {
            files,
            imports,
            dependents,
            path_to_file_id,
            resolved_uses: Default::default(),
            stdlib_path: self.stdlib_path,
            mappings: self.mappings,
            errors,
            global_symbols,
        })
    }

    fn process_provider_path(
        &self,
        provider: &dyn ProjectSourceProvider,
        path: &Path,
    ) -> anyhow::Result<Arc<crate::file_db::FileInfo>> {
        if let Some(info) = self.file_db.get_by_path(path) {
            return Ok(info);
        }
        let Some(source) = provider.source(path)? else {
            anyhow::bail!("source file is not available: {}", path.display());
        };
        match source {
            ProjectSource::Text(text) => self.file_db.process_content(path.to_owned(), &text),
            ProjectSource::Parsed(source_file) => Ok(self
                .file_db
                .process_source_file(path.to_owned(), source_file)),
        }
    }

    fn add_symbol_to_global_index(
        global_symbols: &mut HashMap<Arc<str>, Vec<SymbolId>>,
        symbol: &Symbol,
    ) {
        global_symbols
            .entry(symbol.fqn.clone())
            .or_default()
            .push(symbol.id);

        match &symbol.kind {
            SymbolKind::Struct { fields, .. } => {
                for field in fields {
                    Self::add_symbol_to_global_index(global_symbols, field);
                }
            }
            SymbolKind::Enum { members } => {
                for member in members {
                    Self::add_symbol_to_global_index(global_symbols, member);
                }
            }
            _ => {}
        }
    }
}
