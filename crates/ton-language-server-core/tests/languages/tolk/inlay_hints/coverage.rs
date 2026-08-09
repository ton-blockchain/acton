use super::*;

fn check(source: &str, expect: Expect) {
    let marked = MarkedSource::parse(source);
    let uri = DocumentUri::from("file:///fixture/main.tolk");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TolkLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source().to_owned())
        .expect("Tolk document should open");
    let hints = service
        .inlay_hints(&uri, full_document_range())
        .expect("inlay hints request should succeed");
    let actual = if hints.is_empty() {
        "<none>".to_owned()
    } else {
        hints
            .iter()
            .map(|hint| {
                let kind = match hint.kind {
                    Some(ton_language_server_core::InlayHintKind::Type) => "type",
                    Some(ton_language_server_core::InlayHintKind::Parameter) => "parameter",
                    None => "none",
                };
                format!("{kind}:{}", hint.label.text().trim())
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    expect.assert_eq(&actual);
}

macro_rules! cases {
    ($(($name:ident, $source:literal, $expect:literal)),+ $(,)?) => {
        $(#[test] fn $name() { check($source, expect![$expect]); })+
    };
}

cases!(
    (
        local_integer,
        "fun main(): void { val value = 1; }",
        "type:: int"
    ),
    (
        local_boolean,
        "fun main(): void { val value = true; }",
        "type:: bool"
    ),
    (
        local_cell,
        "fun main(): void { val value = beginCell().endCell(); }",
        "type:: cell"
    ),
    (function_return, "fun answer() { return 42; }", "type:: int"),
    (
        computed_constant,
        "const VALUE = 1 + 2;",
        "type:: int\nnone:/* = 3 (0x3) */"
    ),
    (
        enum_values,
        "enum Color { Red, Blue }",
        "parameter:= 0\nparameter:= 1"
    ),
    (
        get_method_id,
        "get fun seqno(): int { return 0; }",
        "type:(0x14c97)"
    ),
    (
        parameter_name,
        "fun dispatch(params: int): void {}\nfun main(): void { dispatch(1); }",
        "parameter:params:"
    ),
    (
        generic_method_parameter,
        "struct Box {}\nfun Box.set<T>(self, params: int): void {}\nfun main(box: Box): void { box.set<int>(1); }",
        "parameter:params:"
    ),
    (
        explicit_types,
        "const VALUE: int = 1;\nfun main(): void { val value: int = VALUE; }",
        "<none>"
    ),
);
