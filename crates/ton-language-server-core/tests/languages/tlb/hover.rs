#![allow(clippy::needless_raw_string_hashes)]

#[path = "../../support/mod.rs"]
mod support;

use expect_test::{Expect, expect};
use support::MarkedSource;
use ton_language_server_core::languages::tlb::{LANGUAGE_ID, TlbLanguage};
use ton_language_server_core::{DocumentUri, LanguageService, LanguageServiceConfig};

fn check_hover(source: &str, expected: Expect) {
    let marked = MarkedSource::parse(source);
    let uri = DocumentUri::from("file:///workspace/main.tlb");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TlbLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source())
        .expect("TL-B document should open");

    let hover = service
        .hover(&uri, marked.marker("caret").position)
        .expect("hover request should succeed");
    expected.assert_eq(hover.as_ref().map_or("<none>", |hover| &hover.contents));
}

#[test]
fn shows_all_declarations_of_a_combinator_type() {
    check_hover(
        r"
            foo$0 value:# = Message;
            bar$1 value:uint32 = Message;
            wrap$2 value:<caret>Message = Envelope;
        ",
        expect![[r#"
            ```tlb
            foo$0 value:# = Message;

            bar$1 value:uint32 = Message;
            ```"#]],
    );
}

#[test]
fn documents_builtin_and_sized_integer_types() {
    check_hover(
        "foo$0 value:<caret>uint32 = Message;",
        expect![[r#"
            **uint32** - 32-bit unsigned integer

            - **Range**: 0 to 2^32 - 1
            - **Size**: 32 bits"#]],
    );

    check_hover(
        "foo$0 value:<caret># = Message;",
        expect!["Nat, 32-bit unsigned integer"],
    );
}

#[test]
fn ignores_invalid_sized_types_and_constructor_tags() {
    check_hover("foo$<caret>0 value:# = Message;", expect!["<none>"]);

    check_hover("foo$0 value:<caret>uint257 = Message;", expect!["<none>"]);
}
