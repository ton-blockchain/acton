use super::*;

fn check(source: &str, query: &str, expect: Expect) {
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TolkLanguage::new());
    service
        .add_source_file(LANGUAGE_ID, "file:///fixture/main.tolk", source)
        .expect("Tolk source should be added");
    let symbols = service
        .workspace_symbols(query)
        .expect("workspace symbols request should succeed");
    let actual = if symbols.is_empty() {
        "<none>".to_owned()
    } else {
        symbols
            .iter()
            .map(|symbol| format!("{:?}:{}", symbol.kind, symbol.name))
            .collect::<Vec<_>>()
            .join("\n")
    };
    expect.assert_eq(&actual);
}

macro_rules! cases {
    ($(($name:ident, $source:literal, $query:literal, $expect:literal)),+ $(,)?) => {
        $(#[test] fn $name() { check($source, $query, expect![$expect]); })+
    };
}

cases!(
    (
        function,
        "fun SearchFunction() {}",
        "SearchFunction",
        "Function:SearchFunction"
    ),
    (
        case_insensitive,
        "fun MixedCaseNeedle() {}",
        "mixedcaseneedle",
        "Function:MixedCaseNeedle"
    ),
    (
        structure,
        "struct SearchStruct {}",
        "SearchStruct",
        "Struct:SearchStruct"
    ),
    (
        enumeration,
        "enum SearchEnum { Member }",
        "SearchEnum",
        "Enum:SearchEnum\nEnumMember:SearchEnum.Member"
    ),
    (
        enum_member,
        "enum SearchEnum { SearchMember }",
        "SearchMember",
        "EnumMember:SearchEnum.SearchMember"
    ),
    (
        alias,
        "type SearchAlias = int;",
        "SearchAlias",
        "TypeParameter:SearchAlias"
    ),
    (
        constant,
        "const SearchConstant = 1;",
        "SearchConstant",
        "Constant:SearchConstant"
    ),
    (
        global,
        "global SearchGlobal: int;",
        "SearchGlobal",
        "Variable:SearchGlobal"
    ),
    (
        get_method,
        "get SearchGetter(): int {}",
        "SearchGetter",
        "Event:get SearchGetter"
    ),
    (no_match, "fun Existing() {}", "Missing", "<none>"),
);
