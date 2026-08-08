use expect_test::{Expect, expect};
use ton_language_server_core::languages::fift::{FiftLanguage, LANGUAGE_ID};
use ton_language_server_core::{
    DocumentUri, FoldingRange, LanguageService, LanguageServiceConfig, ProfileSummary,
};

fn case_fift_folding(source: &str, expect: Expect) {
    let uri = DocumentUri::from("file:///fixture/main.fif");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(FiftLanguage::new());

    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, source.to_owned())
        .expect("Fift document should open");

    expect.assert_eq(&render_folding_ranges(
        service
            .folding_ranges(&uri)
            .expect("folding range request should succeed"),
    ));
}

fn render_folding_ranges(ranges: Vec<FoldingRange>) -> String {
    if ranges.is_empty() {
        return "<none>".to_owned();
    }

    ranges
        .into_iter()
        .map(|range| format!("[{}, {}]", range.start_line, range.end_line))
        .collect::<Vec<_>>()
        .join(", ")
}

#[test]
fn folds_nested_proc_blocks() {
    case_fift_folding(
        r"PROGRAM{
DECLPROC entry
entry PROC:<{
  IFJMP:<{
    1 PUSHINT
  }>
  REPEAT:<{
    2 PUSHINT
  }>
}>
END>c
",
        expect!["[0, 9], [2, 8], [3, 4], [6, 7]"],
    );
}

#[test]
fn folds_control_flow_and_instruction_block() {
    case_fift_folding(
        r"PROGRAM{
DECLPROC entry
entry PROC:<{
  IF:<{
    1 PUSHINT
  }>ELSE<{
    2 PUSHINT
  }>
  WHILE:<{
    3 PUSHINT
  }>DO<{
    4 PUSHINT
  }>
  UNTIL:<{
    5 PUSHINT
  }>
  <{
    6 PUSHINT
  }>
}>
END>c
",
        expect!["[0, 19], [2, 18], [3, 4], [5, 6], [8, 9], [10, 11], [13, 14], [16, 17]"],
    );
}

#[test]
fn folds_definition_variants() {
    case_fift_folding(
        r"PROGRAM{
foo PROCINLINE:<{
  1 PUSHINT
}>
bar PROCREF:<{
  2 PUSHINT
}>
baz METHOD:<{
  3 PUSHINT
}>
END>c
",
        expect!["[0, 9], [1, 2], [4, 5], [7, 8]"],
    );
}

#[test]
fn single_line_blocks_are_not_foldable() {
    case_fift_folding(
        "PROGRAM{ DECLPROC entry entry PROC:<{ 1 PUSHINT }> END>c",
        expect!["<none>"],
    );
}

#[test]
fn records_folding_profile_spans() {
    let uri = DocumentUri::from("file:///fixture/profiled.fif");
    let mut service = LanguageService::new(LanguageServiceConfig {
        enable_profiling: true,
    });
    service.register_language(FiftLanguage::new());

    service
        .open_document(
            uri.clone(),
            LANGUAGE_ID,
            1,
            "PROGRAM{\nDECLPROC entry\nentry PROC:<{\n  1 PUSHINT\n}>\nEND>c\n".to_owned(),
        )
        .expect("Fift document should open");
    let ranges = service
        .folding_ranges(&uri)
        .expect("folding range request should succeed");

    let summary = service.profiler().summary();
    let actual = format!(
        "folding_ranges={} fift.parse={} folding_ranges.span={} fift.folding_ranges={}",
        ranges.len(),
        event_count(summary, "fift.parse"),
        event_count(summary, "folding_ranges"),
        event_count(summary, "fift.folding_ranges"),
    );
    expect![[r"
        folding_ranges=2 fift.parse=1 folding_ranges.span=1 fift.folding_ranges=1"]]
    .assert_eq(&actual);
}

fn event_count(summary: &ProfileSummary, name: &'static str) -> usize {
    summary
        .events
        .iter()
        .filter(|event| event.name == name)
        .count()
}
