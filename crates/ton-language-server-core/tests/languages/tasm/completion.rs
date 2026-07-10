#[path = "../../support/snapshots.rs"]
mod snapshots;
#[path = "../../support/mod.rs"]
mod support;

use snapshots::assert_file_snapshot;
use support::{MarkedSource, render_completion};
use ton_language_server_core::languages::tasm::{LANGUAGE_ID, TasmLanguage};
use ton_language_server_core::{
    CompletionTrigger, DocumentUri, LanguageService, LanguageServiceConfig,
};

const SPEC_JSON: &str = r#"
{
  "instructions": [
    {
      "name": "ADD",
      "description": {"short": "Adds integers.", "operands": []},
      "signature": {"stack_string": "x:Int y:Int -> sum:Int"}
    },
    {
      "name": "PUSHINT",
      "description": {"short": "Pushes an integer.", "operands": ["value"]},
      "signature": {"stack_string": "-> value:Int"}
    }
  ]
}
"#;

#[test]
fn completion_matches_snapshot() -> anyhow::Result<()> {
    let marked = MarkedSource::parse("PUSH<caret>\n");
    let uri = DocumentUri::from("memory:///main.tasm");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TasmLanguage::with_spec_json(SPEC_JSON)?);
    service.open_document(uri.clone(), LANGUAGE_ID, 1, marked.source().to_owned())?;
    let completion = service.completion(
        &uri,
        marked.marker("caret").position,
        CompletionTrigger::invoked(),
    )?;
    assert_file_snapshot(
        "languages/tasm/completion.snap",
        &render_completion(&completion.items),
    )
}

#[test]
fn completion_is_empty_without_spec() -> anyhow::Result<()> {
    let marked = MarkedSource::parse("<caret>ADD\n");
    let uri = DocumentUri::from("memory:///no-spec.tasm");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TasmLanguage::new());
    service.open_document(uri.clone(), LANGUAGE_ID, 1, marked.source().to_owned())?;
    let completion = service.completion(
        &uri,
        marked.marker("caret").position,
        CompletionTrigger::invoked(),
    )?;
    assert_file_snapshot(
        "languages/tasm/completion_without_spec.snap",
        &render_completion(&completion.items),
    )
}
