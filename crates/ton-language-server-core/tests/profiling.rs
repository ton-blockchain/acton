use ton_language_server_core::languages::tlb::{LANGUAGE_ID as TLB_LANGUAGE_ID, TlbLanguage};
use ton_language_server_core::languages::tolk::{LANGUAGE_ID as TOLK_LANGUAGE_ID, TolkLanguage};
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
        TLB_LANGUAGE_ID,
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
        TLB_LANGUAGE_ID,
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
        TLB_LANGUAGE_ID,
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

#[test]
fn records_tolk_resolve_and_type_inference_spans() -> anyhow::Result<()> {
    let uri = DocumentUri::from("file:///fixture/profiling.tolk");
    let mut service = tolk_profiling_service();
    let source = "struct Storage {\n    counter: int\n}\nfun Storage.save(self) {}\nfun main() {\n    var storage = Storage { counter: 1 };\n    storage.save();\n    storage.counter;\n}\n";

    service.open_document(uri.clone(), TOLK_LANGUAGE_ID, 1, source)?;
    {
        let summary = service.profiler().summary();
        assert_eq!(counter(summary, "document.open"), 1);
        assert_eq!(event_count(summary, "tolk.parse"), 1);
        assert_eq!(event_count(summary, "tolk.snapshot.rebuild"), 1);
        assert_eq!(event_count(summary, "tolk.snapshot.update_files"), 1);
        assert_eq!(event_count(summary, "tolk.snapshot.index"), 1);
        assert_eq!(event_count(summary, "tolk.resolve"), 1);
        assert_eq!(event_count(summary, "tolk.snapshot.materialize"), 1);
        assert_eq!(event_count(summary, "tolk.type_inference"), 1);
    }

    let locations = service.definition(&uri, Position::new(6, 12))?;
    assert_eq!(locations.len(), 1);
    assert_eq!(
        event_count(service.profiler().summary(), "tolk.type_inference"),
        1
    );

    let locations = service.definition(&uri, Position::new(7, 12))?;
    assert_eq!(locations.len(), 1);
    assert_eq!(
        event_count(service.profiler().summary(), "tolk.type_inference"),
        1
    );

    let hints = service.inlay_hints(&uri, range(0, 0, u32::MAX, u32::MAX))?;
    assert!(!hints.is_empty());
    assert_eq!(
        event_count(service.profiler().summary(), "tolk.type_inference"),
        1
    );

    service.change_document(&uri, 2, source.replace("counter: 1", "counter: 2"))?;
    assert_eq!(
        event_count(service.profiler().summary(), "tolk.type_inference"),
        2
    );

    let locations = service.definition(&uri, Position::new(6, 12))?;
    assert_eq!(locations.len(), 1);

    let summary = service.profiler().summary();
    assert_eq!(counter(summary, "document.open"), 1);
    assert_eq!(counter(summary, "document.change"), 1);
    assert_eq!(event_count(summary, "tolk.parse"), 2);
    assert_eq!(event_count(summary, "tolk.snapshot.rebuild"), 2);
    assert_eq!(event_count(summary, "tolk.snapshot.update_files"), 2);
    assert_eq!(event_count(summary, "tolk.snapshot.index"), 2);
    assert_eq!(event_count(summary, "tolk.resolve"), 2);
    assert_eq!(event_count(summary, "tolk.type_inference"), 2);
    assert_eq!(event_count(summary, "tolk.snapshot.materialize"), 2);
    assert_eq!(event_count(summary, "definition"), 3);
    assert_eq!(event_count(summary, "tolk.definition.resolve"), 3);
    assert_eq!(event_count(summary, "inlay_hints"), 1);
    assert_eq!(event_count(summary, "tolk.inlay_hints"), 1);

    Ok(())
}

#[test]
fn records_incremental_tolk_type_inference_by_import_dependents() -> anyhow::Result<()> {
    let lib_uri = DocumentUri::from("file:///fixture/lib.tolk");
    let main_uri = DocumentUri::from("file:///fixture/main.tolk");
    let mut service = tolk_profiling_service();

    service.open_document(
        lib_uri.clone(),
        TOLK_LANGUAGE_ID,
        1,
        "fun helper(): int { return 1; }\n",
    )?;
    service.open_document(
        main_uri,
        TOLK_LANGUAGE_ID,
        1,
        "import \"lib\"\nfun main(): int { return helper(); }\n",
    )?;
    let signature_files_after_open =
        counter(service.profiler().summary(), "tolk.type_signature.file");
    assert_eq!(
        counter(service.profiler().summary(), "tolk.type_inference.file"),
        2
    );

    service.change_document(
        &DocumentUri::from("file:///fixture/main.tolk"),
        2,
        "import \"lib\"\nfun main(): int { return helper() + 1; }\n",
    )?;
    assert_eq!(
        counter(service.profiler().summary(), "tolk.type_signature.file"),
        signature_files_after_open + 1
    );
    assert_eq!(
        counter(service.profiler().summary(), "tolk.type_inference.file"),
        3
    );

    service.change_document(&lib_uri, 2, "fun helper(): int { return 2; }\n")?;
    assert_eq!(
        counter(service.profiler().summary(), "tolk.type_signature.file"),
        signature_files_after_open + 3
    );
    assert_eq!(
        counter(service.profiler().summary(), "tolk.type_inference.file"),
        5
    );

    Ok(())
}

fn profiling_service() -> LanguageService {
    let mut service = LanguageService::new(LanguageServiceConfig {
        enable_profiling: true,
    });
    service.register_language(TlbLanguage::new());
    service
}

fn tolk_profiling_service() -> LanguageService {
    let mut service = LanguageService::new(LanguageServiceConfig {
        enable_profiling: true,
    });
    service.register_language(TolkLanguage::new());
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
