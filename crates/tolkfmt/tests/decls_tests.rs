#[cfg(test)]
mod tests {
    use expect_test::{Expect, expect};
    use tolkfmt::format_source;

    fn check(code: &str, expect: Expect) {
        check_with_width(code, expect, 80)
    }

    fn check_with_width(code: &str, expect: Expect, width: usize) {
        // unsafe { std::env::set_var("UPDATE_EXPECT", "1") }

        let res = format_source(code, width).unwrap();

        let res = res
            .lines()
            .map(|l| if l.trim().is_empty() { "" } else { l })
            .collect::<Vec<_>>()
            .join("\n");

        expect.assert_eq(&res);
    }

    #[test]
    fn test_function_parameters_comments() {
        check(
            "fun test(
                // leading
                a: int, // inline
                // trailing
                b: slice
            ) {}",
            expect![[r#"
                fun test(
                    // leading
                    a: int, // inline
                    // trailing
                    b: slice,
                ) {}"#]],
        );
    }

    #[test]
    fn test_annotation_arguments_comments() {
        check(
            "@test(
                // leading
                1, // inline
                2
                // trailing
            )
            fun main() {}",
            expect![[r#"
                @test(
                    // leading
                    1, // inline
                    2,
                    // trailing
                )
                fun main() {}"#]],
        );
    }

    #[test]
    fn test_type_parameters_comments() {
        check(
            "struct Test<
                // leading
                T, // inline
                U
                // trailing
            > {}",
            expect![[r#"
                struct Test<
                    // leading
                    T, // inline
                    U,
                    // trailing
                > {}"#]],
        );
    }

    #[test]
    fn test_asm_comments() {
        check(
            "fun test() asm(a b -> 1)
                // leading
                \"INC\" // inline
                // trailing
                \"DEC\";",
            expect![[r#"
                fun test()
                    asm(a b -> 1)
                        // leading
                        "INC" // inline
                        // trailing
                        "DEC""#]],
        );
    }

    #[test]
    fn test_annotation_comments() {
        check(
            "// leading list
            @test // inline list
            // trailing list
            @deprecated
            fun main() {}",
            expect![[r#"
                // leading list
                @test // inline list
                // trailing list
                @deprecated
                fun main() {}"#]],
        );
    }

    #[test]
    fn test_tolk_required_version() {
        check("tolk 0.6.0", expect!["tolk 0.6.0"]);
    }

    #[test]
    fn test_import() {
        check(
            "import \"common.tolk\"",
            expect![[r#"import "common.tolk""#]],
        );
    }

    #[test]
    fn test_global_var() {
        check(
            "global x: int;",
            expect![[r#"
                global x: int;"#]],
        );
    }

    #[test]
    fn test_global_var_with_annotations() {
        check(
            "@deprecated\n@custom(42)\nglobal x: int;",
            expect![[r#"
                @deprecated
                @custom(42)
                global x: int;"#]],
        );
    }

    #[test]
    fn test_constant_declaration() {
        check(
            "const x = 42;",
            expect![[r#"
                const x = 42;"#]],
        );
        check(
            "const x: int = 42;",
            expect![[r#"
                const x: int = 42;"#]],
        );
    }

    #[test]
    fn test_constant_with_annotations() {
        check(
            "@deprecated\nconst MAX_SIZE = 100;",
            expect![[r#"
                @deprecated
                const MAX_SIZE = 100;"#]],
        );
    }

    #[test]
    fn test_constant_complex_expression() {
        check(
            "const PI = 3.14159;",
            expect![[r#"
                const PI = 3.14159;"#]],
        );
        check(
            "const MSG = \"hello world\";",
            expect![[r#"
                const MSG = "hello world";"#]],
        );
        check(
            "const FLAG = true;",
            expect![[r#"
                const FLAG = true;"#]],
        );
    }

    #[test]
    fn test_type_alias() {
        check(
            "type MyInt = int;",
            expect![[r#"
                type MyInt = int;"#]],
        );
        check(
            "type MyMap<K, V> = map<K, V>;",
            expect![[r#"
                type MyMap<K, V> = map<K, V>;"#]],
        );
    }

    #[test]
    fn test_type_alias_with_annotations() {
        check(
            "@deprecated\ntype OldType = int;",
            expect![[r#"
                @deprecated
                type OldType = int;"#]],
        );
    }

    #[test]
    fn test_type_alias_union_type() {
        check(
            "type Result = int | slice;",
            expect![[r#"
                type Result = int | slice;"#]],
        );
        check(
            "type ComplexUnion = int | slice | bool;",
            expect![[r#"
                type ComplexUnion = int | slice | bool;"#]],
        );
        check_with_width(
            "type ComplexUnion = int | slice | bool | address;",
            expect![[r#"
                type ComplexUnion =
                    | int
                    | slice
                    | bool
                    | address;"#]],
            20,
        );
    }

    #[test]
    fn test_type_alias_builtin() {
        check(
            "type MyBuiltin = builtin;",
            expect![[r#"
                type MyBuiltin = builtin;"#]],
        );
    }

    #[test]
    fn test_type_alias_complex_types() {
        check(
            "type OptionalInt = int?;",
            expect![[r#"
                type OptionalInt = int?;"#]],
        );
        check(
            "type TupleType = [int, slice];",
            expect![[r#"
                type TupleType = [int, slice];"#]],
        );
        check(
            "type TensorType = (int, slice, bool);",
            expect![[r#"
                type TensorType = (int, slice, bool);"#]],
        );
    }

    #[test]
    fn test_struct_declaration() {
        check(
            "struct Point { x: int, y: int }",
            expect![[r#"
                struct Point {
                    x: int
                    y: int
                }"#]],
        );
        check(
            "struct Point<T> { x: T, y: T }",
            expect![[r#"
                struct Point<T> {
                    x: T
                    y: T
                }"#]],
        );
    }

    #[test]
    fn test_struct_declaration_with_new_lines() {
        check(
            r#"struct Point {
                x: int

                y: int
            }"#,
            expect![[r#"
                struct Point {
                    x: int

                    y: int
                }"#]],
        );
        check(
            r#"struct Point {
                x: int

                y: int

                z: int
                z1: int
            }"#,
            expect![[r#"
                struct Point {
                    x: int

                    y: int

                    z: int
                    z1: int
                }"#]],
        );
    }

    #[test]
    fn test_struct_declaration_with_comments() {
        check(
            r#"struct Point {
                // leadding comment
                x: int // inline comment 1
                y: int, // inline comment 2
                z: int
                // trailing comment
            }"#,
            expect![[r#"
                struct Point {
                    // leadding comment
                    x: int // inline comment 1
                    y: int // inline comment 2
                    z: int
                    // trailing comment
                }"#]],
        );
    }

    #[test]
    fn test_struct_with_pack_prefix() {
        check(
            "struct (1) PackedStruct { x: int }",
            expect![[r#"
                struct (1) PackedStruct {
                    x: int
                }"#]],
        );
    }

    #[test]
    fn test_struct_with_annotations() {
        check(
            "@deprecated\nstruct OldStruct { x: int }",
            expect![[r#"
                @deprecated
                struct OldStruct {
                    x: int
                }"#]],
        );
    }

    #[test]
    fn test_struct_field_modifiers() {
        check(
            "struct Test { readonly x: int, private y: slice, private readonly z: slice }",
            expect![[r#"
                struct Test {
                    readonly x: int
                    private y: slice
                    private readonly z: slice
                }"#]],
        );
    }

    #[test]
    fn test_struct_field_defaults() {
        check(
            "struct Config { timeout: int = 30, enabled: bool = true }",
            expect![[r#"
                struct Config {
                    timeout: int = 30
                    enabled: bool = true
                }"#]],
        );
    }

    #[test]
    fn test_struct_empty() {
        // TODO
        check(
            "struct Empty {}",
            expect![[r#"
                struct Empty {}"#]],
        );
    }

    #[test]
    fn test_struct_complex() {
        check(
            "@custom\nstruct (0x2) Complex<T, U> { readonly x: T = 42, private y: U }",
            expect![[r#"
                @custom
                struct (0x2) Complex<T, U> {
                    readonly x: T = 42
                    private y: U
                }"#]],
        );
    }

    #[test]
    fn test_enum_declaration() {
        check(
            "enum Color { RED, GREEN, BLUE }",
            expect![[r#"
                enum Color {
                    RED
                    GREEN
                    BLUE
                }"#]],
        );
        check(
            "enum Status: int { OK = 0, ERROR = 1 }",
            expect![[r#"
                enum Status: int {
                    OK = 0
                    ERROR = 1
                }"#]],
        );
    }

    #[test]
    fn test_enum_declaration_with_new_lines() {
        check(
            r#"enum Color {
                RED,

                GREEN,

                BLUE
            }"#,
            expect![[r#"
                enum Color {
                    RED

                    GREEN

                    BLUE
                }"#]],
        );
        check(
            r#"enum Color {
                RED,

                GREEN,

                BLUE,
                BLUE2,
            }"#,
            expect![[r#"
                enum Color {
                    RED

                    GREEN

                    BLUE
                    BLUE2
                }"#]],
        );
    }

    #[test]
    fn test_enum_with_annotations() {
        check(
            "@deprecated\nenum OldEnum { A, B }",
            expect![[r#"
                @deprecated
                enum OldEnum {
                    A
                    B
                }"#]],
        );
    }

    #[test]
    fn test_enum_backed_types() {
        check(
            "enum Status: uint8 { OK = 0, ERROR = 1 }",
            expect![[r#"
                enum Status: uint8 {
                    OK = 0
                    ERROR = 1
                }"#]],
        );
    }

    #[test]
    fn test_enum_mixed_values() {
        check(
            "enum Mixed { A, B = 1, C, D = 10 }",
            expect![[r#"
                enum Mixed {
                    A
                    B = 1
                    C
                    D = 10
                }"#]],
        );
    }

    #[test]
    fn test_enum_single_member() {
        check(
            "enum Single { ONLY }",
            expect![[r#"
                enum Single {
                    ONLY
                }"#]],
        );
    }

    #[test]
    fn test_enum_empty() {
        check(
            "enum Empty {}",
            expect![[r#"
                enum Empty {}"#]],
        );
    }

    #[test]
    fn test_enum_comments_alignment() {
        check(
            r#"enum BounceMode {
                NoBounce               // a message will just disappear on error
                Only256BitsOfBody      // `in.bouncedBody` will be "0xFFFFFFFF" and the first 256 bits of outgoing body (most cheap)
                RichBounce             // `in.bouncedBody` will be struct RichBounceBody (most expensive, but allows accessing all data sent)
                RichBounceOnlyRootCell // `in.bouncedBody` will be struct RichBounceBody without refs in `originalBody`
            }"#,
            expect![[r#"
                enum BounceMode {
                    NoBounce               // a message will just disappear on error
                    Only256BitsOfBody      // `in.bouncedBody` will be "0xFFFFFFFF" and the first 256 bits of outgoing body (most cheap)
                    RichBounce             // `in.bouncedBody` will be struct RichBounceBody (most expensive, but allows accessing all data sent)
                    RichBounceOnlyRootCell // `in.bouncedBody` will be struct RichBounceBody without refs in `originalBody`
                }"#]],
        );
    }

    #[test]
    fn test_struct_comments_alignment() {
        check(
            r#"struct Config {
                enabled: bool        // enable feature
                timeout: int         // timeout in seconds
                host: slice          // server host
                port: int            // server port number
            }"#,
            expect![[r#"
                struct Config {
                    enabled: bool // enable feature
                    timeout: int  // timeout in seconds
                    host: slice   // server host
                    port: int     // server port number
                }"#]],
        );
    }

    #[test]
    fn test_function_with_annotations() {
        check(
            "@pure\nfun foo() {}",
            expect![[r#"
                @pure
                fun foo() {}"#]],
        );
    }

    #[test]
    fn test_function_generics() {
        check(
            "fun identity<T>(x: T): T { return x; }",
            expect![[r#"
                fun identity<T>(x: T): T {
                    return x;
                }"#]],
        );
    }
    #[test]
    fn test_function_generics_with_default_type() {
        check(
            "fun identity<T = int>(x: T): T { return x; }",
            expect![[r#"
                fun identity<T = int>(x: T): T {
                    return x;
                }"#]],
        );
    }

    #[test]
    fn test_function_no_generics() {
        // TODO: remove?
        check("fun foo<>() {}", expect!["fun foo<>() {}"]);
    }

    #[test]
    fn test_function_parameters() {
        check(
            "fun add(a: int, b: int): int { return a + b; }",
            expect![[r#"
                fun add(a: int, b: int): int {
                    return a + b;
                }"#]],
        );
    }

    #[test]
    fn test_function_parameter_with_default() {
        check(
            "fun add(a: int = 10, b: int = 20 + 10): int { return a + b; }",
            expect![[r#"
                fun add(a: int = 10, b: int = 20 + 10): int {
                    return a + b;
                }"#]],
        );
    }

    #[test]
    fn test_function_optional_return() {
        check(
            "fun optional(): int? { return null; }",
            expect![[r#"
                fun optional(): int? {
                    return null;
                }"#]],
        );
    }

    #[test]
    fn test_function_complex_return() {
        check(
            "fun complex(): [int, slice] { return [1, \"hello\"]; }",
            expect![[r#"
                fun complex(): [int, slice] {
                    return [1, "hello"];
                }"#]],
        );
    }

    #[test]
    fn test_function_no_return_type() {
        check(
            "fun noReturn() { return; }",
            expect![[r#"
                fun noReturn() {
                    return;
                }"#]],
        );
    }

    #[test]
    fn test_function_no_parameters() {
        check(
            "fun empty(): int { return 42; }",
            expect![[r#"
                fun empty(): int {
                    return 42;
                }"#]],
        );
    }

    #[test]
    fn test_multiple_annotations() {
        check(
            "@pure\n@deprecated\nfun foo() {}",
            expect![[r#"
                @pure
                @deprecated
                fun foo() {}"#]],
        );
    }

    #[test]
    fn test_annotation_with_arguments() {
        check(
            "@deprecated(\"use bar instead\")\nfun foo() {}",
            expect![[r#"
                @deprecated("use bar instead")
                fun foo() {}"#]],
        );
    }

    #[test]
    fn test_annotation_with_multiple_arguments() {
        check(
            "@custom(1, \"hello\", true)\nfun foo() {}",
            expect![[r#"
                @custom(1, "hello", true)
                fun foo() {}"#]],
        );
    }

    #[test]
    fn test_annotation_empty_args() {
        check(
            "@deprecated()\nfun foo() {}",
            expect![[r#"
                @deprecated()
                fun foo() {}"#]],
        );
    }

    #[test]
    fn test_method_declaration() {
        check(
            "fun int.add(other: int): int { return self + other; }",
            expect![[r#"
                fun int.add(other: int): int {
                    return self + other;
                }"#]],
        );
    }

    #[test]
    fn test_method_declaration_with_type_parameters() {
        check(
            "fun int.add<T>(other: T): int | T { return self + other; }",
            expect![[r#"
                fun int.add<T>(other: T): int | T {
                    return self + other;
                }"#]],
        );
    }

    #[test]
    fn test_method_with_annotations() {
        check(
            "@pure\nfun int.abs(): int { return self; }",
            expect![[r#"
                @pure
                fun int.abs(): int {
                    return self;
                }"#]],
        );
    }

    #[test]
    fn test_method_complex_receiver() {
        check(
            "fun [int, slice].first(): int { return self.0; }",
            expect![[r#"
                fun [int, slice].first(): int {
                    return self.0;
                }"#]],
        );
    }

    #[test]
    fn test_method_generics() {
        check(
            "fun map<K, V>.get(key: K): V? { return null; }",
            expect![[r#"
                fun map<K, V>.get(key: K): V? {
                    return null;
                }"#]],
        );
    }

    #[test]
    fn test_method_multiple_parameters() {
        check(
            "fun slice.concat(other: slice, separator: slice): slice { return self; }",
            expect![[r#"
                fun slice.concat(other: slice, separator: slice): slice {
                    return self;
                }"#]],
        );
    }

    #[test]
    fn test_method_no_parameters() {
        check(
            "fun int.double(): int { return self * 2; }",
            expect![[r#"
                fun int.double(): int {
                    return self * 2;
                }"#]],
        );
    }

    #[test]
    fn test_get_method_declaration() {
        check(
            "get fun balance(): int { return 0; }",
            expect![[r#"
                get fun balance(): int {
                    return 0;
                }"#]],
        );
    }

    #[test]
    fn test_get_method_without_fun() {
        check(
            "get balance(): int { return 0; }",
            expect![[r#"
                get fun balance(): int {
                    return 0;
                }"#]],
        );
    }

    #[test]
    fn test_get_method_with_annotations() {
        check(
            "@pure\nget fun value(): int { return 42; }",
            expect![[r#"
                @pure
                get fun value(): int {
                    return 42;
                }"#]],
        );
    }

    #[test]
    fn test_get_method_with_parameters() {
        check(
            "get fun item(index: int): slice? { return null; }",
            expect![[r#"
                get fun item(index: int): slice? {
                    return null;
                }"#]],
        );
    }

    #[test]
    fn test_get_method_declaration_with_builtin() {
        check(
            "get fun balance(): int builtin",
            expect![[r#"
                get fun balance(): int
                    builtin"#]],
        );
    }

    #[test]
    fn test_get_method_declaration_with_asm() {
        check(
            "get fun balance(): int asm \"NOP\"",
            expect![[r#"
                get fun balance(): int
                    asm "NOP""#]],
        );
    }

    #[test]
    fn test_asm_body() {
        check(
            "fun foo() asm \"NOP\";",
            expect![[r#"
                fun foo()
                    asm "NOP""#]],
        );
        check(
            "fun add(a: int, b: int) asm(a b -> 1) \"ADD\";",
            expect![[r#"
                fun add(a: int, b: int)
                    asm(a b -> 1) "ADD""#]],
        );
    }

    #[test]
    fn test_builtin_function() {
        check(
            "fun hash(): int builtin",
            expect![[r#"
                fun hash(): int
                    builtin"#]],
        );
    }

    #[test]
    fn test_complex_asm() {
        check(
            "fun complex(a: int, b: int, c: int) asm(a b c -> 1 2) \"TRIPLE\";",
            expect![[r#"
                fun complex(a: int, b: int, c: int)
                    asm(a b c -> 1 2) "TRIPLE""#]],
        );
    }

    #[test]
    fn test_method_asm() {
        check(
            "fun int.double() asm(self -> 1) \"DBL\";",
            expect![[r#"
                fun int.double()
                    asm(self -> 1) "DBL""#]],
        );
    }

    #[test]
    fn test_empty_statement() {
        check(";", expect![""]);
    }

    #[test]
    fn test_empty_function_body() {
        check(
            "fun empty() {}",
            expect![[r#"
                fun empty() {}"#]],
        );
    }

    #[test]
    fn test_semicolon_after_declaration() {
        check(
            "const x = 1;",
            expect![[r#"
                const x = 1;"#]],
        );
        check(
            "global y: int;",
            expect![[r#"
                global y: int;"#]],
        );
        check(
            "type T = int;",
            expect![[r#"
                type T = int;"#]],
        );
    }

    #[test]
    fn test_semicolon_optional() {
        check(
            "const x = 1",
            expect![[r#"
                const x = 1;"#]],
        );
    }

    #[test]
    fn test_complex_nesting() {
        check(
            "struct Outer { inner: map<int, slice>, data: [int, slice] }",
            expect![[r#"
                struct Outer {
                    inner: map<int, slice>
                    data: [int, slice]
                }"#]],
        );
    }

    #[test]
    fn test_mixed_declarations() {
        check(
            "tolk 0.6.0\nimport \"std\";global x: int;const y = 42;type T = int;struct S { f: T }enum E { A }fun f() {}fun int.m() {}get g() {}",
            expect![[r#"
                tolk 0.6.0

                import "std"

                global x: int;

                const y = 42;

                type T = int;

                struct S {
                    f: T
                }

                enum E {
                    A
                }

                fun f() {}

                fun int.m() {}

                get fun g() {}"#]],
        );
    }

    #[test]
    fn test_several_tolk_required_versions() {
        check(
            r#"
                tolk 1.0.1
                tolk 1.0.0"#,
            expect!["tolk 1.0.1"],
        );
    }

    #[test]
    fn test_tolk_required_version_after_imports() {
        check(
            r#"
                import "a"
                tolk 1.0.0"#,
            expect![[r#"
                tolk 1.0.0

                import "a""#]],
        );
    }

    #[test]
    fn test_tolk_required_version_after_decl() {
        check(
            r#"
                fun foo() {}
                tolk 1.0.0"#,
            expect![[r#"
                tolk 1.0.0

                fun foo() {}"#]],
        );
    }

    #[test]
    fn test_imports() {
        check(
            r#"
                import "a"
                import "b"
                import "c"
                fun foo() {}"#,
            expect![[r#"
                import "a"
                import "b"
                import "c"

                fun foo() {}"#]],
        );
    }

    #[test]
    fn test_imports_with_newlines() {
        check(
            r#"
                import "a"

                import "b"

                import "c"
    
                fun foo() {}"#,
            expect![[r#"
                import "a"

                import "b"

                import "c"

                fun foo() {}"#]],
        );
    }

    #[test]
    fn test_constants_without_newlines() {
        check(
            r#"
                const foo = 1
                const bar = 2
                "#,
            expect![[r#"
                const foo = 1;
                const bar = 2;"#]],
        );
    }

    #[test]
    fn test_constants_with_newlines() {
        check(
            r#"
                const foo = 1
            
                const bar = 2
                "#,
            expect![[r#"
                const foo = 1;

                const bar = 2;"#]],
        );
    }
}
