use super::support::CompletionTest;
use expect_test::expect;

#[test]
fn completes_return_variants_from_declared_type() {
    // A function without a declared return type offers a bare return.
    CompletionTest::new("fun foo() { ret<caret> }")
        .prefix("return")
        .check(expect![[r#"
            label    kind     detail  edit       text
            return;  Keyword          0:12-0:15  return;"#]]);

    // A void function offers a bare return.
    CompletionTest::new("fun foo(): void { ret<caret> }")
        .prefix("return")
        .check(expect![[r#"
            label    kind     detail  edit       text
            return;  Keyword          0:18-0:21  return;"#]]);

    // A bool function offers both literals and an arbitrary expression.
    CompletionTest::new("fun foo(): bool { ret<caret> }")
        .prefix("return")
        .check(expect![[r#"
            label           kind     detail  edit       text
            return true;    Snippet          0:18-0:21  return true;
            return false;   Snippet          0:18-0:21  return false;
            return <expr>;  Keyword          0:18-0:21  return $0;"#]]);

    // An int function offers zero and an arbitrary expression.
    CompletionTest::new("fun foo(): int { ret<caret> }")
        .prefix("return")
        .check(expect![[r#"
            label           kind     detail  edit       text
            return 0;       Snippet          0:17-0:20  return 0;
            return <expr>;  Keyword          0:17-0:20  return $0;"#]]);

    // A nullable function offers null and an arbitrary expression.
    CompletionTest::new("fun foo(): bool? { ret<caret> }")
        .prefix("return")
        .check(expect![[r#"
            label           kind     detail  edit       text
            return null;    Snippet          0:19-0:22  return null;
            return <expr>;  Keyword          0:19-0:22  return $0;"#]]);

    // A variable-width signed integer alias is recognized as integer-like.
    CompletionTest::new(
        "
            type intN = builtin;
            fun foo(): int27 { ret<caret> }
        ",
    )
    .prefix("return")
    .check(expect![[r#"
        label           kind     detail  edit       text
        return 0;       Snippet          1:19-1:22  return 0;
        return <expr>;  Keyword          1:19-1:22  return $0;"#]]);

    // A variable-width unsigned integer alias is recognized as integer-like.
    CompletionTest::new(
        "
            type uintN = builtin;
            fun foo(): uint32 { ret<caret> }
        ",
    )
    .prefix("return")
    .check(expect![[r#"
        label           kind     detail  edit       text
        return 0;       Snippet          1:20-1:23  return 0;
        return <expr>;  Keyword          1:20-1:23  return $0;"#]]);
}

#[test]
fn completes_return_variants_from_inferred_type() {
    // An earlier integer return drives completion when no return type is declared.
    CompletionTest::new(
        "
            fun foo(cond: bool) {
                if (cond) {
                    return 10;
                }
                ret<caret>
            }
        ",
    )
    .prefix("return")
    .check(expect![[r#"
        label           kind     detail  edit     text
        return 0;       Snippet          4:4-4:7  return 0;
        return <expr>;  Keyword          4:4-4:7  return $0;"#]]);

    // An earlier boolean return exposes the boolean-specific variants.
    CompletionTest::new(
        "
            fun foo(cond: bool) {
                if (cond) {
                    return true;
                }
                ret<caret>
            }
        ",
    )
    .prefix("return")
    .check(expect![[r#"
        label           kind     detail  edit     text
        return true;    Snippet          4:4-4:7  return true;
        return false;   Snippet          4:4-4:7  return false;
        return <expr>;  Keyword          4:4-4:7  return $0;"#]]);
}

#[test]
fn applies_return_completion_and_places_the_caret_in_the_expression() {
    // Applying the expression variant replaces the prefix and puts the caret before the semicolon.
    CompletionTest::new("fun foo(): int { ret<caret> }").check_applied(
        "return <expr>;",
        expect!["fun foo(): int { return <caret>; }"],
    );
}
