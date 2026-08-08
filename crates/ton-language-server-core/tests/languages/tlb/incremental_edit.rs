use std::any::Any;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use ton_language_server_core::languages::tlb::LANGUAGE_ID;
use ton_language_server_core::{
    DocumentUri, FeatureSet, LanguageId, LanguagePlugin, LanguageService, LanguageServiceConfig,
    ParseRequest, ParsedDocument, Position, Range, TextEdit, default_language_service,
};
use tree_sitter::Tree;

#[test]
fn edit_document_updates_tlb_definition() -> anyhow::Result<()> {
    let uri = DocumentUri::from("acton://fixture/incremental-definition.tlb");
    let mut service = default_language_service();
    service.open_document(
        uri.clone(),
        LANGUAGE_ID,
        1,
        "foo$0 a:# = Old;\nbar$1 x:Old = Wrap;\n",
    )?;

    service.edit_document(
        &uri,
        2,
        [
            TextEdit::new(range(0, 0, 0, 3), "foobar"),
            TextEdit::new(range(0, 15, 0, 18), "New"),
            TextEdit::new(range(1, 8, 1, 11), "New"),
        ],
    )?;

    let locations = service.definition(&uri, Position::new(1, 8))?;
    assert_eq!(locations.len(), 1);
    assert_eq!(locations[0].range.start, Position::new(0, 15));

    Ok(())
}

#[test]
fn incremental_tlb_parse_matches_clean_parse_after_edits() -> anyhow::Result<()> {
    let incremental_parses = Arc::new(AtomicUsize::new(0));
    let uri = DocumentUri::from("acton://fixture/incremental-tree.tlb");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(CheckingTlbLanguage {
        incremental_parses: incremental_parses.clone(),
    });

    service.open_document(
        uri.clone(),
        CHECKING_LANGUAGE_ID,
        1,
        "foo$0 a:# = Old;\nbar$1 x:Old = Wrap;\n",
    )?;
    service.edit_document(
        &uri,
        2,
        [
            TextEdit::new(range(0, 0, 0, 3), "foobar"),
            TextEdit::new(range(0, 15, 0, 18), "New"),
            TextEdit::new(range(1, 8, 1, 11), "New"),
        ],
    )?;

    assert_eq!(incremental_parses.load(Ordering::SeqCst), 1);
    Ok(())
}

const CHECKING_LANGUAGE_ID: &str = "checking-tlb";

#[derive(Clone)]
struct CheckingTlbLanguage {
    incremental_parses: Arc<AtomicUsize>,
}

impl LanguagePlugin for CheckingTlbLanguage {
    fn language_id(&self) -> LanguageId {
        LanguageId::from(CHECKING_LANGUAGE_ID)
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["tlb"]
    }

    fn capabilities(&self) -> FeatureSet {
        FeatureSet::default()
    }

    fn parse(&self, request: ParseRequest<'_>) -> anyhow::Result<Box<dyn ParsedDocument>> {
        let source_file =
            tlb_syntax::parse_with_old_tree(request.document.text(), request.old_tree)?;

        if request.old_tree.is_some() {
            let clean_source_file = tlb_syntax::parse_with_old_tree(request.document.text(), None)?;
            assert_eq!(
                source_file.tree.root_node().to_sexp(),
                clean_source_file.tree.root_node().to_sexp(),
                "incremental TL-B parse tree should match a clean parse"
            );
            assert!(
                !source_file.tree.root_node().has_error(),
                "incremental TL-B parse tree should not contain syntax errors"
            );
            self.incremental_parses.fetch_add(1, Ordering::SeqCst);
        }

        Ok(Box::new(ParsedTlb { source_file }))
    }
}

struct ParsedTlb {
    source_file: tlb_syntax::SourceFile,
}

impl ParsedDocument for ParsedTlb {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn tree(&self) -> &Tree {
        &self.source_file.tree
    }
}

const fn range(start_line: u32, start_character: u32, end_line: u32, end_character: u32) -> Range {
    Range::new(
        Position::new(start_line, start_character),
        Position::new(end_line, end_character),
    )
}
