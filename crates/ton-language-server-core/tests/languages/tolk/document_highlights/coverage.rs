use super::*;

fn check(source: &str, expect: Expect) {
    let marked = MarkedSource::parse(source);
    let uri = DocumentUri::from("file:///fixture/main.tolk");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TolkLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source().to_owned())
        .expect("Tolk document should open");
    let highlights = service
        .document_highlights(&uri, marked.marker("caret").position)
        .expect("document highlights request should succeed");
    let index = TextIndex::new(marked.source());
    let actual = if highlights.is_empty() {
        "<none>".to_owned()
    } else {
        highlights
            .iter()
            .map(|highlight| {
                let start = index.position_to_offset(marked.source(), highlight.range.start);
                let end = index.position_to_offset(marked.source(), highlight.range.end);
                let text = marked.source().get(start..end).unwrap_or_default();
                let kind = match highlight.kind {
                    Some(DocumentHighlightKind::Read) => "read",
                    Some(DocumentHighlightKind::Write) => "write",
                    Some(DocumentHighlightKind::Text) => "text",
                    None => "none",
                };
                format!("{kind}:{text}")
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
        local_read,
        "fun main() { val <caret>value = 1; value; }",
        "read:value\nread:value"
    ),
    (
        local_write,
        "fun main() { var <caret>value = 1; value = 2; value; }",
        "read:value\nwrite:value\nread:value"
    ),
    (
        parameter,
        "fun main(<caret>value: int) { value; }",
        "read:value\nread:value"
    ),
    (
        global,
        "global <caret>value: int;\nfun main() { value; }",
        "read:value\nread:value"
    ),
    (
        field,
        "struct Box { <caret>value: int }\nfun main(box: Box) { box.value; }",
        "read:value\nread:value"
    ),
    (
        method,
        "struct Box {}\nfun Box.<caret>open(self) {}\nfun main(box: Box) { box.open(); }",
        "read:open\nread:open"
    ),
    (
        constant,
        "const <caret>VALUE = 1;\nfun main() { VALUE; }",
        "read:VALUE\nread:VALUE"
    ),
    (
        enum_member,
        "enum Color { <caret>Red }\nfun main() { Color.Red; }",
        "read:Red\nread:Red"
    ),
    (
        catch_binding,
        "fun main() { try {} catch (<caret>error) { error; } }",
        "read:error\nread:error"
    ),
    (unresolved, "fun main() { <caret>missing; }", "<none>"),
);
