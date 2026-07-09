#[path = "../../support/mod.rs"]
mod support;

use expect_test::{Expect, expect};
use support::{MarkedSource, render_semantic_tokens};
use ton_language_server_core::languages::tolk::{LANGUAGE_ID, TolkLanguage};
use ton_language_server_core::{DocumentUri, LanguageService, LanguageServiceConfig};

fn case_tolk_semantic_tokens(source: &str, expect: Expect) {
    let marked = MarkedSource::parse(source);
    let uri = DocumentUri::from("file:///workspace/main.tolk");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TolkLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source().to_owned())
        .expect("Tolk document should open");

    let tokens = service
        .semantic_tokens(&uri)
        .expect("semantic tokens request should succeed");
    expect.assert_eq(&render_semantic_tokens(marked.source(), &tokens.data));
}

#[test]
fn highlights_declarations_and_locals() {
    case_tolk_semantic_tokens(
        r"
            struct Storage {
                counter: int
            }

            fun Storage.save(mutate self, amount: int) {
                var value = self.counter + amount;
            }
        ",
        expect![[r"
            0:7 14 kind=struct        modifiers=-            text=Storage
            1:4 11 kind=property      modifiers=-            text=counter
            1:13 16 kind=type          modifiers=-            text=int
            4:4 11 kind=struct        modifiers=-            text=Storage
            4:12 16 kind=function      modifiers=-            text=save
            4:24 28 kind=keyword       modifiers=modification text=self
            4:30 36 kind=parameter     modifiers=-            text=amount
            4:38 41 kind=type          modifiers=-            text=int
            5:8 13 kind=variable      modifiers=modification text=value
            5:16 20 kind=keyword       modifiers=modification text=self
            5:21 28 kind=property      modifiers=-            text=counter
            5:31 37 kind=parameter     modifiers=-            text=amount"]],
    );
}

#[test]
fn highlights_type_inferred_field_and_method_usages() {
    case_tolk_semantic_tokens(
        r"
            struct Storage {
                counter: int
            }

            fun Storage.save(mutate self) {
            }

            fun main(arg: int) {
                var storage = Storage { counter: arg };
                storage.counter;
                storage.save();
            }
        ",
        expect![[r"
            0:7 14 kind=struct        modifiers=-            text=Storage
            1:4 11 kind=property      modifiers=-            text=counter
            1:13 16 kind=type          modifiers=-            text=int
            4:4 11 kind=struct        modifiers=-            text=Storage
            4:12 16 kind=function      modifiers=-            text=save
            4:24 28 kind=keyword       modifiers=modification text=self
            7:4 8 kind=function      modifiers=-            text=main
            7:9 12 kind=parameter     modifiers=-            text=arg
            7:14 17 kind=type          modifiers=-            text=int
            8:8 15 kind=variable      modifiers=modification text=storage
            8:18 25 kind=struct        modifiers=-            text=Storage
            8:28 35 kind=property      modifiers=-            text=counter
            8:37 40 kind=parameter     modifiers=-            text=arg
            9:4 11 kind=variable      modifiers=modification text=storage
            9:12 19 kind=property      modifiers=-            text=counter
            10:4 11 kind=variable      modifiers=modification text=storage
            10:12 16 kind=function      modifiers=-            text=save"]],
    );
}
