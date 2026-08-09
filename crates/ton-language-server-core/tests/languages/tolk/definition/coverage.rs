use super::*;

fn check(source: &str, expect: Expect) {
    let marked = MarkedSource::parse(source);
    let uri = DocumentUri::from("file:///fixture/main.tolk");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TolkLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source().to_owned())
        .expect("Tolk document should open");
    let locations = service
        .definition(&uri, marked.marker("caret").position)
        .expect("definition request should succeed");
    let actual = locations.first().map_or_else(
        || "<none>".to_owned(),
        |location| {
            format!(
                "{}:{}:{}",
                location.uri, location.range.start.line, location.range.start.character
            )
        },
    );
    expect.assert_eq(&actual);
}

macro_rules! cases {
    ($(($name:ident, $source:literal, $expect:literal)),+ $(,)?) => {
        $(#[test] fn $name() { check($source, expect![$expect]); })+
    };
}

cases!(
    (
        function,
        "fun target() {}\nfun main() { <caret>target(); }",
        "file:///fixture/main.tolk:0:4"
    ),
    (
        structure,
        "struct Target {}\nfun main(value: <caret>Target) {}",
        "file:///fixture/main.tolk:0:7"
    ),
    (
        enum_member,
        "enum Color { Red }\nfun main() { Color.<caret>Red; }",
        "file:///fixture/main.tolk:0:13"
    ),
    (
        constant,
        "const VALUE = 1;\nfun main() { <caret>VALUE; }",
        "file:///fixture/main.tolk:0:6"
    ),
    (
        global,
        "global value: int;\nfun main() { <caret>value; }",
        "file:///fixture/main.tolk:0:7"
    ),
    (
        method,
        "struct Box {}\nfun Box.open(self) {}\nfun main(box: Box) { box.<caret>open(); }",
        "file:///fixture/main.tolk:1:8"
    ),
    (
        field,
        "struct Box { value: int }\nfun main(box: Box) { box.<caret>value; }",
        "file:///fixture/main.tolk:0:13"
    ),
    (
        local,
        "fun main() {\n    val value = 1;\n    <caret>value;\n}",
        "file:///fixture/main.tolk:1:8"
    ),
    (
        parameter,
        "fun main(value: int) {\n    <caret>value;\n}",
        "file:///fixture/main.tolk:0:9"
    ),
    (unresolved, "fun main() { <caret>missing; }", "<none>"),
);
