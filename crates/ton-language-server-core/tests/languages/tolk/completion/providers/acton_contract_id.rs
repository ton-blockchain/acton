use super::support::CompletionTest;
use expect_test::expect;

#[test]
fn completes_contract_ids_only_in_build_first_argument() {
    let manifest = r#"
        [contracts.counter]
        src = "contracts/counter.tolk"
    "#;

    // Contract IDs are available in the first argument of Acton's build helper.
    CompletionTest::new(r#"fun main() { build("cou<caret>"); }"#)
        .manifest(manifest)
        .labels(&["counter"])
        .check(expect![[r#"
            label    kind   detail  edit       text
            counter  Class          0:20-0:23  counter"#]]);

    // Later build arguments do not receive contract ID completion.
    CompletionTest::new(r#"fun main() { build("code", "cou<caret>"); }"#)
        .manifest(manifest)
        .labels(&["counter"])
        .check(expect!["<none>"]);
}

#[test]
fn applies_contract_id_completion_inside_the_string() {
    // Applying a contract ID replaces only the current string segment.
    CompletionTest::new(r#"fun main() { build("cou<caret>"); }"#)
        .manifest(
            r#"
                [contracts.counter]
                src = "contracts/counter.tolk"
            "#,
        )
        .check_applied(
            "counter",
            expect![[r#"fun main() { build("counter<caret>"); }"#]],
        );
}
