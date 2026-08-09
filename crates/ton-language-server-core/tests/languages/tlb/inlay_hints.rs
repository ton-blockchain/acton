#![allow(clippy::needless_raw_string_hashes)]

use expect_test::{Expect, expect};
use ton_language_server_core::languages::tlb::{LANGUAGE_ID, TlbLanguage};
use ton_language_server_core::{
    DocumentUri, InlayHint, LanguageService, LanguageServiceConfig, Position, Range,
};

fn check_hints(source: &str, range: Range, expected: Expect) {
    let uri = DocumentUri::from("file:///workspace/main.tlb");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TlbLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, source)
        .expect("TL-B document should open");

    let hints = service
        .inlay_hints(&uri, range)
        .expect("inlay hints request should succeed");
    expected.assert_eq(&render_hints(&hints));
}

fn render_hints(hints: &[InlayHint]) -> String {
    hints
        .iter()
        .map(|hint| {
            let edit = hint
                .text_edits
                .first()
                .map(|edit| {
                    format!(
                        "{}:{}={}",
                        edit.range.start.line, edit.range.start.character, edit.new_text
                    )
                })
                .unwrap_or_default();
            format!(
                "{}:{} {} edit={edit}",
                hint.position.line, hint.position.character, hint.label
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

const fn full_range() -> Range {
    Range::new(Position::new(0, 0), Position::new(u32::MAX, u32::MAX))
}

#[test]
fn computes_constructor_tag_and_provides_an_applicable_edit() {
    check_hints(
        "transfer query_id:uint64 amount:Coins destination:MsgAddress = InternalMsgBody;",
        full_range(),
        expect!["0:8 #1f058e44 edit=0:8=#1f058e44"],
    );
}

#[test]
fn skips_explicit_tags_and_hints_outside_the_requested_range() {
    check_hints(
        "transfer#abcd value:# = Message;",
        full_range(),
        expect![""],
    );

    check_hints(
        "first value:# = First;\nsecond value:# = Second;",
        Range::new(Position::new(1, 0), Position::new(1, u32::MAX)),
        expect!["1:6 #86b76ef0 edit=1:6=#86b76ef0"],
    );
}
