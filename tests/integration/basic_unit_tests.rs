use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

#[test]
fn test_passing_test() {
    ProjectBuilder::new("simple")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"

            get fun `test pass`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches("integration/snapshots/test_passing_output.stdout.txt");
}

#[test]
fn test_failing_test() {
    ProjectBuilder::new("simple")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"

            get fun `test fail`() {
                expect(1).toEqual(2);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_snapshot_matches("integration/snapshots/test_failing_output.stdout.txt");
}

#[test]
fn test_multiple_passing_tests() {
    ProjectBuilder::new("simple")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"

            get fun `test pass 1`() {
                expect(1).toEqual(1);
            }

            get fun `test pass 2`() {
                expect(2).toEqual(2);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(2)
        .assert_snapshot_matches("integration/snapshots/test_multiple_passing_output.stdout.txt");
}
