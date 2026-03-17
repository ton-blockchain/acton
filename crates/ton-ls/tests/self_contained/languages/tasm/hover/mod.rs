use expect_test::expect;
use function_name::named;

use crate::self_contained::languages::tasm::helpers::case_tasm_hover;

#[named]
#[test]
fn test_hover_instruction_alias_and_unknown() {
    case_tasm_hover(
        function_name!(),
        r#"
            <caret>PUSHINT_4 1
            <caret>XCHG0
            <caret>UNKNOWNOP
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

            <none>

            ---

            <none>"#]],
    );
}

#[named]
#[test]
fn test_hover_nested_instruction_in_code_argument() {
    case_tasm_hover(
        function_name!(),
        r#"
            PUSHCONT {
              <caret>SWAP
            }
        "#,
        expect![[r#"
            ```
            SWAP
            ```
            - Stack (top is on the right): `x:Any y:Any → y:Any x:Any`
            - Gas: `18`
            - Opcode: `1`

            Interchanges the top two stack items. Takes two elements from the stack and pushes them back in reverse order."#]],
    );
}

#[named]
#[test]
fn test_hover_non_instruction_returns_none() {
    case_tasm_hover(
        function_name!(),
        r#"
            PUSHINT_4 <caret>1
        "#,
        expect!["<none>"],
    );
}
