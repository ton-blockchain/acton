//! In-memory database for source files and their indices.
//!
//! This module provides the `FileDb` struct which manages reading, parsing,
//! and indexing files. It also handles file ID allocation and caching.

use crate::file_index::{FileId, FileIndex, FileSource, Span, Symbol};
use crate::{AstNodeSpanExt, SymbolId};
use dashmap::DashMap;
use log::debug;
use smol_str::SmolStr;
use std::fmt::{Debug, Formatter};
use std::io;
use std::path::{Path, PathBuf};
use std::str::Utf8Error;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use tolk_syntax::{AstNode, ast};
use tree_sitter::{Node, Tree};

/// Holds information about a single processed source file.
#[derive(Clone)]
pub struct FileInfo {
    /// Unique identifier for the file.
    id: FileId,
    /// Pre-computed index of symbols and imports.
    index: Arc<FileIndex>,
    /// The parsed AST and source code.
    source: ast::SourceFile,
    /// Pre-computed line offsets for efficient position mapping.
    line_offsets: Vec<usize>,
}

impl FileInfo {
    pub const fn id(&self) -> FileId {
        self.id
    }

    pub const fn index(&self) -> &Arc<FileIndex> {
        &self.index
    }

    pub fn path(&self) -> &PathBuf {
        &self.index.path
    }

    pub const fn source(&self) -> &ast::SourceFile {
        &self.source
    }

    pub fn line_offsets(&self) -> &[usize] {
        &self.line_offsets
    }

    /// Checks if passed file resides in Tolk standard library.
    pub fn is_stdlib_file(&self) -> bool {
        self.index.source_kind == FileSource::Stdlib
    }

    /// Checks if passed file resides in Acton standard library.
    pub fn is_acton_file(&self) -> bool {
        self.index.source_kind == FileSource::Acton
    }

    /// Checks if passed file resides in workspace, not in Tolk stdlib or Acton files.
    pub fn is_workspace_file(&self) -> bool {
        self.index.source_kind == FileSource::Workspace
    }

    /// Returns the source text associated with a tree-sitter node.
    pub fn text(&self, node: &Node) -> Result<&str, Utf8Error> {
        node.utf8_text(self.source.source.as_ref().as_ref())
    }

    /// Finds the `Symbol` declaration corresponding to an AST node that has a name.
    pub fn find_declaration<'a, Node: AstNode<'a>>(&self, node: &Node) -> Option<&Symbol> {
        let span = node.span();
        let idx = self.index.find_symbol_index_at_offset(span.start())?;
        let symbol = &self.index.decls[idx];
        if symbol.body_span == span {
            Some(symbol)
        } else {
            None
        }
    }

    /// Finds the `Symbol` declaration containing given offset.
    pub fn find_symbol_at(&self, offset: usize) -> Option<&Symbol> {
        let idx = self.index.find_symbol_index_at_offset(offset)?;
        Some(&self.index.decls[idx])
    }

    /// Finds AST node for declaration with given name span.
    pub fn find_syntax_declaration(&self, decl_id: SymbolId) -> Option<ast::TopLevel<'_>> {
        let idx = self.index.symbol_id_to_decl_index.get(&decl_id.local_id)?;
        let child = self.source.root_node().child(*idx)?;
        Some(child.into())
    }
}

/// A thread-safe database that stores `FileInfo` and manages file operations.
pub struct FileDb {
    /// Map from absolute path to `FileInfo`.
    files: DashMap<PathBuf, Arc<FileInfo>>,
    /// Map from `FileId` to `FileInfo`.
    files_by_id: DashMap<FileId, Arc<FileInfo>>,
    /// Cache for canonicalized paths to avoid repeated I/O.
    canonicalize_cache: DashMap<PathBuf, PathBuf>,
    /// Path to Tolk standard library.
    stdlib_path: PathBuf,
    /// Path to Acton standard library.
    acton_stdlib_path: Option<PathBuf>,
    next_id: AtomicU32,
}

impl Debug for FileDb {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "FileDb")
    }
}

impl FileDb {
    /// Creates a new, empty `FileDb`.
    pub fn new(stdlib_path: PathBuf, acton_stdlib_path: Option<PathBuf>) -> Self {
        FileDb {
            files: DashMap::new(),
            files_by_id: DashMap::new(),
            canonicalize_cache: DashMap::new(),
            next_id: AtomicU32::new(0),
            stdlib_path,
            acton_stdlib_path,
        }
    }

    pub fn stdlib_path(&self) -> &Path {
        self.stdlib_path.as_path()
    }

    /// Reads and processes a file from the disk.
    /// Returns a cached version if already processed.
    pub fn process(&self, path: &Path) -> anyhow::Result<Arc<FileInfo>> {
        if let Some(cached) = self.files.get(path) {
            debug!("cache hit for {}", path.display());
            return Ok(cached.clone());
        }
        let content = std::fs::read_to_string(path)?;
        self.process_content(path.to_owned(), &content)
    }

    /// Processes the given content as a file with the specified path.
    pub fn process_content(&self, path: PathBuf, content: &str) -> anyhow::Result<Arc<FileInfo>> {
        self.process_content_incremental(path, content, None)
    }

    /// Processes content incrementally, optionally using an old syntax tree.
    pub fn process_content_incremental(
        &self,
        path: PathBuf,
        content: &str,
        old_tree: Option<&Tree>,
    ) -> anyhow::Result<Arc<FileInfo>> {
        let file = tolk_syntax::parse_with_old_tree(content, old_tree)?;

        let mut line_offsets = vec![0];
        let mut last_offset = 0;
        for line in content.lines() {
            last_offset += line.len() + 1;
            line_offsets.push(last_offset);
        }

        let existing = self.files.get(&path);
        let file_id = existing.map(|e| e.id).unwrap_or_else(|| self.alloc_id());

        let source_kind = match (&self.stdlib_path, &self.acton_stdlib_path) {
            (stdlib_path, _) if path.starts_with(stdlib_path) => FileSource::Stdlib,
            (_, Some(acton_path)) if path.starts_with(acton_path) => FileSource::Acton,
            _ => FileSource::Workspace,
        };
        let info = Arc::new(FileInfo {
            id: file_id,
            index: Arc::new(FileIndex::build(
                content,
                file_id,
                path.clone(),
                &file,
                source_kind,
            )),
            source: file,
            line_offsets,
        });

        // TODO: possible double work on concurrent run
        self.files.insert(path, info.clone());
        self.files_by_id.insert(file_id, info.clone());
        Ok(info)
    }

    /// Checks if passed file resides in Tolk standard library.
    pub fn is_stdlib_file(&self, file_id: FileId) -> bool {
        let Some(info) = self.files_by_id.get(&file_id) else {
            return false;
        };
        info.index.path.starts_with(&self.stdlib_path)
    }

    /// Checks if passed file resides in Acton standard library.
    pub fn is_acton_file(&self, file_id: FileId) -> bool {
        let Some(info) = self.files_by_id.get(&file_id) else {
            return false;
        };
        let Some(acton_stdlib_path) = &self.acton_stdlib_path else {
            return false;
        };

        info.index.path.starts_with(acton_stdlib_path)
    }

    /// Resolves a path to its `FileInfo` if it has already been processed.
    pub fn get_by_path(&self, path: &Path) -> Option<Arc<FileInfo>> {
        self.files.get(path).map(|entry| entry.clone())
    }

    /// Resolves a `FileId` to its `FileInfo` if it has already been processed.
    pub fn get_by_id(&self, file_id: FileId) -> Option<Arc<FileInfo>> {
        self.files_by_id.get(&file_id).map(|entry| entry.clone())
    }

    /// Canonicalizes a path and caches the result.
    pub fn canonicalize<P: AsRef<Path>>(&self, path: P) -> io::Result<PathBuf> {
        let path = path.as_ref();
        if let Some(cached) = self.canonicalize_cache.get(path) {
            return Ok(cached.clone());
        }
        let canonical = path.canonicalize()?;
        self.canonicalize_cache
            .insert(path.to_owned(), canonical.clone());
        Ok(canonical)
    }

    /// Retrieves the text content corresponding to a span in a file.
    pub fn text(&self, file_id: FileId, span: Span) -> Option<SmolStr> {
        let file = self.files_by_id.get(&file_id)?;
        Some(file.source.source.get(span.start()..span.end())?.into())
    }

    /// Retrieves the text content of an AST node.
    pub fn text_of<'a, Node: AstNode<'a>>(&self, file_id: FileId, node: &Node) -> Option<SmolStr> {
        self.text(file_id, node.span())
    }

    /// Efficiently checks if two AST nodes in the same file have the same text.
    pub fn have_same_text<'a, 'b, Left: AstNode<'a>, Right: AstNode<'b>>(
        &self,
        file_id: FileId,
        left: &Left,
        right: &Right,
    ) -> bool {
        if left.text_length() != right.text_length() {
            // fast path
            return false;
        }
        let Some(file) = self.files_by_id.get(&file_id) else {
            return false;
        };
        let source = file.source.source.clone();
        let right_text = right.text(&source);
        left.text_matches(&source, right_text)
    }

    pub fn text_matches<'a, Left: AstNode<'a>>(
        &self,
        file_id: FileId,
        left: &Left,
        expected: &str,
    ) -> bool {
        let syntax = left.syntax();
        let start = syntax.start_byte();
        let end = syntax.end_byte();
        let width = end - start;
        if width != expected.len() {
            // fast path, width of node is not equal to width of expected string
            return false;
        }

        let Some(file) = self.files_by_id.get(&file_id) else {
            return false;
        };
        let source = file.source.source.clone();

        if end > source.len() || start > end {
            return false;
        }

        // don't create an actual string for substring and just compare bytes
        &source.as_bytes()[start..end] == expected.as_bytes()
    }

    pub fn text_contains<'a, Node: AstNode<'a>>(
        &self,
        file_id: FileId,
        node: &Node,
        ch: u8,
    ) -> Option<usize> {
        let file = self.files_by_id.get(&file_id)?;
        node.text_contains(&file.source.source, ch)
    }

    /// Returns an iterator over all processed files.
    pub fn iter(&self) -> impl Iterator<Item = Arc<FileInfo>> + '_ {
        self.files.iter().map(|entry| entry.value().clone())
    }

    fn alloc_id(&self) -> FileId {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }
}
