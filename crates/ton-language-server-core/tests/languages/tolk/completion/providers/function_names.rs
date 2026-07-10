use super::support::CompletionTest;
use expect_test::expect;

#[test]
fn completes_well_known_function_and_serialization_method_names() {
    // An incomplete function declaration offers all well-known entry-point names.
    CompletionTest::new("fun <caret>")
        .labels(&["main", "onInternalMessage"])
        .check(expect![[r#"
            label              kind      detail  edit     text
            main               Function          0:4-0:4  main() {$0}
            onInternalMessage  Function          0:4-0:4  onInternalMessage(in: InMessage) {$0}"#]]);

    // A method name keeps the receiver and inserts the complete serialization signature.
    CompletionTest::new(
        "
            struct Storage {}
            fun Storage.pa<caret>
        ",
    )
    .labels(&["packToBuilder", "unpackFromSlice"])
    .check(expect![[r#"
        label            kind      detail  edit       text
        packToBuilder    Function          1:12-1:14  packToBuilder(self, mutate b: builder) {$0}
        unpackFromSlice  Function          1:12-1:14  unpackFromSlice(mutate s: slice): Storage {$0}"#]]);

    // Existing parameters and body are preserved; only the function name is replaced.
    CompletionTest::new("fun <caret>(): int {}")
        .labels(&["main", "onInternalMessage"])
        .check(expect![[r#"
            label              kind      detail  edit     text
            main               Function          0:4-0:4  main
            onInternalMessage  Function          0:4-0:4  onInternalMessage"#]]);

    // Existing method parameters and body are preserved for packToBuilder.
    CompletionTest::new("fun int.pa<caret>() {}")
        .labels(&["packToBuilder"])
        .check(expect![[r#"
            label          kind      detail  edit      text
            packToBuilder  Function          0:8-0:10  packToBuilder"#]]);

    // A missing method signature includes the receiver-dependent return type.
    CompletionTest::new("fun int.unpa<caret>")
        .labels(&["unpackFromSlice"])
        .check(expect![[r#"
            label            kind      detail  edit      text
            unpackFromSlice  Function          0:8-0:12  unpackFromSlice(mutate s: slice): int {$0}"#]]);

    // An existing method signature receives only the method name.
    CompletionTest::new("fun int.unpa<caret>() {}")
        .labels(&["unpackFromSlice"])
        .check(expect![[r#"
            label            kind      detail  edit      text
            unpackFromSlice  Function          0:8-0:12  unpackFromSlice"#]]);
}

#[test]
fn excludes_already_defined_well_known_functions() {
    // A declaration already present in the file is not suggested again.
    CompletionTest::new(
        "
            fun onInternalMessage(in: InMessage) {}
            fun <caret>
        ",
    )
    .labels(&["onInternalMessage", "onExternalMessage"])
    .check(expect![[r#"
        label              kind      detail  edit     text
        onExternalMessage  Function          1:4-1:4  onExternalMessage(inMsg: slice) {$0}"#]]);
}

#[test]
fn applies_well_known_function_and_method_names() {
    // Applying a function-name completion inserts the missing signature and body.
    CompletionTest::new("fun onInt<caret>").check_applied(
        "onInternalMessage",
        expect!["fun onInternalMessage(in: InMessage) {<caret>}"],
    );

    // Applying a method-name completion preserves the receiver and inserts its signature.
    CompletionTest::new("fun Storage.pa<caret>").check_applied(
        "packToBuilder",
        expect!["fun Storage.packToBuilder(self, mutate b: builder) {<caret>}"],
    );

    // Applying a function name to an existing signature preserves the signature and body.
    CompletionTest::new("fun ma<caret>(): int {}")
        .check_applied("main", expect!["fun main<caret>(): int {}"]);

    // Applying a method name to an existing signature preserves its parameters and body.
    CompletionTest::new("fun int.pa<caret>() {}").check_applied(
        "packToBuilder",
        expect!["fun int.packToBuilder<caret>() {}"],
    );
}
