#[path = "../../support/mod.rs"]
mod support;

use expect_test::{Expect, expect};
use support::{MarkedSource, render_semantic_tokens};
use ton_language_server_core::languages::tlb::{LANGUAGE_ID, TlbLanguage};
use ton_language_server_core::{DocumentUri, LanguageService, LanguageServiceConfig};

fn case_tlb_semantic_tokens(source: &str, expect: Expect) {
    let marked = MarkedSource::parse(source);
    let uri = DocumentUri::from("file:///fixture/main.tlb");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TlbLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source().to_owned())
        .expect("TL-B document should open");

    let tokens = service
        .semantic_tokens(&uri)
        .expect("semantic tokens request should succeed");
    expect.assert_eq(&render_semantic_tokens(marked.source(), &tokens.data));
}

#[test]
fn highlights_basic_declaration() {
    case_tlb_semantic_tokens(
        r"
            foo$0 a:# = CommonMsgInfo;
        ",
        expect![[r"
            0:0 3 kind=type          modifiers=-            text=foo
            0:6 7 kind=property      modifiers=-            text=a
            0:8 9 kind=macro         modifiers=-            text=#
            0:12 25 kind=struct        modifiers=-            text=CommonMsgInfo"]],
    );
}

#[test]
fn highlights_builtins_and_type_parameters() {
    case_tlb_semantic_tokens(
        r"
            box$_ {X:Type} value:X bits:uint32 = Box X;
        ",
        expect![[r"
            0:0 3 kind=type          modifiers=-            text=box
            0:9 13 kind=macro         modifiers=-            text=Type
            0:15 20 kind=property      modifiers=-            text=value
            0:21 22 kind=type          modifiers=-            text=X
            0:23 27 kind=property      modifiers=-            text=bits
            0:28 34 kind=macro         modifiers=-            text=uint32
            0:37 40 kind=struct        modifiers=-            text=Box
            0:41 42 kind=typeParameter modifiers=-            text=X"]],
    );
}
