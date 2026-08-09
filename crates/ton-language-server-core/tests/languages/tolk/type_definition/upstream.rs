#![allow(clippy::needless_raw_string_hashes)]

use super::case_type_definition;
use expect_test::expect;

#[test]
fn upstream_01_type_resolve_for_variable_with_builtin_type() {
    case_type_definition(
        r#"fun test() {
    val <caret>a = 100;
}"#,
        expect![[r#"1:8 unresolved"#]],
    );
}

#[test]
fn upstream_02_type_resolve_for_variable_with_struct_type() {
    case_type_definition(
        r#"struct Foo {}

fun test() {
    val <caret>a: Foo = {};
}"#,
        expect![[r#"3:8 -> 0:7 resolved"#]],
    );
}

#[test]
fn upstream_03_type_resolve_for_field_with_builtin_type() {
    case_type_definition(
        r#"struct Foo {
    value: int,
}

fun test() {
    val a: Foo = { <caret>value: 10 };
}"#,
        expect![[r#"5:19 unresolved"#]],
    );
}

#[test]
fn upstream_04_type_resolve_for_type() {
    case_type_definition(
        r#"struct Foo {}

fun test(): <caret>Foo {
}"#,
        expect![[r#"2:12 -> 0:7 resolved"#]],
    );
}

#[test]
fn upstream_05_type_resolve_for_variable_with_unknown_type() {
    case_type_definition(
        r#"fun test() {
    val <caret>a;
}"#,
        expect![[r#"1:8 unresolved"#]],
    );
}

#[test]
fn upstream_06_type_resolve_for_variable_with_tensor_type() {
    case_type_definition(
        r#"fun test() {
    val <caret>a = (1, 2, 3);
}"#,
        expect![[r#"1:8 unresolved"#]],
    );
}

#[test]
fn upstream_07_type_resolve_for_keyword() {
    case_type_definition(
        r#"<caret>fun test() {
}"#,
        expect![[r#"0:0 unresolved"#]],
    );
}
