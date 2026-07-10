#![allow(clippy::needless_raw_string_hashes)]

use super::{check_rename, check_rename_rejected};
use expect_test::expect;

#[test]
fn upstream_basic_001_rename_local_variable() {
    check_rename(
        r#"fun test() {
    val num = 100;
    if (num == 10) {
        throw <caret>num;
//!           ^ errno
    }
}"#,
        "errno",
        expect![[r#"fun test() {
    val errno = 100;
    if (errno == 10) {
        throw errno;
//!           ^ errno
    }
}"#]],
    );
}

#[test]
fn upstream_basic_002_rename_local_backticked_variable() {
    check_rename(
        r#"fun test() {
    val `hello world` = 100;
    if (<caret>`hello world` == 10) {
//!     ^ `hello earth`
        throw `hello world`;
    }
}"#,
        "`hello earth`",
        expect![[r#"fun test() {
    val `hello earth` = 100;
    if (`hello earth` == 10) {
//!     ^ `hello earth`
        throw `hello earth`;
    }
}"#]],
    );
}

#[test]
fn upstream_basic_003_rename_local_variable_to_non_identifier_name() {
    check_rename(
        r#"fun test() {
    val foo = 100;
    if (<caret>foo == 10) {
//!     ^ `hello world`
        throw foo;
    }
}"#,
        "`hello world`",
        expect![[r#"fun test() {
    val `hello world` = 100;
    if (`hello world` == 10) {
//!     ^ `hello world`
        throw `hello world`;
    }
}"#]],
    );
}

#[test]
fn upstream_basic_004_rename_local_variable_from_different_scope() {
    check_rename(
        r#"fun test() {
    {
        val num = 100;
        if (num == 10) {
            throw <caret>num;
//!               ^ errno
        }
    }
    {
        val num = 100;
        if (num == 10) {
            throw num;
        }
    }
}"#,
        "errno",
        expect![[r#"fun test() {
    {
        val errno = 100;
        if (errno == 10) {
            throw errno;
//!               ^ errno
        }
    }
    {
        val num = 100;
        if (num == 10) {
            throw num;
        }
    }
}"#]],
    );
}

#[test]
fn upstream_basic_005_local_tuple_variable_rename() {
    check_rename(
        r#"fun test() {
    val [num, <caret>other] = [100, 200];
//!           ^ error
    if (num == 10) {
        throw other;
    }
}"#,
        "error",
        expect![[r#"fun test() {
    val [num, error] = [100, 200];
//!           ^ error
    if (num == 10) {
        throw error;
    }
}"#]],
    );
}

#[test]
fn upstream_basic_006_catch_variable_rename() {
    check_rename(
        r#"fun test() {
    try {} catch (error) {
        val e = <caret>error as int;
//!             ^ err
        if (e == 10) {
            throw e;
        }
    }
}"#,
        "err",
        expect![[r#"fun test() {
    try {} catch (err) {
        val e = err as int;
//!             ^ err
        if (e == 10) {
            throw e;
        }
    }
}"#]],
    );
}

#[test]
fn upstream_basic_007_second_catch_variable_rename() {
    check_rename(
        r#"fun test() {
    try {} catch (error, <caret>data) {
//!                      ^ d
        val e = data as int;
        if (e == 10) {
            throw e;
        }
    }
}"#,
        "d",
        expect![[r#"fun test() {
    try {} catch (error, d) {
//!                      ^ d
        val e = d as int;
        if (e == 10) {
            throw e;
        }
    }
}"#]],
    );
}

#[test]
fn upstream_basic_008_parameter_rename() {
    check_rename(
        r#"fun test(foo: int) {
    if (foo == 10) {
        throw <caret>foo;
//!           ^ bar
    }
}"#,
        "bar",
        expect![[r#"fun test(bar: int) {
    if (bar == 10) {
        throw bar;
//!           ^ bar
    }
}"#]],
    );
}

#[test]
fn upstream_basic_009_global_variable_rename() {
    check_rename(
        r#"global <caret>foo: int = 100;
//!    ^ BAR

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
        "BAR",
        expect![[r#"global BAR: int = 100;
//!    ^ BAR

fun test() {
    if (BAR == 10) {
        throw BAR;
    }
}

fun test2() {
    if (BAR == 100) {
        throw BAR + 200;
    }
}"#]],
    );
}

#[test]
fn upstream_basic_010_function_rename() {
    check_rename(
        r#"fun test() {}

fun test2() {
    test();
    <caret>test();
//! ^ someFunction
    test();
}"#,
        "someFunction",
        expect![[r#"fun someFunction() {}

fun test2() {
    someFunction();
    someFunction();
//! ^ someFunction
    someFunction();
}"#]],
    );
}

#[test]
fn upstream_basic_011_static_method_rename() {
    check_rename(
        r#"struct Foo {}

fun Foo.<caret>test() {}
//!     ^ bar

fun test2() {
    Foo.test();
}"#,
        "bar",
        expect![[r#"struct Foo {}

fun Foo.bar() {}
//!     ^ bar

fun test2() {
    Foo.bar();
}"#]],
    );
}

#[test]
fn upstream_basic_012_instance_method_rename() {
    check_rename(
        r#"struct Foo {}

fun Foo.<caret>test(self) {}
//!     ^ bar

fun test2() {
    val foo: Foo = {};
    foo.test();
}"#,
        "bar",
        expect![[r#"struct Foo {}

fun Foo.bar(self) {}
//!     ^ bar

fun test2() {
    val foo: Foo = {};
    foo.bar();
}"#]],
    );
}

#[test]
fn upstream_basic_013_constant_rename() {
    check_rename(
        r#"const <caret>FOO = 100;
//!   ^ BAR

fun test2() {
    if (FOO == 100) {
        throw FOO;
    }
}"#,
        "BAR",
        expect![[r#"const BAR = 100;
//!   ^ BAR

fun test2() {
    if (BAR == 100) {
        throw BAR;
    }
}"#]],
    );
}

#[test]
fn upstream_basic_014_type_alias_rename() {
    check_rename(
        r#"type Int = int;

struct Foo {
    field: <caret>Int;
//!        ^ MyInt
}

fun test2(a: Int): Int {}"#,
        "MyInt",
        expect![[r#"type MyInt = int;

struct Foo {
    field: MyInt;
//!        ^ MyInt
}

fun test2(a: MyInt): MyInt {}"#]],
    );
}

#[test]
fn upstream_basic_015_struct_rename() {
    check_rename(
        r#"struct Foo {
    field: int;
}

fun test2(a: Foo): Foo {
    val foo: Foo = {};
    val bar = <caret>Foo {};
//!           ^ Bar
}"#,
        "Bar",
        expect![[r#"struct Bar {
    field: int;
}

fun test2(a: Bar): Bar {
    val foo: Bar = {};
    val bar = Bar {};
//!           ^ Bar
}"#]],
    );
}

#[test]
fn upstream_basic_016_struct_field_rename() {
    check_rename(
        r#"struct Foo {
    <caret>field: int;
//! ^ newField
}

fun test2(a: Foo) {
    val foo: Foo = { field: 10 };
    foo.field;
    a.field;
}"#,
        "newField",
        expect![[r#"struct Foo {
    newField: int;
//! ^ newField
}

fun test2(a: Foo) {
    val foo: Foo = { newField: 10 };
    foo.newField;
    a.newField;
}"#]],
    );
}

#[test]
fn upstream_basic_017_struct_field_rename_with_cursor_on_like_when_select_all_name_in_editor() {
    check_rename(
        r#"struct Foo {
    field<caret>: int;
//!      ^ newField
}

fun test2(a: Foo) {
    val foo: Foo = { field: 10 };
    foo.field;
    a.field;
}"#,
        "newField",
        expect![[r#"struct Foo {
    newField: int;
//!      ^ newField
}

fun test2(a: Foo) {
    val foo: Foo = { newField: 10 };
    foo.newField;
    a.newField;
}"#]],
    );
}

#[test]
fn upstream_basic_018_struct_field_rename_for_short_init() {
    check_rename(
        r#"struct Foo {
    <caret>field: int;
//! ^ newField
}

fun test2(a: Foo, field: int) {
    val foo: Foo = { field };
    foo.field;
    a.field;
}"#,
        "newField",
        expect![[r#"struct Foo {
    newField: int;
//! ^ newField
}

fun test2(a: Foo, field: int) {
    val foo: Foo = { newField: field };
    foo.newField;
    a.newField;
}"#]],
    );
}

#[test]
fn upstream_basic_019_parameter_rename_for_short_init() {
    check_rename(
        r#"struct Foo {
    field: int;
}

fun test2(a: Foo, <caret>field: int) {
//!               ^ value
    val foo: Foo = { field };
    foo.field;
    a.field;
}"#,
        "value",
        expect![[r#"struct Foo {
    field: int;
}

fun test2(a: Foo, value: int) {
//!               ^ value
    val foo: Foo = { field: value };
    foo.field;
    a.field;
}"#]],
    );
}

#[test]
fn upstream_basic_020_local_variable_rename_for_short_init() {
    check_rename(
        r#"struct Foo {
    field: int;
}

fun test2(a: Foo) {
    val <caret>field = 0;
//!     ^ value
    val foo: Foo = { field };
    foo.field;
    a.field;
}"#,
        "value",
        expect![[r#"struct Foo {
    field: int;
}

fun test2(a: Foo) {
    val value = 0;
//!     ^ value
    val foo: Foo = { field: value };
    foo.field;
    a.field;
}"#]],
    );
}

#[test]
fn upstream_basic_021_rename_keyword() {
    check_rename_rejected(
        r#"fun test() {
    val num = 100;
    if (num == 10) {
        <caret>throw num;
//!     ^ errno
    }
}"#,
        "errno",
        expect![[r#"not renameable"#]],
    );
}

#[test]
fn upstream_basic_022_rename_builtin_type() {
    check_rename_rejected(
        r#"fun test(): <caret>int {
//!         ^ bool
}"#,
        "bool",
        expect![[r#"error: cannot rename an element from the Tolk standard library"#]],
    );
}

#[test]
fn upstream_basic_023_rename_stdlib_function() {
    check_rename_rejected(
        r#"fun test(): int {
    <caret>minMax();
//! ^ otherFunc
}"#,
        "otherFunc",
        expect![[r#"error: cannot rename an element from the Tolk standard library"#]],
    );
}

#[test]
fn upstream_basic_024_wrap_in_backtick_for_keyword_name() {
    check_rename(
        r#"fun foo() {}

fun test(): int {
    <caret>foo();
//! ^ return
}"#,
        "return",
        expect![[r#"fun `return`() {}

fun test(): int {
    `return`();
//! ^ return
}"#]],
    );
}

#[test]
fn upstream_basic_025_wrap_in_backtick_for_keyword_name_2() {
    check_rename(
        r#"fun foo() {}

fun test(): int {
    <caret>foo();
//! ^ match
}"#,
        "match",
        expect![[r#"fun `match`() {}

fun test(): int {
    `match`();
//! ^ match
}"#]],
    );
}

#[test]
fn upstream_basic_026_rename_struct_name_with_methods() {
    check_rename(
        r#"struct <caret>Storage {}
//!    ^ MyStorage

fun Storage.load() {
    return Storage.fromCell(contract.getData());
}

fun Storage.save(self) {
    contract.setData(self.toCell());
}"#,
        "MyStorage",
        expect![[r#"struct MyStorage {}
//!    ^ MyStorage

fun MyStorage.load() {
    return MyStorage.fromCell(contract.getData());
}

fun MyStorage.save(self) {
    contract.setData(self.toCell());
}"#]],
    );
}

#[test]
fn upstream_basic_027_rename_enum() {
    check_rename(
        r#"enum <caret>Color {
//!  ^ MyColor
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
        "MyColor",
        expect![[r#"enum MyColor {
//!  ^ MyColor
    Red = 10,
    Blue = 200 + 100,
}

fun main() {
    MyColor.Red;

    val a: MyColor = MyColor.Blue;
    match (a) {
        MyColor.Red => {}
        MyColor.Blue => {}
    }
}"#]],
    );
}

#[test]
fn upstream_basic_028_rename_enum_member() {
    check_rename(
        r#"enum Color {
    <caret>Red = 10,
//! ^ MyRed
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
        "MyRed",
        expect![[r#"enum Color {
    MyRed = 10,
//! ^ MyRed
    Blue = 200 + 100,
}

fun main() {
    Color.MyRed;

    val a: Color = Color.Blue;
    match (a) {
        Color.MyRed => {}
        Color.Blue => {}
    }
}"#]],
    );
}

#[test]
fn upstream_type_parameters_029_receiver_type_parameters() {
    check_rename(
        r#"struct Foo<T> {}

fun Foo<TName>.foo(): <caret>TName {}
//!                   ^ TValue"#,
        "TValue",
        expect![[r#"struct Foo<T> {}

fun Foo<TValue>.foo(): TValue {}
//!                   ^ TValue"#]],
    );
}

#[test]
fn upstream_type_parameters_030_t_receiver() {
    check_rename(
        r#"fun T.foo(): <caret>T {}
//!          ^ TName"#,
        "TName",
        expect![[r#"fun TName.foo(): TName {}
//!          ^ TName"#]],
    );
}

#[test]
fn upstream_type_parameters_031_t_receiver_from_decl() {
    check_rename(
        r#"fun <caret>T.foo(): T {}
//! ^ TName"#,
        "TName",
        expect![[r#"fun TName.foo(): TName {}
//! ^ TName"#]],
    );
}
