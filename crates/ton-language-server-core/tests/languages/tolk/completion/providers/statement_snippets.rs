use super::support::CompletionTest;
use expect_test::expect;

#[test]
fn completes_statement_and_contextual_catch_snippets() {
    // A statement position exposes every general-purpose statement snippet.
    CompletionTest::new("fun main() { <caret> }")
        .labels(&[
            "val", "var", "valt", "vart", "if", "ife", "while", "do-while", "repeat", "try", "tryc",
        ])
        .check(expect![[r#"
            label     kind     detail  edit       text
            do-while  Snippet          0:13-0:13  do {\n\t$0\n} while (${1:condition});
            if        Snippet          0:13-0:13  if (${1:condition}) {\n\t$0\n}
            ife       Snippet          0:13-0:13  if (${1:condition}) {\n\t$2\n} else {\n\t$0\n}
            repeat    Snippet          0:13-0:13  repeat(${1:count}) {\n\t$0\n}
            try       Snippet          0:13-0:13  try {\n\t$0\n}
            tryc      Snippet          0:13-0:13  try {\n\t$1\n} catch (${2:e}) {\n\t$0\n}
            val       Snippet          0:13-0:13  val ${1:name} = ${2:value};
            valt      Snippet          0:13-0:13  val ${1:name}: ${2:int} = ${3:value};
            var       Snippet          0:13-0:13  var ${1:name} = ${2:value};
            vart      Snippet          0:13-0:13  var ${1:name}: ${2:int} = ${3:value};
            while     Snippet          0:13-0:13  while (${1:condition}) {\n\t$0\n}"#]]);

    // A completed try block offers the contextual catch snippet.
    CompletionTest::new("fun main() { try {} cat<caret> }")
        .labels(&["catch"])
        .check(expect![[r#"
            label  kind     detail  edit       text
            catch  Snippet          0:20-0:23  catch (${1:e}) {\n\t$0\n}"#]]);

    // Catch is not a standalone expression snippet.
    CompletionTest::new("fun main() { val value = cat<caret>; }")
        .labels(&["catch"])
        .check(expect!["<none>"]);

    // A catch-variable declaration is not a statement-completion position.
    CompletionTest::new(
        "
            const errorCode = 100
            fun main() { try {} catch (err<caret>) {} }
        ",
    )
    .check(expect![[r#"
        label                              kind           detail  edit       text
        BounceMode.NoBounce                EnumMember             1:27-1:30  BounceMode.NoBounce
        BounceMode.Only256BitsOfBody       EnumMember             1:27-1:30  BounceMode.Only256BitsOfBody
        BounceMode.RichBounce              EnumMember             1:27-1:30  BounceMode.RichBounce
        BounceMode.RichBounceOnlyRootCell  EnumMember             1:27-1:30  BounceMode.RichBounceOnlyRootCell
        as                                 Keyword                1:27-1:30  as 
        false                              Keyword                1:27-1:30  false
        is                                 Keyword                1:27-1:30  is 
        lazy                               Keyword                1:27-1:30  lazy 
        mutate                             Keyword                1:27-1:30  mutate 
        true                               Keyword                1:27-1:30  true
        match                              Snippet                1:27-1:30  match (${1:condition}) {\n\t$0\n}
        bits256                            TypeParameter          1:27-1:30  bits256
        bits{X}                            TypeParameter          1:27-1:30  bits${1:32}
        bytes32                            TypeParameter          1:27-1:30  bytes32
        bytes{X}                           TypeParameter          1:27-1:30  bytes${1:32}
        int128                             TypeParameter          1:27-1:30  int128
        int16                              TypeParameter          1:27-1:30  int16
        int256                             TypeParameter          1:27-1:30  int256
        int257                             TypeParameter          1:27-1:30  int257
        int32                              TypeParameter          1:27-1:30  int32
        int64                              TypeParameter          1:27-1:30  int64
        int8                               TypeParameter          1:27-1:30  int8
        int{X}                             TypeParameter          1:27-1:30  int${1:32}
        uint128                            TypeParameter          1:27-1:30  uint128
        uint16                             TypeParameter          1:27-1:30  uint16
        uint256                            TypeParameter          1:27-1:30  uint256
        uint32                             TypeParameter          1:27-1:30  uint32
        uint64                             TypeParameter          1:27-1:30  uint64
        uint8                              TypeParameter          1:27-1:30  uint8
        uint{X}                            TypeParameter          1:27-1:30  uint${1:32}"#]]);
}

#[test]
fn applies_statement_snippets() {
    // Variable completion expands defaults and places the caret at the first tab stop.
    CompletionTest::new("fun main() { val<caret> }")
        .check_applied("val", expect!["fun main() { val name<caret> = value; }"]);

    // Typed immutable-variable completion selects the variable-name placeholder first.
    CompletionTest::new("fun main() { valt<caret> }").check_applied(
        "valt",
        expect!["fun main() { val name<caret>: int = value; }"],
    );

    // Mutable-variable completion expands an editable name and value.
    CompletionTest::new("fun main() { var<caret> }")
        .check_applied("var", expect!["fun main() { var name<caret> = value; }"]);

    // Typed mutable-variable completion includes its type placeholder.
    CompletionTest::new("fun main() { vart<caret> }").check_applied(
        "vart",
        expect!["fun main() { var name<caret>: int = value; }"],
    );

    // If completion selects the condition before its body.
    CompletionTest::new("fun main() { if<caret> }").check_applied(
        "if",
        expect![[r#"
            fun main() { if (condition<caret>) {

            } }"#]],
    );

    // If-else completion applies the complete control-flow structure.
    CompletionTest::new("fun main() { ife<caret> }").check_applied(
        "ife",
        expect![[r#"
                fun main() { if (condition<caret>) {

                } else {

                } }"#]],
    );

    // Do-while completion inserts both the body and trailing condition.
    CompletionTest::new("fun main() { do<caret> }").check_applied(
        "do-while",
        expect![[r#"
            fun main() { do {

            } while (condition<caret>); }"#]],
    );

    // Repeat completion selects its iteration count first.
    CompletionTest::new("fun main() { repe<caret> }").check_applied(
        "repeat",
        expect![[r#"
            fun main() { repeat(count<caret>) {

            } }"#]],
    );

    // Try-catch completion places the caret in the try body before later tab stops.
    CompletionTest::new("fun main() { tryc<caret> }").check_applied(
        "tryc",
        expect![[r#"
                fun main() { try {
                	<caret>
                } catch (e) {

                } }"#]],
    );
}
