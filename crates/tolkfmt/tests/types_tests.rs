mod common;

use crate::common::{check_with_width_without_trees, check, check_with_width};
use expect_test::expect;

#[test]
fn test_type_identifier() {
    check("const x: int = 0;", expect!["const x: int = 0"]);
}

#[test]
fn test_type_instantiated_ts() {
    check(
        "const x: map<int, slice> = 0;",
        expect!["const x: map<int, slice> = 0"],
    );
}

#[test]
fn test_single_type_instantiated_ts() {
    check("const x: Foo<int> = 0;", expect!["const x: Foo<int> = 0"]);
}

#[test]
fn test_type_instantiated_ts_breaking() {
    check_with_width_without_trees(
        "const x: VeryLongTypeName<FirstType, SecondType, ThirdType> = 0;",
        expect![[r#"
                const x: VeryLongTypeName<
                    FirstType,
                    SecondType,
                    ThirdType,
                > = 0"#]],
        40,
    );
}

#[test]
fn test_nullable_type() {
    check("const x: int? = 0;", expect!["const x: int? = 0"]);
}

#[test]
fn test_parenthesized_type() {
    check("const x: (int) = 0;", expect!["const x: (int) = 0"]);
}

#[test]
fn test_tensor_type() {
    check(
        "const x: (int, slice) = 0;",
        expect!["const x: (int, slice) = 0"],
    );
    check("const x: () = ();", expect!["const x: () = ()"]);
}
#[test]
fn test_single_tensor_type() {
    check("const x: (int) = 0;", expect!["const x: (int) = 0"]);
}

#[test]
fn test_tensor_type_breaking() {
    check_with_width_without_trees(
        "const x: (FirstType, SecondType, ThirdType) = 0;",
        expect![[r#"
                const x: (
                    FirstType,
                    SecondType,
                    ThirdType,
                ) = 0"#]],
        30,
    );
}

#[test]
fn test_tuple_type() {
    check(
        "const x: [int, slice] = 0;",
        expect!["const x: [int, slice] = 0"],
    );
}

#[test]
fn test_single_tuple_type() {
    check("const x: [int] = 0;", expect!["const x: [int] = 0"]);
}

#[test]
fn test_tuple_type_breaking() {
    check_with_width_without_trees(
        "const x: [FirstType, SecondType, ThirdType] = 0;",
        expect![[r#"
                const x: [
                    FirstType,
                    SecondType,
                    ThirdType,
                ] = 0"#]],
        30,
    );
}

#[test]
fn test_fun_callable_type() {
    check(
        "const x: int -> slice = 0;",
        expect!["const x: int -> slice = 0"],
    );
    check(
        "const x: (int, int) -> int = 0;",
        expect!["const x: (int, int) -> int = 0"],
    );
}

#[test]
fn test_fun_callable_type_without_params() {
    check(
        "const x: () -> slice = 0;",
        expect!["const x: () -> slice = 0"],
    );
}

#[test]
fn test_union_type() {
    check(
        "const x: int | slice = 0;",
        expect!["const x: int | slice = 0"],
    );
    check(
        "const x: int | slice | cell = 0;",
        expect!["const x: int | slice | cell = 0"],
    );
}

#[test]
fn test_union_type_breaking() {
    check_with_width(
        "const x: FirstType | SecondType | ThirdType = 0;",
        expect![[r#"
                const x: FirstType
                    | SecondType
                    | ThirdType = 0"#]],
        30,
    );
}

#[test]
fn test_nested_complex_types() {
    check(
        "const x: map<int, (slice | cell)?> = 0;",
        expect!["const x: map<int, (slice | cell)?> = 0"],
    );
}

#[test]
fn test_null_literal_type() {
    check("const x: null = null;", expect!["const x: null = null"]);
}
