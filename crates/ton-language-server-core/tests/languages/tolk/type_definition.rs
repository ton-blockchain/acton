#[path = "../../support/mod.rs"]
mod support;
#[path = "type_definition/upstream.rs"]
mod upstream;

use expect_test::{Expect, expect};
use support::{MarkedSource, render_definition};
use ton_language_server_core::languages::tolk::{LANGUAGE_ID, TolkLanguage};
use ton_language_server_core::{
    DocumentUri, LanguageService, LanguageServiceConfig, ProfileSummary,
};

fn case_type_definition(source: &str, expect: Expect) {
    let marked = MarkedSource::parse(source);
    let uri = DocumentUri::from("file:///fixture/main.tolk");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TolkLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source().to_owned())
        .expect("Tolk document should open");
    let carets = marked
        .markers()
        .iter()
        .filter(|marker| marker.name == "caret" || marker.name.starts_with("caret:"));
    let actual = carets
        .map(|marker| {
            let locations = service
                .type_definition(&uri, marker.position)
                .expect("type definition request should succeed");

            render_definition(marker.position, &locations)
        })
        .collect::<Vec<_>>()
        .join("\n");

    expect.assert_eq(&actual);
}

#[test]
fn resolves_named_types_for_locals_and_type_references() {
    // A local with a declared struct type points to that struct.
    case_type_definition(
        "
            struct Foo {}

            fun test() {
                val <caret>a: Foo = {};
            }
        ",
        expect!["3:8 -> 0:7 resolved"],
    );

    // A direct type reference points to the same declaration.
    case_type_definition(
        "
            struct Foo {}

            fun test(): <caret>Foo {}
        ",
        expect!["2:12 -> 0:7 resolved"],
    );

    // Inferred struct types work without an explicit local annotation.
    case_type_definition(
        "
            struct Foo {}
            fun makeFoo(): Foo { return {}; }
            fun test() {
                val <caret>a = makeFoo();
            }
        ",
        expect!["3:8 -> 0:7 resolved"],
    );

    // A generic struct instantiated with a type parameter keeps its struct anchor.
    case_type_definition(
        "
            struct Box<T> {}
            fun test<T>() {
                val <caret>box: Box<T> = {};
            }
        ",
        expect!["2:8 -> 0:7 resolved"],
    );
}

#[test]
fn leaves_non_named_types_unresolved() {
    // Builtin integer types do not have user declaration locations.
    case_type_definition(
        "
            fun test() {
                val <caret>a = 100;
            }
        ",
        expect!["1:8 unresolved"],
    );

    // A builtin-typed struct field also has no named type destination.
    case_type_definition(
        "
            struct Foo { value: int }
            fun test() {
                val a: Foo = { <caret>value: 10 };
            }
        ",
        expect!["2:19 unresolved"],
    );

    // Missing initializers leave a local type unknown.
    case_type_definition(
        "
            fun test() {
                val <caret>a;
            }
        ",
        expect!["1:8 unresolved"],
    );

    // Composite tensor types are not represented by one declaration.
    case_type_definition(
        "
            fun test() {
                val <caret>a = (1, 2, 3);
            }
        ",
        expect!["1:8 unresolved"],
    );

    // Keywords are not type-bearing syntax nodes.
    case_type_definition("<caret>fun test() {}", expect!["0:0 unresolved"]);
}

#[test]
fn resolves_alias_and_enum_types() {
    // A local preserves its named alias as the type-definition target.
    case_type_definition(
        "
            type UserId = int;
            fun test(id: UserId) {
                val <caret>copy: UserId = id;
            }
        ",
        expect!["2:8 -> 0:5 resolved"],
    );

    // Enum-typed expressions point to the enum declaration.
    case_type_definition(
        "
            enum Color { Red }
            fun test() {
                val <caret>color = Color.Red;
            }
        ",
        expect!["2:8 -> 0:5 resolved"],
    );
}

#[test]
fn records_type_definition_profile_spans() {
    let marked = MarkedSource::parse(
        "
            struct Foo {}
            fun test() { val <caret>foo: Foo = {}; }
        ",
    );
    let uri = DocumentUri::from("file:///fixture/profiled.tolk");
    let mut service = LanguageService::new(LanguageServiceConfig {
        enable_profiling: true,
    });
    service.register_language(TolkLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source().to_owned())
        .expect("Tolk document should open");
    let locations = service
        .type_definition(&uri, marked.marker("caret").position)
        .expect("type definition should succeed");
    let summary = service.profiler().summary();
    let actual = format!(
        "locations={} type_definition={} tolk.type_definition={}",
        locations.len(),
        event_count(summary, "type_definition"),
        event_count(summary, "tolk.type_definition"),
    );

    expect!["locations=1 type_definition=1 tolk.type_definition=1"].assert_eq(&actual);
}

fn event_count(summary: &ProfileSummary, name: &'static str) -> usize {
    summary
        .events
        .iter()
        .filter(|event| event.name == name)
        .count()
}
