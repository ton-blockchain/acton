use super::*;

fn check(source: &str, expect: Expect) {
    let marked = MarkedSource::parse(source);
    let uri = DocumentUri::from("file:///fixture/main.tolk");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TolkLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source().to_owned())
        .expect("Tolk document should open");
    let tokens = service
        .semantic_tokens(&uri)
        .expect("semantic tokens request should succeed");
    let rendered = render_semantic_tokens(marked.source(), &tokens.data);
    let actual = if rendered == "<none>" {
        rendered
    } else {
        rendered
            .lines()
            .map(|line| {
                let kind = line
                    .split_once("kind=")
                    .and_then(|(_, rest)| rest.split_whitespace().next())
                    .unwrap_or("?");
                let text = line.split_once("text=").map_or("?", |(_, text)| text);
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
        struct_and_field,
        "struct Foo { value: int }",
        "struct:Foo\nproperty:value\ntype:int"
    ),
    (
        enum_and_member,
        "enum Color { Red }",
        "enum:Color\nenumMember:Red"
    ),
    (type_alias, "type Amount = int;", "type:Amount\ntype:int"),
    (constant, "const ANSWER = 42;", "property:ANSWER"),
    (global, "global counter: int;", "variable:counter\ntype:int"),
    (
        function_parameter,
        "fun target(value: int) {}",
        "function:target\nparameter:value\ntype:int"
    ),
    (
        method_self,
        "struct Box {}\nfun Box.open(self) {}",
        "struct:Box\nstruct:Box\nfunction:open\nkeyword:self"
    ),
    (
        catch_binding,
        "fun main() { try {} catch (error) { error; } }",
        "function:main\nvariable:error\nvariable:error"
    ),
    (
        type_parameter,
        "struct Box<T> { value: T }",
        "struct:Box\ntypeParameter:T\nproperty:value\ntypeParameter:T"
    ),
    (
        unresolved_is_excluded,
        "fun main() { missing; }",
        "function:main"
    ),
);
