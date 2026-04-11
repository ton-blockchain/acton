use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

const ASSERT_IMPORTS: &str = r#"
import "../../lib/testing/assert"
import "../../lib/testing/expect"
"#;

#[test]
fn assert_consumes_less_than3_returns_computed_result() {
    let source = format!(
        r"{ASSERT_IMPORTS}

get fun `test bu consumes less than3 returns result`() {{
    val result = Assert.consumesLessThan3(
        fun(a: int, b: int, c: int): int {{
            return a * 100 + b * 10 + c;
        }},
        4,
        2,
        7,
        10000
    );

    expect(result).toEqual(427);
}}
"
    );

    ProjectBuilder::new("bu-stdlib-assert-consumes-less-than3-pass")
        .test_file("assert_consumes_less_than3", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/assert_consumes_less_than3_returns_computed_result/assert_consumes_less_than3_returns_computed_result.stdout.txt",
        );
}

#[test]
fn assert_consumes_less_than3_rejects_wrong_callback_arity() {
    let fixture = FixtureProject::load("basic");
    let test_path = "tests/bu_assert_consumes_less_than3_wrong_arity.test.tolk";
    let source = format!(
        r"{ASSERT_IMPORTS}

get fun `test bu consumes less than3 wrong arity`() {{
    Assert.consumesLessThan3(
        fun(a: int, b: int): int {{
            return a + b;
        }},
        1,
        2,
        3,
        10000
    );
}}
"
    );

    fs::write(fixture.path().join(test_path), source)
        .expect("failed to write bu consumesLessThan3 arity fixture test");

    fixture
        .acton()
        .test()
        .path(test_path)
        .run()
        .failure()
        .assert_failed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/assert_consumes_less_than3_returns_computed_result/assert_consumes_less_than3_rejects_wrong_callback_arity.stdout.txt",
        );
}
