#[path = "../../support.rs"]
mod support;

use std::fmt::Write as _;

use expect_test::{Expect, expect};
use support::MarkedSource;
use ton_language_server_core::languages::tolk::{LANGUAGE_ID, TolkLanguage};
use ton_language_server_core::{
    DocumentUri, LanguageService, LanguageServiceConfig, Position, SelectionRange, TextIndex,
};

fn check(source: &str, marker_names: &[&str], expect: Expect) {
    let marked = MarkedSource::parse(source);
    let uri = DocumentUri::from("file:///fixture/main.tolk");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TolkLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source().to_owned())
        .expect("Tolk document should open");
    let positions = marker_names
        .iter()
        .map(|name| marked.marker(name).position)
        .collect::<Vec<_>>();
    let ranges = service
        .selection_ranges(&uri, &positions)
        .expect("selection range request should succeed");

    expect.assert_eq(&render(marked.source(), &positions, &ranges));
}

fn render(source: &str, positions: &[Position], ranges: &[SelectionRange]) -> String {
    let index = TextIndex::new(source);
    let mut output = String::new();
    for (result_index, (position, range)) in positions.iter().zip(ranges).enumerate() {
        if !output.is_empty() {
            output.push('\n');
        }
        let _ = write!(
            &mut output,
            "position {result_index} at {}:{}",
            position.line, position.character
        );

        let mut depth = 0;
        let mut current = Some(range);
        while let Some(range) = current {
            let start = index.position_to_offset(source, range.range.start);
            let end = index.position_to_offset(source, range.range.end);
            let text = source
                .get(start..end)
                .unwrap_or_default()
                .replace('\\', "\\\\")
                .replace('\n', "\\n")
                .replace('\r', "\\r");
            let _ = write!(
                &mut output,
                "\n  {depth}: {}:{}-{}:{} `{text}`",
                range.range.start.line,
                range.range.start.character,
                range.range.end.line,
                range.range.end.character,
            );
            depth += 1;
            current = range.parent.as_deref();
        }
    }
    output
}

#[test]
fn expands_from_literal_through_expression_statement_function_and_file() {
    check(
        r"
            fun main() {
                val result = foo(1 + <caret>2);
            }
        ",
        &["caret"],
        expect![[r"
            position 0 at 1:25
              0: 1:25-1:26 `2`
              1: 1:21-1:26 `1 + 2`
              2: 1:20-1:27 `(1 + 2)`
              3: 1:17-1:27 `foo(1 + 2)`
              4: 1:4-1:27 `val result = foo(1 + 2)`
              5: 0:11-2:1 `{\n    val result = foo(1 + 2);\n}`
              6: 0:0-2:1 `fun main() {\n    val result = foo(1 + 2);\n}`"]],
    );
}

#[test]
fn selects_the_enclosing_structure_from_whitespace() {
    check(
        r"
            fun main() {
                val first = 1;
                <caret>
                val second = 2;
            }
        ",
        &["caret"],
        expect![[r"
            position 0 at 2:4
              0: 0:11-4:1 `{\n    val first = 1;\n    \n    val second = 2;\n}`
              1: 0:0-4:1 `fun main() {\n    val first = 1;\n    \n    val second = 2;\n}`"]],
    );
}

#[test]
fn handles_comments_and_string_literals_in_one_request() {
    check(
        r#"
            // comment <caret:comment>text
            fun main() {
                val value = "hello <caret:string>world";
            }
        "#,
        &["caret:comment", "caret:string"],
        expect![[r#"
            position 0 at 0:11
              0: 0:0-0:15 `// comment text`
              1: 0:0-3:1 `// comment text\nfun main() {\n    val value = "hello world";\n}`
            position 1 at 2:23
              0: 2:16-2:29 `"hello world"`
              1: 2:4-2:29 `val value = "hello world"`
              2: 1:11-3:1 `{\n    val value = "hello world";\n}`
              3: 1:0-3:1 `fun main() {\n    val value = "hello world";\n}`
              4: 0:0-3:1 `// comment text\nfun main() {\n    val value = "hello world";\n}`"#]],
    );
}

#[test]
fn keeps_ranges_valid_for_incomplete_syntax() {
    check(
        r"
            fun main() {
                val value = foo(<caret>1 + );
        ",
        &["caret"],
        expect![[r"
            position 0 at 1:20
              0: 1:20-1:21 `1`
              1: 1:19-1:25 `(1 + )`
              2: 1:16-1:25 `foo(1 + )`
              3: 1:4-1:25 `val value = foo(1 + )`
              4: 0:11-1:26 `{\n    val value = foo(1 + );`
              5: 0:0-1:26 `fun main() {\n    val value = foo(1 + );`"]],
    );
}

#[test]
fn uses_utf16_columns_after_non_bmp_characters() {
    check(
        r#"fun main() { val face = "😀"; return <caret>face; }"#,
        &["caret"],
        expect![[r#"
            position 0 at 0:37
              0: 0:37-0:41 `face`
              1: 0:30-0:41 `return face`
              2: 0:11-0:44 `{ val face = "😀"; return face; }`
              3: 0:0-0:44 `fun main() { val face = "😀"; return face; }`"#]],
    );
}

#[test]
fn returns_one_empty_range_for_an_empty_document() {
    let uri = DocumentUri::from("file:///fixture/empty.tolk");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TolkLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, String::new())
        .expect("empty Tolk document should open");
    let position = Position::new(0, 0);
    let ranges = service
        .selection_ranges(&uri, &[position])
        .expect("selection range request should succeed");

    expect!["position 0 at 0:0\n  0: 0:0-0:0 ``"].assert_eq(&render("", &[position], &ranges));
}

#[test]
fn preserves_duplicate_positions_and_request_order() {
    check(
        "fun <caret:first>main() { return <caret:second>1; }",
        &["caret:second", "caret:first", "caret:second"],
        expect![[r"
            position 0 at 0:20
              0: 0:20-0:21 `1`
              1: 0:13-0:21 `return 1`
              2: 0:11-0:24 `{ return 1; }`
              3: 0:0-0:24 `fun main() { return 1; }`
            position 1 at 0:4
              0: 0:4-0:8 `main`
              1: 0:0-0:24 `fun main() { return 1; }`
            position 2 at 0:20
              0: 0:20-0:21 `1`
              1: 0:13-0:21 `return 1`
              2: 0:11-0:24 `{ return 1; }`
              3: 0:0-0:24 `fun main() { return 1; }`"]],
    );
}

#[test]
fn refreshes_ranges_after_an_incremental_document_change() {
    let uri = DocumentUri::from("file:///fixture/main.tolk");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TolkLanguage::new());
    service
        .open_document(
            uri.clone(),
            LANGUAGE_ID,
            1,
            "fun main() { return 1; }".to_owned(),
        )
        .expect("Tolk document should open");
    let before_position = Position::new(0, 20);
    let before = service
        .selection_ranges(&uri, &[before_position])
        .expect("selection range request should succeed");

    let changed = "fun main() {\n    return 100 + 200;\n}\n";
    service
        .change_document(&uri, 2, changed.to_owned())
        .expect("Tolk document should change");
    let after_position = Position::new(1, 17);
    let after = service
        .selection_ranges(&uri, &[after_position])
        .expect("selection range request should succeed");

    let actual = format!(
        "before:\n{}\nafter:\n{}",
        render("fun main() { return 1; }", &[before_position], &before),
        render(changed, &[after_position], &after),
    );
    expect![[r"
        before:
        position 0 at 0:20
          0: 0:20-0:21 `1`
          1: 0:13-0:21 `return 1`
          2: 0:11-0:24 `{ return 1; }`
          3: 0:0-0:24 `fun main() { return 1; }`
        after:
        position 0 at 1:17
          0: 1:17-1:20 `200`
          1: 1:11-1:20 `100 + 200`
          2: 1:4-1:20 `return 100 + 200`
          3: 0:11-2:1 `{\n    return 100 + 200;\n}`
          4: 0:0-2:1 `fun main() {\n    return 100 + 200;\n}`
          5: 0:0-3:0 `fun main() {\n    return 100 + 200;\n}\n`"]]
    .assert_eq(&actual);
}

#[test]
fn handles_the_end_of_file_after_trailing_whitespace() {
    check(
        "fun main() {}\n\n<caret>",
        &["caret"],
        expect![[r"
        position 0 at 2:0
          0: 0:0-2:0 `fun main() {}\n\n`"]],
    );
}
