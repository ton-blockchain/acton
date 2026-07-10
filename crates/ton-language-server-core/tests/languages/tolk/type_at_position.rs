#![allow(clippy::needless_raw_string_hashes)]

#[path = "../../support/mod.rs"]
mod support;

use expect_test::{Expect, expect};
use support::MarkedSource;
use ton_language_server_core::languages::tolk::{LANGUAGE_ID, TolkLanguage};
use ton_language_server_core::{
    DocumentUri, LanguageService, LanguageServiceConfig, TypeAtPosition,
};

fn case_tolk_type_at_position(source: &str, expect: Expect) {
    let marked = MarkedSource::parse(source);
    let uri = DocumentUri::from("file:///fixture/main.tolk");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TolkLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source().to_owned())
        .expect("Tolk document should open");

    let actual = marked
        .markers()
        .iter()
        .map(|marker| {
            service
                .type_at_position(&uri, marker.position)
                .expect("type-at-position request should succeed")
                .map_or_else(|| "<none>".to_owned(), render_type)
        })
        .collect::<Vec<_>>()
        .join("\n");

    expect.assert_eq(&actual);
}

fn render_type(result: TypeAtPosition) -> String {
    format!(
        "{} at {}:{}-{}:{}",
        result.type_name,
        result.range.start.line,
        result.range.start.character,
        result.range.end.line,
        result.range.end.character,
    )
}

#[test]
fn infers_locals_and_chained_stdlib_calls() {
    case_tolk_type_at_position(
        r#"
            fun main() {
                val <caret>parsed = "abc-123".beginParse();
                val <caret>cell = beginCell().storeSlice(parsed).endCell();
            }
        "#,
        expect![[r#"
            slice at 1:8-1:14
            cell at 2:8-2:12"#]],
    );
}

#[test]
fn reports_expression_and_smart_cast_types() {
    case_tolk_type_at_position(
        r#"
            fun main(value: int?) {
                val result = <caret>10 + 20;
                if (value != null) {
                    val narrowed = <caret>value;
                }
            }
        "#,
        expect![[r#"
            int at 1:17-1:19
            int at 3:23-3:28"#]],
    );
}

#[test]
fn climbs_from_an_operator_token_to_its_typed_expression() {
    case_tolk_type_at_position(
        r#"
            fun main() {
                val result = 10 <caret>+ 20;
            }
        "#,
        expect!["int at 1:17-1:24"],
    );
}

#[test]
fn reports_declared_field_and_type_reference_types() {
    case_tolk_type_at_position(
        r#"
            struct Item {
                <caret>value: int
            }

            fun main(item: <caret>Item) {
                val copied = item.<caret>value;
            }
        "#,
        expect![[r#"
            int at 1:4-1:9
            Item at 4:15-4:19
            int at 5:22-5:27"#]],
    );
}

#[test]
fn returns_unknown_with_the_original_syntax_range() {
    case_tolk_type_at_position(
        r#"
            fun main() {
                <caret>unresolved;
            }
        "#,
        expect!["void or unknown at 1:4-1:14"],
    );
}

#[test]
fn returns_unknown_for_an_empty_document() {
    case_tolk_type_at_position("<caret>", expect!["void or unknown at 0:0-0:0"]);
}
