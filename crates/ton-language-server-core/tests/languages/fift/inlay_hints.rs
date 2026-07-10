#![allow(clippy::needless_raw_string_hashes)]

use expect_test::{Expect, expect};
use ton_language_server_core::languages::fift::{FiftLanguage, LANGUAGE_ID};
use ton_language_server_core::{
    DocumentUri, InlayHint, LanguageService, LanguageServiceConfig, Position, Range,
};

const SPEC_JSON: &str = r#"
{
  "instructions": [
    {
      "name": "ADD",
      "description": {"gas": [{"value": 18}]},
      "signature": {"stack_string": "x:Int y:Int -> sum:Int"}
    },
    {
      "name": "DYNAMIC",
      "description": {"gas": [{"value": 0, "formula": "base + n"}]}
    }
  ]
}
"#;

fn check_hints(source: &str, range: Range, expected: Expect) {
    let uri = DocumentUri::from("file:///workspace/main.fif");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(
        FiftLanguage::with_spec_json(SPEC_JSON).expect("instruction spec should parse"),
    );
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, source)
        .expect("Fift document should open");

    let hints = service
        .inlay_hints(&uri, range)
        .expect("inlay hints request should succeed");
    expected.assert_eq(&render_hints(&hints));
}

fn render_hints(hints: &[InlayHint]) -> String {
    hints
        .iter()
        .map(|hint| {
            format!(
                "{}:{} {}",
                hint.position.line, hint.position.character, hint.label
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn shows_fixed_and_formula_gas_costs() {
    check_hints(
        "PROGRAM{\nentry PROC:<{\n  ADD\n  DYNAMIC\n  UNKNOWN\n}>\nEND>c\n",
        Range::new(Position::new(0, 0), Position::new(u32::MAX, u32::MAX)),
        expect![[r"
            2:5 18
            3:9 base + n"]],
    );
}

#[test]
fn respects_the_requested_range() {
    check_hints(
        "PROGRAM{\nentry PROC:<{\n  ADD\n  DYNAMIC\n}>\nEND>c\n",
        Range::new(Position::new(3, 0), Position::new(3, u32::MAX)),
        expect!["3:9 base + n"],
    );
}
