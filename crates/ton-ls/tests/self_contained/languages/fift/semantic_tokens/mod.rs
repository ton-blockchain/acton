use expect_test::expect;
use function_name::named;

use crate::self_contained::languages::fift::helpers::case_fift_semantic_tokens;

#[named]
#[test]
fn test_semantic_tokens_proc_symbols() {
    case_fift_semantic_tokens(
        function_name!(),
        r#"
            "Asm.fif" include
            PROGRAM{
              DECLPROC entry
              entry PROC:<{
                entry CALLDICT
              }>
            END>c
        "#,
        expect![[r#"
            2:11 16 kind=function text=entry
            3:2   7 kind=function text=entry
            4:4   9 kind=function text=entry"#]],
    );
}

#[named]
#[test]
fn test_semantic_tokens_definition_kinds_and_resolved_calls() {
    case_fift_semantic_tokens(
        function_name!(),
        r#"
            PROGRAM{
              DECLPROC entry
              10 DECLMETHOD mm
              entry PROC:<{
                inl CALLDICT
                rr CALLDICT
                mm CALLDICT
                missing CALLDICT
              }>
              inl PROCINLINE:<{ }>
              rr PROCREF:<{ }>
              mm METHOD:<{
                rr CALLDICT
              }>
            END>c
        "#,
        expect![[r#"
            1:11 16 kind=function text=entry
            2:16 18 kind=function text=mm
            3:2   7 kind=function text=entry
            4:4   7 kind=function text=inl
            5:4   6 kind=function text=rr
            6:4   6 kind=function text=mm
            9:2   5 kind=function text=inl
            10:2   4 kind=function text=rr
            11:2   4 kind=function text=mm
            12:4   6 kind=function text=rr"#]],
    );
}

#[named]
#[test]
fn test_semantic_tokens_unresolved_identifier_not_highlighted() {
    case_fift_semantic_tokens(
        function_name!(),
        r#"
            PROGRAM{
              DECLPROC entry
              entry PROC:<{
                missing CALLDICT
                unresolved
              }>
            END>c
        "#,
        expect![[r#"
            1:11 16 kind=function text=entry
            2:2   7 kind=function text=entry"#]],
    );
}
