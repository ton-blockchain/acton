use super::support::CompletionTest;
use expect_test::expect;

#[test]
fn completes_non_test_get_methods_in_second_argument() {
    // A get method name is offered only for the second net.runGetMethod argument.
    CompletionTest::new(
        r#"
            get fun currentCounter(): int { return 0 }
            get fun test_helper(): int { return 0 }
            fun main() { net.runGetMethod(null, "cur<caret>", null); }
        "#,
    )
    .labels(&["currentCounter", "test_helper"])
    .check(expect![[r#"
        label           kind    detail  edit       text
        currentCounter  Method          2:37-2:40  currentCounter"#]]);

    // A regular get method merely starting with test remains available.
    CompletionTest::new(
        r#"
            get fun testHelper(): int { return 0 }
            fun main() { net.runGetMethod(null, "test<caret>", null); }
        "#,
    )
    .labels(&["testHelper"])
    .check(expect![[r#"
        label       kind    detail  edit       text
        testHelper  Method          1:37-1:41  testHelper"#]]);
}

#[test]
fn applies_get_method_completion_inside_the_string() {
    // Applying the item replaces only the typed method-name prefix.
    CompletionTest::new(
        r#"
            get fun currentCounter(): int { return 0 }
            fun main() { net.runGetMethod(null, "cur<caret>", null); }
        "#,
    )
    .check_applied(
        "currentCounter",
        expect![[r#"
            get fun currentCounter(): int { return 0 }
            fun main() { net.runGetMethod(null, "currentCounter<caret>", null); }"#]],
    );
}
