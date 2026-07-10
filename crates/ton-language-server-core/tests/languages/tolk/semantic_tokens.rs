#[path = "../../support/mod.rs"]
mod support;

use expect_test::{Expect, expect};
use support::{MarkedSource, render_semantic_tokens};
use ton_language_server_core::languages::tolk::{LANGUAGE_ID, TolkLanguage};
use ton_language_server_core::{
    DocumentUri, LanguageService, LanguageServiceConfig, ProfileSummary,
};

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

#[test]
fn highlights_every_symbol_kind_and_excludes_unresolved_names() {
    case_tolk_semantic_tokens(
        r"
            type Alias = int;
            enum Color {
                Red = 1,
            }
            struct `contract`<T> {
                value: T
            }
            const ANSWER = 42;
            global counter: int;

            get fun getter(): int {
                return ANSWER;
            }

            fun `contract`<int>.update(self, mutate delta: int) {
                val immutable = delta;
                var mutable = immutable;
                try {
                    mutable += counter;
                } catch (error) {
                    unresolved;
                }
                Color.Red;
                getter();
            }
        ",
        expect![[r"
            0:5 10 kind=type          modifiers=-            text=Alias
            0:13 16 kind=type          modifiers=-            text=int
            1:5 10 kind=enum          modifiers=-            text=Color
            2:4 7 kind=enumMember    modifiers=-            text=Red
            4:7 17 kind=macro         modifiers=-            text=`contract`
            4:18 19 kind=typeParameter modifiers=-            text=T
            5:4 9 kind=property      modifiers=-            text=value
            5:11 12 kind=typeParameter modifiers=-            text=T
            7:6 12 kind=property      modifiers=-            text=ANSWER
            8:7 14 kind=variable      modifiers=-            text=counter
            8:16 19 kind=type          modifiers=-            text=int
            10:8 14 kind=function      modifiers=-            text=getter
            10:18 21 kind=type          modifiers=-            text=int
            11:11 17 kind=property      modifiers=-            text=ANSWER
            14:4 14 kind=macro         modifiers=-            text=`contract`
            14:15 18 kind=type          modifiers=-            text=int
            14:20 26 kind=function      modifiers=-            text=update
            14:27 31 kind=keyword       modifiers=-            text=self
            14:40 45 kind=parameter     modifiers=modification text=delta
            14:47 50 kind=type          modifiers=-            text=int
            15:8 17 kind=variable      modifiers=-            text=immutable
            15:20 25 kind=parameter     modifiers=modification text=delta
            16:8 15 kind=variable      modifiers=modification text=mutable
            16:18 27 kind=variable      modifiers=-            text=immutable
            18:8 15 kind=variable      modifiers=modification text=mutable
            18:19 26 kind=variable      modifiers=-            text=counter
            19:13 18 kind=variable      modifiers=-            text=error
            22:4 9 kind=enum          modifiers=-            text=Color
            22:10 13 kind=enumMember    modifiers=-            text=Red
            23:4 10 kind=function      modifiers=-            text=getter"]],
    );
}

#[test]
fn object_shorthand_is_highlighted_as_the_visible_local() {
    case_tolk_semantic_tokens(
        r"
            struct Foo {
                value: int
            }

            fun main(value: int) {
                Foo { value };
            }
        ",
        expect![[r"
            0:7 10 kind=struct        modifiers=-            text=Foo
            1:4 9 kind=property      modifiers=-            text=value
            1:11 14 kind=type          modifiers=-            text=int
            4:4 8 kind=function      modifiers=-            text=main
            4:9 14 kind=parameter     modifiers=-            text=value
            4:16 19 kind=type          modifiers=-            text=int
            5:4 7 kind=struct        modifiers=-            text=Foo
            5:10 15 kind=parameter     modifiers=-            text=value"]],
    );
}

#[test]
fn records_semantic_token_profile_spans() {
    let uri = DocumentUri::from("file:///workspace/profiled.tolk");
    let mut service = LanguageService::new(LanguageServiceConfig {
        enable_profiling: true,
    });
    service.register_language(TolkLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, "fun main() {}".to_owned())
        .expect("Tolk document should open");

    let tokens = service
        .semantic_tokens(&uri)
        .expect("semantic tokens request should succeed");
    let summary = service.profiler().summary();
    let actual = format!(
        "tokens={} semantic_tokens={} tolk.semantic_tokens={}",
        tokens.data.len(),
        event_count(summary, "semantic_tokens"),
        event_count(summary, "tolk.semantic_tokens"),
    );
    expect!["tokens=1 semantic_tokens=1 tolk.semantic_tokens=1"].assert_eq(&actual);
}

fn event_count(summary: &ProfileSummary, name: &'static str) -> usize {
    summary
        .events
        .iter()
        .filter(|event| event.name == name)
        .count()
}
