#![allow(clippy::needless_raw_string_hashes)]

#[path = "../../support/mod.rs"]
mod support;

use expect_test::{Expect, expect};
use support::MarkedSource;
use ton_language_server_core::languages::tolk::{LANGUAGE_ID, TolkLanguage};
use ton_language_server_core::{
    DocumentUri, LanguageService, LanguageServiceConfig, ProfileSummary, SignatureHelp,
};

fn case_signature_help(source: &str, expect: Expect) {
    let marked = MarkedSource::parse(source);
    let uri = DocumentUri::from("file:///fixture/main.tolk");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TolkLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source().to_owned())
        .expect("Tolk document should open");
    let signature_help = service
        .signature_help(&uri, marked.marker("caret").position)
        .expect("signature help request should succeed");

    expect.assert_eq(&render_signature_help(signature_help));
}

fn render_signature_help(signature_help: Option<SignatureHelp>) -> String {
    let Some(signature_help) = signature_help else {
        return "<none>".to_owned();
    };
    let Some(signature) = signature_help.signatures.first() else {
        return "<none>".to_owned();
    };
    let active_parameter = signature
        .active_parameter
        .and_then(|index| signature.parameters.get(index as usize));

    active_parameter.map_or_else(
        || signature.label.clone(),
        |parameter| format!("{parameter}\n{}", signature.label),
    )
}

#[test]
fn shows_functions_with_and_without_parameters() {
    // A zero-parameter call still shows the declaration signature.
    case_signature_help(
        "
            fun foo() {}
            fun test() {
                foo(<caret>);
            }
        ",
        expect!["fun foo()"],
    );

    // The first parameter is active before the first argument.
    case_signature_help(
        "
            fun foo(a: int) {}
            fun test() {
                foo(<caret>);
            }
        ",
        expect![[r"
            a: int
            fun foo(a: int)"]],
    );

    // A comma advances the active parameter.
    case_signature_help(
        "
            fun foo(a: int, b: int) {}
            fun test() {
                foo(10, <caret>);
            }
        ",
        expect![[r"
            b: int
            fun foo(a: int, b: int)"]],
    );

    // A cursor inside the first argument keeps the first parameter active.
    case_signature_help(
        "
            fun foo(a: int, b: int) {}
            fun test() {
                foo(<caret>0, 10);
            }
        ",
        expect![[r"
            a: int
            fun foo(a: int, b: int)"]],
    );
}

#[test]
fn shows_static_and_instance_method_signatures() {
    // Static method parameters are displayed unchanged.
    case_signature_help(
        "
            struct Foo {}
            fun Foo.foo(a: int, b: int) {}
            fun test() {
                Foo.foo(<caret>0, 10);
            }
        ",
        expect![[r"
            a: int
            fun Foo.foo(a: int, b: int)"]],
    );

    // The implicit self parameter is omitted from an instance method.
    case_signature_help(
        "
            struct Foo {}
            fun Foo.bar(self) {}
            fun test() {
                val foo: Foo = {};
                foo.bar(<caret>);
            }
        ",
        expect!["fun Foo.bar()"],
    );

    // Explicit instance-method parameters keep their declaration order after self.
    case_signature_help(
        "
            struct Foo {}
            fun Foo.bar(self, a: int, b: int) {}
            fun test() {
                val foo: Foo = {};
                foo.bar(<caret>0, 10);
            }
        ",
        expect![[r"
            a: int
            fun Foo.bar(a: int, b: int)"]],
    );
}

#[test]
fn selects_the_innermost_nested_call() {
    case_signature_help(
        "
            fun foo(a: int, b: int) {}
            fun getValue(seed: int): int {}
            fun test() {
                foo(10, getValue(<caret>));
            }
        ",
        expect![[r"
            seed: int
            fun getValue(seed: int)"]],
    );
}

#[test]
fn tracks_parameters_across_multiline_calls() {
    // First argument on the line after the opening parenthesis.
    case_signature_help(
        "
            fun foo(a: int, b: int, c: int) {}
            fun test() {
                foo(
                    <caret>10,
                    20,
                    30
                );
            }
        ",
        expect![[r"
            a: int
            fun foo(a: int, b: int, c: int)"]],
    );

    // Second argument on its own line.
    case_signature_help(
        "
            fun foo(a: int, b: int, c: int) {}
            fun test() {
                foo(
                    10,
                    <caret>20,
                    30
                );
            }
        ",
        expect![[r"
            b: int
            fun foo(a: int, b: int, c: int)"]],
    );

    // Third argument on its own line.
    case_signature_help(
        "
            fun foo(a: int, b: int, c: int) {}
            fun test() {
                foo(
                    10,
                    20,
                    <caret>30
                );
            }
        ",
        expect![[r"
            c: int
            fun foo(a: int, b: int, c: int)"]],
    );

    // A cursor immediately after a comma selects the following parameter.
    case_signature_help(
        "
            fun foo(a: int, b: int, c: int) {}
            fun test() {
                foo(
                    10,
                    20,<caret>
                    30
                );
            }
        ",
        expect![[r"
            c: int
            fun foo(a: int, b: int, c: int)"]],
    );

    // A cursor after the opening parenthesis selects the first parameter.
    case_signature_help(
        "
            fun foo(a: int, b: int, c: int) {}
            fun test() {
                foo(<caret>
                    10,
                    20,
                    30
                );
            }
        ",
        expect![[r"
            a: int
            fun foo(a: int, b: int, c: int)"]],
    );
}

#[test]
fn ignores_non_call_and_unresolved_targets() {
    // A resolved call outside its argument list is not a signature-help target.
    case_signature_help(
        "
            fun foo(a: int) {}
            fun test() {
                <caret>foo(10);
            }
        ",
        expect!["<none>"],
    );

    // Unknown callees cannot provide a declaration signature.
    case_signature_help(
        "
            fun test() {
                unknown(<caret>);
            }
        ",
        expect!["<none>"],
    );

    // Callable locals have no source declaration signature to display.
    case_signature_help(
        "
            fun test(callback: (int) -> int) {
                callback(<caret>1);
            }
        ",
        expect!["<none>"],
    );
}

#[test]
fn records_signature_help_profile_spans() {
    let marked = MarkedSource::parse(
        "
            fun foo(a: int) {}
            fun main() { foo(<caret>); }
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
    let signature_help = service
        .signature_help(&uri, marked.marker("caret").position)
        .expect("signature help request should succeed");
    let summary = service.profiler().summary();
    let actual = format!(
        "result={} signature_help={} tolk.signature_help={}",
        signature_help.is_some(),
        event_count(summary, "signature_help"),
        event_count(summary, "tolk.signature_help"),
    );

    expect!["result=true signature_help=1 tolk.signature_help=1"].assert_eq(&actual);
}

fn event_count(summary: &ProfileSummary, name: &'static str) -> usize {
    summary
        .events
        .iter()
        .filter(|event| event.name == name)
        .count()
}
