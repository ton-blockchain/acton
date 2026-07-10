#![allow(clippy::needless_raw_string_hashes)]

#[path = "../../support/mod.rs"]
mod support;

use expect_test::{Expect, expect};
use support::MarkedSource;
use ton_language_server_core::languages::tolk::{LANGUAGE_ID, TolkLanguage};
use ton_language_server_core::{
    DocumentHighlight, DocumentHighlightKind, DocumentUri, LanguageService, LanguageServiceConfig,
    ProfileSummary, TextIndex,
};

fn case_highlights(source: &str, expect: Expect) {
    let marked = MarkedSource::parse(source);
    let uri = DocumentUri::from("file:///fixture/main.tolk");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TolkLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source().to_owned())
        .expect("Tolk document should open");
    let highlights = service
        .document_highlights(&uri, marked.marker("caret").position)
        .expect("document highlight request should succeed");

    expect.assert_eq(&render_highlights(marked.source(), &highlights));
}

fn render_highlights(source: &str, highlights: &[DocumentHighlight]) -> String {
    if highlights.is_empty() {
        return "<none>".to_owned();
    }
    let index = TextIndex::new(source);
    highlights
        .iter()
        .map(|highlight| {
            let start = index.position_to_offset(source, highlight.range.start);
            let end = index.position_to_offset(source, highlight.range.end);
            let text = source.get(start..end).unwrap_or_default();
            let kind = match highlight.kind {
                Some(DocumentHighlightKind::Text) => "text",
                Some(DocumentHighlightKind::Read) => "read",
                Some(DocumentHighlightKind::Write) => "write",
                None => "none",
            };
            format!(
                "{}:{} {kind:<5} {text}",
                highlight.range.start.line, highlight.range.start.character
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn highlights_local_reads_and_assignment_writes() {
    case_highlights(
        "
            fun main() {
                var <caret>value = 0;
                value = 1;
                value += 2;
                throw value;
            }
        ",
        expect![[r"
            1:8 read  value
            2:4 write value
            3:4 write value
            4:10 read  value"]],
    );
}

#[test]
fn treats_mutating_method_calls_as_writes() {
    // A mutable receiver is written by an instance method call.
    case_highlights(
        "
            struct Foo {}
            fun Foo.touch(mutate self) {}
            fun main(<caret>foo: Foo) {
                foo.touch();
                foo;
            }
        ",
        expect![[r"
            2:9 read  foo
            3:4 write foo
            4:4 read  foo"]],
    );

    // The mutable method symbol is also marked as a write at its call site.
    case_highlights(
        "
            struct Foo {}
            fun Foo.<caret>touch(mutate self) {}
            fun main(foo: Foo) { foo.touch(); }
        ",
        expect![[r"
            1:8 read  touch
            2:25 write touch"]],
    );

    // A non-mutating method call remains a read.
    case_highlights(
        "
            struct Foo {}
            fun Foo.inspect(self) {}
            fun main(<caret>foo: Foo) { foo.inspect(); }
        ",
        expect![[r"
            2:9 read  foo
            2:21 read  foo"]],
    );
}

#[test]
fn ignores_unresolved_syntax() {
    case_highlights("fun main() { <caret>throw 1; }", expect!["<none>"]);
}

#[test]
fn records_document_highlight_profile_spans() {
    let marked = MarkedSource::parse("fun main(value: int) { throw <caret>value; }");
    let uri = DocumentUri::from("file:///fixture/profiled.tolk");
    let mut service = LanguageService::new(LanguageServiceConfig {
        enable_profiling: true,
    });
    service.register_language(TolkLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source().to_owned())
        .expect("Tolk document should open");
    let highlights = service
        .document_highlights(&uri, marked.marker("caret").position)
        .expect("document highlights should succeed");
    let summary = service.profiler().summary();
    let actual = format!(
        "highlights={} document_highlights={} tolk.document_highlights={}",
        highlights.len(),
        event_count(summary, "document_highlights"),
        event_count(summary, "tolk.document_highlights"),
    );

    expect!["highlights=2 document_highlights=1 tolk.document_highlights=1"].assert_eq(&actual);
}

fn event_count(summary: &ProfileSummary, name: &'static str) -> usize {
    summary
        .events
        .iter()
        .filter(|event| event.name == name)
        .count()
}
