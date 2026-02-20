use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const ASSERT_IMPORTS: &str = r#"
import "../../lib/testing/assert"
import "../../lib/testing/expect"
"#;

#[test]
fn assert_consumes_less_than1_returns_computed_function_result() {
    let source = format!(
        r#"{ASSERT_IMPORTS}

get fun `test-bt-consumes-less-than1-returns-result`() {{
    val result = Assert.consumesLessThan1(
        fun(value: int): int {{
            return value * 3 + 7;
        }},
        11,
        10000
    );

    expect(result).toEqual(40);
}}
"#
    );

    ProjectBuilder::new("bt-stdlib-assert-consumes-less-than1-pass")
        .test_file("assert_consumes_less_than1", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/assert_consumes_less_than1_returns_computed_function_result/assert_consumes_less_than1_returns_computed_function_result.stdout.txt",
        );
}
