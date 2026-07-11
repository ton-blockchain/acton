use super::support::CompletionTest;
use expect_test::expect;

#[test]
fn completes_missing_contract_fields() {
    // An empty contract body offers every supported metadata field.
    CompletionTest::new(
        "
            contract Counter {
                <caret>
            }
        ",
    )
        .labels(&[
            "author",
            "version",
            "description",
            "incomingMessages",
            "incomingExternal",
            "outgoingMessages",
            "emittedEvents",
            "thrownErrors",
            "storage",
            "storageAtDeployment",
            "forceAbiExport",
        ])
        .check(expect![[r#"
            label                kind   detail                                     edit     text
            author               Field  : Author of the contract                   1:4-1:4  author: $0
            description          Field  : Description of the contract              1:4-1:4  description: $0
            emittedEvents        Field  : Emitted events type                      1:4-1:4  emittedEvents: $0
            forceAbiExport       Field  : Symbols additionally exported to ABI     1:4-1:4  forceAbiExport: $0
            incomingExternal     Field  : Allowed incoming external messages type  1:4-1:4  incomingExternal: $0
            incomingMessages     Field  : Allowed incoming messages type           1:4-1:4  incomingMessages: $0
            outgoingMessages     Field  : Outgoing messages type                   1:4-1:4  outgoingMessages: $0
            storage              Field  : Persistent storage structure             1:4-1:4  storage: $0
            storageAtDeployment  Field  : Storage structure at deployment          1:4-1:4  storageAtDeployment: $0
            thrownErrors         Field  : Thrown errors enum type                  1:4-1:4  thrownErrors: $0
            version              Field  : Version of the contract                  1:4-1:4  version: $0"#]]);
}

#[test]
fn excludes_existing_contract_fields() {
    // A field already declared in the body is not suggested again.
    CompletionTest::new(
        r#"
            contract Counter {
                author: "me"
                <caret>
            }
        "#,
    )
    .labels(&["author", "version", "storage"])
    .check(expect![[r#"
        label    kind   detail                          edit     text
        storage  Field  : Persistent storage structure  2:4-2:4  storage: $0
        version  Field  : Version of the contract       2:4-2:4  version: $0"#]]);
}

#[test]
fn applies_contract_field_completion() {
    // Applying storage inserts the field separator and activates its type placeholder.
    CompletionTest::new("contract Counter { stor<caret> }")
        .check_applied("storage", expect!["contract Counter { storage: <caret> }"]);
}
