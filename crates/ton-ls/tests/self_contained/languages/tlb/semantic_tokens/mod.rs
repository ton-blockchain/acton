use expect_test::expect;
use function_name::named;

use crate::self_contained::languages::tlb::helpers::case_tlb_semantic_tokens;

#[named]
#[test]
fn test_semantic_tokens_basic() {
    case_tlb_semantic_tokens(
        function_name!(),
        r#"
            foo$0 a:# = CommonMsgInfo;
        "#,
        expect![[r#"
            0:0   3 kind=type     text=foo
            0:6   7 kind=property text=a
            0:8   9 kind=macro    text=#
            0:12 25 kind=struct   text=CommonMsgInfo"#]],
    );
}
