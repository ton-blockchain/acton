use super::*;

macro_rules! cases {
    ($(($name:ident, $source:literal, $new_name:literal, $expect:literal)),+ $(,)?) => {
        $(#[test] fn $name() { check_rename($source, $new_name, expect![$expect]); })+
    };
}

cases!(
    (
        local,
        "fun main() { val <caret>value = 1; value; }",
        "next",
        "fun main() { val next = 1; next; }"
    ),
    (
        parameter,
        "fun main(<caret>value: int) { value; }",
        "amount",
        "fun main(amount: int) { amount; }"
    ),
    (
        function,
        "fun <caret>target() {}\nfun main() { target(); }",
        "renamed",
        "fun renamed() {}\nfun main() { renamed(); }"
    ),
    (
        structure,
        "struct <caret>Box {}\nfun main(value: Box) {}",
        "Container",
        "struct Container {}\nfun main(value: Container) {}"
    ),
    (
        field,
        "struct Box { <caret>value: int }\nfun main(box: Box) { box.value; }",
        "amount",
        "struct Box { amount: int }\nfun main(box: Box) { box.amount; }"
    ),
    (
        enum_member,
        "enum Color { <caret>Red }\nfun main() { Color.Red; }",
        "Blue",
        "enum Color { Blue }\nfun main() { Color.Blue; }"
    ),
    (
        type_alias,
        "type <caret>Amount = int;\nfun main(value: Amount) {}",
        "Coins",
        "type Coins = int;\nfun main(value: Coins) {}"
    ),
    (
        backticked,
        "fun `<caret>old name`() {}\nfun main() { `old name`(); }",
        "new name",
        "fun `new name`() {}\nfun main() { `new name`(); }"
    ),
    (
        keyword,
        "fun <caret>target() {}\nfun main() { target(); }",
        "return",
        "fun `return`() {}\nfun main() { `return`(); }"
    ),
);

#[test]
fn rejects_stdlib_symbols() {
    check_rename_rejected(
        "fun main() { <caret>beginCell(); }",
        "builder",
        expect!["error: cannot rename an element from the Tolk standard library"],
    );
}
