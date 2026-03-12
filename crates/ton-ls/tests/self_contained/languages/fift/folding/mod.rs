use expect_test::expect;
use function_name::named;

use crate::self_contained::languages::fift::helpers::case_fift_folding;

#[named]
#[test]
fn test_folding_nested_blocks() {
    case_fift_folding(
        function_name!(),
        r#"
            PROGRAM{
              DECLPROC entry
              entry PROC:<{
                IFJMP:<{
                  1 PUSHINT
                }>
                REPEAT:<{
                  2 PUSHINT
                }>
              }>
            END>c
        "#,
        expect!["[0, 10], [2, 9], [3, 5], [6, 8]"],
    );
}

#[named]
#[test]
fn test_folding_if_else_while_until_and_instruction_block() {
    case_fift_folding(
        function_name!(),
        r#"
            PROGRAM{
              DECLPROC entry
              entry PROC:<{
                IF:<{
                  1 PUSHINT
                }>ELSE<{
                  2 PUSHINT
                }>
                WHILE:<{
                  3 PUSHINT
                }>DO<{
                  4 PUSHINT
                }>
                UNTIL:<{
                  5 PUSHINT
                }>
                <{
                  6 PUSHINT
                }>
              }>
            END>c
        "#,
        expect!["[0, 20], [2, 19], [3, 7], [8, 12], [13, 15], [16, 18]"],
    );
}

#[named]
#[test]
fn test_folding_procinline_procref_and_method() {
    case_fift_folding(
        function_name!(),
        r#"
            PROGRAM{
              DECLPROC entry
              1 DECLMETHOD mm
              entry PROCINLINE:<{
                1 PUSHINT
              }>
              rr PROCREF:<{
                2 PUSHINT
              }>
              mm METHOD:<{
                3 PUSHINT
              }>
            END>c
        "#,
        expect!["[0, 12], [3, 5], [6, 8], [9, 11]"],
    );
}
