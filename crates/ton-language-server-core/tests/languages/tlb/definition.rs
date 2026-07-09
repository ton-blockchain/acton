#[path = "../../support/mod.rs"]
mod support;

use expect_test::{Expect, expect};
use support::{MarkedSource, render_definition};
use ton_language_server_core::languages::tlb::{LANGUAGE_ID, TlbLanguage};
use ton_language_server_core::{
    DocumentUri, LanguageService, LanguageServiceConfig, Position, ProfileSummary,
    default_language_service,
};

fn case_tlb_definition(uri: &str, source: &str, expect: Expect) {
    let marked = MarkedSource::parse(source);
    let caret = marked.marker("caret");
    let uri = DocumentUri::from(uri);
    let mut service = default_language_service();
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source().to_owned())
        .expect("TL-B document should open");
    let actual = service
        .definition(&uri, caret.position)
        .map(|locations| render_definition(caret.position, &locations))
        .expect("definition request should succeed");
    expect.assert_eq(&actual);
}

#[test]
fn resolves_multiple_declarations_for_same_type() {
    case_tlb_definition(
        "file:///fixture/main.tlb",
        r"
            foo$0 a:# = CommonMsgInfo;
            bar$1 b:# = CommonMsgInfo;
            baz$2 x:<caret>CommonMsgInfo = Wrap;
        ",
        expect![[r"
            2:8 -> 0:12 resolved
            2:8 -> 1:12 resolved"]],
    );
}

#[test]
fn resolves_with_virtual_uri() {
    case_tlb_definition(
        "acton://fixture/main.tlb",
        r"
            foo$0 a:# = CommonMsgInfo;
            baz$2 x:<caret>CommonMsgInfo = Wrap;
        ",
        expect![[r"
            1:8 -> 0:12 resolved"]],
    );
}

#[test]
fn unresolved_reference_is_rendered() {
    case_tlb_definition(
        "file:///fixture/main.tlb",
        r"
            baz$2 x:<caret>CommonMsgInfo = Wrap;
        ",
        expect![[r"
            0:8 unresolved"]],
    );
}

#[test]
fn resolves_local_field_reference() {
    case_tlb_definition(
        "file:///fixture/main.tlb",
        "foo$0 a:# b:<caret>a c:a = Bar;",
        expect![[r"
            0:12 -> 0:6 resolved"]],
    );
}

#[test]
fn resolves_declaration_name_to_itself() {
    case_tlb_definition(
        "file:///fixture/main.tlb",
        r"
            foo$0 a:# = <caret>CommonMsgInfo;
        ",
        expect![[r"
            0:12 -> 0:12 resolved"]],
    );
}

#[test]
fn marked_source_handles_utf16_positions() {
    let marked = MarkedSource::parse("x$0 field:𝒯<caret> = T;");
    assert_eq!(marked.marker("caret").position, Position::new(0, 12));
}

#[test]
fn definition_uses_cached_parse_and_records_language_spans() {
    let marked = MarkedSource::parse(
        r"
            foo$0 a:# = CommonMsgInfo;
            baz$2 x:<caret>CommonMsgInfo = Wrap;
        ",
    );
    let caret = marked.marker("caret");
    let uri = DocumentUri::from("acton://fixture/profiled.tlb");
    let mut service = LanguageService::new(LanguageServiceConfig {
        enable_profiling: true,
    });
    service.register_language(TlbLanguage::new());

    service
        .open_document(
            uri.clone(),
            LANGUAGE_ID.to_owned(),
            1,
            marked.source().to_owned(),
        )
        .expect("TL-B document should open");
    assert_eq!(event_count(service.profiler().summary(), "tlb.parse"), 1);
    assert_eq!(event_count(service.profiler().summary(), "tlb.index"), 1);

    let locations = service
        .definition(&uri, caret.position)
        .expect("definition request should succeed");

    assert_eq!(locations.len(), 1);
    assert_eq!(event_count(service.profiler().summary(), "tlb.parse"), 1);
    assert_eq!(event_count(service.profiler().summary(), "tlb.index"), 1);
    assert_eq!(event_count(service.profiler().summary(), "definition"), 1);
    assert_eq!(
        event_count(service.profiler().summary(), "tlb.definition.resolve"),
        1
    );
}

fn event_count(summary: &ProfileSummary, name: &'static str) -> usize {
    summary
        .events
        .iter()
        .filter(|event| event.name == name)
        .count()
}
