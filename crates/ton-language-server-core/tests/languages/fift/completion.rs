#[path = "../../support/snapshots.rs"]
mod snapshots;
#[path = "../../support/mod.rs"]
#[allow(dead_code)]
mod support;

use snapshots::assert_file_snapshot;
use support::render_completion;
use ton_language_server_core::languages::fift::{FiftLanguage, LANGUAGE_ID};
use ton_language_server_core::{
    CompletionTrigger, DocumentUri, LanguageService, LanguageServiceConfig, TextIndex,
};

#[test]
fn completion_matches_snapshot() -> anyhow::Result<()> {
    let source = "PROGRAM{\nDECLPROC helper\nhelper PROC:<{\n  IF\n}>\nEND>c";
    let offset = source.find("IF").expect("IF offset") + 2;
    let position = TextIndex::new(source).offset_to_position(source, offset);
    let uri = DocumentUri::from("memory:///main.fif");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(FiftLanguage::new());
    service.open_document(uri.clone(), LANGUAGE_ID, 1, source)?;
    let completion = service.completion(&uri, position, CompletionTrigger::invoked())?;
    assert_file_snapshot(
        "languages/fift/completion.snap",
        &render_completion(&completion.items),
    )
}
