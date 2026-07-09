#[path = "../../support/mod.rs"]
#[allow(dead_code)]
mod support;

use expect_test::{Expect, expect};
use support::MarkedSource;
use ton_language_server_core::languages::tasm::{LANGUAGE_ID, TasmLanguage};
use ton_language_server_core::{
    DocumentUri, LanguageService, LanguageServiceConfig, ProfileSummary, default_language_service,
};

const SPEC_JSON: &str = r##"
{
  "instructions": [
    {
      "name": "ADD",
      "category": "arithmetic",
      "sub_category": "basic",
      "description": {
        "short": "Adds two integers.",
        "long": "Pops two integers and pushes their sum.",
        "gas": [
          {
            "value": 18,
            "description": "Base gas consumption"
          }
        ]
      },
      "layout": {
        "prefix_str": "A0",
        "tlb": "#a0"
      },
      "signature": {
        "stack_string": "x:Int y:Int -> sum:Int"
      }
    }
  ],
  "fift_instructions": []
}
"##;

fn case_tasm_hover(source: &str, expect: Expect) {
    let marked = MarkedSource::parse(source);
    let caret = marked.marker("caret");
    let uri = DocumentUri::from("file:///fixture/main.tasm");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(
        TasmLanguage::with_spec_json(SPEC_JSON).expect("TASM spec JSON should be valid"),
    );

    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source().to_owned())
        .expect("TASM document should open");
    let actual = service
        .hover(&uri, caret.position)
        .expect("hover request should succeed")
        .map_or_else(
            || {
                format!(
                    "{}:{} unresolved",
                    caret.position.line, caret.position.character
                )
            },
            |hover| hover.contents,
        );

    expect.assert_eq(&actual);
}

#[test]
fn resolves_instruction_hover_from_external_spec() {
    case_tasm_hover(
        r"
            <caret>ADD
        ",
        expect![[r"
            ```
            ADD
            ```
            - Stack (top is on the right): `x:Int y:Int → sum:Int`
            - Gas: `18`
            - Opcode: `A0`
            - TL-B: `#a0`
            - Category: `arithmetic`
            - Subcategory: `basic`

            Adds two integers.

            **Details:**

            Pops two integers and pushes their sum.
            "]],
    );
}

#[test]
fn does_not_hover_arguments() {
    case_tasm_hover(
        r"
            ADD <caret>1
        ",
        expect![[r"
            0:4 unresolved"]],
    );
}

#[test]
fn does_not_hover_without_external_spec() {
    let marked = MarkedSource::parse("<caret>ADD\n");
    let caret = marked.marker("caret");
    let uri = DocumentUri::from("file:///fixture/no-spec.tasm");
    let mut service = default_language_service();

    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source().to_owned())
        .expect("TASM document should open");
    let actual = service
        .hover(&uri, caret.position)
        .expect("hover request should succeed");

    expect![[r"
        unresolved"]]
    .assert_eq(if actual.is_some() {
        "resolved"
    } else {
        "unresolved"
    });
}

#[test]
fn records_hover_profile_spans() {
    let marked = MarkedSource::parse("<caret>ADD\n");
    let caret = marked.marker("caret");
    let uri = DocumentUri::from("file:///fixture/profiled.tasm");
    let mut service = LanguageService::new(LanguageServiceConfig {
        enable_profiling: true,
    });
    service.register_language(
        TasmLanguage::with_spec_json(SPEC_JSON).expect("TASM spec JSON should be valid"),
    );

    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source().to_owned())
        .expect("TASM document should open");
    let hover = service
        .hover(&uri, caret.position)
        .expect("hover request should succeed");

    let summary = service.profiler().summary();
    let actual = format!(
        "hover={} tasm.parse={} hover.span={} tasm.hover={}",
        hover.is_some(),
        event_count(summary, "tasm.parse"),
        event_count(summary, "hover"),
        event_count(summary, "tasm.hover"),
    );
    expect![[r"
        hover=true tasm.parse=1 hover.span=1 tasm.hover=1"]]
    .assert_eq(&actual);
}

fn event_count(summary: &ProfileSummary, name: &'static str) -> usize {
    summary
        .events
        .iter()
        .filter(|event| event.name == name)
        .count()
}
