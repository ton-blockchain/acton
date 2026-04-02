use expect_test::expect;
use function_name::named;

use crate::self_contained::languages::fift::helpers::case_fift_resolve;

#[named]
#[test]
fn test_resolve_proc_call() {
    case_fift_resolve(
        function_name!(),
        r#"
            "Asm.fif" include
            PROGRAM{
              DECLPROC entry
              DECLPROC foo
              entry PROC:<{
                <caret>foo CALLDICT
                <caret>missing CALLDICT
              }>
              foo PROC:<{
                0 PUSHINT
              }>
            END>c
        "#,
        expect![[r#"
            5:4 -> 8:2 resolved
            6:4 unresolved"#]],
    );
}

#[named]
#[test]
fn test_resolve_inline_ref_and_method_definitions() {
    case_fift_resolve(
        function_name!(),
        r#"
            PROGRAM{
              DECLPROC entry
              entry PROC:<{
                <caret>inl CALLDICT
                <caret>rref CALLDICT
                <caret>meth CALLDICT
              }>
              inl PROCINLINE:<{ }>
              rref PROCREF:<{ }>
              meth METHOD:<{ }>
            END>c
        "#,
        expect![[r#"
            3:4 -> 7:2 resolved
            4:4 -> 8:2 resolved
            5:4 -> 9:2 resolved"#]],
    );
}

#[named]
#[test]
fn test_resolve_first_match_for_duplicate_name() {
    case_fift_resolve(
        function_name!(),
        r#"
            PROGRAM{
              DECLPROC entry
              entry PROC:<{
                <caret>foo CALLDICT
              }>
              foo PROC:<{ }>
              foo PROCINLINE:<{ }>
            END>c
        "#,
        expect!["3:4 -> 5:2 resolved"],
    );
}

#[named]
#[test]
fn test_resolve_declared_without_definition_unresolved() {
    case_fift_resolve(
        function_name!(),
        r#"
            PROGRAM{
              DECLPROC entry
              DECLPROC missing
              entry PROC:<{
                <caret>missing CALLDICT
              }>
            END>c
        "#,
        expect!["4:4 unresolved"],
    );
}
