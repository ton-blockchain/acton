use super::support::CompletionTest;
use expect_test::expect;

#[test]
fn completes_variable_size_integer_and_bits_types() {
    // A type position offers fixed-width and placeholder-width integer families.
    CompletionTest::new("fun main(value: int<caret>) {}")
        .labels(&["int8", "int16", "int257", "int{X}", "uint{X}"])
        .check(expect![[r#"
            label    kind           detail  edit       text
            int8     TypeParameter          0:16-0:19  int8
            int16    TypeParameter          0:16-0:19  int16
            int257   TypeParameter          0:16-0:19  int257
            int{X}   TypeParameter          0:16-0:19  int${1:32}
            uint{X}  TypeParameter          0:16-0:19  uint${1:32}"#]]);

    // Variable-size types are also valid in expression positions.
    CompletionTest::new("fun main() { val value = bits<caret>; }")
        .labels(&["bits256", "bits{X}"])
        .check(expect![[r#"
            label    kind           detail  edit       text
            bits256  TypeParameter          0:25-0:29  bits256
            bits{X}  TypeParameter          0:25-0:29  bits${1:32}"#]]);
}

#[test]
fn applies_placeholder_width_type_completion() {
    // Selecting a placeholder-width type expands X to an editable numeric tab stop.
    CompletionTest::new("fun main(value: int<caret>) {}")
        .check_applied("int{X}", expect!["fun main(value: int32<caret>) {}"]);
}
