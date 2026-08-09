use super::*;

fn check(source: &str, include_declaration: bool, expect: Expect) {
    let marked = MarkedSource::parse(source);
    let uri = DocumentUri::from("file:///fixture/main.tolk");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TolkLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source().to_owned())
        .expect("Tolk document should open");
    let locations = service
        .references(&uri, marked.marker("caret").position, include_declaration)
        .expect("references request should succeed");
    expect.assert_eq(&format!("references={}", locations.len()));
}

macro_rules! cases {
    ($(($name:ident, $source:literal, $include:literal, $expect:literal)),+ $(,)?) => {
        $(#[test] fn $name() { check($source, $include, expect![$expect]); })+
    };
}

cases!(
    (
        function_uses,
        "fun <caret>target() {}\nfun a() { target(); }\nfun b() { target(); }",
        false,
        "references=2"
    ),
    (
        function_with_declaration,
        "fun <caret>target() {}\nfun a() { target(); }\nfun b() { target(); }",
        true,
        "references=3"
    ),
    (
        local,
        "fun main() { val <caret>value = 1; value; value; }",
        false,
        "references=2"
    ),
    (
        parameter,
        "fun main(<caret>value: int) { value + value; }",
        false,
        "references=2"
    ),
    (
        field,
        "struct Box { <caret>value: int }\nfun main(box: Box) { Box { value: 1 }; box.value; }",
        false,
        "references=2"
    ),
    (
        enum_member,
        "enum Color { <caret>Red }\nfun main() { Color.Red; Color.Red; }",
        false,
        "references=2"
    ),
    (
        constant,
        "const <caret>VALUE = 1;\nfun main() { VALUE + VALUE; }",
        false,
        "references=2"
    ),
    (
        method,
        "struct Box {}\nfun Box.<caret>open(self) {}\nfun main(a: Box, b: Box) { a.open(); b.open(); }",
        false,
        "references=2"
    ),
    (
        type_alias,
        "type <caret>Amount = int;\nfun a(value: Amount) {}\nfun b(): Amount {}",
        false,
        "references=2"
    ),
    (
        unresolved,
        "fun main() { <caret>missing; missing; }",
        true,
        "references=0"
    ),
);
