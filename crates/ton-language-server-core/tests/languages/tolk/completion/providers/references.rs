use super::support::CompletionTest;
use expect_test::expect;
use ton_language_server_core::CompletionItemKind;

#[test]
fn excludes_already_initialized_fields_in_multiline_literals() {
    // Fields already present in a multiline literal are not offered again.
    CompletionTest::new(
        "
            struct Storage {
                counter: int
                id: int
            }
            fun int.aaa(): Storage {
                return Storage {
                    counter: 10,
                    <caret>
                };
            }
        ",
    )
    .labels(&["counter", "id"])
    .check(expect![[r#"
        label  kind      detail  edit     text
        id     Property          7:8-7:8  id: $1,$0"#]]);
}

#[test]
fn completes_fields_in_short_literal_from_expected_return_type() {
    // A short literal gets its fields from the enclosing function return type.
    CompletionTest::new(
        "
            struct Storage {
                counter: int
                id: int
            }
            fun int.aaa(): Storage {
                return {
                    <caret>
                };
            }
        ",
    )
    .labels(&["counter", "id"])
    .check(expect![[r#"
        label    kind      detail  edit     text
        counter  Property          6:8-6:8  counter: $1,$0
        id       Property          6:8-6:8  id: $1,$0"#]]);
}

#[test]
fn completes_struct_initializer_fields_in_all_expected_contexts() {
    // A named struct literal offers its first missing field.
    CompletionTest::new(
        "
            struct Foo { age: int }
            fun main() { Foo { <caret> }; }
        ",
    )
    .labels(&["age"])
    .check(expect![[r#"
        label  kind      detail  edit       text
        age    Property          1:19-1:19  age: $1$0"#]]);

    // A short literal gets its expected struct type from a variable declaration.
    CompletionTest::new(
        "
            struct Foo { age: int }
            fun main() { val foo: Foo = { <caret> }; }
        ",
    )
    .labels(&["age"])
    .check(expect![[r#"
        label  kind      detail  edit       text
        age    Property          1:30-1:30  age: $1$0"#]]);

    // A short literal gets its expected struct type from a function argument.
    CompletionTest::new(
        "
            struct Foo { age: int }
            fun takeFoo(foo: Foo) {}
            fun main() { takeFoo({ <caret> }); }
        ",
    )
    .labels(&["age"])
    .check(expect![[r#"
        label  kind      detail  edit       text
        age    Property          2:23-2:23  age: $1$0"#]]);

    // A matching local is offered alongside the explicit field initializer.
    CompletionTest::new(
        "
            struct Foo { age: int }
            fun main() { val age = 10; Foo { <caret> }; }
        ",
    )
    .labels(&["age"])
    .check(expect![[r#"
        label  kind      detail  edit       text
        age    Property          1:33-1:33  age: $1$0
        age    Variable  : int   1:33-1:33  age"#]]);

    // A matching parameter is offered alongside the explicit field initializer.
    CompletionTest::new(
        "
            struct Foo { age: int }
            fun main(age: int) { Foo { <caret> }; }
        ",
    )
    .labels(&["age"])
    .check(expect![[r#"
        label  kind      detail  edit       text
        age    Property          1:27-1:27  age: $1$0
        age    Variable  : int   1:27-1:27  age"#]]);

    // An already initialized field is excluded even when completion occurs before a later field.
    CompletionTest::new(
        "
            struct Foo { age: int, value: int }
            fun main() { Foo { a<caret>, value: 10 }; }
        ",
    )
    .labels(&["age", "value"])
    .check(expect![[r#"
        label  kind      detail  edit       text
        age    Property          1:19-1:20  age: $1$0"#]]);

    // A field value is an expression context rather than another field-name position.
    CompletionTest::new(
        "
            struct Foo { age: int }
            fun main(value: int) { Foo { age: va<caret> }; }
        ",
    )
    .labels(&["age", "value"])
    .check(expect![[r#"
        label  kind      detail  edit       text
        value  Variable  : int   1:34-1:36  value"#]]);

    // Generic function argument inference supplies the short literal's struct type.
    CompletionTest::new(
        "
            struct Options<T> { enabled: bool, body: T }
            fun create<T>(options: Options<T>) {}
            fun main() { create({ <caret> }); }
        ",
    )
    .labels(&["enabled", "body"])
    .check(expect!["<none>"]);

    // A default parameter value supplies the short literal's struct type.
    CompletionTest::new(
        "
            struct Foo { first: int, second: slice }
            fun main(foo: Foo = { <caret> }) {}
        ",
    )
    .labels(&["first", "second"])
    .check(expect![[r#"
        label   kind      detail  edit       text
        first   Property          1:22-1:22  first: $1$0
        second  Property          1:22-1:22  second: $1$0"#]]);
}

#[test]
fn applies_struct_initializer_fields_and_shorthand_values() {
    // Applying a field in a single-line literal inserts its value placeholder without a comma.
    CompletionTest::new(
        "
            struct Foo { age: int }
            fun main() { Foo { <caret> }; }
        ",
    )
    .check_applied_kind(
        "age",
        CompletionItemKind::Property,
        expect![[r#"
            struct Foo { age: int }
            fun main() { Foo { age: <caret> }; }"#]],
    );

    // Applying a field in a multiline literal includes its trailing comma.
    CompletionTest::new(
        "
            struct Foo { age: int }
            fun main() {
                Foo {
                    <caret>
                };
            }
        ",
    )
    .check_applied_kind(
        "age",
        CompletionItemKind::Property,
        expect![[r#"
            struct Foo { age: int }
            fun main() {
                Foo {
                    age: <caret>,
                };
            }"#]],
    );

    // Applying a matching local uses shorthand syntax in a single-line literal.
    CompletionTest::new(
        "
            struct Foo { age: int }
            fun main() { val age = 10; Foo { <caret> }; }
        ",
    )
    .check_applied_kind(
        "age",
        CompletionItemKind::Variable,
        expect![[r#"
            struct Foo { age: int }
            fun main() { val age = 10; Foo { age<caret> }; }"#]],
    );

    // Applying a matching local in a multiline literal includes its trailing comma.
    CompletionTest::new(
        "
            struct Foo { age: int }
            fun main() {
                val age = 10;
                Foo {
                    <caret>
                };
            }
        ",
    )
    .check_applied_kind(
        "age",
        CompletionItemKind::Variable,
        expect![[r#"
            struct Foo { age: int }
            fun main() {
                val age = 10;
                Foo {
                    age,<caret>
                };
            }"#]],
    );

    // Applying a matching parameter uses shorthand syntax.
    CompletionTest::new(
        "
            struct Foo { age: int }
            fun main(age: int) { Foo { <caret> }; }
        ",
    )
    .check_applied_kind(
        "age",
        CompletionItemKind::Variable,
        expect![[r#"
            struct Foo { age: int }
            fun main(age: int) { Foo { age<caret> }; }"#]],
    );

    // Applying the second missing field preserves the first initializer.
    CompletionTest::new(
        "
            struct Foo { age: int, value: int }
            fun main() { Foo { age: 10, <caret> }; }
        ",
    )
    .check_applied_kind(
        "value",
        CompletionItemKind::Property,
        expect![[r#"
            struct Foo { age: int, value: int }
            fun main() { Foo { age: 10, value: <caret> }; }"#]],
    );
}

#[test]
fn completes_visible_locals_members_and_backticked_symbols() {
    // Prefix filtering keeps visible locals and parameters from the current scope.
    CompletionTest::new("fun main(parameter: int) { val local = 1; loc<caret>; }")
        .labels(&["local", "parameter"])
        .check(expect![[r#"
            label      kind      detail  edit       text
            local      Variable  : int   0:42-0:45  local
            parameter  Variable  : int   0:42-0:45  parameter"#]]);

    // Member completion uses the inferred type of the qualifier.
    CompletionTest::new(
        "
            struct Foo { first: int, second: int }
            fun main(foo: Foo) { foo.<caret>; }
        ",
    )
    .labels(&["first", "second"])
    .trigger_character(".")
    .check(expect![[r#"
        label   kind      detail      edit       text
        first   Property  Foo.first   1:25-1:25  first
        second  Property  Foo.second  1:25-1:25  second"#]]);

    // Member completion remains available inside an incomplete assert expression.
    CompletionTest::new(
        "
            struct Foo { first: int, second: slice }
            fun main(foo: Foo) { assert(foo.<caret>) }
        ",
    )
    .labels(&["first", "second"])
    .trigger_character(".")
    .check(expect!["<none>"]);

    // A backticked function replaces the complete quoted identifier.
    CompletionTest::new(
        "
            fun `calculate total`(): int { return 1 }
            fun main() { `calculate t<caret>`(); }
        ",
    )
    .labels(&["calculate total"])
    .check(expect![[r#"
        label            kind      detail           edit       text
        calculate total  Function  calculate total  1:13-1:26  `calculate total`$0"#]]);
}

#[test]
fn completes_locals_from_destructuring_catch_and_match_scopes() {
    // Tuple destructuring contributes every declared local with its inferred type.
    CompletionTest::new(
        r#"
            fun main() {
                val [some, someOther] = [10, "hello"];
                some<caret>
            }
        "#,
    )
    .labels(&["some", "someOther"])
    .check(expect![[r#"
        label      kind      detail    edit     text
        some       Variable  : int     2:4-2:8  some
        someOther  Variable  : string  2:4-2:8  someOther"#]]);

    // Both catch variables are visible inside the catch body, including unknown types.
    CompletionTest::new(
        "
            fun main() {
                try {} catch (error, errorData) {
                    err<caret>
                }
            }
        ",
    )
    .labels(&["error", "errorData"])
    .check(expect![[r#"
        label      kind      detail     edit      text
        error      Variable  : int      2:8-2:11  error
        errorData  Variable  : unknown  2:8-2:11  errorData"#]]);

    // A value declared by a match expression is visible only inside its arms.
    CompletionTest::new(
        "
            fun main() {
                match (val data = 100) {
                    10 => { dat<caret> }
                }
            }
        ",
    )
    .labels(&["data"])
    .check(expect![[r#"
        label  kind      detail  edit       text
        data   Variable  : int   2:16-2:19  data"#]]);

    // Locals remain available inside nested match-arm blocks.
    CompletionTest::new(
        "
            fun main() {
                val some = 10;
                val someOther = 20;
                match (some) {
                    10 => { some<caret> }
                }
            }
        ",
    )
    .labels(&["some", "someOther"])
    .check(expect![[r#"
        label      kind      detail  edit       text
        some       Variable  : int   4:16-4:20  some
        someOther  Variable  : int   4:16-4:20  someOther"#]]);
}

#[test]
fn completes_references_in_incomplete_expressions() {
    // An incomplete if condition still resolves parameters from the enclosing function.
    CompletionTest::new(
        "
            fun main(someParameter: int) {
                if (some<caret>)
            }
        ",
    )
    .labels(&["someParameter"])
    .check(expect![[r#"
        label          kind      detail  edit      text
        someParameter  Variable  : int   1:8-1:12  someParameter"#]]);

    // An incomplete match condition preserves the same lexical scope.
    CompletionTest::new(
        "
            fun main(someParameter: int) {
                match (some<caret>)
            }
        ",
    )
    .labels(&["someParameter"])
    .check(expect![[r#"
        label          kind      detail  edit       text
        someParameter  Variable  : int   1:11-1:15  someParameter"#]]);

    // An incomplete assert condition also remains an expression context.
    CompletionTest::new(
        "
            fun main(someParameter: int) {
                assert(some<caret>)
            }
        ",
    )
    .labels(&["someParameter"])
    .check(expect![[r#"
        label          kind      detail  edit       text
        someParameter  Variable  : int   1:11-1:15  someParameter"#]]);
}

#[test]
fn separates_type_parameters_from_value_completion() {
    // Function type parameters are offered in a type position.
    CompletionTest::new("fun foo<TName, TValue>(value: TN<caret>) {}")
        .labels(&["TName", "TValue"])
        .check(expect![[r#"
            label   kind           detail    edit       text
            TName   TypeParameter  : TName   0:30-0:32  TName
            TValue  TypeParameter  : TValue  0:30-0:32  TValue"#]]);

    // Every function type parameter is offered for a shared prefix.
    CompletionTest::new("fun generic<TName, TValue, TOther>(): T<caret> {}")
        .labels(&["TName", "TValue", "TOther"])
        .check(expect![[r#"
            label   kind           detail    edit       text
            TName   TypeParameter  : TName   0:38-0:39  TName
            TOther  TypeParameter  : TOther  0:38-0:39  TOther
            TValue  TypeParameter  : TValue  0:38-0:39  TValue"#]]);

    // A type parameter with a default remains available by its declared name.
    CompletionTest::new("fun generic<TName = int>(): TNam<caret> {}")
        .labels(&["TName"])
        .check(expect![[r#"
            label  kind           detail   edit       text
            TName  TypeParameter  : TName  0:28-0:32  TName"#]]);

    // Struct type parameters are visible in field type declarations.
    CompletionTest::new(
        "
            struct Box<TName, TValue> {
                value: TN<caret>
            }
        ",
    )
    .labels(&["TName", "TValue"])
    .check(expect![[r#"
        label   kind           detail    edit       text
        TName   TypeParameter  : TName   1:11-1:13  TName
        TValue  TypeParameter  : TValue  1:11-1:13  TValue"#]]);

    // Every struct type parameter is visible in a field type.
    CompletionTest::new(
        "
            struct Generic<TName, TValue, TOther> {
                field: T<caret>
            }
        ",
    )
    .labels(&["TName", "TValue", "TOther"])
    .check(expect![[r#"
        label   kind           detail    edit       text
        TName   TypeParameter  : TName   1:11-1:12  TName
        TOther  TypeParameter  : TOther  1:11-1:12  TOther
        TValue  TypeParameter  : TValue  1:11-1:12  TValue"#]]);

    // Type parameters are not valid expression values.
    CompletionTest::new("fun foo<TName>() { TNam<caret>; }")
        .labels(&["TName"])
        .check(expect!["<none>"]);
}

#[test]
fn applies_receiver_type_parameter_completion() {
    // A method receiver type parameter replaces the typed return-type prefix.
    CompletionTest::new(
        "
            struct Foo<T> {}
            fun Foo<TName>.foo(): TNa<caret> {}
        ",
    )
    .check_applied(
        "TName",
        expect![[r#"
            struct Foo<T> {}
            fun Foo<TName>.foo(): TName<caret> {}"#]],
    );
}

#[test]
fn completes_global_symbol_kinds_in_matching_contexts() {
    // Constants and global variables participate in expression completion.
    CompletionTest::new(
        "
            const MAX_VALUE: int = 10
            global globalValue: int
            fun main() { val value = MAX<caret>; }
        ",
    )
    .labels(&["MAX_VALUE", "globalValue"])
    .check(expect![[r#"
        label        kind      detail       edit       text
        MAX_VALUE    Constant  MAX_VALUE    2:25-2:28  MAX_VALUE
        globalValue  Variable  globalValue  2:25-2:28  globalValue"#]]);

    // Constants with inferred non-integer types remain available together.
    CompletionTest::new(
        r#"
            const CONSTANT_TEXT = ""
            const CONSTANT_FLAG = true
            fun main() { CONSTANT<caret> }
        "#,
    )
    .labels(&["CONSTANT_TEXT", "CONSTANT_FLAG"])
    .check(expect![[r#"
        label          kind      detail         edit       text
        CONSTANT_FLAG  Constant  CONSTANT_FLAG  2:13-2:21  CONSTANT_FLAG
        CONSTANT_TEXT  Constant  CONSTANT_TEXT  2:13-2:21  CONSTANT_TEXT"#]]);

    // Multiple globals sharing a prefix are all offered.
    CompletionTest::new(
        "
            global globalFirst: int
            global globalSecond: int
            fun main() { global<caret> }
        ",
    )
    .labels(&["globalFirst", "globalSecond"])
    .check(expect![[r#"
        label         kind      detail        edit       text
        globalFirst   Variable  globalFirst   2:13-2:19  globalFirst
        globalSecond  Variable  globalSecond  2:13-2:19  globalSecond"#]]);

    // Structs are inserted as types in a type position.
    CompletionTest::new(
        "
            struct Storage { value: int }
            fun main(value: Sto<caret>) {}
        ",
    )
    .labels(&["Storage"])
    .check(expect![[r#"
            label    kind    detail   edit       text
            Storage  Struct  Storage  1:16-1:19  Storage"#]]);

    // Non-empty structs are inserted as object literals in expression positions.
    CompletionTest::new(
        "
            struct Storage { value: int }
            fun main() { val value = Sto<caret>; }
        ",
    )
    .labels(&["Storage"])
    .check(expect![[r#"
            label    kind    detail   edit       text
            Storage  Struct  Storage  1:25-1:28  Storage {$1}$0"#]]);

    // Empty structs do not receive an unnecessary object-literal placeholder.
    CompletionTest::new(
        "
            struct Empty {}
            fun main() { val value = Emp<caret>; }
        ",
    )
    .labels(&["Empty"])
    .check(expect![[r#"
            label  kind    detail  edit       text
            Empty  Struct  Empty   1:25-1:28  Empty"#]]);

    // Enums are available both as types and as expression qualifiers.
    CompletionTest::new(
        "
            enum Color { Red, Blue }
            fun main(value: Colo<caret>) {}
        ",
    )
    .labels(&["Color"])
    .check(expect![[r#"
        label  kind  detail  edit       text
        Color  Enum  Color   1:16-1:20  Color"#]]);

    // An enum expression offers the enum itself and its qualified members.
    CompletionTest::new(
        "
            enum Color { Red, Blue }
            fun main() { Colo<caret> }
        ",
    )
    .labels(&["Color", "Color.Red", "Color.Blue"])
    .check(expect![[r#"
        label       kind        detail  edit       text
        Color.Blue  EnumMember          1:13-1:17  Color.Blue
        Color.Red   EnumMember          1:13-1:17  Color.Red
        Color       Enum        Color   1:13-1:17  Color"#]]);

    // Type aliases are available as types.
    CompletionTest::new(
        "
            type Amount = int;
            fun main(value: Amo<caret>) {}
        ",
    )
    .labels(&["Amount"])
    .check(expect![[r#"
            label   kind           detail  edit       text
            Amount  TypeParameter  Amount  1:16-1:19  Amount"#]]);

    // Type aliases are also available as expression/static-receiver values.
    CompletionTest::new(
        "
            type Amount = int;
            fun main() { Amo<caret>; }
        ",
    )
    .labels(&["Amount"])
    .check(expect![[r#"
        label   kind           detail  edit       text
        Amount  TypeParameter  Amount  1:13-1:16  Amount"#]]);

    // A struct field type position exposes both local and stdlib types.
    CompletionTest::new(
        "
            struct Foo {
                value: Fo<caret>
            }
        ",
    )
    .labels(&["Foo", "int", "Cell"])
    .check(expect![[r#"
        label  kind           detail  edit       text
        Foo    Struct         Foo     1:11-1:13  Foo
        Cell   Struct         Cell    1:11-1:13  Cell
        int    TypeParameter  int     1:11-1:13  int"#]]);
}

#[test]
fn resolves_alias_and_generic_member_types() {
    // Member lookup unwraps a direct alias to its underlying struct.
    CompletionTest::new(
        "
            struct Foo { first: int, second: slice }
            type Alias = Foo;
            fun main() { val foo = Alias {}; foo.<caret>; }
        ",
    )
    .labels(&["first", "second"])
    .trigger_character(".")
    .check(expect![[r#"
        label   kind      detail      edit       text
        first   Property  Foo.first   2:37-2:37  first
        second  Property  Foo.second  2:37-2:37  second"#]]);

    // Generic struct instantiations expose the fields of their base type.
    CompletionTest::new(
        "
            struct Foo<T> { first: T, second: slice }
            fun main() { val foo = Foo<int> {}; foo.<caret>; }
        ",
    )
    .labels(&["first", "second"])
    .trigger_character(".")
    .check(expect![[r#"
        label   kind      detail      edit       text
        first   Property  Foo.first   1:40-1:40  first
        second  Property  Foo.second  1:40-1:40  second"#]]);

    // An alias of an instantiated generic struct exposes its base fields.
    CompletionTest::new(
        "
            struct Foo<T> { first: T, second: slice }
            type IntFoo = Foo<int>
            fun main() { val foo = IntFoo {}; foo.<caret>; }
        ",
    )
    .labels(&["first", "second"])
    .trigger_character(".")
    .check(expect![[r#"
        label   kind      detail      edit       text
        first   Property  Foo.first   2:38-2:38  first
        second  Property  Foo.second  2:38-2:38  second"#]]);

    // Private fields are excluded from member completion outside the owner.
    CompletionTest::new(
        "
            struct Foo { first: int, private second: slice }
            fun main() { val foo = Foo {}; foo.<caret>; }
        ",
    )
    .labels(&["first", "second"])
    .trigger_character(".")
    .check(expect![[r#"
        label  kind      detail     edit       text
        first  Property  Foo.first  1:35-1:35  first"#]]);
}

#[test]
fn completes_static_and_generic_methods_for_compatible_receivers() {
    // Every static method declared for a concrete struct is offered after its type.
    CompletionTest::new(
        "
            struct Foo {}
            fun Foo.bar() {}
            fun Foo.baz() {}
            fun Foo.bad() {}
            fun main() { Foo.ba<caret> }
        ",
    )
    .labels(&["bar", "baz", "bad"])
    .trigger_character(".")
    .check(expect![[r#"
        label  kind    detail   edit       text
        bad    Method  Foo.bad  4:17-4:19  bad();$0
        bar    Method  Foo.bar  4:17-4:19  bar();$0
        baz    Method  Foo.baz  4:17-4:19  baz();$0"#]]);

    // Methods from an unrelated generic receiver are not offered.
    CompletionTest::new(
        "
            struct Second<T> {}
            fun Second<T>.new(): Second<T> { return Second<T> {} }
            fun main() { First<int>.<caret> }
        ",
    )
    .labels(&["new"])
    .trigger_character(".")
    .check(expect!["<none>"]);

    // A generic method is offered for a compatible instantiated struct receiver.
    CompletionTest::new(
        "
            struct Second<T> {}
            fun Second<T>.new(): Second<T> { return Second<T> {} }
            fun Second<int>.new(): Second<int> { return Second<int> {} }
            fun main() { Second<int>.<caret> }
        ",
    )
    .labels(&["new"])
    .trigger_character(".")
    .check(expect![[r#"
        label  kind    detail           edit       text
        new    Method  Second<int>.new  3:25-3:25  new();$0"#]]);

    // Generic and concrete instance methods are both available on a compatible value.
    CompletionTest::new(
        "
            struct Collection<T> {}
            fun Collection<T>.add(self) {}
            fun Collection<int>.addInt(self) {}
            fun main(value: Collection<int>) { value.a<caret> }
        ",
    )
    .labels(&["add", "addInt"])
    .trigger_character(".")
    .check(expect![[r#"
        label   kind    detail                  edit       text
        add     Method  Collection<T>.add       3:41-3:42  add();$0
        addInt  Method  Collection<int>.addInt  3:41-3:42  addInt();$0"#]]);

    // Static generic methods retain their type parameters in completion detail.
    CompletionTest::new(
        "
            struct Foo {}
            fun Foo.bar<T>(value: T) {}
            fun Foo.baz<T, U>(value: T): U {}
            fun main() { Foo.ba<caret> }
        ",
    )
    .labels(&["bar", "baz"])
    .trigger_character(".")
    .check(expect![[r#"
        label  kind    detail   edit       text
        bar    Method  Foo.bar  3:17-3:19  bar(${1:value});$0
        baz    Method  Foo.baz  3:17-3:19  baz(${1:value});$0"#]]);
}

#[test]
fn completes_methods_through_aliases_and_smart_casts() {
    // An alias value receives methods declared on both the alias and its base type.
    CompletionTest::new(
        "
            type Alias = int
            fun int.subtract(self, value: int): int { return self - value }
            fun Alias.add(self, value: int): int { return self + value }
            fun main(alias: Alias) { alias.<caret> }
        ",
    )
    .labels(&["subtract", "add"])
    .trigger_character(".")
    .check(expect![[r#"
        label     kind    detail        edit       text
        add       Method  Alias.add     3:31-3:31  add(${1:value});$0
        subtract  Method  int.subtract  3:31-3:31  subtract(${1:value});$0"#]]);

    // A chain of aliases receives methods declared at every compatible level.
    CompletionTest::new(
        "
            type Alias = int
            type AliasForAlias = Alias
            fun int.multiply(self, value: int): int { return self * value }
            fun Alias.add(self, value: int): int { return self + value }
            fun AliasForAlias.subtract(self, value: int): int { return self - value }
            fun main(alias: AliasForAlias) { alias.<caret> }
        ",
    )
    .labels(&["multiply", "add", "subtract"])
    .trigger_character(".")
    .check(expect![[r#"
        label     kind    detail                  edit       text
        add       Method  Alias.add               5:39-5:39  add(${1:value});$0
        multiply  Method  int.multiply            5:39-5:39  multiply(${1:value});$0
        subtract  Method  AliasForAlias.subtract  5:39-5:39  subtract(${1:value});$0"#]]);

    // A nullable alias does not expose methods requiring its non-null base type.
    CompletionTest::new(
        "
            type Alias = int
            type OptionalAlias = Alias?
            fun Alias.add(self, value: int): int { return self + value }
            fun main(alias: OptionalAlias) { alias.<caret> }
        ",
    )
    .labels(&["add"])
    .trigger_character(".")
    .check(expect!["<none>"]);

    // A null check smart-casts a nullable field before member completion.
    CompletionTest::new(
        "
            struct Data { data: Cell<int>? }
            fun main(value: Data) {
                if (value.data == null) { return }
                value.data.<caret>
            }
        ",
    )
    .labels(&["beginParse", "tvmCell"])
    .trigger_character(".")
    .check(expect![[r#"
        label       kind    detail              edit       text
        beginParse  Method  Cell<T>.beginParse  3:15-3:15  beginParse();$0"#]]);
}

#[test]
fn hides_private_cell_storage_fields_from_member_completion() {
    // Cell's internal tvmCell storage field is not a user-visible member.
    CompletionTest::new(
        "
            struct Data { data: Cell<int> }
            fun main(value: Data) { value.data.tvm<caret> }
        ",
    )
    .labels(&["tvmCell"])
    .trigger_character(".")
    .check(expect!["<none>"]);
}

#[test]
fn completes_methods_on_string_literals() {
    // String literals have the built-in string type and expose common stdlib methods.
    CompletionTest::new(
        r#"
            fun main() {
                val valid = "abc-123".beginP<caret>;
            }
        "#,
    )
    .labels(&["beginParse"])
    .trigger_character(".")
    .check(expect![[r#"
        label       kind    detail             edit       text
        beginParse  Method  string.beginParse  1:26-1:32  beginParse()$0"#]]);
}

#[test]
fn excludes_declaration_names_and_internal_symbols() {
    // A local declaration name is not treated as a reference-completion position.
    CompletionTest::new("fun main() { val from<caret> = 10; }")
        .labels(&["fromCell", "from"])
        .check(expect!["<none>"]);

    // Internal functions whose names start with double underscores stay hidden.
    CompletionTest::new(
        "
            fun __hidden() {}
            fun main() { __h<caret>; }
        ",
    )
    .labels(&["__hidden"])
    .check(expect!["<none>"]);

    // Wildcard locals are never offered as references.
    CompletionTest::new("fun main() { val _ = 10; <caret> }")
        .labels(&["_"])
        .check(expect!["<none>"]);

    // Acton test get methods with a space after test are hidden from references.
    CompletionTest::new(
        "
            get fun `test counter`(): int { return 0 }
            fun main() { test<caret>; }
        ",
    )
    .labels(&["test counter"])
    .check(expect!["<none>"]);

    // Acton test get methods with an underscore after test are hidden from references.
    CompletionTest::new(
        "
            get fun test_counter(): int { return 0 }
            fun main() { test<caret>; }
        ",
    )
    .labels(&["test_counter"])
    .check(expect!["<none>"]);

    // Acton test get methods with a hyphen after test are hidden from references.
    CompletionTest::new(
        "
            get fun `test-counter`(): int { return 0 }
            fun main() { test<caret>; }
        ",
    )
    .labels(&["test-counter"])
    .check(expect!["<none>"]);

    // Typing a top-level declaration name does not leak ordinary references.
    CompletionTest::new("struct Stor<caret> {}")
        .labels(&["Storage", "struct"])
        .check(expect!["<none>"]);
}

#[test]
fn excludes_every_declaration_name_position() {
    // Contract declaration names do not receive completion.
    CompletionTest::new("contract from<caret> {}").check(expect!["<none>"]);

    // Struct declaration names do not receive completion.
    CompletionTest::new("struct from<caret> {}").check(expect!["<none>"]);

    // Enum declaration names do not receive completion.
    CompletionTest::new("enum from<caret> {}").check(expect!["<none>"]);

    // Type-alias declaration names do not receive completion.
    CompletionTest::new("type from<caret> = int;").check(expect!["<none>"]);

    // Global-variable declaration names do not receive completion.
    CompletionTest::new("global from<caret>: int;").check(expect!["<none>"]);

    // Constant declaration names do not receive completion.
    CompletionTest::new("const from<caret> = 10;").check(expect!["<none>"]);

    // Struct-field declaration names do not receive completion.
    CompletionTest::new("struct Foo { from<caret>: int }").check(expect![[r#"
        label     kind     detail  edit       text
        private   Keyword          0:13-0:17  private 
        readonly  Keyword          0:13-0:17  readonly "#]]);

    // Function parameter declaration names do not receive completion.
    CompletionTest::new("fun foo(from<caret>: int) {}").check(expect!["<none>"]);

    // Type-parameter declaration names do not receive completion.
    CompletionTest::new("fun foo<from<caret>>() {}").check(expect!["<none>"]);

    // Get-method declaration names do not receive completion.
    CompletionTest::new("get fun from<caret>() {}").check(expect!["<none>"]);
}

#[test]
fn applies_function_completions_in_statement_and_expression_contexts() {
    // A no-argument function used as a statement receives parentheses and a semicolon.
    CompletionTest::new(
        "
            fun foo() {}
            fun main() { fo<caret> }
        ",
    )
    .check_applied(
        "foo",
        expect![[r#"
            fun foo() {}
            fun main() { foo();<caret> }"#]],
    );

    // A pre-existing semicolon is preserved rather than duplicated.
    CompletionTest::new(
        "
            fun foo() {}
            fun main() { fo<caret>; }
        ",
    )
    .check_applied(
        "foo",
        expect![[r#"
            fun foo() {}
            fun main() { foo()<caret>; }"#]],
    );

    // A no-argument function used as an initializer does not receive a semicolon.
    CompletionTest::new(
        "
            fun foo() {}
            fun main() { val value = fo<caret>; }
        ",
    )
    .check_applied(
        "foo",
        expect![[r#"
            fun foo() {}
            fun main() { val value = foo()<caret>; }"#]],
    );

    // Existing call parentheses are preserved rather than duplicated.
    CompletionTest::new(
        "
            fun foo() {}
            fun main() { fo<caret>() }
        ",
    )
    .check_applied(
        "foo",
        expect![[r#"
            fun foo() {}
            fun main() { foo<caret>() }"#]],
    );

    // Function parameters become ordered snippet tab stops.
    CompletionTest::new(
        "
            fun foo(first: int, second: int) {}
            fun main() { fo<caret> }
        ",
    )
    .check_applied(
        "foo",
        expect![[r#"
            fun foo(first: int, second: int) {}
            fun main() { foo(first<caret>, second); }"#]],
    );

    // Function arguments are inserted before an existing statement semicolon.
    CompletionTest::new(
        "
            fun foo(value: int) {}
            fun main() { fo<caret>; }
        ",
    )
    .check_applied(
        "foo",
        expect![[r#"
            fun foo(value: int) {}
            fun main() { foo(value<caret>); }"#]],
    );

    // Function arguments are inserted when the call is used as an initializer.
    CompletionTest::new(
        "
            fun foo(value: int): int { return value }
            fun main() { val value = fo<caret> }
        ",
    )
    .check_applied(
        "foo",
        expect![[r#"
            fun foo(value: int): int { return value }
            fun main() { val value = foo(value<caret>); }"#]],
    );

    // Existing call parentheses suppress generated argument placeholders.
    CompletionTest::new(
        "
            fun foo(value: int): int { return value }
            fun main() { val value = fo<caret>() }
        ",
    )
    .check_applied(
        "foo",
        expect![[r#"
            fun foo(value: int): int { return value }
            fun main() { val value = foo<caret>() }"#]],
    );

    // Function completion works on the right side of an assignment.
    CompletionTest::new(
        "
            fun foo(value: int): int { return value }
            fun main() { var value = 0; value = fo<caret>; }
        ",
    )
    .check_applied(
        "foo",
        expect![[r#"
            fun foo(value: int): int { return value }
            fun main() { var value = 0; value = foo(value<caret>); }"#]],
    );

    // A function call inside a match initializer does not receive a statement semicolon.
    CompletionTest::new(
        "
            fun foo(): int { return 0 }
            fun main() { match (val value = fo<caret>) {} }
        ",
    )
    .check_applied(
        "foo",
        expect![[r#"
            fun foo(): int { return 0 }
            fun main() { match (val value = foo()<caret>) {} }"#]],
    );

    // A function call inside an if assignment does not receive a statement semicolon.
    CompletionTest::new(
        "
            fun foo(): int { return 0 }
            fun main() { var value = 0; if (value = fo<caret>) {} }
        ",
    )
    .check_applied(
        "foo",
        expect![[r#"
            fun foo(): int { return 0 }
            fun main() { var value = 0; if (value = foo()<caret>) {} }"#]],
    );
}

#[test]
fn applies_static_and_instance_method_completions() {
    // Static method completion inserts all declared parameters.
    CompletionTest::new(
        "
            struct Foo {}
            fun Foo.create(value: int): Foo { return Foo {} }
            fun main() { Foo.cre<caret>; }
        ",
    )
    .trigger_character(".")
    .check_applied(
        "create",
        expect![[r#"
            struct Foo {}
            fun Foo.create(value: int): Foo { return Foo {} }
            fun main() { Foo.create(value<caret>); }"#]],
    );

    // Instance method completion omits the self parameter from snippet arguments.
    CompletionTest::new(
        "
            struct Foo {}
            fun Foo.update(self, value: int) {}
            fun main(foo: Foo) { foo.upd<caret>; }
        ",
    )
    .trigger_character(".")
    .check_applied(
        "update",
        expect![[r#"
            struct Foo {}
            fun Foo.update(self, value: int) {}
            fun main(foo: Foo) { foo.update(value<caret>); }"#]],
    );

    // Method completion remains available at the end of a member-access chain.
    CompletionTest::new(
        "
            struct Foo { child: Foo? }
            fun Foo.update(self, value: int) {}
            fun main(foo: Foo) { foo.child!.upd<caret>; }
        ",
    )
    .trigger_character(".")
    .check_applied(
        "update",
        expect![[r#"
            struct Foo { child: Foo? }
            fun Foo.update(self, value: int) {}
            fun main(foo: Foo) { foo.child!.update(value<caret>); }"#]],
    );

    // Static methods declared on an enum complete after the enum name.
    CompletionTest::new(
        "
            enum Color { Red, Blue }
            fun Color.max(): Color { return Color.Red }
            fun main() { Color.ma<caret> }
        ",
    )
    .trigger_character(".")
    .check_applied(
        "max",
        expect![[r#"
            enum Color { Red, Blue }
            fun Color.max(): Color { return Color.Red }
            fun main() { Color.max();<caret> }"#]],
    );

    // Instance methods declared on an enum complete after an enum value.
    CompletionTest::new(
        "
            enum Color { Red, Blue }
            fun Color.isRed(self): bool { return self == Color.Red }
            fun main(color: Color) { color.isR<caret> }
        ",
    )
    .trigger_character(".")
    .check_applied(
        "isRed",
        expect![[r#"
            enum Color { Red, Blue }
            fun Color.isRed(self): bool { return self == Color.Red }
            fun main(color: Color) { color.isRed();<caret> }"#]],
    );

    // A static call inside a match initializer does not receive a statement semicolon.
    CompletionTest::new(
        "
            fun int.build(): int { return 0 }
            fun main() { match (val value = int.bui<caret>) {} }
        ",
    )
    .trigger_character(".")
    .check_applied(
        "build",
        expect![[r#"
            fun int.build(): int { return 0 }
            fun main() { match (val value = int.build()<caret>) {} }"#]],
    );

    // A static call on the right side of an assignment receives the statement semicolon.
    CompletionTest::new(
        "
            fun int.build(): int { return 0 }
            fun main() { var value = 0; value = int.bui<caret> }
        ",
    )
    .trigger_character(".")
    .check_applied(
        "build",
        expect![[r#"
            fun int.build(): int { return 0 }
            fun main() { var value = 0; value = int.build();<caret> }"#]],
    );

    // A static call inside an if assignment does not receive a statement semicolon.
    CompletionTest::new(
        "
            fun int.build(): int { return 0 }
            fun main() { var value = 0; if (value = int.bui<caret>) {} }
        ",
    )
    .trigger_character(".")
    .check_applied(
        "build",
        expect![[r#"
            fun int.build(): int { return 0 }
            fun main() { var value = 0; if (value = int.build()<caret>) {} }"#]],
    );

    // A static call in a field chain inserts parentheses before the following dot.
    CompletionTest::new(
        "
            fun int.build(): int { return 0 }
            fun main() { val value = int.bui<caret>.toString }
        ",
    )
    .trigger_character(".")
    .check_applied(
        "build",
        expect![[r#"
            fun int.build(): int { return 0 }
            fun main() { val value = int.build();<caret>.toString }"#]],
    );
}

#[test]
fn applies_struct_and_backticked_reference_completions() {
    // A struct selected in a type position is inserted without an object literal.
    CompletionTest::new(
        "
            struct Storage { value: int }
            fun main(): Sto<caret> {}
        ",
    )
    .check_applied(
        "Storage",
        expect![[r#"
            struct Storage { value: int }
            fun main(): Storage<caret> {}"#]],
    );

    // A non-empty struct in an expression expands to an object literal.
    CompletionTest::new(
        "
            struct Storage { value: int }
            fun main() { val storage = Sto<caret>; }
        ",
    )
    .check_applied(
        "Storage",
        expect![[r#"
            struct Storage { value: int }
            fun main() { val storage = Storage {<caret>}; }"#]],
    );

    // An empty struct selected in an expression does not add an empty object literal.
    CompletionTest::new(
        "
            struct Debug {}
            fun main() { val value = Deb<caret>; }
        ",
    )
    .check_applied(
        "Debug",
        expect![[r#"
            struct Debug {}
            fun main() { val value = Debug<caret>; }"#]],
    );

    // Backticked completion replaces both existing backticks as one range.
    CompletionTest::new(
        "
            fun `calculate total`(): int { return 1 }
            fun main() { `calculate t<caret>`(); }
        ",
    )
    .check_applied(
        "calculate total",
        expect![[r#"
            fun `calculate total`(): int { return 1 }
            fun main() { `calculate total`<caret>(); }"#]],
    );
}

#[test]
fn applies_backticked_symbols_in_every_reference_context() {
    // A backticked static method keeps the receiver and inserts call parentheses.
    CompletionTest::new(
        "
            fun int.`calculate static total`() {}
            fun main() { int.calculate<caret> }
        ",
    )
    .check_applied(
        "calculate static total",
        expect![[r#"
            fun int.`calculate static total`() {}
            fun main() { int.`calculate static total`();<caret> }"#]],
    );

    // A backticked instance method is inserted after the value receiver.
    CompletionTest::new(
        "
            fun int.`calculate instance total`(self) {}
            fun main() { 10.calculate<caret> }
        ",
    )
    .check_applied(
        "calculate instance total",
        expect![[r#"
            fun int.`calculate instance total`(self) {}
            fun main() { 10.`calculate instance total`();<caret> }"#]],
    );

    // A backticked struct is inserted with its complete quoted name.
    CompletionTest::new(
        "
            struct `account storage` {}
            fun main() { account<caret> }
        ",
    )
    .check_applied(
        "account storage",
        expect![[r#"
            struct `account storage` {}
            fun main() { `account storage`<caret> }"#]],
    );

    // A backticked type alias remains quoted in a type position.
    CompletionTest::new(
        "
            type `coin amount` = int
            fun main(value: coin<caret>) {}
        ",
    )
    .check_applied(
        "coin amount",
        expect![[r#"
            type `coin amount` = int
            fun main(value: `coin amount`<caret>) {}"#]],
    );

    // A backticked constant is inserted as an expression value.
    CompletionTest::new(
        "
            const `default amount` = 100
            fun main() { default<caret> }
        ",
    )
    .check_applied(
        "default amount",
        expect![[r#"
            const `default amount` = 100
            fun main() { `default amount`<caret> }"#]],
    );

    // A backticked global variable is inserted as an expression value.
    CompletionTest::new(
        "
            global `current amount`: int
            fun main() { current<caret> }
        ",
    )
    .check_applied(
        "current amount",
        expect![[r#"
            global `current amount`: int
            fun main() { `current amount`<caret> }"#]],
    );

    // A backticked field is inserted after its inferred struct receiver.
    CompletionTest::new(
        "
            struct Account { `current amount`: int }
            fun main() {
                val account = Account {};
                account.current<caret>
            }
        ",
    )
    .check_applied(
        "current amount",
        expect![[r#"
            struct Account { `current amount`: int }
            fun main() {
                val account = Account {};
                account.`current amount`<caret>
            }"#]],
    );

    // A backticked catch variable is preserved inside the catch body.
    CompletionTest::new(
        "
            fun main() {
                try {} catch (`error code`) {
                    error<caret>
                }
            }
        ",
    )
    .check_applied(
        "error code",
        expect![[r#"
            fun main() {
                try {} catch (`error code`) {
                    `error code`<caret>
                }
            }"#]],
    );

    // A backticked local variable is inserted with its quotes.
    CompletionTest::new(
        "
            fun main() {
                val `current amount` = 123;
                current<caret>
            }
        ",
    )
    .check_applied(
        "current amount",
        expect![[r#"
            fun main() {
                val `current amount` = 123;
                `current amount`<caret>
            }"#]],
    );

    // A backticked parameter is inserted with its quotes.
    CompletionTest::new(
        "
            fun main(`initial amount`: int) {
                initial<caret>
            }
        ",
    )
    .check_applied(
        "initial amount",
        expect![[r#"
            fun main(`initial amount`: int) {
                `initial amount`<caret>
            }"#]],
    );
}

#[test]
fn applies_multifile_reference_completions_and_auto_imports() {
    // Selecting an unimported stdlib function inserts its stdlib import.
    CompletionTest::new("fun main() { getTvmRegisterC3<caret>; }").check_applied(
        "getTvmRegisterC3",
        expect![[r#"
            import "@stdlib/tvm-lowlevel"

            fun main() { getTvmRegisterC3()<caret>; }"#]],
    );

    // Selecting a uniquely declared function from another file inserts a local import.
    CompletionTest::new("fun main() { someGlobalFunction<caret>; }")
        .file("other.tolk", "fun someGlobalFunction() {}")
        .check_applied(
            "someGlobalFunction",
            expect![[r#"
                import "other"

                fun main() { someGlobalFunction()<caret>; }"#]],
        );

    // An existing import prevents a duplicate additional edit.
    CompletionTest::new(
        r#"
            import "./other"
            fun main() { someGlobalFunction<caret>; }
        "#,
    )
    .file("other.tolk", "fun someGlobalFunction() {}")
    .check_applied(
        "someGlobalFunction",
        expect![[r#"
            import "./other"
            fun main() { someGlobalFunction()<caret>; }"#]],
    );

    // A new import appended after an existing import keeps a blank line before code.
    CompletionTest::new(
        r#"
            import "./existing"
            fun main() { uniqueFunction<caret>; }
        "#,
    )
    .file("existing.tolk", "fun existingFunction() {}")
    .file("other.tolk", "fun uniqueFunction() {}")
    .check_applied(
        "uniqueFunction",
        expect![[r#"
            import "./existing"
            import "other"

            fun main() { uniqueFunction()<caret>; }"#]],
    );

    // A nested document uses a parent-relative import for a workspace sibling.
    CompletionTest::new("fun main() { parentFunction<caret>; }")
        .uri("file:///workspace/contracts/main.tolk")
        .file("parent.tolk", "fun parentFunction() {}")
        .check_applied(
            "parentFunction",
            expect![[r#"
                import "../parent"

                fun main() { parentFunction()<caret>; }"#]],
        );

    // A local-path import also prevents duplication through an import mapping alias.
    CompletionTest::new(
        r#"
            import "contracts/errors"
            fun main() { someGlobalFunction<caret>; }
        "#,
    )
    .manifest(
        r#"
            import-mappings = { "@contracts" = "contracts" }
        "#,
    )
    .file("contracts/errors.tolk", "fun someGlobalFunction() {}")
    .check_applied(
        "someGlobalFunction",
        expect![[r#"
            import "contracts/errors"
            fun main() { someGlobalFunction()<caret>; }"#]],
    );

    // A unique mapped-file symbol receives the mapping import when no local import exists.
    CompletionTest::new("fun main() { mappedFunction<caret>; }")
        .manifest(
            r#"
                import-mappings = { "@contracts" = "contracts" }
            "#,
        )
        .file("contracts/errors.tolk", "fun mappedFunction() {}")
        .check_applied(
            "mappedFunction",
            expect![[r#"
                import "@contracts/errors"

                fun main() { mappedFunction()<caret>; }"#]],
        );

    // A new import is inserted after the tolk version directive.
    CompletionTest::new(
        "
            tolk 1.0
            fun main() { versionedFunction<caret>; }
        ",
    )
    .file("versioned.tolk", "fun versionedFunction() {}")
    .check_applied(
        "versionedFunction",
        expect![[r#"
                tolk 1.0

                import "versioned"

                fun main() { versionedFunction()<caret>; }"#]],
    );

    // A symbol already provided by the stdlib prelude does not add an explicit import.
    CompletionTest::new("fun main() { minMax<caret>; }")
        .check_applied("minMax", expect!["fun main() { minMax(x<caret>, y); }"]);

    // Ambiguous declarations from several files do not choose an arbitrary import source.
    CompletionTest::new("fun main() { someGlobalFunction<caret>; }")
        .file("other1.tolk", "fun someGlobalFunction() {}")
        .file("other2.tolk", "fun someGlobalFunction() {}")
        .check_applied(
            "someGlobalFunction",
            expect!["fun main() { someGlobalFunction()<caret>; }"],
        );
}
