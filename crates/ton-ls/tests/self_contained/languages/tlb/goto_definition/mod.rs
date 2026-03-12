use expect_test::expect;
use function_name::named;

use crate::self_contained::languages::tlb::helpers::case_tlb_resolve;

#[named]
#[test]
fn test_resolve_multi_definition() {
    case_tlb_resolve(
        function_name!(),
        r#"
            foo$0 a:# = CommonMsgInfo;
            bar$1 b:# = CommonMsgInfo;
            baz$2 x:<caret>CommonMsgInfo = Wrap;
        "#,
        expect![[r#"
            2:8 -> 0:12 resolved
            2:8 -> 1:12 resolved"#]],
    );
}

#[named]
#[test]
fn test_resolve_unresolved_symbol() {
    case_tlb_resolve(
        function_name!(),
        r#"
            foo$0 a:# = CommonMsgInfo;
            bar$1 b:<caret>MissingType = Wrap;
        "#,
        expect!["1:8 unresolved"],
    );
}
