#![allow(clippy::needless_raw_string_hashes)]

use expect_test::{Expect, expect};
use ton_language_server_core::languages::fift::{FiftLanguage, LANGUAGE_ID};
use ton_language_server_core::{CodeLens, DocumentUri, LanguageService, LanguageServiceConfig};

fn check_code_lenses(source: &str, expected: Expect) {
    let uri = DocumentUri::from("file:///workspace/main.fif");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(FiftLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, source)
        .expect("Fift document should open");

    let lenses = service
        .code_lens(&uri)
        .expect("code lens request should succeed");
    expected.assert_eq(&render_lenses(&lenses));
}

fn render_lenses(lenses: &[CodeLens]) -> String {
    lenses
        .iter()
        .map(|lens| {
            let command = lens.command.as_ref().expect("lens should have a command");
            format!(
                "{}:{} {} {:?}",
                lens.range.start.line, lens.range.start.character, command.title, command.arguments,
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn links_generated_definitions_to_their_tolk_sources() {
    check_code_lenses(
        r"PROGRAM{
// contracts/main.tolk:42
entry PROC:<{ }>
// contracts/lib.tolk:7
helper PROCINLINE:<{ }>
END>c",
        expect![[r#"
            2:0 Go to Tolk sources (contracts/main.tolk:42) ["contracts/main.tolk", "42"]
            4:0 Go to Tolk sources (contracts/lib.tolk:7) ["contracts/lib.tolk", "7"]"#]],
    );
}

#[test]
fn ignores_unrelated_and_malformed_comments() {
    check_code_lenses(
        r"PROGRAM{
// generated definition
entry PROC:<{ }>
// missing-line
helper PROC:<{ }>
END>c",
        expect![""],
    );
}
