#![allow(clippy::needless_raw_string_hashes)]

use expect_test::{Expect, expect};
use ton_language_server_core::languages::tolk::{LANGUAGE_ID, TolkLanguage};
use ton_language_server_core::{
    DocumentUri, FoldingRange, LanguageService, LanguageServiceConfig, ProfileSummary,
};

fn case_tolk_folding(source: &str, expect: Expect) {
    let uri = DocumentUri::from("file:///fixture/main.tolk");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TolkLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, source.to_owned())
        .expect("Tolk document should open");

    let ranges = service
        .folding_ranges(&uri)
        .expect("folding range request should succeed");
    expect.assert_eq(&render_folding_ranges(ranges));
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
fn folds_function_and_nested_control_flow_blocks() {
    case_tolk_folding(
        r"fun test() {
    val num = 100;
    if (num == 10) {
        throw num;
    }
}",
        expect!["[0, 5], [2, 4]"],
    );

    case_tolk_folding(
        r"fun Foo.test() {
    val num = 100;
    if (num == 10) {
        throw num;
    }
}",
        expect!["[0, 5], [2, 4]"],
    );
}

#[test]
fn folds_object_struct_and_enum_bodies() {
    case_tolk_folding(
        r#"fun test() {
    Foo {
        foo: 10,
        bar: "",
    }
}"#,
        expect!["[0, 5], [1, 4]"],
    );

    case_tolk_folding(
        r"struct Foo {
    val: int,
    other: string,
}",
        expect!["[0, 3]"],
    );

    case_tolk_folding(
        r"enum Color {
    Red = 10,
    Blue = 200 + 100,
}",
        expect!["[0, 3]"],
    );
}

#[test]
fn folds_match_and_arm_bodies() {
    case_tolk_folding(
        r"fun test() {
    match (foo) {
        10 => return,
        20 => {
            return;
        }
    }
}",
        expect!["[0, 7], [1, 6], [3, 5]"],
    );
}

#[test]
fn single_line_bodies_are_not_foldable() {
    case_tolk_folding(
        "fun test() { if (true) { return; } } struct Foo { value: int }",
        expect!["<none>"],
    );
}

#[test]
fn records_folding_profile_spans() {
    let uri = DocumentUri::from("file:///fixture/profiled.tolk");
    let mut service = LanguageService::new(LanguageServiceConfig {
        enable_profiling: true,
    });
    service.register_language(TolkLanguage::new());
    service
        .open_document(
            uri.clone(),
            LANGUAGE_ID,
            1,
            "fun main() {\n    return;\n}\n".to_owned(),
        )
        .expect("Tolk document should open");

    let ranges = service
        .folding_ranges(&uri)
        .expect("folding range request should succeed");
    let summary = service.profiler().summary();
    let actual = format!(
        "ranges={} folding={} tolk.folding={}",
        ranges.len(),
        event_count(summary, "folding_ranges"),
        event_count(summary, "tolk.folding_ranges"),
    );
    expect!["ranges=1 folding=1 tolk.folding=1"].assert_eq(&actual);
}

fn event_count(summary: &ProfileSummary, name: &'static str) -> usize {
    summary
        .events
        .iter()
        .filter(|event| event.name == name)
        .count()
}
