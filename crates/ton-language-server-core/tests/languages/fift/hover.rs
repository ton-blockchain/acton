#![allow(clippy::needless_raw_string_hashes)]

#[path = "../../support/mod.rs"]
mod support;

use expect_test::{Expect, expect};
use support::MarkedSource;
use ton_language_server_core::languages::fift::{FiftLanguage, LANGUAGE_ID};
use ton_language_server_core::{DocumentUri, LanguageService, LanguageServiceConfig};

const SPEC_JSON: &str = r#"
{
  "instructions": [
    {
      "name": "PUSHINT_4",
      "description": {
        "short": "Pushes a tiny integer.",
        "operands": ["i"],
        "gas": [{"value": 18}]
      },
      "layout": {"prefix_str": "7"},
      "signature": {"stack_string": "-> x:Int"}
    }
  ]
}
"#;

fn check_hover(source: &str, with_spec: bool, expected: Expect) {
    let marked = MarkedSource::parse(source);
    let uri = DocumentUri::from("file:///workspace/main.fif");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    let language = if with_spec {
        FiftLanguage::with_spec_json(SPEC_JSON).expect("instruction spec should parse")
    } else {
        FiftLanguage::new()
    };
    service.register_language(language);
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source())
        .expect("Fift document should open");

    let result = service
        .hover(&uri, marked.marker("caret").position)
        .expect("hover request should succeed")
        .map_or_else(|| "<none>".to_owned(), |hover| hover.contents);
    expected.assert_eq(&result);
}

#[test]
fn documents_resolved_definitions_without_an_instruction_spec() {
    check_hover(
        r"
            PROGRAM{
              entry PROC:<{
                <caret>foo CALLDICT
              }>
              foo PROC:<{ 1 PUSHINT }>
            END>c
        ",
        false,
        expect![[r#"
            ```fift
            foo PROC:<{ 1 PUSHINT }>
            ```"#]],
    );
}

#[test]
fn selects_the_instruction_variant_from_inline_arguments() {
    check_hover(
        r"
            PROGRAM{
              entry PROC:<{
                1 <caret>PUSHINT
              }>
            END>c
        ",
        true,
        expect![[r#"
            ```
            PUSHINT_4 [i]
            ```
            - Stack (top is on the right): `→ x:Int`
            - Gas: `18`
            - Opcode: `7`

            Pushes a tiny integer.
            "#]],
    );
}

#[test]
fn does_not_document_instruction_arguments_or_unknown_words() {
    check_hover("1 <caret>PUSHINT", false, expect!["<none>"]);
    check_hover("<caret>UNKNOWN", true, expect!["<none>"]);
}
