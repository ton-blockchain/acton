use super::support::CompletionTest;
use expect_test::expect;

#[test]
fn completes_wallet_names_only_in_scripts_wallet_first_argument() {
    let manifest = r#"
        [wallets.deployer]
    "#;

    // Wallet names are available through the qualified scripts.wallet helper.
    CompletionTest::new(r#"fun main() { scripts.wallet("dep<caret>"); }"#)
        .manifest(manifest)
        .labels(&["deployer"])
        .check(expect![[r#"
            label     kind   detail    edit       text
            deployer  Value   (local)  0:29-0:32  deployer"#]]);

    // An unqualified wallet call is not treated as the Acton helper.
    CompletionTest::new(r#"fun main() { wallet("dep<caret>"); }"#)
        .manifest(manifest)
        .labels(&["deployer"])
        .check(expect!["<none>"]);
}

#[test]
fn applies_wallet_name_completion_inside_the_string() {
    // Applying a wallet name preserves the surrounding string literal.
    CompletionTest::new(r#"fun main() { scripts.wallet("dep<caret>"); }"#)
        .manifest(
            r#"
                [wallets.deployer]
            "#,
        )
        .check_applied(
            "deployer",
            expect![[r#"fun main() { scripts.wallet("deployer<caret>"); }"#]],
        );
}
