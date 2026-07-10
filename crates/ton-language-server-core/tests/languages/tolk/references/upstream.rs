#![allow(clippy::needless_raw_string_hashes)]

use super::case_tolk_references;
use expect_test::expect;

#[test]
fn upstream_basic_001_local_variable_references() {
    case_tolk_references(
        "file:///fixture/main.tolk",
        r#"fun test() {
    val <caret>num = 100;
    if (num == 10) {
        throw num;
    }
}"#,
        false,
        |_| {},
        expect![[r#"1:8 -> file:///fixture/main.tolk 2:8 reference
1:8 -> file:///fixture/main.tolk 3:14 reference"#]],
    );
}

#[test]
fn upstream_basic_002_local_backticked_variable_references() {
    case_tolk_references(
        "file:///fixture/main.tolk",
        r#"fun test() {
    val <caret>`hello world` = 100;
    if (`hello world` == 10) {
        throw `hello world`;
    }
}"#,
        false,
        |_| {},
        expect![[r#"1:8 -> file:///fixture/main.tolk 2:8 reference
1:8 -> file:///fixture/main.tolk 3:14 reference"#]],
    );
}

#[test]
fn upstream_basic_003_local_tuple_variable_references() {
    case_tolk_references(
        "file:///fixture/main.tolk",
        r#"fun test() {
    val [
        <caret>num,
        <caret>other
    ] = [100, 200];
    if (num == 10) {
        throw other;
    }
}"#,
        false,
        |_| {},
        expect![[r#"2:8 -> file:///fixture/main.tolk 5:8 reference
3:8 -> file:///fixture/main.tolk 6:14 reference"#]],
    );
}

#[test]
fn upstream_basic_004_local_variable_references_from_different_scopes() {
    case_tolk_references(
        "file:///fixture/main.tolk",
        r#"fun test() {
    {
        val <caret>num = 100;
        if (num == 10) {
            throw num;
        }
    }

    {
        val <caret>num = 500;
        if (num == 100) {
            throw num;
        }
    }
}"#,
        false,
        |_| {},
        expect![[r#"2:12 -> file:///fixture/main.tolk 3:12 reference
2:12 -> file:///fixture/main.tolk 4:18 reference
9:12 -> file:///fixture/main.tolk 10:12 reference
9:12 -> file:///fixture/main.tolk 11:18 reference"#]],
    );
}

#[test]
fn upstream_basic_005_local_variable_with_redef_references() {
    case_tolk_references(
        "file:///fixture/main.tolk",
        r#"fun test() {
    val <caret>a = 100;
    val a redef = 100;
    a;
}"#,
        false,
        |_| {},
        expect![[r#"1:8 -> file:///fixture/main.tolk 2:8 reference
1:8 -> file:///fixture/main.tolk 3:4 reference"#]],
    );
}

#[test]
fn upstream_basic_006_local_variable_with_redef_references_from_redef_itself() {
    case_tolk_references(
        "file:///fixture/main.tolk",
        r#"fun test() {
    val a = 100;
    val <caret>a redef = 100;
    a;
}"#,
        false,
        |_| {},
        expect![[r#"2:8 -> file:///fixture/main.tolk 2:8 reference
2:8 -> file:///fixture/main.tolk 3:4 reference"#]],
    );
}

#[test]
fn upstream_basic_007_local_variable_references_to_struct_init_short_field() {
    case_tolk_references(
        "file:///fixture/main.tolk",
        r#"struct Foo {
    value: int,
}

fun test() {
    val <caret>value = 100;
    Foo { value };
}"#,
        false,
        |_| {},
        expect![[r#"5:8 -> file:///fixture/main.tolk 6:10 reference"#]],
    );
}

#[test]
fn upstream_basic_008_catch_variable_references() {
    case_tolk_references(
        "file:///fixture/main.tolk",
        r#"fun test() {
    try {} catch (<caret>error) {
        val e = error as int;
        if (e == 10) {
            throw e;
        }
    }
}"#,
        false,
        |_| {},
        expect![[r#"1:18 -> file:///fixture/main.tolk 2:16 reference"#]],
    );
}

#[test]
fn upstream_basic_009_second_catch_variable_references() {
    case_tolk_references(
        "file:///fixture/main.tolk",
        r#"fun test() {
    try {} catch (error, <caret>data) {
        val e = data as int;
        if (e == 10) {
            throw e;
        }
    }
}"#,
        false,
        |_| {},
        expect![[r#"1:25 -> file:///fixture/main.tolk 2:16 reference"#]],
    );
}

#[test]
fn upstream_basic_010_parameter_references() {
    case_tolk_references(
        "file:///fixture/main.tolk",
        r#"fun test(<caret>foo: int) {
    if (foo == 10) {
        throw foo;
    }
}"#,
        false,
        |_| {},
        expect![[r#"0:9 -> file:///fixture/main.tolk 1:8 reference
0:9 -> file:///fixture/main.tolk 2:14 reference"#]],
    );
}

#[test]
fn upstream_basic_011_parameter_references_inside_instance_method() {
    case_tolk_references(
        "file:///fixture/main.tolk",
        r#"fun int.test(self, <caret>foo: int) {
    assert(self != 0) throw 12;

    if (foo == 10) {
        throw foo;
    }
}"#,
        false,
        |_| {},
        expect![[r#"0:19 -> file:///fixture/main.tolk 3:8 reference
0:19 -> file:///fixture/main.tolk 4:14 reference"#]],
    );
}

#[test]
fn upstream_basic_012_parameter_references_to_struct_init_short_field() {
    case_tolk_references(
        "file:///fixture/main.tolk",
        r#"struct Foo {
    value: int,
}

fun test(<caret>value: int) {
    Foo { value };
}"#,
        false,
        |_| {},
        expect![[r#"4:9 -> file:///fixture/main.tolk 5:10 reference"#]],
    );
}

#[test]
fn upstream_basic_013_global_variable_references() {
    case_tolk_references(
        "file:///fixture/main.tolk",
        r#"global <caret>foo: int = 100;

fun test() {
    if (foo == 10) {
        throw foo;
    }
}

fun test2() {
    if (foo == 100) {
        throw foo + 200;
    }
}"#,
        false,
        |_| {},
        expect![[r#"0:7 -> file:///fixture/main.tolk 3:8 reference
0:7 -> file:///fixture/main.tolk 4:14 reference
0:7 -> file:///fixture/main.tolk 9:8 reference
0:7 -> file:///fixture/main.tolk 10:14 reference"#]],
    );
}

#[test]
fn upstream_basic_014_function_references() {
    case_tolk_references(
        "file:///fixture/main.tolk",
        r#"fun <caret>test() {}

fun test2() {
    test();
    test();
    test();
}"#,
        false,
        |_| {},
        expect![[r#"0:4 -> file:///fixture/main.tolk 3:4 reference
0:4 -> file:///fixture/main.tolk 4:4 reference
0:4 -> file:///fixture/main.tolk 5:4 reference"#]],
    );
}

#[test]
fn upstream_basic_015_static_method_references() {
    case_tolk_references(
        "file:///fixture/main.tolk",
        r#"struct Foo {}

fun Foo.<caret>test() {}

fun test2() {
    Foo.test();
}"#,
        false,
        |_| {},
        expect![[r#"2:8 -> file:///fixture/main.tolk 5:8 reference"#]],
    );
}

#[test]
fn upstream_basic_016_instance_method_references() {
    case_tolk_references(
        "file:///fixture/main.tolk",
        r#"struct Foo {}

fun Foo.<caret>test(self) {}

fun test2() {
    val foo = Foo {};
    foo.test();
}"#,
        false,
        |_| {},
        expect![[r#"2:8 -> file:///fixture/main.tolk 6:8 reference"#]],
    );
}

#[test]
fn upstream_basic_017_instance_method_references_via_alias() {
    case_tolk_references(
        "file:///fixture/main.tolk",
        r#"struct Foo {}

fun Foo.<caret>test(self) {}

type FooAlias = Foo;

fun test2() {
    val foo: FooAlias = {};
    foo.test();
}"#,
        false,
        |_| {},
        expect![[r#"2:8 -> file:///fixture/main.tolk 8:8 reference"#]],
    );
}

#[test]
fn upstream_basic_018_constant_references() {
    case_tolk_references(
        "file:///fixture/main.tolk",
        r#"const <caret>FOO = 100;

fun test2() {
    if (FOO == 100) {
        throw FOO;
    }
}"#,
        false,
        |_| {},
        expect![[r#"0:6 -> file:///fixture/main.tolk 3:8 reference
0:6 -> file:///fixture/main.tolk 4:14 reference"#]],
    );
}

#[test]
fn upstream_basic_019_type_alias_references() {
    case_tolk_references(
        "file:///fixture/main.tolk",
        r#"type <caret>Int = int;

struct Foo {
    field: Int;
}

fun test2(a: Int): Int {}"#,
        false,
        |_| {},
        expect![[r#"0:5 -> file:///fixture/main.tolk 3:11 reference
0:5 -> file:///fixture/main.tolk 6:13 reference
0:5 -> file:///fixture/main.tolk 6:19 reference"#]],
    );
}

#[test]
fn upstream_basic_020_type_alias_references_from_usage() {
    case_tolk_references(
        "file:///fixture/main.tolk",
        r#"type Int = int;

struct Foo {
    field: Int;
}

fun test2(a: <caret>Int): Int {}"#,
        false,
        |_| {},
        expect![[r#"6:13 -> file:///fixture/main.tolk 3:11 reference
6:13 -> file:///fixture/main.tolk 6:13 reference
6:13 -> file:///fixture/main.tolk 6:19 reference"#]],
    );
}

#[test]
fn upstream_basic_021_struct_references() {
    case_tolk_references(
        "file:///fixture/main.tolk",
        r#"struct <caret>Foo {
    field: Int;
}

fun test2(a: Foo): Foo {
    val foo: Foo = {};
    val bar = Foo {};
}"#,
        false,
        |_| {},
        expect![[r#"0:7 -> file:///fixture/main.tolk 4:13 reference
0:7 -> file:///fixture/main.tolk 4:19 reference
0:7 -> file:///fixture/main.tolk 5:13 reference
0:7 -> file:///fixture/main.tolk 6:14 reference"#]],
    );
}

#[test]
fn upstream_basic_022_struct_keyword_references() {
    case_tolk_references(
        "file:///fixture/main.tolk",
        r#"type Int = int;

<caret>struct Foo {
    field: Int;
}

fun test2(a: Int): Int {}"#,
        false,
        |_| {},
        expect![[r#"2:0 unresolved"#]],
    );
}

#[test]
fn upstream_basic_023_get_method_references() {
    case_tolk_references(
        "file:///fixture/main.tolk",
        r#"get fun someData(): int {}

fun test() {
    <caret>someData();
}"#,
        false,
        |_| {},
        expect![[r#"3:4 -> file:///fixture/main.tolk 3:4 reference"#]],
    );
}

#[test]
fn upstream_basic_024_do_while_references() {
    case_tolk_references(
        "file:///fixture/main.tolk",
        r#"fun main() {
    do {
        var <caret>a = 10;
    } while (a);
}"#,
        false,
        |_| {},
        expect![[r#"2:12 -> file:///fixture/main.tolk 3:13 reference"#]],
    );
}

#[test]
fn upstream_basic_025_struct_references_with_generic_type() {
    case_tolk_references(
        "file:///fixture/main.tolk",
        r#"struct <caret>Config {}

struct Storage {
  config: Cell<Config>
}

fun Storage.load() {
    return Storage.fromCell(contract.getData())
}

fun name() {
    var st = Storage.load();
    val config = st.config.load();
}"#,
        false,
        |_| {},
        expect![[r#"0:7 -> file:///fixture/main.tolk 3:15 reference"#]],
    );
}

#[test]
fn upstream_basic_026_struct_with_methods() {
    case_tolk_references(
        "file:///fixture/main.tolk",
        r#"struct <caret>Storage {}

fun Storage.load() {
    return Storage.fromCell(contract.getData());
}

fun Storage.save(self) {
    contract.setData(self.toCell());
}"#,
        false,
        |_| {},
        expect![[r#"0:7 -> file:///fixture/main.tolk 2:4 reference
0:7 -> file:///fixture/main.tolk 3:11 reference
0:7 -> file:///fixture/main.tolk 6:4 reference"#]],
    );
}

#[test]
fn upstream_basic_027_enum_references() {
    case_tolk_references(
        "file:///fixture/main.tolk",
        r#"enum <caret>Color {
    Red = 10,
    Blue = 200 + 100,
}

fun main() {
    Color.Red;

    val a: Color = Color.Blue;
    match (a) {
        Color.Red => {}
        Color.Blue => {}
    }
}"#,
        false,
        |_| {},
        expect![[r#"0:5 -> file:///fixture/main.tolk 6:4 reference
0:5 -> file:///fixture/main.tolk 8:11 reference
0:5 -> file:///fixture/main.tolk 8:19 reference
0:5 -> file:///fixture/main.tolk 10:8 reference
0:5 -> file:///fixture/main.tolk 11:8 reference"#]],
    );
}

#[test]
fn upstream_basic_028_enum_member_references() {
    case_tolk_references(
        "file:///fixture/main.tolk",
        r#"enum Color {
    <caret>Red = 10,
    Blue = 200 + 100,
}

fun main() {
    Color.Red;

    val a: Color = Color.Blue;
    match (a) {
        Color.Red => {}
        Color.Blue => {}
    }
}"#,
        false,
        |_| {},
        expect![[r#"1:4 -> file:///fixture/main.tolk 6:10 reference
1:4 -> file:///fixture/main.tolk 10:14 reference"#]],
    );
}

#[test]
fn upstream_lambdas_029_lambda_parameter_without_type() {
    case_tolk_references(
        "file:///fixture/main.tolk",
        r#"fun test() {
    fun (<caret>a) {
        a;
    };
}"#,
        false,
        |_| {},
        expect![[r#"1:9 -> file:///fixture/main.tolk 2:8 reference"#]],
    );
}

#[test]
fn upstream_lambdas_030_lambda_parameter_with_type() {
    case_tolk_references(
        "file:///fixture/main.tolk",
        r#"fun test() {
    fun (<caret>a: int) {
        a;
    };
}"#,
        false,
        |_| {},
        expect![[r#"1:9 -> file:///fixture/main.tolk 2:8 reference"#]],
    );
}

#[test]
fn upstream_lambdas_031_nested_lambda_parameter() {
    case_tolk_references(
        "file:///fixture/main.tolk",
        r#"fun test() {
    fun (<caret>a: int) {
        fun (a: int) {
            a;
        };
    };
}"#,
        false,
        |_| {},
        expect![[r#"1:9 unresolved"#]],
    );
}

#[test]
fn upstream_lambdas_032_nested_lambda_parameter_2() {
    case_tolk_references(
        "file:///fixture/main.tolk",
        r#"fun test() {
    fun (a: int) {
        fun (<caret>a: int) {
            a;
        };
    };
}"#,
        false,
        |_| {},
        expect![[r#"2:13 -> file:///fixture/main.tolk 3:12 reference"#]],
    );
}

#[test]
fn upstream_type_parameters_033_receiver_type_parameters() {
    case_tolk_references(
        "file:///fixture/main.tolk",
        r#"struct Foo<T> {}

fun Foo<<caret>TName>.foo(): TName {}"#,
        false,
        |_| {},
        expect![[r#"2:8 -> file:///fixture/main.tolk 2:22 reference"#]],
    );
}

#[test]
fn upstream_type_parameters_034_t_receiver() {
    case_tolk_references(
        "file:///fixture/main.tolk",
        r#"fun T.foo(): <caret>T {}"#,
        false,
        |_| {},
        expect![[r#"0:13 -> file:///fixture/main.tolk 0:13 reference"#]],
    );
}

#[test]
fn upstream_type_parameters_035_t_receiver_from_decl() {
    case_tolk_references(
        "file:///fixture/main.tolk",
        r#"fun <caret>T.foo(): T {}"#,
        false,
        |_| {},
        expect![[r#"0:4 -> file:///fixture/main.tolk 0:13 reference"#]],
    );
}

#[test]
fn upstream_type_parameters_036_function_type_parameters() {
    case_tolk_references(
        "file:///fixture/main.tolk",
        r#"fun foo<<caret>T>(a: T): T {}"#,
        false,
        |_| {},
        expect![[r#"0:8 -> file:///fixture/main.tolk 0:14 reference
0:8 -> file:///fixture/main.tolk 0:18 reference"#]],
    );
}

#[test]
fn upstream_type_parameters_037_method_type_parameters() {
    case_tolk_references(
        "file:///fixture/main.tolk",
        r#"fun int.foo<<caret>T>(a: T): T {}"#,
        false,
        |_| {},
        expect![[r#"0:12 -> file:///fixture/main.tolk 0:18 reference
0:12 -> file:///fixture/main.tolk 0:22 reference"#]],
    );
}

#[test]
fn upstream_type_parameters_038_struct_type_parameters() {
    case_tolk_references(
        "file:///fixture/main.tolk",
        r#"struct Foo<<caret>T> {
    field: T
    other: Bar<T>
}"#,
        false,
        |_| {},
        expect![[r#"0:11 -> file:///fixture/main.tolk 1:11 reference
0:11 -> file:///fixture/main.tolk 2:15 reference"#]],
    );
}

#[test]
fn upstream_type_parameters_039_type_alias_type_parameters() {
    case_tolk_references(
        "file:///fixture/main.tolk",
        r#"type Foo<<caret>T> = T | null"#,
        false,
        |_| {},
        expect![[r#"0:9 -> file:///fixture/main.tolk 0:14 reference"#]],
    );
}
