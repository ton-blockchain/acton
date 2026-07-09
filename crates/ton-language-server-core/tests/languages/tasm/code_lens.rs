#[path = "../../support/mod.rs"]
#[allow(dead_code)]
mod support;

use expect_test::{Expect, expect};
use support::MarkedSource;
use ton_language_server_core::languages::tasm::{
    LANGUAGE_ID, STACK_EFFECT_CODE_LENS_COMMAND, TasmLanguage,
};
use ton_language_server_core::{
    CodeLens, DocumentUri, LanguageService, LanguageServiceConfig, ProfileSummary,
    default_language_service,
};

const SPEC_JSON: &str = r#"
{
  "instructions": [
    {
      "name": "PUSHINT_4",
      "signature": {
        "stack_string": "??? ??? x:Int"
      }
    },
    {
      "name": "PUSHCONT",
      "signature": {
        "stack_string": "??? ??? result:Continuation"
      }
    },
    {
      "name": "DUP",
      "signature": {
        "stack_string": "x:Any ??? x:Any x:Any"
      }
    },
    {
      "name": "DROP",
      "signature": {
        "stack_string": "x:Any ??? ???"
      }
    },
    {
      "name": "SWAP",
      "signature": {
        "stack_string": "x:Any y:Any ??? y:Any x:Any"
      }
    }
  ],
  "fift_instructions": []
}
"#;

fn case_tasm_code_lens(source: &str, expect: Expect) {
    let marked = MarkedSource::parse(source);
    let uri = DocumentUri::from("file:///fixture/main.tasm");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(
        TasmLanguage::with_spec_json(SPEC_JSON).expect("TASM spec JSON should be valid"),
    );

    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source().to_owned())
        .expect("TASM document should open");

    expect.assert_eq(&render_code_lenses(
        service
            .code_lens(&uri)
            .expect("code lens request should succeed"),
    ));
}

fn render_code_lenses(lenses: Vec<CodeLens>) -> String {
    if lenses.is_empty() {
        return "<none>".to_owned();
    }

    lenses
        .into_iter()
        .map(|lens| {
            let command = lens.command.expect("code lens should have a command");
            assert_eq!(command.command, STACK_EFFECT_CODE_LENS_COMMAND);
            assert!(command.arguments.is_empty());
            format!(
                "{}:{} title={}",
                lens.range.start.line, lens.range.start.character, command.title
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn resolves_top_level_and_nested_lenses() {
    case_tasm_code_lens(
        r"
            PUSHINT_4 1
            FOOOP
            ref {
              SWAP
              PUSHDICT [
                1 => {
                  XCHG0
                }
              ]
            }
        ",
        expect![[r"
            0:0 title=??? ??? x: Int
            1:0 title=N/A
            3:2 title=x y ??? y x
            4:2 title=N/A
            6:6 title=N/A"]],
    );
}

#[test]
fn resolves_instruction_argument_variants() {
    case_tasm_code_lens(
        r"
            PUSHCONT {
              DUP
            }
            PUSHDICT [
              42 => {
                DROP
              }
            ]
        ",
        expect![[r"
            0:0 title=??? ??? result: Continuation
            1:2 title=x ??? x x
            3:0 title=N/A
            5:4 title=x ??? ???"]],
    );
}

#[test]
fn empty_file_has_no_lenses() {
    case_tasm_code_lens("", expect!["<none>"]);
}

#[test]
fn does_not_resolve_lenses_without_external_spec() {
    let uri = DocumentUri::from("file:///fixture/no-spec.tasm");
    let mut service = default_language_service();

    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, "ADD".to_owned())
        .expect("TASM document should open");

    assert_eq!(
        render_code_lenses(
            service
                .code_lens(&uri)
                .expect("code lens request should succeed")
        ),
        "<none>"
    );
}

#[test]
fn records_code_lens_profile_spans() {
    let uri = DocumentUri::from("file:///fixture/profile.tasm");
    let mut service = LanguageService::new(LanguageServiceConfig {
        enable_profiling: true,
    });
    service.register_language(
        TasmLanguage::with_spec_json(SPEC_JSON).expect("TASM spec JSON should be valid"),
    );

    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, "PUSHINT_4 1".to_owned())
        .expect("TASM document should open");
    let lenses = service
        .code_lens(&uri)
        .expect("code lens request should succeed");

    assert_eq!(lenses.len(), 1);
    let summary = service.profiler().summary();
    let actual = format!(
        "code_lens={} tasm.parse={} code_lens.span={} tasm.code_lens={}",
        lenses.len(),
        event_count(summary, "tasm.parse"),
        event_count(summary, "code_lens"),
        event_count(summary, "tasm.code_lens"),
    );
    expect![[r"
        code_lens=1 tasm.parse=1 code_lens.span=1 tasm.code_lens=1"]]
    .assert_eq(&actual);
}

fn event_count(summary: &ProfileSummary, name: &'static str) -> usize {
    summary
        .events
        .iter()
        .filter(|event| event.name == name)
        .count()
}
