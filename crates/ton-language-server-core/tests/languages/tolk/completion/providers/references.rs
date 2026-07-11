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
        label  kind      detail             edit     text
        id     Property  : int  of Storage  7:8-7:8  id: $1,$0"#]]);
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
        label    kind      detail             edit     text
        counter  Property  : int  of Storage  6:8-6:8  counter: $1,$0
        id       Property  : int  of Storage  6:8-6:8  id: $1,$0"#]]);
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
        label  kind      detail         edit       text
        age    Property  : int  of Foo  1:19-1:19  age: $1$0"#]]);

    // A short literal gets its expected struct type from a variable declaration.
    CompletionTest::new(
        "
            struct Foo { age: int }
            fun main() { val foo: Foo = { <caret> }; }
        ",
    )
    .labels(&["age"])
    .check(expect![[r#"
        label  kind      detail         edit       text
        age    Property  : int  of Foo  1:30-1:30  age: $1$0"#]]);

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
        label  kind      detail         edit       text
        age    Property  : int  of Foo  2:23-2:23  age: $1$0"#]]);

    // A matching local is offered alongside the explicit field initializer.
    CompletionTest::new(
        "
            struct Foo { age: int }
            fun main() { val age = 10; Foo { <caret> }; }
        ",
    )
    .labels(&["age"])
    .check(expect![[r#"
        label  kind      detail         edit       text
        age    Property  : int  of Foo  1:33-1:33  age: $1$0
        age    Variable  int            1:33-1:33  age"#]]);

    // A matching parameter is offered alongside the explicit field initializer.
    CompletionTest::new(
        "
            struct Foo { age: int }
            fun main(age: int) { Foo { <caret> }; }
        ",
    )
    .labels(&["age"])
    .check(expect![[r#"
        label  kind      detail         edit       text
        age    Property  : int  of Foo  1:27-1:27  age: $1$0
        age    Variable  int            1:27-1:27  age"#]]);

    // An already initialized field is excluded even when completion occurs before a later field.
    CompletionTest::new(
        "
            struct Foo { age: int, value: int }
            fun main() { Foo { a<caret>, value: 10 }; }
        ",
    )
    .labels(&["age", "value"])
    .check(expect![[r#"
        label  kind      detail         edit       text
        age    Property  : int  of Foo  1:19-1:20  age: $1$0"#]]);

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
        value  Variable  int     1:34-1:36  value"#]]);

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
        label   kind      detail           edit       text
        first   Property  : int  of Foo    1:22-1:22  first: $1$0
        second  Property  : slice  of Foo  1:22-1:22  second: $1$0"#]]);
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
            local      Variable  int     0:42-0:45  local
            parameter  Variable  int     0:42-0:45  parameter"#]]);

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
        label   kind      detail         edit       text
        first   Property  : int  of Foo  1:25-1:25  first
        second  Property  : int  of Foo  1:25-1:25  second"#]]);

    // Member completion remains available inside an assert expression.
    CompletionTest::new(
        "
            struct Foo { first: int, second: slice }
            fun main(foo: Foo) { assert(foo.<caret>) throw 1 }
        ",
    )
    .labels(&["first", "second"])
    .trigger_character(".")
    .check(expect![[r#"
        label   kind      detail           edit       text
        first   Property  : int  of Foo    1:32-1:32  first
        second  Property  : slice  of Foo  1:32-1:32  second"#]]);

    // A malformed member access keeps the type inferred from an imported factory call.
    CompletionTest::new(
        r#"
            import "wallet"

            fun consume(value: address, other: int) {}
            fun main() {
                val wallet = createWallet();
                consume(wallet.<caret>, 1);
            }
        "#,
    )
    .file(
        "wallet.tolk",
        "
            struct Wallet { address: address, stateInit: cell }
            fun createWallet(): Wallet {
                return Wallet { address: addressNone(), stateInit: beginCell().endCell() };
            }
        ",
    )
    .labels(&["address", "stateInit"])
    .trigger_character(".")
    .check(expect![[r#"
        label      kind      detail                edit       text
        address    Property  : address  of Wallet  5:19-5:19  address
        stateInit  Property  : cell  of Wallet     5:19-5:19  stateInit"#]]);

    // A backticked function replaces the complete quoted identifier.
    CompletionTest::new(
        "
            fun `calculate total`(): int { return 1 }
            fun main() { `calculate t<caret>`(); }
        ",
    )
    .labels(&["calculate total"])
    .check(expect![[r#"
        label            kind      detail   edit       text
        calculate total  Function  (): int  1:13-1:26  `calculate total`$0"#]]);
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
        label      kind      detail  edit     text
        some       Variable  int     2:4-2:8  some
        someOther  Variable  string  2:4-2:8  someOther"#]]);

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
        label      kind      detail   edit      text
        error      Variable  int      2:8-2:11  error
        errorData  Variable  unknown  2:8-2:11  errorData"#]]);

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
        data   Variable  int     2:16-2:19  data"#]]);

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
        some       Variable  int     4:16-4:20  some
        someOther  Variable  int     4:16-4:20  someOther"#]]);
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
        someParameter  Variable  int     1:8-1:12  someParameter"#]]);

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
        someParameter  Variable  int     1:11-1:15  someParameter"#]]);

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
        someParameter  Variable  int     1:11-1:15  someParameter"#]]);
}

#[test]
fn separates_type_parameters_from_value_completion() {
    // Function type parameters are offered in a type position.
    CompletionTest::new("fun foo<TName, TValue>(value: TN<caret>) {}")
        .labels(&["TName", "TValue"])
        .check(expect![[r#"
            label   kind           detail          edit       text
            TName   TypeParameter  type parameter  0:30-0:32  TName
            TValue  TypeParameter  type parameter  0:30-0:32  TValue"#]]);

    // Every function type parameter is offered for a shared prefix.
    CompletionTest::new("fun generic<TName, TValue, TOther>(): T<caret> {}")
        .labels(&["TName", "TValue", "TOther"])
        .check(expect![[r#"
            label   kind           detail          edit       text
            TName   TypeParameter  type parameter  0:38-0:39  TName
            TOther  TypeParameter  type parameter  0:38-0:39  TOther
            TValue  TypeParameter  type parameter  0:38-0:39  TValue"#]]);

    // A type parameter with a default remains available by its declared name.
    CompletionTest::new("fun generic<TName = int>(): TNam<caret> {}")
        .labels(&["TName"])
        .check(expect![[r#"
            label  kind           detail                  edit       text
            TName  TypeParameter   = int  type parameter  0:28-0:32  TName"#]]);

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
        label   kind           detail          edit       text
        TName   TypeParameter  type parameter  1:11-1:13  TName
        TValue  TypeParameter  type parameter  1:11-1:13  TValue"#]]);

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
        label   kind           detail          edit       text
        TName   TypeParameter  type parameter  1:11-1:12  TName
        TOther  TypeParameter  type parameter  1:11-1:12  TOther
        TValue  TypeParameter  type parameter  1:11-1:12  TValue"#]]);

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
        label        kind      detail      edit       text
        MAX_VALUE    Constant  : int = 10  2:25-2:28  MAX_VALUE
        globalValue  Variable  : int       2:25-2:28  globalValue"#]]);

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
        CONSTANT_FLAG  Constant  : bool = true  2:13-2:21  CONSTANT_FLAG
        CONSTANT_TEXT  Constant  : string = ""  2:13-2:21  CONSTANT_TEXT"#]]);

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
        label         kind      detail  edit       text
        globalFirst   Variable  : int   2:13-2:19  globalFirst
        globalSecond  Variable  : int   2:13-2:19  globalSecond"#]]);

    // Structs are inserted as types in a type position.
    CompletionTest::new(
        "
            struct Storage { value: int }
            fun main(value: Sto<caret>) {}
        ",
    )
    .labels(&["Storage"])
    .check(expect![[r#"
        label    kind    detail  edit       text
        Storage  Struct          1:16-1:19  Storage"#]]);

    // Non-empty structs are inserted as object literals in expression positions.
    CompletionTest::new(
        "
            struct Storage { value: int }
            fun main() { val value = Sto<caret>; }
        ",
    )
    .labels(&["Storage"])
    .check(expect![[r#"
        label    kind    detail  edit       text
        Storage  Struct   {}     1:25-1:28  Storage {$1}$0"#]]);

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
        Empty  Struct          1:25-1:28  Empty"#]]);

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
        Color  Enum          1:16-1:20  Color"#]]);

    // An enum expression offers the enum itself and its qualified members.
    CompletionTest::new(
        "
            enum Color { Red, Blue }
            fun main() { Colo<caret> }
        ",
    )
    .labels(&["Color", "Color.Red", "Color.Blue"])
    .check(expect![[r#"
        label       kind        detail    edit       text
        Color.Blue  EnumMember  of Color  1:13-1:17  Color.Blue
        Color.Red   EnumMember  of Color  1:13-1:17  Color.Red
        Color       Enum                  1:13-1:17  Color"#]]);

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
        Amount  TypeParameter          1:16-1:19  Amount"#]]);

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
        Amount  TypeParameter          1:13-1:16  Amount"#]]);

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
        Foo    Struct                 1:11-1:13  Foo
        Cell   Struct                 1:11-1:13  Cell
        int    TypeParameter          1:11-1:13  int"#]]);
}

#[test]
fn completes_types_in_every_typed_contract_field() {
    // Internal incoming message metadata accepts type declarations only.
    CompletionTest::new(
        "
            struct ContractType {}
            fun contractValue() {}
            contract C { incomingMessages: ContractT<caret> }
        ",
    )
    .labels(&["ContractType", "contractValue", "author"])
    .check(expect![[r#"
        label         kind    detail  edit       text
        ContractType  Struct          2:31-2:40  ContractType"#]]);

    // External incoming message metadata accepts type declarations only.
    CompletionTest::new(
        "
            struct ContractType {}
            fun contractValue() {}
            contract C { incomingExternal: ContractT<caret> }
        ",
    )
    .labels(&["ContractType", "contractValue", "author"])
    .check(expect![[r#"
        label         kind    detail  edit       text
        ContractType  Struct          2:31-2:40  ContractType"#]]);

    // Outgoing message metadata accepts type declarations only.
    CompletionTest::new(
        "
            struct ContractType {}
            fun contractValue() {}
            contract C { outgoingMessages: ContractT<caret> }
        ",
    )
    .labels(&["ContractType", "contractValue", "author"])
    .check(expect![[r#"
        label         kind    detail  edit       text
        ContractType  Struct          2:31-2:40  ContractType"#]]);

    // Emitted event metadata accepts type declarations only.
    CompletionTest::new(
        "
            struct ContractType {}
            fun contractValue() {}
            contract C { emittedEvents: ContractT<caret> }
        ",
    )
    .labels(&["ContractType", "contractValue", "author"])
    .check(expect![[r#"
        label         kind    detail  edit       text
        ContractType  Struct          2:28-2:37  ContractType"#]]);

    // Thrown error metadata accepts enum and other type declarations only.
    CompletionTest::new(
        "
            struct ContractType {}
            fun contractValue() {}
            contract C { thrownErrors: ContractT<caret> }
        ",
    )
    .labels(&["ContractType", "contractValue", "author"])
    .check(expect![[r#"
        label         kind    detail  edit       text
        ContractType  Struct          2:27-2:36  ContractType"#]]);

    // Persistent storage metadata accepts type declarations only.
    CompletionTest::new(
        "
            struct ContractType {}
            fun contractValue() {}
            contract C {
                storage:
                    ContractT<caret>
            }
        ",
    )
    .labels(&["ContractType", "contractValue", "author"])
    .check(expect![[r#"
        label         kind    detail  edit      text
        ContractType  Struct          4:8-4:17  ContractType"#]]);

    // Deployment storage metadata accepts type declarations only.
    CompletionTest::new(
        "
            struct ContractType {}
            fun contractValue() {}
            contract C { storageAtDeployment: ContractT<caret> }
        ",
    )
    .labels(&["ContractType", "contractValue", "author"])
    .check(expect![[r#"
        label         kind    detail  edit       text
        ContractType  Struct          2:34-2:43  ContractType"#]]);

    // Forced ABI exports accept type declarations only.
    CompletionTest::new(
        "
            struct ContractType {}
            fun contractValue() {}
            contract C { forceAbiExport: ContractT<caret> }
        ",
    )
    .labels(&["ContractType", "contractValue", "author"])
    .check(expect![[r#"
        label         kind    detail  edit       text
        ContractType  Struct          2:29-2:38  ContractType"#]]);
}

#[test]
fn completes_types_in_an_empty_contract_field_value() {
    // An empty value is represented by recovery syntax before the synthetic completion name.
    CompletionTest::new(
        "
            struct Storage {}
            fun storageValue() {}
            contract C {
                storage:
                    <caret>
            }
        ",
    )
    .labels(&["Storage", "storageValue", "author"])
    .check(expect![[r#"
        label    kind    detail  edit     text
        Storage  Struct          4:8-4:8  Storage"#]]);
}

#[test]
fn applies_auto_imported_type_in_contract_metadata() {
    // Contract type completion keeps the normal cross-file auto-import behavior.
    CompletionTest::new("contract C { storage: Remote<caret> }")
        .file("types.tolk", "struct RemoteStorage {}")
        .check_applied(
            "RemoteStorage",
            expect![[r#"
                import "types"

                contract C { storage: RemoteStorage<caret> }"#]],
        );
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
        label   kind      detail           edit       text
        first   Property  : int  of Foo    2:37-2:37  first
        second  Property  : slice  of Foo  2:37-2:37  second"#]]);

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
        label   kind      detail           edit       text
        first   Property  : T  of Foo      1:40-1:40  first
        second  Property  : slice  of Foo  1:40-1:40  second"#]]);

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
        label   kind      detail           edit       text
        first   Property  : T  of Foo      2:38-2:38  first
        second  Property  : slice  of Foo  2:38-2:38  second"#]]);

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
        label  kind      detail         edit       text
        first  Property  : int  of Foo  1:35-1:35  first"#]]);
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
        label  kind    detail      edit       text
        bad    Method  ()  of Foo  4:17-4:19  bad();$0
        bar    Method  ()  of Foo  4:17-4:19  bar();$0
        baz    Method  ()  of Foo  4:17-4:19  baz();$0"#]]);

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
        label  kind    detail                           edit       text
        new    Method  (): Second<int>  of Second<int>  3:25-3:25  new();$0"#]]);

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
        label   kind    detail  edit       text
        add     Method  (self)  3:41-3:42  add();$0
        addInt  Method  (self)  3:41-3:42  addInt();$0"#]]);

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
        label  kind    detail                       edit       text
        bar    Method  <T>(value: T)  of Foo        3:17-3:19  bar(${1:value});$0
        baz    Method  <T, U>(value: T): U  of Foo  3:17-3:19  baz(${1:value});$0"#]]);
}

#[test]
fn completes_only_methods_from_indexed_stdlib_files() {
    // Embedded stdlib modules are available as sources but are not implicit project roots.
    CompletionTest::new(
        "
            struct Storage {}
            fun main(storage: Storage) { storage.<caret> }
        ",
    )
    .labels(&["toCell", "iDictGet"])
    .trigger_character(".")
    .check(expect![[r#"
        label   kind    detail                                      edit       text
        toCell  Method  (self, options: PackOptions = {}): Cell<T>  1:37-1:37  toCell(${1:options});$0"#]]);

    // Importing a specialized stdlib module indexes and exposes its methods normally.
    CompletionTest::new(
        r#"
            import "@stdlib/tvm-dicts"

            fun main(value: dict) { value.<caret> }
        "#,
    )
    .labels(&["toCell", "iDictGet"])
    .trigger_character(".")
    .check(expect![[r#"
        label     kind    detail                                         edit       text
        iDictGet  Method  (self, keyLen: int, key: int): (slice?, bool)  2:30-2:30  iDictGet(${1:keyLen}, ${2:key});$0
        toCell    Method  (self, options: PackOptions = {}): Cell<T>     2:30-2:30  toCell(${1:options});$0"#]]);

    // Indexing the module must not make dict-only methods compatible with arbitrary structs.
    CompletionTest::new(
        r#"
            import "@stdlib/tvm-dicts"

            struct Storage {}
            fun main(value: Storage) { value.<caret> }
        "#,
    )
    .labels(&["iDictGet"])
    .trigger_character(".")
    .check(expect!["<none>"]);
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
        label     kind    detail                   edit       text
        add       Method  (self, value: int): int  3:31-3:31  add(${1:value});$0
        subtract  Method  (self, value: int): int  3:31-3:31  subtract(${1:value});$0"#]]);

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
        label     kind    detail                   edit       text
        add       Method  (self, value: int): int  5:39-5:39  add(${1:value});$0
        multiply  Method  (self, value: int): int  5:39-5:39  multiply(${1:value});$0
        subtract  Method  (self, value: int): int  5:39-5:39  subtract(${1:value});$0"#]]);

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
        label       kind    detail         edit       text
        beginParse  Method  (self): slice  3:15-3:15  beginParse();$0"#]]);
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
        label       kind    detail         edit       text
        beginParse  Method  (self): slice  1:26-1:32  beginParse()$0"#]]);
}

#[test]
fn filters_methods_by_receiver_kind_and_nominal_type() {
    // An instance method is offered on its nominal struct receiver.
    CompletionTest::new(
        "
            struct Left {}
            fun Left.onlyLeft(self) {}
            fun main(value: Left) { value.only<caret> }
        ",
    )
    .labels(&["onlyLeft"])
    .trigger_character(".")
    .check(expect![[r#"
        label     kind    detail  edit       text
        onlyLeft  Method  (self)  2:30-2:34  onlyLeft();$0"#]]);

    // Structurally equal structs do not inherit each other's nominal methods.
    CompletionTest::new(
        "
            struct Left { value: int }
            struct Right { value: int }
            fun Left.onlyLeft(self) {}
            fun main(value: Right) { value.only<caret> }
        ",
    )
    .labels(&["onlyLeft"])
    .trigger_character(".")
    .check(expect!["<none>"]);

    // Static methods are not offered after an instance expression.
    CompletionTest::new(
        "
            struct Foo {}
            fun Foo.staticOnly() {}
            fun main(value: Foo) { value.static<caret> }
        ",
    )
    .labels(&["staticOnly"])
    .trigger_character(".")
    .check(expect!["<none>"]);

    // Instance methods are not offered after a type expression.
    CompletionTest::new(
        "
            struct Foo {}
            fun Foo.instanceOnly(self) {}
            fun main() { Foo.instance<caret> }
        ",
    )
    .labels(&["instanceOnly"])
    .trigger_character(".")
    .check(expect!["<none>"]);

    // A method declared specifically for a nullable receiver remains available.
    CompletionTest::new(
        "
            struct Foo {}
            fun Foo?.recover(self): Foo { return self! }
            fun main(value: Foo?) { value.reco<caret> }
        ",
    )
    .labels(&["recover"])
    .trigger_character(".")
    .check(expect![[r#"
        label    kind    detail       edit       text
        recover  Method  (self): Foo  2:30-2:34  recover();$0"#]]);

    // A method requiring a non-null receiver is hidden until the value is narrowed.
    CompletionTest::new(
        "
            struct Foo {}
            fun Foo.touch(self) {}
            fun main(value: Foo?) { value.tou<caret> }
        ",
    )
    .labels(&["touch"])
    .trigger_character(".")
    .check(expect!["<none>"]);
}

#[test]
fn completes_generic_methods_with_exact_overload_semantics() {
    // A generic receiver method is instantiated for a concrete type argument.
    CompletionTest::new(
        "
            struct Box<T> {}
            fun Box<T>.take(self): T {}
            fun main(value: Box<bool>) { value.ta<caret> }
        ",
    )
    .labels(&["take"])
    .trigger_character(".")
    .check(expect![[r#"
        label  kind    detail     edit       text
        take   Method  (self): T  2:35-2:37  take();$0"#]]);

    // A generic method from another nominal type is not considered compatible.
    CompletionTest::new(
        "
            struct Box<T> {}
            struct Bag<T> {}
            fun Box<T>.take(self): T {}
            fun main(value: Bag<int>) { value.ta<caret> }
        ",
    )
    .labels(&["take"])
    .trigger_character(".")
    .check(expect!["<none>"]);

    // A concrete overload wins over a generic overload with the same name.
    CompletionTest::new(
        "
            struct Box<T> {}
            fun Box<T>.pick(self): T {}
            fun Box<int>.pick(self): int { return 1 }
            fun main(value: Box<int>) { value.pi<caret> }
        ",
    )
    .labels(&["pick"])
    .trigger_character(".")
    .check(expect![[r#"
        label  kind    detail       edit       text
        pick   Method  (self): int  3:34-3:36  pick();$0"#]]);

    // Nested generic receiver shapes are matched recursively.
    CompletionTest::new(
        "
            struct Box<T> {}
            fun Box<Box<T>>.deep(self): T {}
            fun main(value: Box<Box<int>>) { value.de<caret> }
        ",
    )
    .labels(&["deep"])
    .trigger_character(".")
    .check(expect![[r#"
        label  kind    detail     edit       text
        deep   Method  (self): T  2:39-2:41  deep();$0"#]]);

    // The more specific nested generic overload wins for the same method name.
    CompletionTest::new(
        "
            struct Box<T> {}
            fun Box<T>.select(self): int { return 1 }
            fun Box<Box<T>>.select(self): int { return 2 }
            fun main(value: Box<Box<int>>) { value.se<caret> }
        ",
    )
    .labels(&["select"])
    .trigger_character(".")
    .check(expect![[r#"
        label   kind    detail       edit       text
        select  Method  (self): int  3:39-3:41  select();$0"#]]);

    // Repeated receiver type parameters accept matching concrete arguments.
    CompletionTest::new(
        "
            struct Pair<A, B> {}
            fun Pair<T, T>.same(self) {}
            fun main(value: Pair<int, int>) { value.sa<caret> }
        ",
    )
    .labels(&["same"])
    .trigger_character(".")
    .check(expect![[r#"
        label  kind    detail  edit       text
        same   Method  (self)  2:40-2:42  same();$0"#]]);

    // Repeated receiver type parameters reject conflicting concrete arguments.
    CompletionTest::new(
        "
            struct Pair<A, B> {}
            fun Pair<T, T>.same(self) {}
            fun main(value: Pair<int, bool>) { value.sa<caret> }
        ",
    )
    .labels(&["same"])
    .trigger_character(".")
    .check(expect!["<none>"]);
}

#[test]
fn chooses_the_most_specific_generic_receiver_shape() {
    // A structured map receiver is more specific than a bare type parameter.
    CompletionTest::new(
        "
            fun T.select(self) {}
            fun map<K, V>.select(self) {}
            fun main(value: map<int, slice>) { value.sel<caret> }
        ",
    )
    .labels(&["select"])
    .trigger_character(".")
    .check(expect![[r#"
        label   kind    detail  edit       text
        select  Method  (self)  2:41-2:44  select();$0"#]]);

    // A constrained map receiver dominates the fully generic map receiver.
    CompletionTest::new(
        "
            fun map<K, V>.select(self) {}
            fun map<int, V>.select(self) {}
            fun main(value: map<int, slice>) { value.sel<caret> }
        ",
    )
    .labels(&["select"])
    .trigger_character(".")
    .check(expect![[r#"
        label   kind    detail  edit       text
        select  Method  (self)  2:41-2:44  select();$0"#]]);

    // An instantiated array receiver is more specific than a bare type parameter.
    CompletionTest::new(
        "
            fun T.select(self) {}
            fun array<T>.select(self) {}
            fun main(value: array<int>) { value.sel<caret> }
        ",
    )
    .labels(&["select"])
    .trigger_character(".")
    .check(expect![[r#"
        label   kind    detail  edit       text
        select  Method  (self)  2:36-2:39  select();$0"#]]);

    // A tensor receiver is more specific than a bare type parameter.
    CompletionTest::new(
        "
            fun T.select(self) {}
            fun [T, int].select(self) {}
            fun main(value: [bool, int]) { value.sel<caret> }
        ",
    )
    .labels(&["select"])
    .trigger_character(".")
    .check(expect![[r#"
        label   kind    detail  edit       text
        select  Method  (self)  2:37-2:40  select();$0"#]]);

    // A nullable receiver shape wins over the unconstrained generic receiver.
    CompletionTest::new(
        "
            fun T.select(self) {}
            fun T?.select(self) {}
            fun main(value: int?) { value.sel<caret> }
        ",
    )
    .labels(&["select"])
    .trigger_character(".")
    .check(expect![[r#"
        label   kind    detail  edit       text
        select  Method  (self)  2:30-2:33  select();$0"#]]);

    // A concrete tensor overload wins over a compatible generic tensor overload.
    CompletionTest::new(
        "
            fun [T, int].select(self) {}
            fun [bool, int].select(self) {}
            fun main(value: [bool, int]) { value.sel<caret> }
        ",
    )
    .labels(&["select"])
    .trigger_character(".")
    .check(expect![[r#"
        label   kind    detail  edit       text
        select  Method  (self)  2:37-2:40  select();$0"#]]);
}

#[test]
fn completes_methods_for_all_supported_receiver_expressions() {
    // Parentheses around a receiver do not lose its inferred type.
    CompletionTest::new(
        "
            struct Foo {}
            fun Foo.touch(self) {}
            fun main(value: Foo) { (value).tou<caret> }
        ",
    )
    .labels(&["touch"])
    .trigger_character(".")
    .check(expect![[r#"
        label  kind    detail  edit       text
        touch  Method  (self)  2:31-2:34  touch();$0"#]]);

    // A freshly constructed object exposes its instance methods.
    CompletionTest::new(
        "
            struct Foo {}
            fun Foo.touch(self) {}
            fun main() { (Foo {}).tou<caret> }
        ",
    )
    .labels(&["touch"])
    .trigger_character(".")
    .check(expect![[r#"
        label  kind    detail  edit       text
        touch  Method  (self)  2:22-2:25  touch();$0"#]]);

    // A function-call result exposes methods from its inferred return type.
    CompletionTest::new(
        "
            struct Foo {}
            fun Foo.touch(self) {}
            fun makeFoo(): Foo { return Foo {} }
            fun main() { makeFoo().tou<caret> }
        ",
    )
    .labels(&["touch"])
    .trigger_character(".")
    .check(expect![[r#"
        label  kind    detail  edit       text
        touch  Method  (self)  3:23-3:26  touch();$0"#]]);

    // An instantiated generic type exposes compatible static methods.
    CompletionTest::new(
        "
            struct Box<T> {}
            fun Box<T>.create(): Box<T> { return Box<T> {} }
            fun main() { Box<int>.cre<caret> }
        ",
    )
    .labels(&["create"])
    .trigger_character(".")
    .check(expect![[r#"
        label   kind    detail                 edit       text
        create  Method  (): Box<T>  of Box<T>  2:22-2:25  create();$0"#]]);

    // A non-null assertion exposes methods of the narrowed receiver type.
    CompletionTest::new(
        "
            struct Foo {}
            fun Foo.touch(self) {}
            fun main(value: Foo?) { value!.tou<caret> }
        ",
    )
    .labels(&["touch"])
    .trigger_character(".")
    .check(expect![[r#"
        label  kind    detail  edit       text
        touch  Method  (self)  2:31-2:34  touch();$0"#]]);

    // An explicit cast determines the receiver used for method lookup.
    CompletionTest::new(
        "
            struct Foo {}
            fun Foo.touch(self) {}
            fun main(value: unknown) { (value as Foo).tou<caret> }
        ",
    )
    .labels(&["touch"])
    .trigger_character(".")
    .check(expect![[r#"
        label  kind    detail  edit       text
        touch  Method  (self)  2:42-2:45  touch();$0"#]]);

    // A previous method call can provide the receiver for the next completion.
    CompletionTest::new(
        "
            struct Foo {}
            fun Foo.next(self): Foo { return self }
            fun Foo.touch(self) {}
            fun main(value: Foo) { value.next().tou<caret> }
        ",
    )
    .labels(&["touch"])
    .trigger_character(".")
    .check(expect![[r#"
        label  kind    detail  edit       text
        touch  Method  (self)  3:36-3:39  touch();$0"#]]);
}

#[test]
fn applies_method_completion_at_edit_boundaries() {
    // Existing call parentheses are preserved instead of duplicated.
    CompletionTest::new(
        "
            struct Foo {}
            fun Foo.touch(self) {}
            fun main(value: Foo) { value.tou<caret>() }
        ",
    )
    .trigger_character(".")
    .check_applied(
        "touch",
        expect![[r#"
        struct Foo {}
        fun Foo.touch(self) {}
        fun main(value: Foo) { value.touch<caret>() }"#]],
    );

    // An existing statement semicolon is preserved instead of duplicated.
    CompletionTest::new(
        "
            struct Foo {}
            fun Foo.touch(self) {}
            fun main(value: Foo) { value.tou<caret>; }
        ",
    )
    .trigger_character(".")
    .check_applied(
        "touch",
        expect![[r#"
        struct Foo {}
        fun Foo.touch(self) {}
        fun main(value: Foo) { value.touch()<caret>; }"#]],
    );

    // A method inserted as a call argument does not append a statement semicolon.
    CompletionTest::new(
        "
            struct Foo {}
            fun Foo.value(self): int { return 1 }
            fun consume(value: int) {}
            fun main(foo: Foo) { consume(foo.val<caret>); }
        ",
    )
    .trigger_character(".")
    .check_applied(
        "value",
        expect![[r#"
            struct Foo {}
            fun Foo.value(self): int { return 1 }
            fun consume(value: int) {}
            fun main(foo: Foo) { consume(foo.value()<caret>); }"#]],
    );

    // A method inserted in a return expression does not append a statement semicolon.
    CompletionTest::new(
        "
            struct Foo {}
            fun Foo.value(self): int { return 1 }
            fun read(foo: Foo): int { return foo.val<caret> }
        ",
    )
    .trigger_character(".")
    .check_applied(
        "value",
        expect![[r#"
        struct Foo {}
        fun Foo.value(self): int { return 1 }
        fun read(foo: Foo): int { return foo.value()<caret> }"#]],
    );

    // Backticked method names replace both existing backticks as one range.
    CompletionTest::new(
        "
            struct Foo {}
            fun Foo.`touch me`(self) {}
            fun main(value: Foo) { value.`touch m<caret>`; }
        ",
    )
    .trigger_character(".")
    .check_applied(
        "touch me",
        expect![[r#"
        struct Foo {}
        fun Foo.`touch me`(self) {}
        fun main(value: Foo) { value.`touch me`()<caret>; }"#]],
    );

    // Mutate-self is omitted while ordinary parameters remain snippet arguments.
    CompletionTest::new(
        "
            struct Foo {}
            fun Foo.update(mutate self, value: int) {}
            fun main(value: Foo) { value.upd<caret> }
        ",
    )
    .trigger_character(".")
    .check_applied(
        "update",
        expect![[r#"
        struct Foo {}
        fun Foo.update(mutate self, value: int) {}
        fun main(value: Foo) { value.update(value<caret>); }"#]],
    );
}

#[test]
fn handles_method_visibility_across_workspace_files() {
    // Methods from an imported file participate in receiver completion.
    CompletionTest::new(
        r#"
            import "lib"
            fun main(value: Foo) { value.imp<caret> }
        "#,
    )
    .file(
        "lib.tolk",
        "
            struct Foo {}
            fun Foo.imported(self) {}
        ",
    )
    .labels(&["imported"])
    .trigger_character(".")
    .check(expect![[r#"
        label     kind    detail  edit       text
        imported  Method  (self)  1:29-1:32  imported();$0"#]]);

    // Selecting a compatible method from an unimported file adds its import.
    CompletionTest::new(
        "
            struct Foo {}
            fun main(value: Foo) { value.hid<caret> }
        ",
    )
    .file("unrelated.tolk", "fun Foo.hidden(self) {}")
    .trigger_character(".")
    .check_applied(
        "hidden",
        expect![[r#"
        import "unrelated"

        struct Foo {}
        fun main(value: Foo) { value.hidden();<caret> }"#]],
    );

    // A same-file method remains visible without any imports.
    CompletionTest::new(
        "
            struct Foo {}
            fun Foo.localMethod(self) {}
            fun main(value: Foo) { value.local<caret> }
        ",
    )
    .labels(&["localMethod"])
    .trigger_character(".")
    .check(expect![[r#"
        label        kind    detail  edit       text
        localMethod  Method  (self)  2:29-2:34  localMethod();$0"#]]);

    // Static and instance methods keep their receiver mode across an import boundary.
    CompletionTest::new(
        r#"
            import "lib"
            fun main(value: Foo) { value.cross<caret> }
        "#,
    )
    .file(
        "lib.tolk",
        "
            struct Foo {}
            fun Foo.crossStatic() {}
            fun Foo.crossInstance(self) {}
        ",
    )
    .labels(&["crossStatic", "crossInstance"])
    .trigger_character(".")
    .check(expect![[r#"
        label          kind    detail  edit       text
        crossInstance  Method  (self)  1:29-1:34  crossInstance();$0"#]]);

    // Error-tolerant parsing still completes a method at the end of an open block.
    CompletionTest::new(
        "
            struct Foo {}
            fun Foo.touch(self) {}
            fun main(value: Foo) {
                value.tou<caret>
        ",
    )
    .labels(&["touch"])
    .trigger_character(".")
    .check(expect![[r#"
        label  kind    detail  edit       text
        touch  Method  (self)  3:10-3:13  touch();$0"#]]);
}

#[test]
fn excludes_declaration_names_and_internal_symbols() {
    // A local declaration name produces no completion items from any provider.
    CompletionTest::new("fun main() { val d<caret> = value; }").check(expect!["<none>"]);

    // Mutable local names use the same declaration-only context.
    CompletionTest::new("fun main() { var d<caret> = value; }").check(expect!["<none>"]);

    // Names inside a destructuring declaration do not receive value or snippet completion.
    CompletionTest::new("fun main() { val [first, sec<caret>] = pair; }").check(expect!["<none>"]);

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
        private   Keyword          0:13-0:17  private\s
        readonly  Keyword          0:13-0:17  readonly\s"#]]);

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
            fun main() { val value = int.build()<caret>.toString }"#]],
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
