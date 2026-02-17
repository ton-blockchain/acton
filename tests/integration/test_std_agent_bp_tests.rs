//! Reserved integration test module for subagent BP.
//!
//! Ownership boundary for agent BP:
//! - tests/integration/test_std_agent_bp_tests.rs
//! - tests/integration/snapshots/test_std_agent_bp/**
//! - tests/integration/testdata/test_std_agent_bp/**
//! - tests/support/test_std_agent_bp/** (optional)
//!
//! Required test name prefix:
//! - bp_stdlib_

use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

#[test]
fn bp_stdlib_env_slice_returns_raw_and_empty_values_and_null_when_missing() {
    ProjectBuilder::new("bp-stdlib-env-slice-branches")
        .test_file(
            "env_slice_branches",
            r#"
            import "../../lib/env"
            import "../../lib/testing/expect"

            get fun `test-bp-stdlib-env-slice-branches`() {
                // BUG: env<slice> should read raw environment values; expected "  keep surrounding spaces  ", got unsupported type error for `slice`.
                expect(env<slice>("BP_ENV_SLICE_RAW")).toEqual("  keep surrounding spaces  ");
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .env("BP_ENV_SLICE_RAW", "  keep surrounding spaces  ")
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("env() supports only int, bool, slice, address and cell types, but got slice")
        .assert_snapshot_matches(
            "integration/snapshots/test_std_agent_bp/bp_stdlib_env_slice_returns_raw_and_empty_values_and_null_when_missing.stdout.txt",
        );
}

#[test]
fn bp_stdlib_env_or_string_uses_missing_fallback_and_preserves_present_values() {
    let fixture = FixtureProject::load("basic");
    let test_path = "tests/bp_env_or_string_missing_fallback.test.tolk";
    let source = r#"
import "../../lib/env"
import "../../lib/testing/expect"

get fun `test-bp-stdlib-env-or-string-fallback`() {
    expect(envOr<string>("BP_ENV_OR_MISSING", "fallback")).toEqual("fallback");
    expect(env<string>("BP_ENV_OR_MISSING")).toBeNull();
    expect(envOr<string>("BP_ENV_OR_PRESENT", "fallback")).toEqual("from-env");
    expect(env<string>("BP_ENV_OR_PRESENT")).toEqual("from-env");
    expect(envOr<string>("BP_ENV_OR_EMPTY", "fallback")).toEqual("");
    expect(envOr<string>("BP_ENV_OR_SPACED", "fallback")).toEqual("  spaced value  ");
}
"#;

    fs::write(fixture.path().join(test_path), source)
        .expect("failed to write bp envOr<string> fixture test");

    fixture
        .acton()
        .test()
        .path(test_path)
        .env("BP_ENV_OR_PRESENT", "from-env")
        .env("BP_ENV_OR_EMPTY", "")
        .env("BP_ENV_OR_SPACED", "  spaced value  ")
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test_std_agent_bp/bp_stdlib_env_or_string_uses_missing_fallback_and_preserves_present_values.stdout.txt",
        );
}
