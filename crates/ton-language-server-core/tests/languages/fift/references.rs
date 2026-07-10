#![allow(clippy::needless_raw_string_hashes)]

#[path = "../../support/mod.rs"]
mod support;

use expect_test::{Expect, expect};
use support::MarkedSource;
use ton_language_server_core::languages::fift::{FiftLanguage, LANGUAGE_ID};
use ton_language_server_core::{DocumentUri, LanguageService, LanguageServiceConfig, Location};

fn check_references(source: &str, include_declaration: bool, expected: Expect) {
    let marked = MarkedSource::parse(source);
    let uri = DocumentUri::from("file:///workspace/main.fif");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(FiftLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source())
        .expect("Fift document should open");

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
fn finds_references_across_nested_blocks() {
    check_references(
        r"
            PROGRAM{
              entry PROC:<{
                foo CALLDICT
                foo CALLDICT
              }>
              <caret>foo PROC:<{
                foo CALLDICT
              }>
              other PROCREF:<{
                foo CALLDICT
              }>
            END>c
        ",
        false,
        expect![[r"
            2:4-2:7
            3:4-3:7
            6:4-6:7
            9:4-9:7"]],
    );
}

#[test]
fn optionally_includes_the_definition() {
    check_references(
        r"
            PROGRAM{
              entry PROC:<{
                <caret>foo CALLDICT
              }>
              foo PROCINLINE:<{ }>
            END>c
        ",
        true,
        expect![[r"
            4:2-4:5
            2:4-2:7"]],
    );
}

#[test]
fn unresolved_identifier_has_no_references() {
    check_references(
        r"
            PROGRAM{
              entry PROC:<{
                <caret>missing CALLDICT
              }>
            END>c
        ",
        false,
        expect![""],
    );
}
