#![allow(clippy::needless_raw_string_hashes)]

#[path = "../../support/mod.rs"]
mod support;

use expect_test::{Expect, expect};
use support::MarkedSource;
use ton_language_server_core::languages::tlb::{LANGUAGE_ID, TlbLanguage};
use ton_language_server_core::{DocumentUri, LanguageService, LanguageServiceConfig, Location};

fn check_references(source: &str, include_declaration: bool, expected: Expect) {
    let marked = MarkedSource::parse(source);
    let uri = DocumentUri::from("file:///workspace/main.tlb");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TlbLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source())
        .expect("TL-B document should open");

    let locations = service
        .references(&uri, marked.marker("caret").position, include_declaration)
        .expect("references request should succeed");
    expected.assert_eq(&render_locations(&locations));
}

fn render_locations(locations: &[Location]) -> String {
    locations
        .iter()
        .map(|location| {
            format!(
                "{}:{}-{}:{}",
                location.range.start.line,
                location.range.start.character,
                location.range.end.line,
                location.range.end.character,
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn finds_uses_without_returning_the_declaration() {
    check_references(
        r"
            foo$0 value:# = Message;
            wrap$1 first:Message second:Message = Envelope;
            use$2 value:<caret>Message = Result;
        ",
        false,
        expect![[r"
            1:13-1:20
            1:28-1:35
            2:12-2:19"]],
    );
}

#[test]
fn includes_all_matching_declarations_when_requested() {
    check_references(
        r"
            foo$0 value:# = Message;
            bar$1 value:# = Message;
            use$2 value:<caret>Message = Result;
        ",
        true,
        expect![[r"
            0:16-0:23
            1:16-1:23
            2:12-2:19"]],
    );
}

#[test]
fn keeps_local_fields_separate_from_same_named_fields_in_other_declarations() {
    check_references(
        r"
            one$0 value:# copy:value = First;
            two$1 value:# copy:<caret>value = Second;
        ",
        false,
        expect!["1:19-1:24"],
    );
}
