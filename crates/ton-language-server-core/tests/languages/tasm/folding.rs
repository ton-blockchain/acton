#![allow(clippy::needless_raw_string_hashes)]

use expect_test::{Expect, expect};
use ton_language_server_core::languages::tasm::{LANGUAGE_ID, TasmLanguage};
use ton_language_server_core::{DocumentUri, FoldingRange, LanguageService, LanguageServiceConfig};

fn check_folding(source: &str, expected: Expect) {
    let uri = DocumentUri::from("file:///workspace/main.tasm");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TasmLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, source)
        .expect("TASM document should open");

    let ranges = service
        .folding_ranges(&uri)
        .expect("folding request should succeed");
    expected.assert_eq(&render_ranges(&ranges));
}

fn render_ranges(ranges: &[FoldingRange]) -> String {
    ranges
        .iter()
        .map(|range| format!("[{}, {}]", range.start_line, range.end_line))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn folds_nested_code_and_dictionary_blocks() {
    check_folding(
        r"PUSHCONT {
              PUSHDICT [
                1 => {
                  SWAP
                }
              ]
            }",
        expect![[r"
            [0, 6]
            [1, 5]
            [2, 4]"]],
    );
}

#[test]
fn folds_explicit_refs_and_instruction_code_arguments() {
    check_folding(
        r"ref {
              PUSHINT_4 1
            }
            PUSHCONT {
              DUP
            }",
        expect![[r"
            [0, 2]
            [3, 5]"]],
    );
}

#[test]
fn skips_single_line_blocks() {
    check_folding(
        r"ref { SWAP }
            PUSHCONT { DUP }
        ",
        expect![""],
    );
}
