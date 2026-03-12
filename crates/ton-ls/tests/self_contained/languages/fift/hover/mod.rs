use expect_test::expect;
use function_name::named;

use crate::self_contained::languages::fift::helpers::case_fift_hover;

#[named]
#[test]
fn test_hover_instruction_variants() {
    case_fift_hover(
        function_name!(),
        r#"
            PROGRAM{
              DECLPROC entry
              entry PROC:<{
                0 <caret>PUSHINT
                1000 <caret>PUSHINT
                s0 <caret>PUSH
                <caret>XCHG0
              }>
            END>c
        "#,
        expect![[r#"
            ```
            PUSHINT_4 [i]
            ```
            - Stack (top is on the right): `∅ → x:Int`
            - Gas: `18`
            - Opcode: `7`

            Pushes tiny signed integer `x` onto the stack. Note that the instruction does not have the usual 4-bit range for the argument, but `-5 <= x <= 10`!

            ---

            ```
            PUSHINT_16 [x]
            ```
            - Stack (top is on the right): `∅ → x:Int`
            - Gas: `34`
            - Opcode: `81`

            Pushes 16-bit signed integer `x` onto the stack.

            ---

            ```
            PUSH [i]
            ```
            - Stack (top is on the right): `∅ → value:Any`
            - Gas: `18`
            - Opcode: `22`

            Copy value of `s(i)` and pushes it onto the stack.

            ---

            ```
            XCHG_0I [i]
            ```
            - Gas: `18`
            - Opcode: `2`

            Interchanges top element with element at index `i`."#]],
    );
}

#[named]
#[test]
fn test_hover_pushint_boundaries_and_multiline_argument() {
    case_fift_hover(
        function_name!(),
        r#"
            PROGRAM{
              DECLPROC entry
              entry PROC:<{
                16 <caret>PUSHINT
                40000 <caret>PUSHINT
                16
                <caret>PUSHINT
              }>
            END>c
        "#,
        expect![[r#"
            ```
            PUSHINT_8 [x]
            ```
            - Stack (top is on the right): `∅ → x:Int`
            - Gas: `26`
            - Opcode: `80`

            Pushes small 8-bit signed integer `x` onto the stack.

            ---

            ```
            PUSHINT_LONG [x]
            ```
            - Stack (top is on the right): `∅ → x:Int`
            - Gas: `23`
            - Opcode: `1040`

            Pushes big signed integer `x`.

            ---

            ```
            PUSHINT_4 [i]
            ```
            - Stack (top is on the right): `∅ → x:Int`
            - Gas: `18`
            - Opcode: `7`

            Pushes tiny signed integer `x` onto the stack. Note that the instruction does not have the usual 4-bit range for the argument, but `-5 <= x <= 10`!"#]],
    );
}

#[named]
#[test]
fn test_hover_push_variants_and_xchg_ij() {
    case_fift_hover(
        function_name!(),
        r#"
            PROGRAM{
              DECLPROC entry
              entry PROC:<{
                s0 s1 <caret>PUSH
                s0 s1 s2 <caret>PUSH
                s0 s1 <caret>XCHG
              }>
            END>c
        "#,
        expect![[r#"
            ```
            PUSH2 [i] [j]
            ```
            - Gas: `26`
            - Opcode: `53`

            Pushes two values from `i`-th and `j`-th positions (`PUSH s(i)`, `PUSH s(j+1)`).

            ---

            ```
            PUSH3 [i] [j] [k]
            ```
            - Gas: `34`
            - Opcode: `547`

            Pushes three values from different stack positions (`PUSH s(i)`, `PUSH2 s(j+1) s(k+1)`).

            ---

            ```
            XCHG_IJ [i] [j]
            ```
            - Gas: `26`
            - Opcode: `10`

            Interchanges elements at indices `i` and `j`."#]],
    );
}

#[named]
#[test]
fn test_hover_unknown_instruction() {
    case_fift_hover(
        function_name!(),
        r#"
            PROGRAM{
              DECLPROC entry
              entry PROC:<{
                <caret>UNKNOWNOP
              }>
            END>c
        "#,
        expect!["<none>"],
    );
}

#[named]
#[test]
fn test_hover_multiline_argument_with_crlf() {
    case_fift_hover(
        function_name!(),
        "PROGRAM{\r\n  DECLPROC entry\r\n  entry PROC:<{\r\n    16\r\n    <caret>PUSHINT\r\n  }>\r\nEND>c\r\n",
        expect![[r#"
            ```
            PUSHINT_4 [i]
            ```
            - Stack (top is on the right): `∅ → x:Int`
            - Gas: `18`
            - Opcode: `7`

            Pushes tiny signed integer `x` onto the stack. Note that the instruction does not have the usual 4-bit range for the argument, but `-5 <= x <= 10`!"#]],
    );
}
