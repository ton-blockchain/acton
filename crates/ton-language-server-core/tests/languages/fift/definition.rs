#![allow(clippy::needless_raw_string_hashes)]

#[path = "../../support/mod.rs"]
mod support;

use expect_test::{Expect, expect};
use support::MarkedSource;
use ton_language_server_core::languages::fift::{FiftLanguage, LANGUAGE_ID};
use ton_language_server_core::{DocumentUri, LanguageService, LanguageServiceConfig};

fn check_definition(source: &str, expected: Expect) {
    let marked = MarkedSource::parse(source);
    let uri = DocumentUri::from("file:///workspace/main.fif");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(FiftLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source())
        .expect("Fift document should open");

    let position = marked.marker("caret").position;
    let locations = service
        .definition(&uri, position)
        .expect("definition request should succeed");
    let result = locations.first().map_or_else(
        || "unresolved".to_owned(),
        |location| {
            format!(
                "{}:{}-{}:{}",
                location.range.start.line,
                location.range.start.character,
                location.range.end.line,
                location.range.end.character,
            )
        },
    );
    expected.assert_eq(&result);
}

#[test]
fn resolves_all_supported_definition_kinds() {
    check_definition(
        r"
            PROGRAM{
              entry PROC:<{
                <caret>inl CALLDICT
              }>
              inl PROCINLINE:<{ }>
            END>c
        ",
        expect!["4:2-4:5"],
    );

    check_definition(
        r"
            PROGRAM{
              entry PROC:<{
                <caret>refProc CALLDICT
              }>
              refProc PROCREF:<{ }>
            END>c
        ",
        expect!["4:2-4:9"],
    );

    check_definition(
        r"
            PROGRAM{
              entry PROC:<{
                <caret>method CALLDICT
              }>
              method METHOD:<{ }>
            END>c
        ",
        expect!["4:2-4:8"],
    );
}

#[test]
fn resolves_first_duplicate_and_ignores_declarations_without_bodies() {
    check_definition(
        r"
            PROGRAM{
              DECLPROC missing
              entry PROC:<{
                <caret>missing CALLDICT
              }>
            END>c
        ",
        expect!["unresolved"],
    );

    check_definition(
        r"
            PROGRAM{
              entry PROC:<{
                <caret>foo CALLDICT
              }>
              foo PROC:<{ }>
              foo PROCINLINE:<{ }>
            END>c
        ",
        expect!["4:2-4:5"],
    );
}
