use expect_test::expect;
use function_name::named;

use crate::self_contained::languages::tasm::helpers::case_tasm_folding;

#[named]
#[test]
fn test_folding_nested_code_and_dictionary() {
    case_tasm_folding(
        function_name!(),
        r#"
            PUSHCONT {
              PUSHDICT [
                1 => {
                  SWAP
                }
              ]
            }
        "#,
        expect!["[0, 6], [1, 5], [2, 4]"],
    );
}

#[named]
#[test]
fn test_folding_explicit_ref_and_instruction_code() {
    case_tasm_folding(
        function_name!(),
        r#"
            ref {
              PUSHINT_4 1
            }
            PUSHCONT {
              DUP
            }
        "#,
        expect!["[0, 2], [3, 5]"],
    );
}

#[named]
#[test]
fn test_folding_single_line_code_has_none() {
    case_tasm_folding(
        function_name!(),
        r#"
            ref { SWAP }
            PUSHCONT { DUP }
        "#,
        expect!["<none>"],
    );
}
