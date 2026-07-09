use ton_language_server_core::languages::tlb::{LANGUAGE_ID, TlbLanguage};
use ton_language_server_core::{
    DocumentUri, LanguageService, LanguageServiceConfig, Position, ProfileSummary, Profiler, Range,
    TextEdit, default_language_service,
};

#[test]
fn disabled_profiler_does_not_record_events_or_counters() {
    let mut profiler = Profiler::disabled();
    let started_at = profiler.start();

    profiler.finish("test.event", started_at);
    profiler.increment("test.counter");

    assert!(!profiler.is_enabled());
    assert!(profiler.summary().events.is_empty());
    assert!(profiler.summary().counters.is_empty());
}

#[test]
fn profiling_is_disabled_by_default_for_language_service() -> anyhow::Result<()> {
    let uri = DocumentUri::from("acton://fixture/profiling-disabled.tlb");
    let mut service = default_language_service();

    service.open_document(
        uri.clone(),
        LANGUAGE_ID,
        1,
        "foo$0 a:# = CommonMsgInfo;\nbar$1 x:CommonMsgInfo = Wrap;\n",
    )?;
    let locations = service.definition(&uri, Position::new(1, 8))?;

    assert_eq!(locations.len(), 1);
    assert!(!service.profiler().is_enabled());
    assert!(service.profiler().summary().events.is_empty());
    assert!(service.profiler().summary().counters.is_empty());

    Ok(())
}

#[test]
fn records_document_lifecycle_counters() -> anyhow::Result<()> {
    let uri = DocumentUri::from("acton://fixture/profiling-counters.tlb");
    let mut service = profiling_service();

    service.open_document(
        uri.clone(),
        LANGUAGE_ID,
        1,
        "foo$0 a:# = Old;\nbar$1 x:Old = Wrap;\n",
    )?;
    service.change_document(&uri, 2, "foo$0 a:# = New;\nbar$1 x:New = Wrap;\n")?;
    service.edit_document(&uri, 3, [TextEdit::new(range(1, 8, 1, 11), "Box")])?;

    let summary = service.profiler().summary();
    assert_eq!(counter(summary, "document.open"), 1);
    assert_eq!(counter(summary, "document.change"), 1);
    assert_eq!(counter(summary, "document.edit"), 1);
    assert_eq!(event_count(summary, "tlb.parse"), 3);
    assert_eq!(event_count(summary, "tlb.index"), 3);

    Ok(())
}

#[test]
fn records_incremental_edit_and_hot_definition_spans() -> anyhow::Result<()> {
    let uri = DocumentUri::from("acton://fixture/profiling-edit.tlb");
    let mut service = profiling_service();

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
            TextEdit::new(range(0, 12, 0, 15), "New"),
            TextEdit::new(range(1, 8, 1, 11), "New"),
        ],
    )?;
    let locations = service.definition(&uri, Position::new(1, 8))?;

    let summary = service.profiler().summary();
    assert_eq!(locations.len(), 1);
    assert_eq!(counter(summary, "document.open"), 1);
    assert_eq!(counter(summary, "document.edit"), 1);
    assert_eq!(event_count(summary, "tlb.parse"), 2);
    assert_eq!(event_count(summary, "tlb.index"), 2);
    assert_eq!(event_count(summary, "definition"), 1);
    assert_eq!(event_count(summary, "tlb.definition.resolve"), 1);

    Ok(())
}

fn profiling_service() -> LanguageService {
    let mut service = LanguageService::new(LanguageServiceConfig {
        enable_profiling: true,
    });
    service.register_language(TlbLanguage::new());
    service
}

fn event_count(summary: &ProfileSummary, name: &'static str) -> usize {
    summary
        .events
        .iter()
        .filter(|event| event.name == name)
        .count()
}

fn counter(summary: &ProfileSummary, name: &'static str) -> u64 {
    summary.counters.get(name).copied().unwrap_or_default()
}

const fn range(start_line: u32, start_character: u32, end_line: u32, end_character: u32) -> Range {
    Range::new(
        Position::new(start_line, start_character),
        Position::new(end_line, end_character),
    )
}
