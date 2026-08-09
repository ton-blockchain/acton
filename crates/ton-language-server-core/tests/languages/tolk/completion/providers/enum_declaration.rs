use super::support::CompletionTest;
use expect_test::expect;

#[test]
fn completes_enum_member_declaration() {
    // An enum body offers the member declaration snippet.
    CompletionTest::new(
        r#"
            enum Mode {
                <caret>
            }
        "#,
    )
    .labels(&["member"])
    .check(expect![[r#"
        label   kind     detail  edit     text
        member  Snippet          1:4-1:4  ${1:MEMBER} = ${2:0}$0"#]]);
}

#[test]
fn excludes_an_existing_enum_member_name() {
    // Typing an enum member declaration name does not offer a new-member snippet.
    CompletionTest::new("enum Mode { Fir<caret>, Second }").check(expect![[r#"
        label   kind     detail  edit       text
        member  Snippet          0:12-0:15  ${1:MEMBER} = ${2:0}$0"#]]);
}

#[test]
fn applies_enum_member_declaration() {
    // Applying the snippet selects the member-name placeholder first.
    CompletionTest::new(
        r#"
            enum Mode {
                <caret>
            }
        "#,
    )
    .check_applied(
        "member",
        expect![[r#"
            enum Mode {
                MEMBER<caret> = 0
            }"#]],
    );
}
