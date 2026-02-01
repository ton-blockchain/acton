//! Project-wide index of files, symbols, and resolved imports.
//!
//! This module provides the `ProjectIndex` struct which aggregates multiple
//! `FileIndex`es and tracks the relationships between files through imports.

use crate::file_db::FileDb;
use crate::file_index::{FileId, FileIndex, FileSource, Import, Symbol, SymbolId, SymbolKind};
use crate::resolve_index::{FileResolveIndex, NameUse};
use rustc_hash::FxHashMap;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;

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
    pub const fn import(&self) -> &Import {
        &self.import
    }

    /// Return `FileId` which is imported.
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
    /// Map from symbol name to all `SymbolId`s that declare it across the project.
    pub(crate) global_symbols: HashMap<Arc<str>, Vec<SymbolId>>,
    /// List of errors encountered during project indexing.
    pub(crate) errors: Vec<String>,
    /// Map from `FileId` to its name resolution index.
    pub resolved_uses: FxHashMap<FileId, Arc<FileResolveIndex>>,
}

impl ProjectIndex {
    /// Creates a new builder for `ProjectIndex`.
    pub const fn builder(file_db: &'_ FileDb, root_path: PathBuf) -> ProjectIndexBuilder<'_> {
        ProjectIndexBuilder::new(file_db, root_path)
    }

    pub const fn files(&self) -> &FxHashMap<FileId, Arc<FileIndex>> {
        &self.files
    }

    pub const fn imports(&self) -> &FxHashMap<FileId, Vec<ResolvedImport>> {
        &self.imports
    }

    pub const fn path_to_file_id(&self) -> &HashMap<PathBuf, FileId> {
        &self.path_to_file_id
    }

    pub const fn resolved_uses(&self) -> &FxHashMap<FileId, Arc<FileResolveIndex>> {
        &self.resolved_uses
    }

    pub const fn dependents(&self) -> &FxHashMap<FileId, Vec<FileId>> {
        &self.dependents
    }

    /// Returns a list of all file IDs that directly import the given file.
    pub fn direct_dependents(&self, file_id: FileId) -> Vec<FileId> {
        let Some(file) = self.files.get(&file_id) else {
            // very unlikely and likely a bug
            return Vec::new();
        };
        let is_common = file.source_kind == FileSource::Stdlib
            && file.path.file_name().is_some_and(|n| n == "common.tolk");

        if is_common {
            // all files depend on common.tolk
            return self.files.keys().cloned().collect();
        }

        let mut result = vec![file_id]; // include the file itself
        if let Some(dependents) = self.dependents.get(&file_id) {
            result.extend(dependents);
        }

        result
    }

    pub fn stdlib_path(&self) -> Option<&Path> {
        self.stdlib_path.as_deref()
    }

    pub fn errors(&self) -> &[String] {
        &self.errors
    }

    pub fn get_file_index(&self, file_id: FileId) -> Option<&Arc<FileIndex>> {
        self.files.get(&file_id)
    }

    pub fn get_resolved_uses(&self, file_id: FileId) -> Option<&Arc<FileResolveIndex>> {
        self.resolved_uses.get(&file_id)
    }

    pub const fn global_symbols(&self) -> &HashMap<Arc<str>, Vec<SymbolId>> {
        &self.global_symbols
    }

    /// Returns a list of all file IDs that are recursively reachable from the given file.
    pub fn reachable_files(&self, file_id: FileId) -> Vec<FileId> {
        // TODO: add common.tolk
        let mut queue = VecDeque::new();
        queue.push_back(file_id);

        let mut result = vec![file_id];
        while let Some(file_id) = queue.pop_front() {
            let Some(imports) = self.imports.get(&file_id) else {
                continue;
            };

            let imported_files = imports.iter().flat_map(|import| import.target);
            result.extend(imported_files.clone());
            queue.extend(imported_files);
        }

        result
    }

    /// Resolves a `SymbolId` to its corresponding `Symbol` declaration.
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
    pub fn find_use(&self, file_id: FileId, pos: usize) -> Option<&NameUse> {
        self.resolved_uses.get(&file_id)?.find_use(pos)
    }

    pub fn get_file_by_path(&self, path: &Path) -> Option<FileId> {
        self.path_to_file_id.get(path).cloned()
    }

    fn resolve_imports(
        index: &FileIndex,
        path_to_id: &HashMap<PathBuf, FileId>,
        file_db: &FileDb,
        stdlib_path: Option<&Path>,
    ) -> (Vec<ResolvedImport>, Vec<String>) {
        let mut errors = vec![];
        let mut file_imports = Vec::with_capacity(index.imports.len());
        for import in &index.imports {
            let resolved = match Self::resolve_path(&import.path, &index.path, file_db, stdlib_path)
            {
                Ok(resolved) => resolved,
                Err(err) => {
                    errors.push(format!("{:#?}", err));
                    continue;
                }
            };
            let file_id = path_to_id.get(&resolved);
            file_imports.push(ResolvedImport {
                import: import.clone(),
                target: file_id.cloned(),
            });
        }
        (file_imports, errors)
    }

    fn resolve_path(
        import: &Arc<str>,
        file: &Path,
        file_db: &FileDb,
        stdlib_path: Option<&Path>,
    ) -> anyhow::Result<PathBuf> {
        if let Some(relative_path) = import.strip_prefix("@stdlib/") {
            let Some(stdlib) = stdlib_path else {
                anyhow::bail!("Stdlib path not provided for @stdlib import: {}", import);
            };
            let abs_path = stdlib.join(relative_path);
            let abs_path = Self::append_tolk_extension_if_needed(abs_path);
            return Ok(file_db.canonicalize(&abs_path)?);
        }

        let dir = match file.parent() {
            Some(dir) => dir,
            None => {
                anyhow::bail!("No parent directory found");
            }
        };
        let abs_path = dir.join(import.as_ref());
        let abs_path = Self::append_tolk_extension_if_needed(abs_path);
        Ok(file_db.canonicalize(&abs_path)?)
    }

    fn append_tolk_extension_if_needed(abs_path: PathBuf) -> PathBuf {
        if abs_path.ends_with(".tolk") {
            abs_path
        } else {
            abs_path.with_extension("tolk")
        }
    }
}

/// A builder for creating a `ProjectIndex`.
pub struct ProjectIndexBuilder<'a> {
    file_db: &'a FileDb,
    root_path: PathBuf,
    stdlib_path: Option<PathBuf>,
}

impl<'a> ProjectIndexBuilder<'a> {
    pub const fn new(file_db: &'a FileDb, root_path: PathBuf) -> Self {
        Self {
            file_db,
            root_path,
            stdlib_path: None,
        }
    }

    pub fn with_stdlib(mut self, path: PathBuf) -> Self {
        self.stdlib_path = Some(path);
        self
    }

    /// Builds the `ProjectIndex` by recursively following imports from the root file.
    pub fn build(self) -> anyhow::Result<ProjectIndex> {
        let mut errors = vec![];
        let root_path = self.file_db.canonicalize(self.root_path)?;

        let root = match self.file_db.process(&root_path) {
            Ok(info) => info.index().clone(),
            Err(err) => {
                anyhow::bail!("Cannot process root file: {err}")
            }
        };

        let mut files = FxHashMap::default();
        let mut queue = VecDeque::new();
        queue.extend(
            root.imports
                .iter()
                .map(|import| (root.id, import.path.clone())),
        );

        let mut path_to_file_id = HashMap::new();
        path_to_file_id.insert(root.path.clone(), root.id);
        files.insert(root.id, root);

        // process common.tolk file if stdlib_path is provided
        if let Some(ref stdlib) = self.stdlib_path {
            let common_tolk = stdlib.join("common.tolk");
            match self.file_db.process(&common_tolk) {
                Ok(info) => {
                    let index = info.index().clone();
                    let file_id = index.id;
                    path_to_file_id.insert(common_tolk, file_id);
                    files.insert(file_id, index);
                }
                Err(err) => {
                    errors.push(format!("Cannot process common.tolk file: {err}"));
                }
            };
        }

        while let Some((root_file_id, import)) = queue.pop_front() {
            let Some(root_file) = files.get(&root_file_id).map(|file| &file.path) else {
                continue;
            };
            let resolved = match ProjectIndex::resolve_path(
                &import,
                root_file,
                self.file_db,
                self.stdlib_path.as_deref(),
            ) {
                Ok(resolved) => resolved,
                Err(err) => {
                    errors.push(format!("{:#?}", err));
                    continue;
                }
            };

            if path_to_file_id.contains_key(&resolved) {
                continue;
            }

            let index = match self.file_db.process(&resolved) {
                Ok(info) => info.index().clone(),
                Err(err) => {
                    errors.push(format!("{:#?}", err));
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
            let (file_imports, file_errors) = ProjectIndex::resolve_imports(
                index,
                &path_to_file_id,
                self.file_db,
                self.stdlib_path.as_deref(),
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
            errors,
            global_symbols,
        })
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
