mod common;

use crate::common::check;
use expect_test::expect;

#[test]
fn test_fmt_ignore_top_level_function_declaration() {
    check(
        "
// fmt-ignore
fun foo(){
    val   x   =   1;
    return    x;
}

fun bar() {
    val   y   =   2;
    return y;
}",
        expect![[r"
// fmt-ignore
fun foo(){
    val   x   =   1;
    return    x;
}

fun bar() {
    val y = 2;
    return y;
}"]],
    );
}

#[test]
fn test_fmt_ignore_struct_declaration() {
    check(
        "
// fmt-ignore
struct MyStruct {
    field1:   int;
    field2:   slice;
}

struct OtherStruct {
    field1:   int;
    field2:   slice;
}",
        expect![[r"
            // fmt-ignore
            struct MyStruct {
                field1:   int;
                field2:   slice;
            }

            struct OtherStruct {
                field1: int
                field2: slice
            }"]],
    );
}

#[test]
fn test_fmt_ignore_statements_in_block() {
    check(
        "
fun foo() {
    val x = 1;

    // fmt-ignore
    val   y   =   2;

    val z = 3;
    return x + y + z;
}",
        expect![[r"
fun foo() {
    val x = 1;

    // fmt-ignore
    val   y   =   2;

    val z = 3;
    return x + y + z;
}"]],
    );
}

#[test]
fn test_fmt_ignore_multiple_consecutive_statements() {
    check(
        "
fun foo() {
    val x = 1;

    // fmt-ignore
    val   y   =   2;
    // fmt-ignore
    val   z   =   3;

    return x + y + z;
}",
        expect![[r"
fun foo() {
    val x = 1;

    // fmt-ignore
    val   y   =   2;
    // fmt-ignore
    val   z   =   3;

    return x + y + z;
}"]],
    );
}

#[test]
fn test_fmt_ignore_global_variable_declaration() {
    check(
        "
// fmt-ignore
global   global_var   :   int;

global other_var: int",
        expect![[r"
// fmt-ignore
global   global_var   :   int;

global other_var: int"]],
    );
}

#[test]
fn test_fmt_ignore_type_alias() {
    check(
        "
// fmt-ignore
type   MyType   =   int   |   slice;

type OtherType = int   |   slice;",
        expect![[r"
// fmt-ignore
type   MyType   =   int   |   slice;

type OtherType = int | slice"]],
    );
}

#[test]
fn test_fmt_ignore_with_comments_on_same_line() {
    check(
        "
fun foo() {
    val x = 1;

    // fmt-ignore
    val   y   =   2;  // this is a comment

    val z = 3;
}",
        expect![[r"
            fun foo() {
                val x = 1;

                // fmt-ignore
                val   y   =   2; // this is a comment

                val z = 3;
            }"]],
    );
}

#[test]
fn test_fmt_ignore_with_extra_whitespace() {
    check(
        "
//   fmt-ignore
fun foo(){
    val   x   =   1;
    return x;
}",
        expect![[r"
            //   fmt-ignore
            fun foo() {
                val x = 1;
                return x;
            }"]],
    );
}

#[test]
fn test_fmt_ignore_with_incorrect_directive() {
    check(
        "
// fmt-ignore-wrong
fun foo(){
    val   x   =   1;
    return x;
}

// format-ignore
fun bar(){
    val   y   =   2;
    return y;
}",
        expect![[r"
// fmt-ignore-wrong
fun foo() {
    val x = 1;
    return x;
}

// format-ignore
fun bar() {
    val y = 2;
    return y;
}"]],
    );
}

#[test]
fn test_fmt_ignore_if_statement() {
    check(
        "
fun foo() {
    val x = 1;

    // fmt-ignore
    if(x   >   0){
        return   true;
    }

    return false;
}",
        expect![[r"
fun foo() {
    val x = 1;

    // fmt-ignore
    if(x   >   0){
        return   true;
    }

    return false;
}"]],
    );
}

#[test]
fn test_fmt_ignore_while_statement() {
    check(
        "
fun foo() {
    // fmt-ignore
    while(x   <   10){
        x   =   x   +   1;
    }
}",
        expect![[r"
fun foo() {
    // fmt-ignore
    while(x   <   10){
        x   =   x   +   1;
    }
}"]],
    );
}

#[test]
fn test_fmt_ignore_block_formatting() {
    check(
        "
fun foo() {
    // fmt-ignore
    {
        val   x   =   1;
        val   y   =   2;
    }

    {
        val   a   =   3;
        val   b   =   4;
    }
}",
        expect![[r"
fun foo() {
    // fmt-ignore
    {
        val   x   =   1;
        val   y   =   2;
    }

    {
        val a = 3;
        val b = 4;
    }
}"]],
    );
}

#[test]
fn test_fmt_ignore_local_vars_declaration() {
    check(
        "
fun foo() {
    val x = 1;

    // fmt-ignore
    val   y   =   2   +   3;

    val z = 4;
}",
        expect![[r"
fun foo() {
    val x = 1;

    // fmt-ignore
    val   y   =   2   +   3;

    val z = 4;
}"]],
    );
}

#[test]
fn test_fmt_ignore_do_while_statement() {
    check(
        "
fun foo() {
    val x = 0;

    // fmt-ignore
    do{
        x   =   x   +   1;
    }while(x   <   10);

    val y = x;
}",
        expect![[r"
fun foo() {
    val x = 0;

    // fmt-ignore
    do{
        x   =   x   +   1;
    }while(x   <   10);

    val y = x;
}"]],
    );
}

#[test]
fn test_fmt_ignore_break_statement() {
    check(
        "
fun foo() {
    val x = 0;

    while (x < 10) {
        // fmt-ignore
        break;

        x = x + 1;
    }
}",
        expect![[r"
fun foo() {
    val x = 0;

    while (x < 10) {
        // fmt-ignore
        break;

        x = x + 1;
    }
}"]],
    );
}

#[test]
fn test_fmt_ignore_continue_statement() {
    check(
        "
fun foo() {
    val x = 0;

    while (x < 10) {
        x = x + 1;

        // fmt-ignore
        continue;

        val y = x;
    }
}",
        expect![[r"
fun foo() {
    val x = 0;

    while (x < 10) {
        x = x + 1;

        // fmt-ignore
        continue;

        val y = x;
    }
}"]],
    );
}

#[test]
fn test_fmt_ignore_throw_statement() {
    check(
        "
fun foo() {
    val x = 1;

    // fmt-ignore
    throw    x;

    val y = 2;
}",
        expect![[r"
fun foo() {
    val x = 1;

    // fmt-ignore
    throw    x;

    val y = 2;
}"]],
    );
}

#[test]
fn test_fmt_ignore_assert_statement() {
    check(
        "
fun foo() {
    val x = 1;

    // fmt-ignore
    assert(x   >   0,   123);

    val y = 2;
}",
        expect![[r"
fun foo() {
    val x = 1;

    // fmt-ignore
    assert(x   >   0,   123);

    val y = 2;
}"]],
    );
}

#[test]
fn test_fmt_ignore_expression_statement() {
    check(
        "
fun foo() {
    val x = 1;

    // fmt-ignore
    x   +   2;

    val y = 3;
}",
        expect![[r"
fun foo() {
    val x = 1;

    // fmt-ignore
    x   +   2;

    val y = 3;
}"]],
    );
}

#[test]
fn test_fmt_ignore_complex_tuple() {
    check(
        "
            fun foo() {
                val x = 1;

                // fmt-ignore
                val   matrix   =   [
                    [1,   2,   3,   4,   5],
                    [6,   7,   8,   9,   10],
                    [11,  12,  13,  14,  15],
                    [16,  17,  18,  19,  20]
                ];

                val y = 2;
            }",
        expect![[r"
            fun foo() {
                val x = 1;

                // fmt-ignore
                val   matrix   =   [
                                [1,   2,   3,   4,   5],
                                [6,   7,   8,   9,   10],
                                [11,  12,  13,  14,  15],
                                [16,  17,  18,  19,  20]
                            ];

                val y = 2;
            }"]],
    );
}

#[test]
fn test_fmt_ignore_type_parameters() {
    check(
        "
// fmt-ignore
type   MyType   <   T   ,   U   >   =   tuple   <   T   ,   U   >   ;

type OtherType<T, U> = tuple<T, U>;
",
        expect![[r"
            // fmt-ignore
            type   MyType   <   T   ,   U   >   =   tuple   <   T   ,   U   >   ;

            type OtherType<T, U> = tuple<T, U>"]],
    );
}

#[test]
fn test_fmt_ignore_call_argument_separators() {
    check(
        r"fun add(x: int, y: int): int { return x+y; }
get fun f(): int { return add(
// fmt-ignore
1  +  2, // keep this argument
3); }",
        expect![[r"
fun add(x: int, y: int): int {
    return x + y;
}

get fun f(): int {
    return add(
        // fmt-ignore
        1  +  2, // keep this argument
        3,
    );
}"]],
    );
}

#[test]
fn test_fmt_ignore_parameter_separators() {
    check(
        r"get fun f(
// fmt-ignore
x : int,
y: int): int { return x+y; }",
        expect![[r"
get fun f(
    // fmt-ignore
    x : int,
    y: int,
): int {
    return x + y;
}"]],
    );
}

#[test]
fn test_fmt_ignore_tuple_separators() {
    check(
        r"get fun f(): [int,int] { return [
// fmt-ignore
1  +  2,
// fmt-ignore
3  +  4]; }",
        expect![[r"
get fun f(): [int, int] {
    return [
        // fmt-ignore
        1  +  2,
        // fmt-ignore
        3  +  4,
    ];
}"]],
    );
}

#[test]
fn test_fmt_ignore_type_separators() {
    check(
        r"type Pair = [
// fmt-ignore
int,
int]
get fun f(): Pair { return [1,2]; }",
        expect![[r"
type Pair = [
    // fmt-ignore
    int,
    int,
]

get fun f(): Pair {
    return [1, 2];
}"]],
    );
}

#[test]
fn test_fmt_ignore_generic_separators() {
    check(
        r"struct Pair<
// fmt-ignore
X,
Y> { x:X y:Y }
type IntPair = Pair<
// fmt-ignore
int,
int>",
        expect![[r"
struct Pair<
    // fmt-ignore
    X,
    Y,
> {
    x: X
    y: Y
}

type IntPair = Pair<
    // fmt-ignore
    int,
    int,
>"]],
    );
}

#[test]
fn test_fmt_ignore_object_field_separators() {
    check(
        r"struct Pair { x:int y:int }
get fun f(): Pair { return Pair {
// fmt-ignore
x: 1  +  2,
y: 3}; }",
        expect![[r"
struct Pair {
    x: int
    y: int
}

get fun f(): Pair {
    return Pair {
        // fmt-ignore
        x: 1  +  2,
        y: 3,
    };
}"]],
    );
}
