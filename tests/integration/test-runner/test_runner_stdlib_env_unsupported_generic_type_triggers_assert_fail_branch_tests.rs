use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

#[test]
fn env_unsupported_generic_type_triggers_assert_fail_branch() {
    ProjectBuilder::new("bq-stdlib-env-unsupported-generic")
        .test_file(
            "env_unsupported_generic",
            r#"
            import "../../lib/env"

            struct Unsupported {
                value: int,
            }

            get fun `test-bq-stdlib-env-unsupported-generic`() {
                env<Unsupported>("BQ_ENV_UNSUPPORTED");
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains(
            "env() supports only int, bool, string, slice, address and cell types, but got Unsupported",
        )
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_stdlib_env_unsupported_generic_type_triggers_assert_fail_branch_tests/env_unsupported_generic_type_triggers_assert_fail_branch.stdout.txt",
        );
}
