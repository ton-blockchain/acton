use super::*;

macro_rules! cases {
    ($(($name:ident, $source:literal, $expect:literal)),+ $(,)?) => {
        $(#[test] fn $name() { case_tolk_folding($source, expect![$expect]); })+
    };
}

cases!(
    (function, "fun main() {\n    return;\n}", "[0, 2]"),
    (structure, "struct Box {\n    value: int\n}", "[0, 2]"),
    (enumeration, "enum Color {\n    Red,\n    Blue\n}", "[0, 3]"),
    (
        if_statement,
        "fun main() {\n    if (true) {\n        return;\n    }\n}",
        "[0, 4], [1, 3]"
    ),
    (
        nested_block,
        "fun main() {\n    {\n        return;\n    }\n}",
        "[0, 4], [1, 3]"
    ),
    (
        object_literal,
        "struct Box { value: int }\nfun main() {\n    Box {\n        value: 1\n    };\n}",
        "[1, 5], [2, 4]"
    ),
    (
        match_expression,
        "fun main(value: int) {\n    match (value) {\n        0 => return,\n        else => return\n    }\n}",
        "[0, 5], [1, 4]"
    ),
    (
        match_arm,
        "fun main(value: int) {\n    match (value) {\n        0 => {\n            return;\n        }\n    }\n}",
        "[0, 6], [1, 5], [2, 4]"
    ),
    (
        contract_header,
        "contract Demo {\n    storage: Storage\n}",
        "<none>"
    ),
    (single_line, "fun main() { return; }", "<none>"),
);
