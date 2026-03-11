use crate::languages::engine::adapter::SyntaxAdapter;
use crate::languages::engine::edits::apply_lsp_changes;
use crate::languages::engine::text_index::TextIndex;
use dashmap::DashMap;
use lsp_types::{Position, Range, TextDocumentContentChangeEvent, Url};
use std::marker::PhantomData;
use std::sync::Arc;
use ton_syntax::ast::PreorderTraverse;
use tree_sitter::{InputEdit, Node, Point};

pub trait HasRootNode {
    fn root_node<'tree>(&'tree self) -> Node<'tree>;
}

impl HasRootNode for tasm_syntax::SourceFile {
    fn root_node<'tree>(&'tree self) -> Node<'tree> {
        self.tree.root_node()
    }
}

impl HasRootNode for fift_syntax::SourceFile {
    fn root_node<'tree>(&'tree self) -> Node<'tree> {
        self.tree.root_node()
    }
}

impl HasRootNode for toml_syntax::SourceFile {
    fn root_node<'tree>(&'tree self) -> Node<'tree> {
        self.tree.root_node()
    }
}

#[derive(Debug, Clone)]
pub struct ParsedSnapshot<TSourceFile> {
    pub uri: Url,
    pub version: i32,
    pub text: Arc<str>,
    pub source_file: Arc<TSourceFile>,
    pub(crate) text_index: Arc<TextIndex>,
}

impl<TSourceFile> ParsedSnapshot<TSourceFile> {
    #[must_use]
    pub fn source(&self) -> &str {
        self.text.as_ref()
    }

    #[must_use]
    pub fn syntax(&self) -> &TSourceFile {
        self.source_file.as_ref()
    }

    pub fn line_offsets(&self) -> &[usize] {
        self.text_index.line_starts()
    }

    pub fn point(&self, position: Position) -> Point {
        self.text_index.position_to_point(self.source(), position)
    }

    pub fn position_to_offset(&self, position: Position) -> usize {
        self.text_index.position_to_offset(self.source(), position)
    }

    pub fn position(&self, offset: usize) -> Position {
        self.text_index.offset_to_position(self.source(), offset)
    }

    pub fn range_of(&self, node: Node) -> Range {
        self.position_range(node.start_byte(), node.end_byte())
    }

    pub(crate) fn position_range(&self, start_offset: usize, end_offset: usize) -> Range {
        Range::new(self.position(start_offset), self.position(end_offset))
    }

    pub fn text_of<'tree>(&'tree self, node: Node<'tree>) -> &'tree str {
        node.utf8_text(self.text.as_bytes()).unwrap_or("<invalid>")
    }

    pub fn new(
        uri: Url,
        version: i32,
        text: impl Into<Arc<str>>,
        source_file: Arc<TSourceFile>,
    ) -> Self {
        let text = text.into();
        let text_index = Arc::new(TextIndex::new(&text));

        Self {
            uri,
            version,
            text,
            source_file,
            text_index,
        }
    }
}

impl<TSourceFile: HasRootNode> ParsedSnapshot<TSourceFile> {
    pub fn traverse(&self) -> PreorderTraverse<'_> {
        PreorderTraverse::new(self.syntax().root_node().walk())
    }

    pub fn node_at(&self, position: Position) -> Option<Node<'_>> {
        let point = self.point(position);
        self.syntax()
            .root_node()
            .descendant_for_point_range(point, point)
    }

    pub fn find_node_at(&self, position: Position) -> Option<Node<'_>> {
        self.node_at(position)
    }
}

#[derive(Debug, Clone)]
struct CachedDocument<TSourceFile> {
    version: i32,
    text: Arc<str>,
    snapshot: Option<ParsedSnapshot<TSourceFile>>,
}

#[derive(Debug, Clone)]
pub struct CacheSyncResult<TSourceFile> {
    pub snapshot: Option<ParsedSnapshot<TSourceFile>>,
    pub parse_failed: bool,
}

pub struct IncrementalParseCache<A: SyntaxAdapter> {
    docs: DashMap<Url, CachedDocument<A::SourceFile>>,
    _adapter: PhantomData<A>,
}

impl<A: SyntaxAdapter> IncrementalParseCache<A> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            docs: DashMap::new(),
            _adapter: PhantomData,
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.docs.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.docs.len()
    }

    pub fn remove(&self, uri: &Url) -> Option<ParsedSnapshot<A::SourceFile>> {
        self.docs
            .remove(uri)
            .and_then(|(_, document)| document.snapshot)
    }

    pub fn snapshot(&self, uri: &Url) -> Option<ParsedSnapshot<A::SourceFile>> {
        self.docs
            .get(uri)
            .and_then(|document| document.snapshot.clone())
    }

    pub fn text(&self, uri: &Url) -> Option<Arc<str>> {
        self.docs.get(uri).map(|document| document.text.clone())
    }

    pub fn open(
        &self,
        uri: &Url,
        version: i32,
        text: &str,
    ) -> anyhow::Result<ParsedSnapshot<A::SourceFile>> {
        let text = Arc::<str>::from(text);
        match A::parse(text.as_ref()) {
            Ok(parsed) => {
                let snapshot =
                    ParsedSnapshot::new(uri.clone(), version, text.clone(), Arc::new(parsed));
                self.docs.insert(
                    uri.clone(),
                    CachedDocument {
                        version,
                        text,
                        snapshot: Some(snapshot.clone()),
                    },
                );
                Ok(snapshot)
            }
            Err(error) => {
                self.docs.insert(
                    uri.clone(),
                    CachedDocument {
                        version,
                        text,
                        snapshot: None,
                    },
                );
                Err(error)
            }
        }
    }

    pub fn sync_changes(
        &self,
        uri: &Url,
        version: i32,
        changes: &[TextDocumentContentChangeEvent],
    ) -> anyhow::Result<Option<CacheSyncResult<A::SourceFile>>> {
        let Some(current) = self.docs.get(uri).map(|document| document.clone()) else {
            return Ok(None);
        };

        if version < current.version {
            return Ok(Some(CacheSyncResult {
                snapshot: current.snapshot,
                parse_failed: false,
            }));
        }

        let applied = apply_lsp_changes(current.text.as_ref(), changes);
        let next_text = Arc::<str>::from(applied.text);

        let parsed = if let Some(snapshot) = current.snapshot.as_ref() {
            parse_with_incremental_fallback::<A>(
                snapshot,
                next_text.as_ref(),
                applied.incremental_edits.as_deref(),
            )
        } else {
            A::parse(next_text.as_ref())
        };

        match parsed {
            Ok(parsed) => {
                let snapshot =
                    ParsedSnapshot::new(uri.clone(), version, next_text.clone(), Arc::new(parsed));
                self.docs.insert(
                    uri.clone(),
                    CachedDocument {
                        version,
                        text: next_text.clone(),
                        snapshot: Some(snapshot.clone()),
                    },
                );
                Ok(Some(CacheSyncResult {
                    snapshot: Some(snapshot),
                    parse_failed: false,
                }))
            }
            Err(error) => {
                log::debug!("self-contained parse failed for {uri}: {error}");
                self.docs.insert(
                    uri.clone(),
                    CachedDocument {
                        version,
                        text: next_text.clone(),
                        snapshot: None,
                    },
                );
                Ok(Some(CacheSyncResult {
                    snapshot: None,
                    parse_failed: true,
                }))
            }
        }
    }
}

impl<A: SyntaxAdapter> Default for IncrementalParseCache<A> {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_with_incremental_fallback<A: SyntaxAdapter>(
    current: &ParsedSnapshot<A::SourceFile>,
    source: &str,
    incremental_edits: Option<&[InputEdit]>,
) -> anyhow::Result<A::SourceFile> {
    let Some(incremental_edits) = incremental_edits else {
        return A::parse(source);
    };

    let mut old_tree = A::tree(current.source_file.as_ref()).clone();
    for edit in incremental_edits {
        old_tree.edit(edit);
    }

    match A::parse_with_old_tree(source, Some(&old_tree)) {
        Ok(parsed) => Ok(parsed),
        Err(incremental_error) => {
            log::debug!(
                "incremental parse failed, falling back to full parse: {incremental_error}"
            );
            A::parse(source)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::bail;
    use lsp_types::{Position, Range, TextDocumentContentChangeEvent};
    use std::sync::Arc;
    use tree_sitter::Tree;

    use crate::languages::engine::adapter::{SyntaxAdapter, TasmSyntaxAdapter};

    fn pos(line: u32, character: u32) -> Position {
        Position { line, character }
    }

    fn range(start_line: u32, start_character: u32, end_line: u32, end_character: u32) -> Range {
        Range {
            start: pos(start_line, start_character),
            end: pos(end_line, end_character),
        }
    }

    fn change(range: Option<Range>, text: &str) -> TextDocumentContentChangeEvent {
        TextDocumentContentChangeEvent {
            range,
            range_length: None,
            text: text.to_string(),
        }
    }

    #[test]
    fn applies_incremental_reparse() -> anyhow::Result<()> {
        let cache = IncrementalParseCache::<TasmSyntaxAdapter>::new();
        let uri = Url::parse("file:///tmp/cache_test.tasm")?;

        cache.open(&uri, 1, "PUSHINT_4 1\n")?;
        let changes = vec![change(Some(range(0, 10, 0, 11)), "2")];

        let updated = cache
            .sync_changes(&uri, 2, &changes)?
            .expect("snapshot should be present");
        let snapshot = updated
            .snapshot
            .expect("parsed snapshot should be present after successful parse");

        assert_eq!(snapshot.version, 2);
        assert_eq!(snapshot.text.as_ref(), "PUSHINT_4 2\n");
        assert_eq!(snapshot.source_file.top_levels().count(), 1);
        Ok(())
    }

    #[derive(Debug, Clone, Copy, Default)]
    struct ForcedFallbackAdapter;

    impl SyntaxAdapter for ForcedFallbackAdapter {
        type SourceFile = tasm_syntax::SourceFile;

        fn parse(source: &str) -> anyhow::Result<Self::SourceFile> {
            tasm_syntax::parse(source)
        }

        fn parse_with_old_tree(
            _source: &str,
            old_tree: Option<&Tree>,
        ) -> anyhow::Result<Self::SourceFile> {
            if old_tree.is_some() {
                bail!("forced incremental parse error");
            }
            unreachable!("incremental parse path should always pass old_tree")
        }

        fn tree(source_file: &Self::SourceFile) -> &Tree {
            &source_file.tree
        }
    }

    #[test]
    fn falls_back_to_full_parse_when_incremental_fails() -> anyhow::Result<()> {
        let cache = IncrementalParseCache::<ForcedFallbackAdapter>::new();
        let uri = Url::parse("file:///tmp/cache_fallback.tasm")?;

        cache.open(&uri, 1, "PUSHINT_4 1\n")?;
        let changes = vec![change(Some(range(0, 10, 0, 11)), "2")];

        let updated = cache
            .sync_changes(&uri, 2, &changes)?
            .expect("snapshot should be present");
        let snapshot = updated
            .snapshot
            .expect("parsed snapshot should be present after fallback parse");

        assert_eq!(snapshot.text.as_ref(), "PUSHINT_4 2\n");
        assert_eq!(snapshot.source_file.top_levels().count(), 1);
        Ok(())
    }

    #[derive(Debug, Clone, Copy, Default)]
    struct RejectingAdapter;

    impl SyntaxAdapter for RejectingAdapter {
        type SourceFile = tasm_syntax::SourceFile;

        fn parse(source: &str) -> anyhow::Result<Self::SourceFile> {
            if source.contains("INVALID") {
                bail!("rejected source");
            }
            tasm_syntax::parse(source)
        }

        fn parse_with_old_tree(
            source: &str,
            old_tree: Option<&Tree>,
        ) -> anyhow::Result<Self::SourceFile> {
            if source.contains("INVALID") {
                bail!("rejected source");
            }
            tasm_syntax::parse_with_old_tree(source, old_tree)
        }

        fn tree(source_file: &Self::SourceFile) -> &Tree {
            &source_file.tree
        }
    }

    #[test]
    fn keeps_latest_text_and_version_when_parse_fails_then_recovers() -> anyhow::Result<()> {
        let cache = IncrementalParseCache::<RejectingAdapter>::new();
        let uri = Url::parse("file:///tmp/cache_recovery.tasm")?;

        cache.open(&uri, 1, "PUSHINT_4 1\n")?;

        let break_parse = vec![change(None, "INVALID\n")];
        let failed = cache
            .sync_changes(&uri, 2, &break_parse)?
            .expect("doc state should exist");

        assert!(failed.parse_failed);
        assert!(failed.snapshot.is_none());
        let failed_text = cache
            .text(&uri)
            .expect("recoverable text should be available after parse failure");
        assert_eq!(failed_text.as_ref(), "INVALID\n");
        assert!(cache.snapshot(&uri).is_none());

        let recover = vec![change(None, "PUSHINT_4 3\n")];
        let recovered = cache
            .sync_changes(&uri, 3, &recover)?
            .expect("doc state should exist");
        let recovered_snapshot = recovered
            .snapshot
            .expect("parsed snapshot should recover on valid source");

        assert!(!recovered.parse_failed);
        assert_eq!(recovered_snapshot.text.as_ref(), "PUSHINT_4 3\n");
        let recovered_text = cache
            .text(&uri)
            .expect("recoverable text should be available after parse recovery");
        assert_eq!(recovered_text.as_ref(), "PUSHINT_4 3\n");
        assert_eq!(recovered_snapshot.version, 3);
        assert_eq!(recovered_snapshot.source_file.top_levels().count(), 1);
        Ok(())
    }

    #[test]
    fn keeps_recoverable_state_when_open_parse_fails() -> anyhow::Result<()> {
        let cache = IncrementalParseCache::<RejectingAdapter>::new();
        let uri = Url::parse("file:///tmp/cache_open_fail.tasm")?;

        let open_error = cache
            .open(&uri, 1, "INVALID\n")
            .expect_err("open should fail for rejected source");
        assert!(open_error.to_string().contains("rejected source"));
        let failed_open_text = cache
            .text(&uri)
            .expect("recoverable text should be available after failed open");
        assert_eq!(failed_open_text.as_ref(), "INVALID\n");
        assert!(cache.snapshot(&uri).is_none());

        let recover = vec![change(None, "PUSHINT_4 10\n")];
        let recovered = cache
            .sync_changes(&uri, 2, &recover)?
            .expect("doc state should exist after failed open");
        let recovered_snapshot = recovered
            .snapshot
            .expect("snapshot should recover without re-open");

        assert_eq!(recovered_snapshot.version, 2);
        assert_eq!(recovered_snapshot.text.as_ref(), "PUSHINT_4 10\n");
        Ok(())
    }

    #[test]
    fn snapshot_position_and_offset_roundtrip() {
        let uri = Url::parse("file:///tmp/position_snapshot.tasm").expect("uri should parse");
        let text = "a😀b\n😄";
        let source_file = Arc::new(tasm_syntax::parse(text).expect("sample text should parse"));
        let snapshot = ParsedSnapshot::new(uri, 1, text, source_file);

        let position = Position::new(0, 3);
        let point = snapshot.point(position);
        assert_eq!(point.row, 0);
        assert_eq!(point.column, 5);

        let byte_offset = snapshot.position_to_offset(position);
        assert_eq!(byte_offset, 5);
        assert_eq!(snapshot.position(byte_offset), position);
    }

    #[test]
    fn snapshot_position_and_offset_roundtrip_with_crlf() {
        let uri = Url::parse("file:///tmp/position_snapshot_crlf.tasm").expect("uri should parse");
        let text = "ab\r\nc😀d\r\n";
        let source_file = Arc::new(tasm_syntax::parse(text).expect("sample text should parse"));
        let snapshot = ParsedSnapshot::new(uri, 1, text, source_file);

        let position = Position::new(1, 3);
        let point = snapshot.point(position);
        assert_eq!(point.row, 1);
        assert_eq!(point.column, 5);

        let byte_offset = snapshot.position_to_offset(position);
        assert_eq!(byte_offset, 9);
        assert_eq!(snapshot.position(byte_offset), position);
    }
}
