use super::*;

fn check(source: &str, expect: Expect) {
    let marked = MarkedSource::parse(source);
    let uri = DocumentUri::from("file:///fixture/main.tolk");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TolkLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source().to_owned())
        .expect("Tolk document should open");
    let help = service
        .signature_help(&uri, marked.marker("caret").position)
        .expect("signature help request should succeed");
    let actual = help
        .and_then(|help| help.signatures.into_iter().next())
        .map_or_else(
            || "<none>".to_owned(),
            |signature| {
                let active = signature
                    .active_parameter
                    .and_then(|index| signature.parameters.get(index as usize))
                    .map_or("-", String::as_str);
                format!("active={active}\n{}", signature.label)
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
        zero_parameters,
        "fun target() {}\nfun main() { target(<caret>); }",
        "active=-\nfun target()"
    ),
    (
        first_parameter,
        "fun target(a: int, b: int) {}\nfun main() { target(<caret>1, 2); }",
        "active=a: int\nfun target(a: int, b: int)"
    ),
    (
        second_parameter,
        "fun target(a: int, b: int) {}\nfun main() { target(1, <caret>2); }",
        "active=b: int\nfun target(a: int, b: int)"
    ),
    (
        third_parameter,
        "fun target(a: int, b: int, c: int) {}\nfun main() { target(1, 2, <caret>3); }",
        "active=c: int\nfun target(a: int, b: int, c: int)"
    ),
    (
        instance_method,
        "struct Box {}\nfun Box.open(self, value: int) {}\nfun main(box: Box) { box.open(<caret>1); }",
        "active=value: int\nfun Box.open(value: int)"
    ),
    (
        static_method,
        "struct Box {}\nfun Box.make(value: int) {}\nfun main() { Box.make(<caret>1); }",
        "active=value: int\nfun Box.make(value: int)"
    ),
    (
        generic_function,
        "fun target<T>(value: T) {}\nfun main() { target<int>(<caret>1); }",
        "active=value: T\nfun target(value: T)"
    ),
    (
        nested_call,
        "fun outer(a: int) {}\nfun inner(value: int) {}\nfun main() { outer(inner(<caret>1)); }",
        "active=value: int\nfun inner(value: int)"
    ),
    (
        default_parameter,
        "fun target(a: int = 1, b: int = 2) {}\nfun main() { target(1, <caret>); }",
        "active=b: int = 2\nfun target(a: int = 1, b: int = 2)"
    ),
    (
        unresolved_call,
        "fun main() { missing(<caret>1); }",
        "<none>"
    ),
);
