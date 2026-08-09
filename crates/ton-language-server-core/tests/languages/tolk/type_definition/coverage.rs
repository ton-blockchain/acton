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
        .type_definition(&uri, marked.marker("caret").position)
        .expect("type definition request should succeed");
    let actual = locations.first().map_or_else(
        || "<none>".to_owned(),
        |location| {
            format!(
                "{}:{}",
                location.range.start.line, location.range.start.character
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
        explicit_struct,
        "struct Box {}\nfun main() { val <caret>box: Box = {}; }",
        "0:7"
    ),
    (
        inferred_struct,
        "struct Box {}\nfun make(): Box {}\nfun main() { val <caret>box = make(); }",
        "0:7"
    ),
    (
        parameter,
        "struct Box {}\nfun main(<caret>box: Box) {}",
        "0:7"
    ),
    (
        enumeration,
        "enum Color { Red }\nfun main() { val <caret>color = Color.Red; }",
        "0:5"
    ),
    (
        alias,
        "type Amount = int;\nfun main(value: Amount) { val <caret>copy: Amount = value; }",
        "0:5"
    ),
    (
        generic_struct,
        "struct Box<T> {}\nfun main() { val <caret>box: Box<int> = {}; }",
        "0:7"
    ),
    (builtin, "fun main() { val <caret>value = 1; }", "<none>"),
    (boolean, "fun main() { val <caret>value = true; }", "<none>"),
    (tuple, "fun main() { val <caret>value = (1, 2); }", "<none>"),
    (unknown, "fun main() { <caret>missing; }", "<none>"),
);
