#[path = "../../support/mod.rs"]
mod support;

use expect_test::{Expect, expect};
use support::{MarkedSource, render_semantic_tokens};
use ton_language_server_core::languages::fift::{FiftLanguage, LANGUAGE_ID};
use ton_language_server_core::{DocumentUri, LanguageService, LanguageServiceConfig};

fn case_fift_semantic_tokens(source: &str, expect: Expect) {
    let uri = DocumentUri::from("file:///fixture/main.fif");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(FiftLanguage::new());
    let marked = MarkedSource::parse(source);
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source().to_owned())
        .expect("Fift document should open");

    let tokens = service
        .semantic_tokens(&uri)
        .expect("semantic tokens request should succeed");
    expect.assert_eq(&render_semantic_tokens(marked.source(), &tokens.data));
}

#[test]
fn highlights_proc_symbols() {
    case_fift_semantic_tokens(
        "
            \"Asm.fif\" include
            PROGRAM{
              DECLPROC entry
              entry PROC:<{
                entry CALLDICT
              }>
            END>c
        ",
        expect![[r"
            2:11 16 kind=function      modifiers=-            text=entry
            3:2 7 kind=function      modifiers=-            text=entry
            4:4 9 kind=function      modifiers=-            text=entry"]],
    );
}

#[test]
fn highlights_definition_kinds_and_resolved_calls() {
    case_fift_semantic_tokens(
        r"
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
        ",
        expect![[r"
            1:11 16 kind=function      modifiers=-            text=entry
            2:16 18 kind=function      modifiers=-            text=mm
            3:2 7 kind=function      modifiers=-            text=entry
            4:4 7 kind=function      modifiers=-            text=inl
            5:4 6 kind=function      modifiers=-            text=rr
            6:4 6 kind=function      modifiers=-            text=mm
            9:2 5 kind=function      modifiers=-            text=inl
            10:2 4 kind=function      modifiers=-            text=rr
            11:2 4 kind=function      modifiers=-            text=mm
            12:4 6 kind=function      modifiers=-            text=rr"]],
    );
}

#[test]
fn skips_unresolved_identifiers() {
    case_fift_semantic_tokens(
        r"
            PROGRAM{
              DECLPROC entry
              entry PROC:<{
                missing CALLDICT
                unresolved
              }>
            END>c
        ",
        expect![[r"
            1:11 16 kind=function      modifiers=-            text=entry
            2:2 7 kind=function      modifiers=-            text=entry"]],
    );
}
