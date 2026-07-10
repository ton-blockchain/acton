#[path = "../../support/mod.rs"]
mod support;

use expect_test::{Expect, expect};
use std::fmt::Write as _;
use support::MarkedSource;
use ton_language_server_core::languages::tolk::{LANGUAGE_ID, TolkLanguage};
use ton_language_server_core::{
    DocumentUri, LanguageService, LanguageServiceConfig, Location, Position, ProfileSummary,
};

fn case_tolk_references(
    uri: &str,
    source: &str,
    include_declaration: bool,
    configure: impl FnOnce(&mut LanguageService),
    expect: Expect,
) {
    let marked = MarkedSource::parse(source);
    let caret = marked.marker("caret");
    let uri = DocumentUri::from(uri);
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TolkLanguage::new());
    configure(&mut service);
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source().to_owned())
        .expect("Tolk document should open");

    let locations = service
        .references(&uri, caret.position, include_declaration)
        .expect("references request should succeed");
    expect.assert_eq(&render_references(caret.position, &locations));
}

#[test]
fn finds_global_references_across_files() {
    case_tolk_references(
        "file:///workspace/main.tolk",
        r#"
            import "lib"
            import "other"
            fun main(): int { return <caret>helper(); }
        "#,
        false,
        |service| {
            service
                .add_source_file(
                    LANGUAGE_ID,
                    "file:///workspace/lib.tolk",
                    "fun helper(): int { return 1; }\nfun other(): int { return helper(); }\n",
                )
                .expect("provider file should be added");
            service
                .add_source_file(
                    LANGUAGE_ID,
                    "file:///workspace/other.tolk",
                    "import \"lib\"\nfun use(): int { return helper(); }\n",
                )
                .expect("provider file should be added");
        },
        expect![[r"
            2:25 -> file:///workspace/lib.tolk 1:26 reference
            2:25 -> file:///workspace/main.tolk 2:25 reference
            2:25 -> file:///workspace/other.tolk 1:24 reference"]],
    );
}

#[test]
fn include_declaration_adds_global_definition() {
    case_tolk_references(
        "file:///workspace/main.tolk",
        r"
            fun helper(): int { return 1; }
            fun main(): int { return <caret>helper(); }
        ",
        true,
        |_| {},
        expect![[r"
            1:25 -> file:///workspace/main.tolk 0:4 reference
            1:25 -> file:///workspace/main.tolk 1:25 reference"]],
    );
}

#[test]
fn finds_local_references_from_declaration() {
    case_tolk_references(
        "file:///workspace/main.tolk",
        r"
            fun main(): int {
                var <caret>value = 1;
                return value + value;
            }
        ",
        false,
        |_| {},
        expect![[r"
            1:8 -> file:///workspace/main.tolk 2:11 reference
            1:8 -> file:///workspace/main.tolk 2:19 reference"]],
    );
}

#[test]
fn include_declaration_adds_local_definition() {
    case_tolk_references(
        "file:///workspace/main.tolk",
        r"
            fun main(): int {
                var value = 1;
                return <caret>value;
            }
        ",
        true,
        |_| {},
        expect![[r"
            2:11 -> file:///workspace/main.tolk 1:8 reference
            2:11 -> file:///workspace/main.tolk 2:11 reference"]],
    );
}

#[test]
fn finds_field_references_with_type_inference() {
    case_tolk_references(
        "file:///workspace/main.tolk",
        r"
            struct Storage {
                counter: int
            }
            fun main() {
                var storage = Storage { counter: 1 };
                storage.<caret>counter;
                storage.counter = 2;
            }
        ",
        true,
        |_| {},
        expect![[r"
            5:12 -> file:///workspace/main.tolk 1:4 reference
            5:12 -> file:///workspace/main.tolk 4:28 reference
            5:12 -> file:///workspace/main.tolk 5:12 reference
            5:12 -> file:///workspace/main.tolk 6:12 reference"]],
    );
}

#[test]
fn unresolved_symbol_has_no_references() {
    case_tolk_references(
        "file:///workspace/main.tolk",
        r"
            fun main(): int { return <caret>missing(); }
        ",
        true,
        |_| {},
        expect![[r"
            0:25 unresolved"]],
    );
}

#[test]
fn finds_destructured_backticked_redefined_and_shorthand_locals() {
    case_tolk_references(
        "file:///workspace/main.tolk",
        r"
            fun main() {
                val <caret>`hello world` = 1;
                `hello world` + `hello world`;
            }
        ",
        false,
        |_| {},
        expect![[r"
            1:8 -> file:///workspace/main.tolk 2:4 reference
            1:8 -> file:///workspace/main.tolk 2:20 reference"]],
    );

    case_tolk_references(
        "file:///workspace/main.tolk",
        r"
            fun main() {
                val [<caret>first, second] = [1, 2];
                first + second;
            }
        ",
        false,
        |_| {},
        expect![[r"
            1:9 -> file:///workspace/main.tolk 2:4 reference"]],
    );

    case_tolk_references(
        "file:///workspace/main.tolk",
        r"
            fun main() {
                val <caret>value = 1;
                val value redef = 2;
                value;
            }
        ",
        false,
        |_| {},
        expect![[r"
            1:8 -> file:///workspace/main.tolk 2:8 reference
            1:8 -> file:///workspace/main.tolk 3:4 reference"]],
    );

    case_tolk_references(
        "file:///workspace/main.tolk",
        r"
            struct Foo { value: int }
            fun main(<caret>value: int) {
                Foo { value };
            }
        ",
        false,
        |_| {},
        expect![[r"
            1:9 -> file:///workspace/main.tolk 2:10 reference"]],
    );
}

#[test]
fn keeps_shadowed_local_and_lambda_scopes_separate() {
    case_tolk_references(
        "file:///workspace/main.tolk",
        r"
            fun main() {
                {
                    val <caret>value = 1;
                    value;
                }
                {
                    val value = 2;
                    value;
                }
            }
        ",
        false,
        |_| {},
        expect![[r"
            2:12 -> file:///workspace/main.tolk 3:8 reference"]],
    );

    case_tolk_references(
        "file:///workspace/main.tolk",
        r"
            fun main() {
                fun (<caret>value: int) {
                    value;
                    fun (value: int) {
                        value;
                    };
                };
            }
        ",
        false,
        |_| {},
        expect![[r"
            1:9 -> file:///workspace/main.tolk 2:8 reference"]],
    );

    case_tolk_references(
        "file:///workspace/main.tolk",
        r"
            fun main() {
                try {} catch (<caret>error, data) {
                    error + data;
                }
            }
        ",
        false,
        |_| {},
        expect![[r"
            1:18 -> file:///workspace/main.tolk 2:8 reference"]],
    );
}

#[test]
fn finds_references_for_every_global_symbol_kind() {
    case_tolk_references(
        "file:///workspace/main.tolk",
        r"
            type <caret>Alias = int;
            struct Box { value: Alias }
            fun use(value: Alias): Alias { return value; }
        ",
        false,
        |_| {},
        expect![[r"
            0:5 -> file:///workspace/main.tolk 1:20 reference
            0:5 -> file:///workspace/main.tolk 2:15 reference
            0:5 -> file:///workspace/main.tolk 2:23 reference"]],
    );

    case_tolk_references(
        "file:///workspace/main.tolk",
        r"
            struct <caret>Box { value: int }
            fun Box.load(): Box { return Box { value: 1 }; }
        ",
        false,
        |_| {},
        expect![[r"
            0:7 -> file:///workspace/main.tolk 1:4 reference
            0:7 -> file:///workspace/main.tolk 1:16 reference
            0:7 -> file:///workspace/main.tolk 1:29 reference"]],
    );

    case_tolk_references(
        "file:///workspace/main.tolk",
        r"
            enum Color { <caret>Red, Blue }
            fun main() { Color.Red; match (Color.Blue) { Color.Red => {} } }
        ",
        false,
        |_| {},
        expect![[r"
            0:13 -> file:///workspace/main.tolk 1:19 reference
            0:13 -> file:///workspace/main.tolk 1:51 reference"]],
    );

    case_tolk_references(
        "file:///workspace/main.tolk",
        r"
            const <caret>ANSWER = 42;
            global counter: int;
            fun helper() { return ANSWER + counter; }
            get fun getter() { return helper(); }
        ",
        false,
        |_| {},
        expect![[r"
            0:6 -> file:///workspace/main.tolk 2:22 reference"]],
    );
}

#[test]
fn finds_function_method_and_type_parameter_references() {
    case_tolk_references(
        "file:///workspace/main.tolk",
        r"
            fun <caret>helper() {}
            fun main() { helper(); helper(); }
        ",
        false,
        |_| {},
        expect![[r"
            0:4 -> file:///workspace/main.tolk 1:13 reference
            0:4 -> file:///workspace/main.tolk 1:23 reference"]],
    );

    case_tolk_references(
        "file:///workspace/main.tolk",
        r"
            struct Box {}
            fun Box.<caret>touch(self) {}
            fun main(box: Box) { box.touch(); }
        ",
        false,
        |_| {},
        expect![[r"
            1:8 -> file:///workspace/main.tolk 2:25 reference"]],
    );

    case_tolk_references(
        "file:///workspace/main.tolk",
        r"
            struct Box<<caret>T> {
                value: T
            }
            fun unwrap(value: Box<T>): T { return value.value; }
        ",
        false,
        |_| {},
        expect![[r"
            0:11 -> file:///workspace/main.tolk 1:11 reference"]],
    );
}

#[test]
fn records_reference_profile_spans() {
    let marked = MarkedSource::parse("fun <caret>main() {}\n");
    let uri = DocumentUri::from("file:///workspace/profiled.tolk");
    let mut service = LanguageService::new(LanguageServiceConfig {
        enable_profiling: true,
    });
    service.register_language(TolkLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source().to_owned())
        .expect("Tolk document should open");

    let references = service
        .references(&uri, marked.marker("caret").position, false)
        .expect("references request should succeed");
    let summary = service.profiler().summary();
    let actual = format!(
        "references={} references.span={} tolk.references={}",
        references.len(),
        event_count(summary, "references"),
        event_count(summary, "tolk.references.resolve"),
    );
    expect!["references=0 references.span=1 tolk.references=1"].assert_eq(&actual);
}

fn event_count(summary: &ProfileSummary, name: &'static str) -> usize {
    summary
        .events
        .iter()
        .filter(|event| event.name == name)
        .count()
}

fn render_references(caret_position: Position, locations: &[Location]) -> String {
    if locations.is_empty() {
        return format!("{} unresolved", format_position(caret_position));
    }

    let mut locations = locations.to_vec();
    locations.sort_by(|left, right| {
        left.uri
            .as_str()
            .cmp(right.uri.as_str())
            .then_with(|| left.range.start.cmp(&right.range.start))
            .then_with(|| left.range.end.cmp(&right.range.end))
    });

    let mut output = String::new();
    for location in locations {
        if !output.is_empty() {
            output.push('\n');
        }
        let _ = write!(
            output,
            "{} -> {} {} reference",
            format_position(caret_position),
            location.uri,
            format_position(location.range.start)
        );
    }
    output
}

fn format_position(position: Position) -> String {
    format!("{}:{}", position.line, position.character)
}
