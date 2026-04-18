use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

#[test]
fn env_slice_returns_raw_and_empty_values_and_null_when_missing() {
    ProjectBuilder::new("bp-stdlib-env-slice-branches")
        .test_file(
            "env_slice_branches",
            r#"
            import "../../lib/env"
            import "../../lib/testing/expect"

            get fun `test bp stdlib env slice branches`() {
                expect(env<slice>("BP_ENV_SLICE_RAW")).toEqual("  keep surrounding spaces  ");
                expect(env<slice>("BP_ENV_SLICE_EMPTY")).toEqual("");
                expect(env<slice>("BP_ENV_SLICE_MISSING")).toBeNull();
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .env("BP_ENV_SLICE_RAW", "  keep surrounding spaces  ")
        .env("BP_ENV_SLICE_EMPTY", "")
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/env_slice_returns_raw_and_empty_values_and_null_when_missing/env_slice_returns_raw_and_empty_values_and_null_when_missing.stdout.txt",
        );
}

#[test]
fn env_or_string_uses_missing_fallback_and_preserves_present_values() {
    let fixture = FixtureProject::load("basic");
    let test_path = "tests/bp_env_or_string_missing_fallback.test.tolk";
    let source = r#"
import "../../lib/env"
import "../../lib/testing/expect"

get fun `test bp stdlib env or string fallback`() {
    expect(env<string>("BP_ENV_OR_MISSING") ?? "fallback").toEqual("fallback");
    expect(env<string>("BP_ENV_OR_MISSING")).toBeNull();
    expect(env<string>("BP_ENV_OR_PRESENT") ?? "fallback").toEqual("from-env");
    expect(env<string>("BP_ENV_OR_PRESENT")).toEqual("from-env");
    expect(env<string>("BP_ENV_OR_EMPTY") ?? "fallback").toEqual("");
    expect(env<string>("BP_ENV_OR_SPACED") ?? "fallback").toEqual("  spaced value  ");
}
"#;

    fs::write(fixture.path().join(test_path), source)
        .expect("failed to write bp env<string> fallback fixture test");

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
            "integration/snapshots/test-runner/env_slice_returns_raw_and_empty_values_and_null_when_missing/env_or_string_uses_missing_fallback_and_preserves_present_values.stdout.txt",
        );
}

#[test]
fn env_or_slice_uses_fallback_for_missing_and_present_value_when_set() {
    ProjectBuilder::new("bp-stdlib-env-or-slice-fallback-vs-present")
        .test_file(
            "env_or_slice",
            r#"
            import "../../lib/env"
            import "../../lib/testing/expect"

            get fun `test bp stdlib env or slice fallback vs present`() {
                val fallbackOpt = env<slice>("BP_ENV_OR_SLICE_FALLBACK_SOURCE");
                expect(fallbackOpt).toBeNotNull();

                val fallback = fallbackOpt!;
                expect(env<slice>("BP_ENV_OR_SLICE_MISSING") ?? fallback).toEqual(fallback);
                expect(env<slice>("BP_ENV_OR_SLICE_PRESENT") ?? fallback).toEqual("present-slice-value");
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .env("BP_ENV_OR_SLICE_FALLBACK_SOURCE", "fallback-slice-value")
        .env("BP_ENV_OR_SLICE_PRESENT", "present-slice-value")
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/env_slice_returns_raw_and_empty_values_and_null_when_missing/env_or_slice_uses_fallback_for_missing_and_present_value_when_set.stdout.txt",
        );
}

#[test]
fn env_string_and_slice_support_long_values_without_truncation() {
    let long_value = "long-segment-".repeat(150);
    let source = format!(
        r#"
            import "../../lib/env"
            import "../../lib/testing/expect"

            get fun `test bp stdlib env string and slice long values`() {{
                val asString = env<string>("BP_ENV_LONG_VALUE");
                val asSlice = env<slice>("BP_ENV_LONG_VALUE");

                expect(asString).toBeNotNull();
                expect(asSlice).toBeNotNull();
                expect(asString!).toEqual("{long_value}");
                expect(asSlice!).toEqual("{long_value}");
            }}
        "#
    );

    ProjectBuilder::new("bp-stdlib-env-string-slice-long-values")
        .test_file("env_long_values", &source)
        .build()
        .acton()
        .test()
        .env("BP_ENV_LONG_VALUE", &long_value)
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/env_slice_returns_raw_and_empty_values_and_null_when_missing/env_string_and_slice_support_long_values_without_truncation.stdout.txt",
        );
}
