use super::support::MarkedSource;
use expect_test::expect;
use ton_language_server_core::languages::tolk::{LANGUAGE_ID, TolkLanguage};
use ton_language_server_core::{
    DocumentUri, LanguageService, LanguageServiceConfig, ProfileSummary, Range,
};

#[test]
fn records_code_action_profile_spans() {
    let marked = MarkedSource::parse("struct Foo { value: int } fun main() { Foo {<caret>} }");
    let uri = DocumentUri::from("file:///fixture/profiled.tolk");
    let mut service = LanguageService::new(LanguageServiceConfig {
        enable_profiling: true,
    });
    service.register_language(TolkLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source().to_owned())
        .expect("Tolk document should open");
    let position = marked.marker("caret").position;
    let actions = service
        .code_actions(&uri, Range::new(position, position))
        .expect("code actions should succeed");
    let summary = service.profiler().summary();
    let actual = format!(
        "actions={} code_actions={} tolk.code_actions={}",
        actions.len(),
        event_count(summary, "code_actions"),
        event_count(summary, "tolk.code_actions"),
    );

    expect!["actions=2 code_actions=1 tolk.code_actions=1"].assert_eq(&actual);
}

fn event_count(summary: &ProfileSummary, name: &'static str) -> usize {
    summary
        .events
        .iter()
        .filter(|event| event.name == name)
        .count()
}
