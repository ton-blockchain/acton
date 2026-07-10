#![allow(clippy::needless_raw_string_hashes)]

use super::case_tolk_definition;
use expect_test::expect;

#[test]
fn upstream_aliases_001_alias_method() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"type Alias = int;

fun Alias.add(self, a: int, b: int): int {
    return a + b;
}

fun test() {
    val alias: Alias = 10;
    <caret>alias.add(1, 2);
}"#,
        |_| {},
        expect![[r#"8:4 -> file:///fixture/main.tolk 7:8 resolved"#]],
    );
}

#[test]
fn upstream_aliases_002_alias_for_alias_with_method() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"type Alias = int;

fun Alias.add(self, a: int, b: int): int {
    return a + b;
}

type AliasForAlias = Alias;

fun test() {
    val alias: AliasForAlias = 10;
    <caret>alias.add(1, 2);
}"#,
        |_| {},
        expect![[r#"10:4 -> file:///fixture/main.tolk 9:8 resolved"#]],
    );
}

#[test]
fn upstream_aliases_003_alias_for_option_alias_with_method() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"type Alias = int;

fun Alias.add(self, a: int, b: int): int {
    return a + b;
}

type AliasForAlias = Alias?;

fun test() {
    val alias: AliasForAlias = 10;
    <caret>alias.add(1, 2);
}"#,
        |_| {},
        expect![[r#"10:4 -> file:///fixture/main.tolk 9:8 resolved"#]],
    );
}

#[test]
fn upstream_basic_004_function_resolve() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"fun add(a: int, b: int): int {
    return a + b;
}

fun test() {
    <caret>add(1, 2);
}"#,
        |_| {},
        expect![[r#"5:4 -> file:///fixture/main.tolk 0:4 resolved"#]],
    );
}

#[test]
fn upstream_basic_005_local_variable() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"fun test() {
    val a = 100;
    <caret>a;
}"#,
        |_| {},
        expect![[r#"2:4 -> file:///fixture/main.tolk 1:8 resolved"#]],
    );
}

#[test]
fn upstream_basic_006_local_variable_with_redef() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"fun test() {
    val a = 100;
    val a redef = 100;
    <caret>a;
}"#,
        |_| {},
        expect![[r#"3:4 -> file:///fixture/main.tolk 1:8 resolved"#]],
    );
}

#[test]
fn upstream_basic_007_local_variable_with_redef_resolving() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"fun test() {
    val a = 100;
    val <caret>a redef = 100;
    a;
}"#,
        |_| {},
        expect![[r#"2:8 -> file:///fixture/main.tolk 1:8 resolved"#]],
    );
}

#[test]
fn upstream_basic_008_local_variable_with_tuple() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"fun test() {
    val [a, b] = [100, true];
    <caret>a;
    <caret>b;
}"#,
        |_| {},
        expect![[r#"2:4 -> file:///fixture/main.tolk 1:9 resolved
3:4 -> file:///fixture/main.tolk 1:12 resolved"#]],
    );
}

#[test]
fn upstream_basic_009_local_variable_with_deep_tuple() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"fun test() {
    val [a, [[b, c], d]] = [];
    <caret>a;
    <caret>b;
    <caret>c;
    <caret>d;
}"#,
        |_| {},
        expect![[r#"2:4 -> file:///fixture/main.tolk 1:9 resolved
3:4 -> file:///fixture/main.tolk 1:14 resolved
4:4 -> file:///fixture/main.tolk 1:17 resolved
5:4 -> file:///fixture/main.tolk 1:21 resolved"#]],
    );
}

#[test]
fn upstream_basic_010_local_variable_with_tensor() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"fun test() {
    val (a, b) = (100, true);
    <caret>a;
    <caret>b;
}"#,
        |_| {},
        expect![[r#"2:4 -> file:///fixture/main.tolk 1:9 resolved
3:4 -> file:///fixture/main.tolk 1:12 resolved"#]],
    );
}

#[test]
fn upstream_basic_011_local_variable_from_parent_scope() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"fun test() {
    val a = 100;
    if (a == 10) {
        <caret>a;
    }
}"#,
        |_| {},
        expect![[r#"3:8 -> file:///fixture/main.tolk 1:8 resolved"#]],
    );
}

#[test]
fn upstream_basic_012_local_variable_from_other_scope() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"fun test() {
    if (true) {
        val a = 100;
        <caret>a;
    } else {
        <caret>a;
    }
}"#,
        |_| {},
        expect![[r#"3:8 -> file:///fixture/main.tolk 2:12 resolved
5:8 unresolved"#]],
    );
}

#[test]
fn upstream_basic_013_local_variable_before_declaration() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"fun test() {
    <caret>a;
    val a = 100;
}"#,
        |_| {},
        expect![[r#"1:4 unresolved"#]],
    );
}

#[test]
fn upstream_basic_014_variable_from_catch_clause() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"fun test() {
    try {

    } catch (error) {
        <caret>error;
    }
}"#,
        |_| {},
        expect![[r#"4:8 -> file:///fixture/main.tolk 3:13 resolved"#]],
    );
}

#[test]
fn upstream_basic_015_second_variable_from_catch_clause() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"fun test() {
    try {

    } catch (error, data) {
        <caret>data;
    }
}"#,
        |_| {},
        expect![[r#"4:8 -> file:///fixture/main.tolk 3:20 resolved"#]],
    );
}

#[test]
fn upstream_basic_016_variable_from_match_expression() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"fun test() {
    match (val res = 10) {
        10 => {
            <caret>res;
        }
    }
}"#,
        |_| {},
        expect![[r#"3:12 -> file:///fixture/main.tolk 1:15 resolved"#]],
    );
}

#[test]
fn upstream_basic_017_variable_from_match_expression_with_tuple() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"fun test() {
    match (val [res, res2] = [10, 20]) {
        10 => {
            <caret>res;
            <caret>res2;
        }
    }
}"#,
        |_| {},
        expect![[r#"3:12 -> file:///fixture/main.tolk 1:16 resolved
4:12 -> file:///fixture/main.tolk 1:21 resolved"#]],
    );
}

#[test]
fn upstream_basic_018_constant() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"const FOO = 100;

fun test() {
    <caret>FOO;
}"#,
        |_| {},
        expect![[r#"3:4 -> file:///fixture/main.tolk 0:6 resolved"#]],
    );
}

#[test]
fn upstream_basic_019_global_variable() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"global foo: int;

fun test() {
    <caret>foo;
}"#,
        |_| {},
        expect![[r#"3:4 -> file:///fixture/main.tolk 0:7 resolved"#]],
    );
}

#[test]
fn upstream_basic_020_global_variable_in_parameter_default_value() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"global foo: int;

fun test(param: int = <caret>foo) {
}"#,
        |_| {},
        expect![[r#"2:22 -> file:///fixture/main.tolk 0:7 resolved"#]],
    );
}

#[test]
fn upstream_basic_021_type_alias() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"type Int = int;

fun test(): <caret>Int {
}"#,
        |_| {},
        expect![[r#"2:12 -> file:///fixture/main.tolk 0:5 resolved"#]],
    );
}

#[test]
fn upstream_basic_022_struct() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Foo {}

fun test(): <caret>Foo {
}"#,
        |_| {},
        expect![[r#"2:12 -> file:///fixture/main.tolk 0:7 resolved"#]],
    );
}

#[test]
fn upstream_basic_023_function_parameters() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"fun test(param: int) {
    <caret>param;
}"#,
        |_| {},
        expect![[r#"1:4 -> file:///fixture/main.tolk 0:9 resolved"#]],
    );
}

#[test]
fn upstream_basic_024_function_parameters_in_get_methods() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"get fun test(param: int) {
    <caret>param;
}"#,
        |_| {},
        expect![[r#"1:4 -> file:///fixture/main.tolk 0:13 resolved"#]],
    );
}

#[test]
fn upstream_basic_025_method_self_parameter() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"fun int.test(self) {
    <caret>self;
}"#,
        |_| {},
        expect![[r#"1:4 -> file:///fixture/main.tolk 0:13 resolved"#]],
    );
}

#[test]
fn upstream_basic_026_function_type_parameters() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"fun Foo.test<T, U>(
    param: <caret>T,
): <caret>U {
}"#,
        |_| {},
        expect![[r#"1:11 -> file:///fixture/main.tolk 0:13 resolved
2:3 -> file:///fixture/main.tolk 0:16 resolved"#]],
    );
}

#[test]
fn upstream_basic_027_struct_type_parameters() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Foo<T> {
    field: <caret>T;
}"#,
        |_| {},
        expect![[r#"1:11 -> file:///fixture/main.tolk 0:11 resolved"#]],
    );
}

#[test]
fn upstream_basic_028_type_alias_type_parameters() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"type Generic<TName> = Foo<<caret>TName>;"#,
        |_| {},
        expect![[r#"0:26 -> file:///fixture/main.tolk 0:13 resolved"#]],
    );
}

#[test]
fn upstream_basic_029_get_method_call() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"get fun someData(): int {}

fun test() {
    <caret>someData();
}"#,
        |_| {},
        expect![[r#"3:4 -> file:///fixture/main.tolk 0:8 resolved"#]],
    );
}

#[test]
fn upstream_basic_030_function_and_type_with_the_same_name() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"type address = builtin;

fun address(): <caret>address {}

fun test() {
    <caret>address();
}"#,
        |_| {},
        expect![[r#"2:15 -> file:///fixture/main.tolk 0:5 resolved
5:4 -> file:///fixture/main.tolk 2:4 resolved"#]],
    );
}

#[test]
fn upstream_basic_031_function_and_type_with_the_same_name_2() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"type address = builtin;

fun address(): <caret>address {}

fun test() {
    val addr: <caret>address
        = <caret>address();
}"#,
        |_| {},
        expect![[r#"2:15 -> file:///fixture/main.tolk 0:5 resolved
5:14 -> file:///fixture/main.tolk 0:5 resolved
6:10 -> file:///fixture/main.tolk 2:4 resolved"#]],
    );
}

#[test]
fn upstream_basic_032_variable_shadowing_for_function() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"type address = builtin;

fun address(): <caret>address {}

fun test() {
    val address: address = <caret>address();

    <caret>address;

    val foo: <caret>address;
}"#,
        |_| {},
        expect![[r#"2:15 -> file:///fixture/main.tolk 0:5 resolved
5:27 -> file:///fixture/main.tolk 2:4 resolved
7:4 -> file:///fixture/main.tolk 5:8 resolved
9:13 -> file:///fixture/main.tolk 0:5 resolved"#]],
    );
}

#[test]
fn upstream_basic_033_resolve_keyword() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"<caret>fun test() {
}"#,
        |_| {},
        expect![[r#"0:0 unresolved"#]],
    );
}

#[test]
fn upstream_basic_034_asm_shuffle_arguments_resolve() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"@pure
fun builder.storeDict(mutate self, c: dict): self
    asm(
        <caret>c
        <caret>self
    ) "STDICT";"#,
        |_| {},
        expect![[r#"3:8 -> file:///fixture/main.tolk 1:35 resolved
4:8 -> file:///fixture/main.tolk 1:29 resolved"#]],
    );
}

#[test]
fn upstream_basic_035_do_while_resolving() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"fun main() {
    do {
        var a = 10;
    } while (<caret>a);
}"#,
        |_| {},
        expect![[r#"3:13 -> file:///fixture/main.tolk 2:12 resolved"#]],
    );
}

#[test]
fn upstream_basic_036_match_with_constant_expression() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"const FOO = 100;

fun main() {
    match (10) {
        <caret>FOO => {}
    }
}"#,
        |_| {},
        expect![[r#"4:8 -> file:///fixture/main.tolk 0:6 resolved"#]],
    );
}

#[test]
fn upstream_basic_037_match_with_constant_expression_and_type() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"const FOO = 100

type Foo = int

fun main() {
    match (10) {
        <caret>FOO => {}
        <caret>Foo => {}
    }
}"#,
        |_| {},
        expect![[r#"6:8 -> file:///fixture/main.tolk 0:6 resolved
7:8 -> file:///fixture/main.tolk 2:5 resolved"#]],
    );
}

#[test]
fn upstream_enums_038_enum_resolving() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"enum Color {
    Red = 10,
    Blue = 200 + 100,
}

fun main() {
    <caret>Color.Red;

    val a: <caret>Color = Color.Blue;
    match (a) {
        <caret>Color.Red => {}
        <caret>Color.Blue => {}
    }
}"#,
        |_| {},
        expect![[r#"6:4 -> file:///fixture/main.tolk 0:5 resolved
8:11 -> file:///fixture/main.tolk 0:5 resolved
10:8 -> file:///fixture/main.tolk 0:5 resolved
11:8 -> file:///fixture/main.tolk 0:5 resolved"#]],
    );
}

#[test]
fn upstream_enums_039_enum_member_resolving() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"enum Color {
    <caret>Red = 10,
    <caret>Blue = 200 + 100,
}

fun main() {
    Color.<caret>Red;

    val a: Color = Color.<caret>Blue;
    match (a) {
        Color.<caret>Red => {}
        Color.<caret>Blue => {}
    }
}"#,
        |_| {},
        expect![[r#"1:4 -> file:///fixture/main.tolk 1:4 resolved
2:4 -> file:///fixture/main.tolk 2:4 resolved
6:10 -> file:///fixture/main.tolk 1:4 resolved
8:25 -> file:///fixture/main.tolk 2:4 resolved
10:14 -> file:///fixture/main.tolk 1:4 resolved
11:14 -> file:///fixture/main.tolk 2:4 resolved"#]],
    );
}

#[test]
fn upstream_enums_040_enum_member_resolving_via_alias() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"enum ColorImpl {
    <caret>Red = 10,
    <caret>Blue = 200 + 100,
}

type Color = ColorImpl

fun main() {
    Color.<caret>Red;

    val a: Color = Color.<caret>Blue;
    match (a) {
        Color.<caret>Red => {}
        Color.<caret>Blue => {}
    }
}"#,
        |_| {},
        expect![[r#"1:4 -> file:///fixture/main.tolk 1:4 resolved
2:4 -> file:///fixture/main.tolk 2:4 resolved
8:10 -> file:///fixture/main.tolk 1:4 resolved
10:25 -> file:///fixture/main.tolk 2:4 resolved
12:14 -> file:///fixture/main.tolk 1:4 resolved
13:14 -> file:///fixture/main.tolk 2:4 resolved"#]],
    );
}

#[test]
fn upstream_enums_041_enum_static_method_resolving() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"enum Color {
    Red = 10,
    Blue = 200 + 100,
}

fun Color.max() {
    return Color.Red;
}

fun main() {
    Color.<caret>max();
}"#,
        |_| {},
        expect![[r#"10:10 -> file:///fixture/main.tolk 5:10 resolved"#]],
    );
}

#[test]
fn upstream_enums_042_enum_instance_method_resolving() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"enum Color {
    Red = 10,
    Blue = 200 + 100,
}

fun Color.isRed(self) {
    return self == Color.Red;
}

fun main(c: Color) {
    c.<caret>isRed();
}"#,
        |_| {},
        expect![[r#"10:6 -> file:///fixture/main.tolk 5:10 resolved"#]],
    );
}

#[test]
fn upstream_instance_methods_043_struct_instance_method() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Foo {}

fun Foo.bar(self) {}

fun test() {
    val foo: Foo = {};
    foo.<caret>bar();
}"#,
        |_| {},
        expect![[r#"6:8 -> file:///fixture/main.tolk 2:8 resolved"#]],
    );
}

#[test]
fn upstream_instance_methods_044_struct_instance_method_on_alias() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Foo {}

fun Foo.bar(self) {}

type Bar = Foo;

fun test() {
    val bar: Bar = {};
    bar.<caret>bar();
}"#,
        |_| {},
        expect![[r#"8:8 -> file:///fixture/main.tolk 2:8 resolved"#]],
    );
}

#[test]
fn upstream_instance_methods_045_struct_static_method_as_instance() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Foo {}

fun Foo.bar() {}

fun test() {
    val foo: Foo = {};
    foo.<caret>bar();
}"#,
        |_| {},
        expect![[r#"6:8 -> file:///fixture/main.tolk 2:8 resolved"#]],
    );
}

#[test]
fn upstream_instance_methods_046_struct_instance_method_chaining() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Foo {}

fun Foo.bar(self): self {}
fun Foo.baz(self): self {}
fun Foo.bad(self): self {}

fun test() {
    val foo: Foo = {};
    foo
        .<caret>bar()
        .<caret>baz()
        .<caret>bad();
}"#,
        |_| {},
        expect![[r#"9:9 -> file:///fixture/main.tolk 2:8 resolved
10:9 -> file:///fixture/main.tolk 3:8 resolved
11:9 -> file:///fixture/main.tolk 4:8 resolved"#]],
    );
}

#[test]
fn upstream_instance_methods_047_structs_instance_method_chaining() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Foo {}
struct Baz {}
struct Bad {}

fun Foo.bar(self): Baz {}
fun Baz.baz(self): Bad {}
fun Bad.bad(self): Foo {}

fun test() {
    val foo: Foo = {};
    foo
        .<caret>bar()
        .<caret>baz()
        .<caret>bad();
}"#,
        |_| {},
        expect![[r#"11:9 -> file:///fixture/main.tolk 4:8 resolved
12:9 -> file:///fixture/main.tolk 5:8 resolved
13:9 -> file:///fixture/main.tolk 6:8 resolved"#]],
    );
}

#[test]
fn upstream_instance_methods_048_type_alias_instance_method() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"type Int = int;

fun Int.bar(self) {}

fun test() {
    val val: Int = 10;
    val.<caret>bar();
}"#,
        |_| {},
        expect![[r#"6:8 -> file:///fixture/main.tolk 6:8 resolved"#]],
    );
}

#[test]
fn upstream_instance_methods_049_struct_instance_method_inside_other_instance_method_via_sel() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Foo {}

fun Foo.baz(self) {}
fun Foo.bar(self) {
    self.<caret>baz();
}"#,
        |_| {},
        expect![[r#"4:9 -> file:///fixture/main.tolk 2:8 resolved"#]],
    );
}

#[test]
fn upstream_instance_methods_050_struct_instance_method_via_optional_with_null_init() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Foo {}

fun Foo.bar(self) {}

fun test() {
    val foo: Foo? = null;
    foo.<caret>bar(); // unresolved since foo has null type actually
}"#,
        |_| {},
        expect![[r#"6:8 unresolved"#]],
    );
}

#[test]
fn upstream_instance_methods_051_struct_instance_method_via_optional() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Foo {}

fun Foo.bar(self) {}

fun test(cond: bool) {
    val foo: Foo? = cond ? Foo {} : null;
    foo.<caret>bar();
}"#,
        |_| {},
        expect![[r#"6:8 unresolved"#]],
    );
}

#[test]
fn upstream_instance_methods_052_struct_instance_method_via_optional_with_not_null_operator() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Foo {}

fun Foo.bar(self) {}

fun test(cond: bool) {
    val foo: Foo? = cond ? Foo {} : null;
    foo!.<caret>bar();
}"#,
        |_| {},
        expect![[r#"6:9 -> file:///fixture/main.tolk 2:8 resolved"#]],
    );
}

#[test]
fn upstream_instance_methods_053_generic_struct_instance_method() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Foo<T> {}

fun Foo<T>.bar(self) {}

fun test() {
    val foo: Foo<int> = {};
    foo.<caret>bar();
}"#,
        |_| {},
        expect![[r#"6:8 -> file:///fixture/main.tolk 2:11 resolved"#]],
    );
}

#[test]
fn upstream_instance_methods_054_generic_struct_instance_with_specific_type_method() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Foo<T> {}

fun Foo<int>.bar(self) {}

fun test() {
    val foo: Foo<int> = {};
    foo.<caret>bar();
}"#,
        |_| {},
        expect![[r#"6:8 -> file:///fixture/main.tolk 2:13 resolved"#]],
    );
}

#[test]
fn upstream_instance_methods_055_generic_struct_instance_with_different_specific_type_method() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Foo<T> {}

fun Foo<bool>.bar(self) {}

fun test() {
    val foo: Foo<int> = {};
    foo.<caret>bar();
}"#,
        |_| {},
        expect![[r#"6:8 unresolved"#]],
    );
}

#[test]
fn upstream_instance_methods_056_tocell_method() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Foo {}

fun test() {
    val foo: Foo = {};
    foo.<caret>toCell();
}"#,
        |_| {},
        expect![[r#"4:8 -> file:///__tolk_stdlib__/common.tolk 473:6 resolved"#]],
    );
}

#[test]
fn upstream_instance_methods_057_generic_struct_alias_method_resolving() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Foo<T> {
    value: T,
}

type IntFoo = Foo<int>;

fun IntFoo.someMethod(self) {
    throw 4;
}

fun Foo<T>.someMethod(self) {
    throw 3;
}

fun Foo<int>.someMethod(self) {
    throw 2;
}

fun T.someMethod(self) {
    throw 1;
}

fun main(a: Foo<slice>, b: Foo<int>, c: int, d: IntFoo): void {
    a.<caret>someMethod();
    b.<caret>someMethod();
    c.<caret>someMethod();
    d.<caret>someMethod();
}"#,
        |_| {},
        expect![[r#"23:6 -> file:///fixture/main.tolk 10:11 resolved
24:6 unresolved
25:6 -> file:///fixture/main.tolk 18:6 resolved
26:6 unresolved"#]],
    );
}

#[test]
fn upstream_instance_methods_058_generic_struct_method_resolving() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Foo<T> {
    value: T,
}

fun Foo<T>.someMethod(self) {
    throw 3;
}

fun Foo<int>.someMethod(self) {
    throw 2;
}

fun T.someMethod(self) {
    throw 1;
}

fun main(a: Foo<slice>, b: Foo<int>, c: int): void {
    a.<caret>someMethod();
    b.<caret>someMethod();
    c.<caret>someMethod();
}"#,
        |_| {},
        expect![[r#"17:6 -> file:///fixture/main.tolk 4:11 resolved
18:6 -> file:///fixture/main.tolk 8:13 resolved
19:6 -> file:///fixture/main.tolk 12:6 resolved"#]],
    );
}

#[test]
fn upstream_instance_methods_059_tuple_method_resolving() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"fun [int, int].someMethod(self) {
    throw 3;
}

fun [T, int].someMethod(self) {
    throw 2;
}

fun T.someMethod(self) {
    throw 1;
}

fun main(a: [int, int], b: [bool, int], c: [bool]): void {
    a.<caret>someMethod();
    b.<caret>someMethod();
    c.<caret>someMethod();
}"#,
        |_| {},
        expect![[r#"13:6 -> file:///fixture/main.tolk 0:15 resolved
14:6 -> file:///fixture/main.tolk 4:13 resolved
15:6 -> file:///fixture/main.tolk 8:6 resolved"#]],
    );
}

#[test]
fn upstream_instance_methods_060_type_alias_method_resolving() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"fun dict.someMethod(self) {
    throw 1;
}

fun main(a: dict, b: cell?): void {
    a.<caret>someMethod();
    b.<caret>someMethod();
}"#,
        |_| {},
        expect![[r#"5:6 -> file:///fixture/main.tolk 0:9 resolved
6:6 -> file:///fixture/main.tolk 0:9 resolved"#]],
    );
}

#[test]
fn upstream_instance_methods_061_instance_method_for_type_with_same_function_name() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"type someType = int

fun someType.bar() {}

fun someType() {}

fun main(): void {
    <caret>someType
        .<caret>bar();

    <caret>someType();
}"#,
        |_| {},
        expect![[r#"7:4 -> file:///fixture/main.tolk 0:5 resolved
8:9 -> file:///fixture/main.tolk 2:13 resolved
10:4 -> file:///fixture/main.tolk 4:4 resolved"#]],
    );
}

#[test]
fn upstream_instance_methods_062_instance_methods_for_generic() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct First<T> {}
fun First<T>.new(): First<T> {}

struct Second<T> {}
fun Second<T>.new(): Second<T> {}
fun Second<int>.new(): Second<int> {}

fun main() {
    val first = First<int>.<caret>new();
    val second = Second<int>.<caret>new();
    val third = Second<slice>.<caret>new();
}"#,
        |_| {},
        expect![[r#"8:27 -> file:///fixture/main.tolk 1:13 resolved
9:29 -> file:///fixture/main.tolk 5:16 resolved
10:30 -> file:///fixture/main.tolk 4:14 resolved"#]],
    );
}

#[test]
fn upstream_lambdas_063_lambda_parameter_without_type() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"fun test() {
    fun (a) {
        <caret>a;
    };
}"#,
        |_| {},
        expect![[r#"2:8 -> file:///fixture/main.tolk 1:9 resolved"#]],
    );
}

#[test]
fn upstream_lambdas_064_lambda_parameter_with_type() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"fun test() {
    fun (a: int) {
        <caret>a;
    };
}"#,
        |_| {},
        expect![[r#"2:8 -> file:///fixture/main.tolk 1:9 resolved"#]],
    );
}

#[test]
fn upstream_lambdas_065_nested_lambda_parameter() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"fun test() {
    fun (a: int) {
        fun (a: int) {
            <caret>a;
        };
    };
}"#,
        |_| {},
        expect![[r#"3:12 -> file:///fixture/main.tolk 2:13 resolved"#]],
    );
}

#[test]
fn upstream_static_methods_066_struct_static_method() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Foo {}

fun Foo.bar() {}

fun test() {
    Foo.<caret>bar();
}"#,
        |_| {},
        expect![[r#"5:8 -> file:///fixture/main.tolk 2:8 resolved"#]],
    );
}

#[test]
fn upstream_static_methods_067_struct_instance_method_as_static() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Foo {}

fun Foo.bar(self) {}

fun test() {
    Foo.<caret>bar();
}"#,
        |_| {},
        expect![[r#"5:8 -> file:///fixture/main.tolk 2:8 resolved"#]],
    );
}

#[test]
fn upstream_static_methods_068_type_alias_static_method() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"type Foo = int;

fun Foo.bar() {}

fun test() {
    Foo.<caret>bar();
}"#,
        |_| {},
        expect![[r#"5:8 -> file:///fixture/main.tolk 2:8 resolved"#]],
    );
}

#[test]
fn upstream_static_methods_069_builtin_type_static_method() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"fun int.bar() {}

fun test() {
    int.<caret>bar();
}"#,
        |_| {},
        expect![[r#"3:8 -> file:///fixture/main.tolk 0:8 resolved"#]],
    );
}

#[test]
fn upstream_static_methods_070_generic_struct_static_method() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Generic<T> {}

fun Generic<T>.foo() {}

fun test() {
    Generic<int>.<caret>foo({});
}"#,
        |_| {},
        expect![[r#"5:17 -> file:///fixture/main.tolk 2:15 resolved"#]],
    );
}

#[test]
fn upstream_static_methods_071_instantiated_generic_static_method() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Foo<T> {}

fun Foo<T>.generic(value: T): T {}

fun test() {
    Foo<int>.<caret>generic(10);
}"#,
        |_| {},
        expect![[r#"5:13 -> file:///fixture/main.tolk 2:11 resolved"#]],
    );
}

#[test]
fn upstream_static_methods_072_generic_type_alias_static_method() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Generic<T> {}

type Alias<T> = Generic<T>

fun Alias<T>.foo() {}

fun test() {
    Alias<int>.<caret>foo({});
}"#,
        |_| {},
        expect![[r#"7:15 -> file:///fixture/main.tolk 4:13 resolved"#]],
    );
}

#[test]
fn upstream_static_methods_073_int8_static_method() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"fun int.calcExactF(self) { return self; }
fun int8.calcExactF(self) { return self * 2; }
fun int8?.calcExactF(self) { return self == null ? 0 : self * 3; }

fun main() {
    int8.<caret>calcExactF(x)
}"#,
        |_| {},
        expect![[r#"5:9 -> file:///fixture/main.tolk 1:9 resolved"#]],
    );
}

#[test]
fn upstream_static_methods_074_same_name_methods_for_several_generic_structs() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Pair {}

fun Pair<A,B>.createFrom<U,V>(first: U, second: V): Pair<A,B> {}

struct Wrapper<T> { item: T; }

fun Wrapper<T>.createFrom<U>(item: U): Wrapper<T> { return {item}; }

fun test18() {
    Wrapper<int?>.<caret>createFrom<int8>;
}"#,
        |_| {},
        expect![[r#"9:18 -> file:///fixture/main.tolk 6:15 resolved"#]],
    );
}

#[test]
fn upstream_struct_fields_075_struct_instance_field_resolving() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Data {
    value: int,
}

fun test() {
    Data { <caret>value: 10 };
}"#,
        |_| {},
        expect![[r#"5:11 -> file:///fixture/main.tolk 1:4 resolved"#]],
    );
}

#[test]
fn upstream_struct_fields_076_struct_instance_field_resolving_with_alias() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Data {
    value: int,
}

type DataAlias = Data;

fun test() {
    DataAlias { <caret>value: 10 };
}"#,
        |_| {},
        expect![[r#"7:16 -> file:///fixture/main.tolk 1:4 resolved"#]],
    );
}

#[test]
fn upstream_struct_fields_077_struct_instance_field_resolving_in_variable_declaration() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Data {
    value: int,
}

fun test() {
    val data: Data = { <caret>value: 10 };
}"#,
        |_| {},
        expect![[r#"5:23 -> file:///fixture/main.tolk 1:4 resolved"#]],
    );
}

#[test]
fn upstream_struct_fields_078_struct_instance_field_resolving_in_variable_declaration_with_a() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Data {
    value: int,
}

type DataAlias = Data;

fun test() {
    val data: DataAlias = { <caret>value: 10 };
}"#,
        |_| {},
        expect![[r#"7:28 -> file:///fixture/main.tolk 1:4 resolved"#]],
    );
}

#[test]
fn upstream_struct_fields_079_struct_instance_field_resolving_in_function_call() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Data {
    value: int,
}

fun takeData(data: Data) {}

fun test() {
    takeData({ <caret>value: 10 });
}"#,
        |_| {},
        expect![[r#"7:15 -> file:///fixture/main.tolk 1:4 resolved"#]],
    );
}

#[test]
fn upstream_struct_fields_080_struct_instance_field_resolving_in_function_call_with_alias() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Data {
    value: int,
}

fun takeData(data: Data) {}

fun test() {
    takeData({ <caret>value: 10 });
}"#,
        |_| {},
        expect![[r#"7:15 -> file:///fixture/main.tolk 1:4 resolved"#]],
    );
}

#[test]
fn upstream_struct_fields_081_struct_instance_field_resolving_in_static_method_call() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Data {
    value: int,
}

fun Data.takeData(data: Data) {}

fun test() {
    Data.takeData({ <caret>value: 10 });
}"#,
        |_| {},
        expect![[r#"7:20 -> file:///fixture/main.tolk 1:4 resolved"#]],
    );
}

#[test]
fn upstream_struct_fields_082_struct_instance_field_resolving_in_instance_method_call() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Data {
    value: int,
}

type Some = int;

fun Some.takeData(self, data: Data) {}

fun test() {
    val some: Some = 10;
    some.takeData({ <caret>value: 10 });
}"#,
        |_| {},
        expect![[r#"10:20 -> file:///fixture/main.tolk 1:4 resolved"#]],
    );
}

#[test]
fn upstream_struct_fields_083_struct_instance_field_resolving_in_instance_method_call_2() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Data {
    value: int,
    data: slice,
}

type Some = int;

fun Some.takeData(self, value: int, data: Data, data2: Data) {}

fun test() {
    val some: Some = 10;
    some.takeData(
        10,
        { <caret>value: 10 },
        { <caret>data: 10 }
    );
}"#,
        |_| {},
        expect![[r#"13:10 -> file:///fixture/main.tolk 1:4 resolved
14:10 -> file:///fixture/main.tolk 2:4 resolved"#]],
    );
}

#[test]
fn upstream_struct_fields_084_struct_field_access() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Data {
    value: int,
}

fun test() {
    val data = Data { value: 10 };
    data.<caret>value;
}"#,
        |_| {},
        expect![[r#"6:9 -> file:///fixture/main.tolk 1:4 resolved"#]],
    );
}

#[test]
fn upstream_struct_fields_085_struct_field_access_via_alias() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Data {
    value: int,
}

type MyData = Data;

fun test() {
    val data: MyData = { value: 10 };
    data.<caret>value;
}"#,
        |_| {},
        expect![[r#"8:9 -> file:///fixture/main.tolk 1:4 resolved"#]],
    );
}

#[test]
fn upstream_struct_fields_086_struct_field_access_via_self() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Data {
    value: int;
}

fun Data.bar(self) {
    self.<caret>value;
}"#,
        |_| {},
        expect![[r#"5:9 -> file:///fixture/main.tolk 1:4 resolved"#]],
    );
}

#[test]
fn upstream_struct_fields_087_struct_instance_field_resolving_for_generic_struct() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Data<T> {
    value: T,
}

fun test() {
    Data<int> { <caret>value: 10 };
}"#,
        |_| {},
        expect![[r#"5:16 -> file:///fixture/main.tolk 1:4 resolved"#]],
    );
}

#[test]
fn upstream_struct_fields_088_struct_instance_field_resolving_for_generic_struct_alias() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Data<T> {
    value: T,
}

type IntData = Data<int>;

fun test() {
    IntData { <caret>value: 10 };
}"#,
        |_| {},
        expect![[r#"7:14 -> file:///fixture/main.tolk 1:4 resolved"#]],
    );
}

#[test]
fn upstream_struct_fields_089_struct_field_resolving_for_generic_struct() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Data<T> {
    value: T,
}

fun test() {
    val data: Data<int> = {};
    data.<caret>value;
}"#,
        |_| {},
        expect![[r#"6:9 -> file:///fixture/main.tolk 1:4 resolved"#]],
    );
}

#[test]
fn upstream_struct_fields_090_struct_field_access_resolving_for_generic_struct_alias() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Data<T> {
    value: T,
}

type IntData = Data<int>;

fun test() {
    val data: IntData = {};
    data.<caret>value;
}"#,
        |_| {},
        expect![[r#"8:9 -> file:///fixture/main.tolk 1:4 resolved"#]],
    );
}

#[test]
fn upstream_struct_fields_091_struct_instance_field_resolving_for_return_statement_inside_fu() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Data {
    value: int,
}

fun test(): Data {
    return {
        <caret>value: 10;
    }
}"#,
        |_| {},
        expect![[r#"6:8 -> file:///fixture/main.tolk 1:4 resolved"#]],
    );
}

#[test]
fn upstream_struct_fields_092_struct_instance_field_resolving_for_return_statement_inside_me() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Data {
    value: int,
}

fun int.test(): Data {
    return {
        <caret>value: 10;
    }
}"#,
        |_| {},
        expect![[r#"6:8 -> file:///fixture/main.tolk 1:4 resolved"#]],
    );
}

#[test]
fn upstream_struct_fields_093_struct_instance_field_resolving_for_return_statement_inside_ge() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Data {
    value: int,
}

get fun test(): Data {
    return {
        <caret>value: 10;
    }
}"#,
        |_| {},
        expect![[r#"6:8 -> file:///fixture/main.tolk 1:4 resolved"#]],
    );
}

#[test]
fn upstream_struct_fields_094_struct_instance_field_resolving_for_return_statement_and_alias() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Data {
    value: int,
}

type IntData = Data;

fun test(): IntData {
    return {
        <caret>value: 10;
    }
}"#,
        |_| {},
        expect![[r#"8:8 -> file:///fixture/main.tolk 1:4 resolved"#]],
    );
}

#[test]
fn upstream_struct_fields_095_struct_instance_field_resolving_for_return_statement_and_alias() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Data<T> {
    value: T,
}

type IntData = Data<int>;

fun test(): IntData {
    return {
        <caret>value: 10;
    }
}"#,
        |_| {},
        expect![[r#"8:8 -> file:///fixture/main.tolk 1:4 resolved"#]],
    );
}

#[test]
fn upstream_struct_fields_096_struct_instance_field_resolving_for_field_init() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Data {
    value: int,
}

struct Other {
    data: Data
}

fun test() {
    Other {
       data: { <caret>value: 10 }
    }
}"#,
        |_| {},
        expect![[r#"10:15 -> file:///fixture/main.tolk 1:4 resolved"#]],
    );
}

#[test]
fn upstream_struct_fields_097_struct_instance_field_resolving_for_field_init_with_alias() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Data {
    value: int,
}

type DataAlias = Data;

struct Other {
    data: DataAlias
}

fun test() {
    Other {
       data: { <caret>value: 10 }
    }
}"#,
        |_| {},
        expect![[r#"12:15 -> file:///fixture/main.tolk 1:4 resolved"#]],
    );
}

#[test]
fn upstream_struct_fields_098_struct_instance_field_resolving_for_field_init_with_generic() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Data<T> {
    value: T,
}

struct Other {
    data: Data<int>
}

fun test() {
    Other {
       data: { <caret>value: 10 }
    }
}"#,
        |_| {},
        expect![[r#"10:15 -> file:///fixture/main.tolk 1:4 resolved"#]],
    );
}

#[test]
fn upstream_struct_fields_099_struct_instance_field_resolving_for_field_init_with_alias_to_g() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Data<T> {
    value: T,
}

type IntData = Data<int>;

struct Other {
    data: IntData
}

fun test() {
    Other {
       data: { <caret>value: 10 }
    }
}"#,
        |_| {},
        expect![[r#"12:15 -> file:///fixture/main.tolk 1:4 resolved"#]],
    );
}

#[test]
fn upstream_struct_fields_100_struct_instance_field_resolving_for_field_init_with_union_type() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Data {
    value: int,
}

struct Other {
    data: Data | slice
}

fun test() {
    Other {
       data: { <caret>value: 10 }
    }
}"#,
        |_| {},
        expect![[r#"10:15 -> file:///fixture/main.tolk 1:4 resolved"#]],
    );
}

#[test]
fn upstream_struct_fields_101_struct_instance_field_resolving_for_field_init_with_generic_an() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Data<T> {
    value: int,
}

struct Other {
    data: Data<int> | slice
}

fun test() {
    Other {
       data: { <caret>value: 10 }
    }
}"#,
        |_| {},
        expect![[r#"10:15 -> file:///fixture/main.tolk 1:4 resolved"#]],
    );
}

#[test]
fn upstream_struct_fields_102_struct_instance_field_resolving_for_field_init_with_generic_al() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Data<T> {
    value: int,
}

type DataInt = Data<int>;

struct Other {
    data: DataInt | slice
}

fun test() {
    Other {
       data: { <caret>value: 10 }
    }
}"#,
        |_| {},
        expect![[r#"12:15 -> file:///fixture/main.tolk 1:4 resolved"#]],
    );
}

#[test]
fn upstream_struct_fields_103_struct_instance_field_resolving_for_field_init_with_generic_op() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Data<T> {
    value: int,
}

type DataInt = Data<int>?;

struct Other {
    data: DataInt | slice
}

fun test() {
    Other {
       data: { <caret>value: 10 }
    }
}"#,
        |_| {},
        expect![[r#"12:15 -> file:///fixture/main.tolk 1:4 resolved"#]],
    );
}

#[test]
fn upstream_struct_fields_104_struct_instance_as_argument_of_instance_method_called_as_stati() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Foo {
    value: int,
}

fun Foo.name(self): self {}

fun main() {
    val foo: Foo = {};
    val res = Foo.name({
        <caret>value: 10,
    });
}"#,
        |_| {},
        expect![[r#"9:8 -> file:///fixture/main.tolk 1:4 resolved"#]],
    );
}

#[test]
fn upstream_struct_fields_105_struct_instance_short_field_resolving_with_parameter() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Data {
    value: int,
}

fun test(value: int) {
    Data {
        <caret>value
    }
}"#,
        |_| {},
        expect![[r#"6:8 -> file:///fixture/main.tolk 1:4 resolved
6:8 -> file:///fixture/main.tolk 4:9 resolved"#]],
    );
}

#[test]
fn upstream_struct_fields_106_struct_instance_short_field_resolving_with_variable() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Data {
    value: int,
}

fun test() {
    val value = 0;
    Data {
        <caret>value
    }
}"#,
        |_| {},
        expect![[r#"7:8 -> file:///fixture/main.tolk 1:4 resolved
7:8 -> file:///fixture/main.tolk 5:8 resolved"#]],
    );
}

#[test]
fn upstream_struct_fields_107_struct_instance_full_field_resolving_with_parameter() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Data {
    value: int,
}

fun test(value: int) {
    Data {
        <caret>value:
            <caret>value
    }
}"#,
        |_| {},
        expect![[r#"6:8 -> file:///fixture/main.tolk 1:4 resolved
7:12 -> file:///fixture/main.tolk 4:9 resolved"#]],
    );
}

#[test]
fn upstream_struct_fields_108_struct_instance_full_field_resolving_with_variable() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Data {
    value: int,
}

fun test() {
    val value = 0;
    Data {
        <caret>value:
            <caret>value
    }
}"#,
        |_| {},
        expect![[r#"7:8 -> file:///fixture/main.tolk 1:4 resolved
8:12 -> file:///fixture/main.tolk 5:8 resolved"#]],
    );
}

#[test]
fn upstream_type_parameters_109_receiver_type_parameters() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Foo<T> {}

fun Foo<TName>.foo(): <caret>TName {}"#,
        |_| {},
        expect![[r#"2:22 -> file:///fixture/main.tolk 2:8 resolved"#]],
    );
}

#[test]
fn upstream_type_parameters_110_default_type_for_function_type_parameter() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"fun foo<T=<caret>int>() {}"#,
        |_| {},
        expect![[r#"0:10 -> file:///__tolk_stdlib__/common.tolk 10:5 resolved"#]],
    );
}

#[test]
fn upstream_type_parameters_111_default_type_for_struct_type_parameter() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"struct Foo<T=<caret>int> {}"#,
        |_| {},
        expect![[r#"0:13 -> file:///__tolk_stdlib__/common.tolk 10:5 resolved"#]],
    );
}

#[test]
fn upstream_type_parameters_112_default_type_for_type_alias_type_parameter() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"type Foo<T=<caret>int> = T | null;"#,
        |_| {},
        expect![[r#"0:11 -> file:///__tolk_stdlib__/common.tolk 10:5 resolved"#]],
    );
}

#[test]
fn upstream_type_parameters_113_t_receiver() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"fun T.foo(): <caret>T {}"#,
        |_| {},
        expect![[r#"0:13 -> file:///fixture/main.tolk 0:4 resolved"#]],
    );
}

#[test]
fn upstream_type_parameters_114_t_receiver_from_decl() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r#"fun <caret>T.foo(): T {}"#,
        |_| {},
        expect![[r#"0:4 -> file:///fixture/main.tolk 0:4 resolved"#]],
    );
}
