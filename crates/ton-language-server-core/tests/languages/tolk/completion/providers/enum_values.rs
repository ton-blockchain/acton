use super::support::CompletionTest;
use expect_test::expect;

#[test]
fn completes_prefixed_enum_members_in_expressions() {
    // Bare enum-member prefixes produce qualified enum values.
    CompletionTest::new(
        r#"
            enum Mode { First, Second }
            fun main() { val mode = Fir<caret>; }
        "#,
    )
    .labels(&["Mode.First", "Mode.Second"])
    .check(expect![[r#"
        label        kind        detail   edit       text
        Mode.First   EnumMember  of Mode  1:24-1:27  Mode.First
        Mode.Second  EnumMember  of Mode  1:24-1:27  Mode.Second"#]]);
}

#[test]
fn completes_enum_members_after_the_enum_name() {
    // Qualified enum completion inserts only the member after an existing dot.
    CompletionTest::new(
        "
            enum Mode { First, Second }
            fun main() { val mode = Mode.<caret>; }
        ",
    )
    .labels(&["First", "Second"])
    .trigger_character(".")
    .check(expect![[r#"
        label   kind        detail   edit       text
        First   EnumMember  of Mode  1:29-1:29  First
        Second  EnumMember  of Mode  1:29-1:29  Second"#]]);
}

#[test]
fn includes_enum_member_values_and_owner_metadata() {
    // Explicit enum values remain visible next to both qualified and unqualified
    // completion labels.
    CompletionTest::new(
        "
            enum Mode { First = 10, Second = 20 }
            fun main() { val mode = Mode.<caret>; }
        ",
    )
    .labels(&["First", "Second"])
    .trigger_character(".")
    .check(expect![[r#"
        label   kind        detail          edit       text
        First   EnumMember   = 10  of Mode  1:29-1:29  First
        Second  EnumMember   = 20  of Mode  1:29-1:29  Second"#]]);
}

#[test]
fn applies_qualified_enum_member_completion() {
    // Applying the candidate replaces the bare prefix with the qualified member.
    CompletionTest::new(
        "
            enum Mode { First, Second }
            fun main() { val mode = Fir<caret>; }
        ",
    )
    .check_applied(
        "Mode.First",
        expect![[r#"
            enum Mode { First, Second }
            fun main() { val mode = Mode.First<caret>; }"#]],
    );

    // Applying a member after the enum name preserves the qualifier and dot.
    CompletionTest::new(
        "
            enum Mode { First, Second }
            fun main() { val mode = Mode.<caret>; }
        ",
    )
    .trigger_character(".")
    .check_applied(
        "First",
        expect![[r#"
            enum Mode { First, Second }
            fun main() { val mode = Mode.First<caret>; }"#]],
    );
}
