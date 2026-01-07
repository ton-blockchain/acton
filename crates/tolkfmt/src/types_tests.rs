#[cfg(test)]
mod tests {
    use crate::{Context, decls};
    use expect_test::{Expect, expect};
    use std::collections::HashMap;
    use tolk_ast::SourceFile;

    fn check(code: &str, expect: Expect) {
        check_with_width(code, expect, 80)
    }

    fn check_with_width(code: &str, expect: Expect, width: usize) {
        // unsafe { std::env::set_var("UPDATE_EXPECT", "1") }

        let tree = tolk_parser::parser::parse(code).expect("Failed to parse");
        let source_file = SourceFile {
            tree: tree.clone(),
            source: code.into(),
        };
        let ctx = Context {
            code: code.into(),
            comments: HashMap::new(),
        };
        let doc = decls::print_source_file(&ctx, &source_file).unwrap();
        let mut out = Vec::new();
        doc.render(width, &mut out).unwrap();
        let res = String::from_utf8(out).unwrap();

        let res = res
            .lines()
            .map(|l| if l.trim().is_empty() { "" } else { l })
            .collect::<Vec<_>>()
            .join("\n");

        expect.assert_eq(&res);
    }

    #[test]
    fn test_type_identifier() {
        check("const x: int = 0;", expect!["const x: int = 0;"]);
    }

    #[test]
    fn test_type_instantiated_ts() {
        check(
            "const x: map<int, slice> = 0;",
            expect!["const x: map<int, slice> = 0;"],
        );
    }

    #[test]
    fn test_empty_type_instantiated_ts() {
        // TODO: remove?
        check("const x: Foo<> = 0;", expect!["const x: Foo<> = 0;"]);
    }

    #[test]
    fn test_single_type_instantiated_ts() {
        check("const x: Foo<int> = 0;", expect!["const x: Foo<int> = 0;"]);
    }

    #[test]
    fn test_type_instantiated_ts_breaking() {
        check_with_width(
            "const x: VeryLongTypeName<FirstType, SecondType, ThirdType> = 0;",
            expect![[r#"
                const x: VeryLongTypeName<
                    FirstType, SecondType, ThirdType
                > = 0;"#]],
            40,
        );
    }

    #[test]
    fn test_nullable_type() {
        check("const x: int? = 0;", expect!["const x: int? = 0;"]);
    }

    #[test]
    fn test_parenthesized_type() {
        // TODO: unwrap?
        check("const x: (int) = 0;", expect!["const x: (int) = 0;"]);
    }

    #[test]
    fn test_tensor_type() {
        check(
            "const x: (int, slice) = 0;",
            expect!["const x: (int, slice) = 0;"],
        );
        check("const x: () = ();", expect!["const x: () = ();"]);
    }
    #[test]
    fn test_single_tensor_type() {
        check("const x: (int) = 0;", expect!["const x: (int) = 0;"]);
    }

    #[test]
    fn test_tensor_type_breaking() {
        check_with_width(
            "const x: (FirstType, SecondType, ThirdType) = 0;",
            expect![[r#"
                const x: (
                    FirstType, SecondType, ThirdType
                ) = 0;"#]],
            30,
        );
    }

    #[test]
    fn test_tuple_type() {
        check(
            "const x: [int, slice] = 0;",
            expect!["const x: [int, slice] = 0;"],
        );
    }

    #[test]
    fn test_single_tuple_type() {
        check("const x: [int] = 0;", expect!["const x: [int] = 0;"]);
    }

    #[test]
    fn test_tuple_type_breaking() {
        check_with_width(
            "const x: [FirstType, SecondType, ThirdType] = 0;",
            expect![[r#"
                const x: [
                    FirstType, SecondType, ThirdType
                ] = 0;"#]],
            30,
        );
    }

    #[test]
    fn test_fun_callable_type() {
        check(
            "const x: int -> slice = 0;",
            expect!["const x: int -> slice = 0;"],
        );
        check(
            "const x: (int, int) -> int = 0;",
            expect!["const x: (int, int) -> int = 0;"],
        );
    }

    #[test]
    fn test_fun_callable_type_without_params() {
        check(
            "const x: () -> slice = 0;",
            expect!["const x: () -> slice = 0;"],
        );
    }

    #[test]
    fn test_union_type() {
        check(
            "const x: int | slice = 0;",
            expect!["const x: int | slice = 0;"],
        );
        check(
            "const x: int | slice | cell = 0;",
            expect!["const x: int | slice | cell = 0;"],
        );
    }

    #[test]
    fn test_union_type_breaking() {
        // TODO:
        check_with_width(
            "const x: FirstType | SecondType | ThirdType = 0;",
            expect![[r#"
                const x:     | FirstType
                    | SecondType
                    | ThirdType = 0;"#]],
            30,
        );
    }

    #[test]
    fn test_nested_complex_types() {
        check(
            "const x: map<int, (slice | cell)?> = 0;",
            expect!["const x: map<int, (slice | cell)?> = 0;"],
        );
    }

    #[test]
    fn test_null_literal_type() {
        check("const x: null = null;", expect!["const x: null = null;"]);
    }
}
