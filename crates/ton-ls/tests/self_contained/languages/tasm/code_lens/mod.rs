use expect_test::expect;
use function_name::named;

use crate::self_contained::languages::tasm::helpers::case_tasm_code_lens;

#[named]
#[test]
fn test_code_lens_top_level_and_nested() {
    case_tasm_code_lens(
        function_name!(),
        r#"
            PUSHINT_4 1
            FOOOP
            ref {
              SWAP
              PUSHDICT [
                1 => {
                  XCHG0
                }
              ]
            }
        "#,
        expect![[r#"
            0:0 title=∅ → x: Int
            1:0 title=N/A
            3:2 title=x y → y x
            4:2 title=N/A
            6:6 title=N/A"#]],
    );
}

#[named]
#[test]
fn test_code_lens_instruction_argument_variants() {
    case_tasm_code_lens(
        function_name!(),
        r#"
            PUSHCONT {
              DUP
            }
            PUSHDICT [
              42 => {
                DROP
              }
            ]
        "#,
        expect![[r#"
            0:0 title=∅ → result: Continuation
            1:2 title=x → x x
            3:0 title=N/A
            5:4 title=x → ∅"#]],
    );
}

#[named]
#[test]
fn test_code_lens_empty_file_has_none() {
    case_tasm_code_lens(function_name!(), "", expect!["<none>"]);
}
