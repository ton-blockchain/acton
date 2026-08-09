#![allow(clippy::needless_raw_string_hashes)]

use expect_test::{Expect, expect};
use ton_language_server_core::languages::tlb::{LANGUAGE_ID, TlbLanguage};
use ton_language_server_core::{
    DocumentSymbol, DocumentUri, LanguageService, LanguageServiceConfig,
};

fn check_symbols(source: &str, expected: Expect) {
    let uri = DocumentUri::from("file:///workspace/main.tlb");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TlbLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, source)
        .expect("TL-B document should open");

    let symbols = service
        .document_symbols(&uri)
        .expect("document symbols request should succeed");
    expected.assert_eq(&render_symbols(&symbols));
}

fn render_symbols(symbols: &[DocumentSymbol]) -> String {
    symbols
        .iter()
        .map(|symbol| {
            format!(
                "{} {:?} selection={}:{} detail={}",
                symbol.name,
                symbol.kind,
                symbol.selection_range.start.line,
                symbol.selection_range.start.character,
                symbol.detail.as_deref().unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn lists_constructor_result_types_in_source_order() {
    check_symbols(
        r"foo$0 value:# = Message;
bar$1 value:uint32 = Message;
wrap$2 value:Message = Envelope;",
        expect![[r"
            Message Class selection=0:16 detail=foo$0 value:# = Message;
            Message Class selection=1:21 detail=bar$1 value:uint32 = Message;
            Envelope Class selection=2:23 detail=wrap$2 value:Message = Envelope;"]],
    );
}

#[test]
fn empty_document_has_no_symbols() {
    check_symbols("", expect![""]);
}
