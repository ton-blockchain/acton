use expect_test::expect;
use function_name::named;

use crate::self_contained::languages::fift::helpers::{
    case_fift_references, case_fift_references_with_declaration,
};

#[named]
#[test]
fn test_references_proc() {
    case_fift_references(
        function_name!(),
        r#"
            PROGRAM{
              DECLPROC entry
              entry PROC:<{
                foo CALLDICT
                foo CALLDICT
              }>
              <caret>foo PROC:<{
                <caret>foo CALLDICT
              }>
            END>c
        "#,
        expect![[r#"
            6:2 refs=[3:4, 4:4, 7:4]
            7:4 refs=[3:4, 4:4, 7:4]"#]],
    );
}

#[named]
#[test]
fn test_references_with_declaration_proc() {
    case_fift_references_with_declaration(
        function_name!(),
        r#"
            PROGRAM{
              DECLPROC entry
              entry PROC:<{
                foo CALLDICT
                foo CALLDICT
              }>
              foo PROC:<{
                <caret>foo CALLDICT
              }>
            END>c
        "#,
        expect![[r#"
            7:4 refs=[3:4, 4:4, 6:2, 7:4]"#]],
    );
}

#[named]
#[test]
fn test_references_across_definition_kinds() {
    case_fift_references(
        function_name!(),
        r#"
            PROGRAM{
              DECLPROC entry
              entry PROC:<{
                inl CALLDICT
                inl CALLDICT
              }>
              <caret>inl PROCINLINE:<{
                inl CALLDICT
              }>
              rref PROCREF:<{
                inl CALLDICT
              }>
            END>c
        "#,
        expect!["6:2 refs=[3:4, 4:4, 7:4, 10:4]"],
    );
}

#[named]
#[test]
fn test_references_unresolved_symbol() {
    case_fift_references(
        function_name!(),
        r#"
            PROGRAM{
              DECLPROC entry
              entry PROC:<{
                <caret>missing CALLDICT
              }>
              foo PROC:<{ }>
            END>c
        "#,
        expect!["3:4 refs=unresolved"],
    );
}

#[named]
#[test]
fn test_references_with_declaration_for_method() {
    case_fift_references_with_declaration(
        function_name!(),
        r#"
            PROGRAM{
              DECLPROC entry
              100 DECLMETHOD mm
              entry PROC:<{
                mm CALLDICT
              }>
              mm METHOD:<{
                <caret>mm CALLDICT
              }>
            END>c
        "#,
        expect!["7:4 refs=[2:17, 4:4, 6:2, 7:4]"],
    );
}
