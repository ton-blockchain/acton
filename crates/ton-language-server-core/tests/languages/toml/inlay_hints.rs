#[path = "../../support.rs"]
mod support;

use expect_test::{Expect, expect};
use support::MarkedSource;
use ton_language_server_core::languages::toml::{LANGUAGE_ID, TomlLanguage};
use ton_language_server_core::{
    DocumentUri, InlayHint, LanguageService, LanguageServiceConfig, Position, Range, TextIndex,
};

const ACTON_VERSION: &str = "1.1.0 (abc123 2026-08-09)";

fn check_hints(uri: &str, source: &str, expected: Expect) {
    let marked = MarkedSource::parse(source);
    let uri = DocumentUri::from(uri);
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TomlLanguage::with_acton_version(ACTON_VERSION));
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source())
        .expect("TOML document should open");
    let hints = service
        .inlay_hints(&uri, full_document_range())
        .expect("inlay hints request should succeed");
    expected.assert_eq(&source_with_inline_hints(marked.source(), &hints));
}

fn source_with_inline_hints(source: &str, hints: &[InlayHint]) -> String {
    let index = TextIndex::new(source);
    let mut insertions = hints
        .iter()
        .enumerate()
        .map(|(order, hint)| {
            (
                index.position_to_offset(source, hint.position),
                order,
                format!("/* {} */", hint.label.trim()),
            )
        })
        .collect::<Vec<_>>();
    insertions.sort_by_key(|(offset, order, _)| std::cmp::Reverse((*offset, *order)));

    let mut annotated = source.to_owned();
    for (offset, _, text) in insertions {
        annotated.insert_str(offset, &text);
    }
    annotated
}

const fn full_document_range() -> Range {
    Range::new(Position::new(0, 0), Position::new(u32::MAX, u32::MAX))
}

#[test]
fn shows_installed_acton_version_for_toolchain_requirement() {
    check_hints(
        "file:///workspace/Acton.toml",
        r#"
            [toolchain]
            acton = "1.1.0"
        "#,
        expect![[r#"
            [toolchain]
            acton = "1.1.0"/* installed: 1.1.0 (abc123 2026-08-09) */"#]],
    );

    check_hints(
        "file:///workspace/Acton.toml",
        r#"
            "toolchain"."acton" = "trunk"
        "#,
        expect![[r#""toolchain"."acton" = "trunk"/* installed: 1.1.0 (abc123 2026-08-09) */"#]],
    );
}

#[test]
fn ignores_other_fields_and_toml_files() {
    check_hints(
        "file:///workspace/Acton.toml",
        r#"
            [package]
            name = "acton"
        "#,
        expect![[r#"
            [package]
            name = "acton""#]],
    );

    check_hints(
        "file:///workspace/Cargo.toml",
        r#"
            [toolchain]
            acton = "1.1.0"
        "#,
        expect![[r#"
            [toolchain]
            acton = "1.1.0""#]],
    );
}
