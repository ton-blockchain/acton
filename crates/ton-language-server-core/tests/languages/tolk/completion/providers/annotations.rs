use super::support::CompletionTest;
use expect_test::expect;

#[test]
fn completes_function_get_method_and_entry_point_annotations() {
    // An ordinary function offers function-level annotations but not get-method-only ones.
    CompletionTest::new(
        "
            @<caret>
            fun foo() {}
        ",
    )
    .labels(&[
        "custom",
        "deprecated",
        "inline",
        "inline_ref",
        "method_id",
        "noinline",
        "pure",
        "test",
    ])
    .check(expect![[r#"
        label       kind   detail  edit     text
        custom      Event          0:1-0:1  custom($0)
        deprecated  Event          0:1-0:1  deprecated("$0")
        inline      Event          0:1-0:1  inline
        inline_ref  Event          0:1-0:1  inline_ref
        method_id   Event          0:1-0:1  method_id(${1:0x1})$0
        noinline    Event          0:1-0:1  noinline
        pure        Event          0:1-0:1  pure"#]]);

    // A get method additionally offers @test and @method_id.
    CompletionTest::new(
        "
            @<caret>
            get fun foo() {}
        ",
    )
    .labels(&["test", "inline", "method_id"])
    .check(expect![[r#"
        label      kind   detail  edit     text
        inline     Event          0:1-0:1  inline
        method_id  Event          0:1-0:1  method_id(${1:0x1})$0
        test       Event          0:1-0:1  test"#]]);

    // An internal-message entry point offers its bounced-message policy annotation.
    CompletionTest::new(
        "
            @<caret>
            fun onInternalMessage(in: InMessage) {}
        ",
    )
    .labels(&["on_bounced_policy", "inline", "custom"])
    .check(expect![[r#"
        label              kind   detail  edit     text
        custom             Event          0:1-0:1  custom($0)
        inline             Event          0:1-0:1  inline
        on_bounced_policy  Event          0:1-0:1  on_bounced_policy("${1:manual}")$0"#]]);
}

#[test]
fn filters_struct_and_field_annotations_by_path_and_owner() {
    // A struct offers only annotations valid on struct declarations.
    CompletionTest::new(
        "
            @<caret>
            struct Message {}
        ",
    )
    .labels(&[
        "abi.minimalMsgValue",
        "abi.preferredSendMode",
        "custom",
        "deprecated",
        "overflow1023_policy",
        "inline",
    ])
    .check(expect![[r#"
        label                  kind   detail  edit     text
        abi.minimalMsgValue    Event          0:1-0:1  abi.minimalMsgValue($0)
        abi.preferredSendMode  Event          0:1-0:1  abi.preferredSendMode($0)
        custom                 Event          0:1-0:1  custom($0)
        deprecated             Event          0:1-0:1  deprecated("$0")
        overflow1023_policy    Event          0:1-0:1  overflow1023_policy("${1:suppress}")$0"#]]);

    // An abi. prefix narrows struct completion to that annotation namespace.
    CompletionTest::new(
        "
            @abi.<caret>
            struct Message {}
        ",
    )
    .labels(&["minimalMsgValue", "preferredSendMode", "clientType"])
    .check(expect![[r#"
        label              kind   detail  edit     text
        minimalMsgValue    Event          0:5-0:5  minimalMsgValue($0)
        preferredSendMode  Event          0:5-0:5  preferredSendMode($0)"#]]);

    // A struct field offers field-level annotations but not function-level ones.
    CompletionTest::new(
        "
            struct Message {
                @<caret>
                body: cell
            }
        ",
    )
    .labels(&["abi.clientType", "custom", "deprecated", "inline"])
    .check(expect![[r#"
        label           kind   detail  edit     text
        abi.clientType  Event          1:5-1:5  abi.clientType($0)
        custom          Event          1:5-1:5  custom($0)
        deprecated      Event          1:5-1:5  deprecated("$0")"#]]);

    // An abi. prefix narrows field completion to clientType.
    CompletionTest::new(
        "
            struct Message {
                @abi.<caret>
                body: cell
            }
        ",
    )
    .labels(&["clientType", "minimalMsgValue"])
    .check(expect![[r#"
        label       kind   detail  edit     text
        clientType  Event          1:9-1:9  clientType($0)"#]]);
}

#[test]
fn completes_test_annotations_and_suppresses_invalid_nested_paths() {
    // A test. prefix offers all test-control annotations and their overloads.
    CompletionTest::new(
        "
            @test.<caret>
            get fun balance(): int { return 0; }
        ",
    )
    .labels(&["fail_with", "fuzz", "gas_limit", "skip", "todo"])
    .check(expect![[r#"
        label      kind   detail  edit     text
        fail_with  Event          0:6-0:6  fail_with($0)
        fuzz       Event          0:6-0:6  fuzz
        fuzz       Event          0:6-0:6  fuzz($0)
        gas_limit  Event          0:6-0:6  gas_limit($0)
        skip       Event          0:6-0:6  skip
        todo       Event          0:6-0:6  todo
        todo       Event          0:6-0:6  todo("$0")"#]]);

    // The abi namespace is unavailable on ordinary functions.
    CompletionTest::new(
        "
            @abi.<caret>
            fun foo() {}
        ",
    )
    .labels(&["minimalMsgValue", "preferredSendMode", "clientType"])
    .check(expect!["<none>"]);

    // An annotation already present on a field is not offered again.
    CompletionTest::new(
        "
            struct Message {
                @abi.clientType(Cell)
                @abi.<caret>
                body: cell
            }
        ",
    )
    .labels(&["clientType"])
    .check(expect!["<none>"]);
}

#[test]
fn emits_annotation_insertion_snippets() {
    // Deprecated expands its required reason argument.
    CompletionTest::new(
        "
            @depreca<caret>
            fun foo() {}
        ",
    )
    .labels(&["deprecated"])
    .check(expect![[r#"
        label       kind   detail  edit     text
        deprecated  Event          0:1-0:8  deprecated("$0")"#]]);

    // Overflow policy expands its default suppress value.
    CompletionTest::new(
        "
            @overfl<caret>
            struct Message {}
        ",
    )
    .labels(&["overflow1023_policy"])
    .check(expect![[r#"
        label                kind   detail  edit     text
        overflow1023_policy  Event          0:1-0:7  overflow1023_policy("${1:suppress}")$0"#]]);

    // Method ID expands its numeric argument.
    CompletionTest::new(
        "
            @method<caret>
            get fun foo() {}
        ",
    )
    .labels(&["method_id"])
    .check(expect![[r#"
        label      kind   detail  edit     text
        method_id  Event          0:1-0:7  method_id(${1:0x1})$0"#]]);

    // A field ABI annotation is filtered by its nested prefix.
    CompletionTest::new(
        "
            struct Message {
                @abi.client<caret>
                body: cell
            }
        ",
    )
    .labels(&["clientType"])
    .check(expect![[r#"
        label       kind   detail  edit      text
        clientType  Event          1:9-1:15  clientType($0)"#]]);
}

#[test]
fn applies_parameterized_annotation_completions() {
    // Inline completion replaces only its typed annotation prefix.
    CompletionTest::new(
        "
            @inl<caret>
            fun foo() {}
        ",
    )
    .check_applied(
        "inline",
        expect![[r#"
            @inline<caret>
            fun foo() {}"#]],
    );

    // Inline-ref completion replaces only its typed annotation prefix.
    CompletionTest::new(
        "
            @inline_r<caret>
            fun foo() {}
        ",
    )
    .check_applied(
        "inline_ref",
        expect![[r#"
            @inline_ref<caret>
            fun foo() {}"#]],
    );

    // Deprecated completion replaces only the annotation name and selects its reason.
    CompletionTest::new(
        "
            @depreca<caret>
            fun foo() {}
        ",
    )
    .check_applied(
        "deprecated",
        expect![[r#"
            @deprecated("<caret>")
            fun foo() {}"#]],
    );

    // Overflow policy completion expands its default policy argument.
    CompletionTest::new(
        "
            @overfl<caret>
            struct Message {}
        ",
    )
    .check_applied(
        "overflow1023_policy",
        expect![[r#"
            @overflow1023_policy("suppress<caret>")
            struct Message {}"#]],
    );

    // Noinline completion replaces only its typed annotation prefix.
    CompletionTest::new(
        "
            @noinli<caret>
            fun foo() {}
        ",
    )
    .check_applied(
        "noinline",
        expect![[r#"
            @noinline<caret>
            fun foo() {}"#]],
    );

    // Bounced-policy completion expands its default policy argument.
    CompletionTest::new(
        "
            @on_bounced_po<caret>
            fun onBouncedMessage() {}
        ",
    )
    .check_applied(
        "on_bounced_policy",
        expect![[r#"
            @on_bounced_policy("manual<caret>")
            fun onBouncedMessage() {}"#]],
    );

    // Method-ID completion expands its numeric identifier argument.
    CompletionTest::new(
        "
            @method<caret>
            get fun foo() {}
        ",
    )
    .check_applied(
        "method_id",
        expect![[r#"
            @method_id(0x1<caret>)
            get fun foo() {}"#]],
    );

    // Custom completion expands its user-defined value argument.
    CompletionTest::new(
        "
            @custo<caret>
            get fun foo() {}
        ",
    )
    .check_applied(
        "custom",
        expect![[r#"
            @custom(<caret>)
            get fun foo() {}"#]],
    );

    // A nested struct ABI annotation preserves the abi. qualifier.
    CompletionTest::new(
        "
            @abi.min<caret>
            struct Message {}
        ",
    )
    .check_applied(
        "minimalMsgValue",
        expect![[r#"
            @abi.minimalMsgValue(<caret>)
            struct Message {}"#]],
    );

    // A nested field ABI annotation preserves indentation and its qualifier.
    CompletionTest::new(
        "
            struct Message {
                @abi.<caret>
                body: cell
            }
        ",
    )
    .check_applied(
        "clientType",
        expect![[r#"
            struct Message {
                @abi.clientType(<caret>)
                body: cell
            }"#]],
    );

    // A partially typed field ABI annotation replaces only its final name segment.
    CompletionTest::new(
        "
            struct Message {
                @abi.client<caret>
                body: cell
            }
        ",
    )
    .check_applied(
        "clientType",
        expect![[r#"
            struct Message {
                @abi.clientType(<caret>)
                body: cell
            }"#]],
    );

    // Test failure completion expands its expected exit-code argument.
    CompletionTest::new(
        "
            @test.fail<caret>
            get fun foo() {}
        ",
    )
    .check_applied(
        "fail_with",
        expect![[r#"
            @test.fail_with(<caret>)
            get fun foo() {}"#]],
    );
}
