//! Reserved integration test module for subagent BQ.
//!
//! Ownership boundary for agent BQ:
//! - tests/integration/test_std_agent_bq_tests.rs
//! - tests/integration/snapshots/test_std_agent_bq/**
//! - tests/integration/testdata/test_std_agent_bq/**
//! - tests/support/test_std_agent_bq/** (optional)
//!
//! Required test name prefix:
//! - bq_stdlib_

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

#[test]
fn bq_stdlib_env_unsupported_generic_type_triggers_assert_fail_branch() {
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
            "integration/snapshots/test_std_agent_bq/bq_stdlib_env_unsupported_generic_type_triggers_assert_fail_branch.stdout.txt",
        );
}
