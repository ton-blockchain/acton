use expect_test::expect;
use ton_language_server_core::languages::tolk::{LANGUAGE_ID, TolkLanguage};
use ton_language_server_core::{
    CompletionTrigger, DocumentUri, LanguageService, LanguageServiceConfig, Position,
};

#[test]
fn completion_records_stable_profile_events() -> anyhow::Result<()> {
    let uri = DocumentUri::from("memory:///profile.tolk");
    let mut service = LanguageService::new(LanguageServiceConfig {
        enable_profiling: true,
    });
    service.register_language(TolkLanguage::new());
    service.open_document(
        uri.clone(),
        LANGUAGE_ID,
        1,
        "tolk 1.0\nfun main() { beginC }".to_owned(),
    )?;
    service.completion(&uri, Position::new(1, 19), CompletionTrigger::invoked())?;

    let names = service
        .profiler()
        .summary()
        .events
        .iter()
        .map(|event| event.name)
        .filter(|name| *name == "completion" || *name == "tolk.completion")
        .collect::<Vec<_>>()
        .join("\n");
    expect![[r#"tolk.completion
completion"#]]
    .assert_eq(&names);
    Ok(())
}
