#[path = "../../support/snapshots.rs"]
mod snapshots;
#[path = "../../support/mod.rs"]
mod support;

use snapshots::assert_file_snapshot;
use std::fmt::Write as _;
use support::{MarkedSource, render_completion};
use ton_language_server_core::languages::tlb::{LANGUAGE_ID, TlbLanguage};
use ton_language_server_core::{
    CompletionTrigger, DocumentUri, LanguageService, LanguageServiceConfig,
};

#[test]
fn completion_matches_snapshot() -> anyhow::Result<()> {
    let marked = MarkedSource::parse(
        r"
            first$0 left:uint32 right:uint32 = First;
            second$1 item:First = Second;
            probe$2 value:Fi<caret:type> = Probe;
            <caret:value>third$3 a:uint32 b:uint32 = Third;
        ",
    );
    let uri = DocumentUri::from("memory:///schema.tlb");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TlbLanguage::new());
    service.open_document(uri.clone(), LANGUAGE_ID, 1, marked.source().to_owned())?;

    let mut output = String::new();
    for marker in marked.markers() {
        let completion = service.completion(&uri, marker.position, CompletionTrigger::invoked())?;
        let _ = writeln!(output, "[{}]", marker.name);
        output.push_str(&render_completion(&completion.items));
        output.push('\n');
    }
    assert_file_snapshot("languages/tlb/completion.snap", output.trim_end())
}
