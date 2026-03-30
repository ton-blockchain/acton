use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

#[test]
fn fmt_supports_mixed_hex_ton_and_plain_placeholders() {
    ProjectBuilder::new("t-lib-api-fmt-mixed-placeholders")
        .test_file(
            "fmt_env",
            r#"
            import "../../lib/fmt"
            import "../../lib/testing/expect"

            get fun `test-fmt-mixed-placeholders`() {
                val rendered = format3("hex={:x} ton={:ton} label={}", 255, 1500000000, "ok");
                expect(rendered).toEqual("hex=ff ton=1.5 TON label=ok");
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/api_fmt_env/fmt_supports_mixed_hex_ton_and_plain_placeholders.stdout.txt",
        );
}

#[test]
fn fmt_plain_and_hex_placeholders_should_follow_argument_order_bug() {
    ProjectBuilder::new("t-lib-api-fmt-placeholder-order-bug")
        .test_file(
            "fmt_env",
            r#"
            import "../../lib/fmt"
            import "../../lib/testing/expect"

            get fun `test-fmt-placeholder-order`() {
                val rendered = format2("{} {:x}", 255, 16);
                expect(rendered).toEqual("255 10");
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/api_fmt_env/fmt_plain_and_hex_placeholders_should_follow_argument_order_bug.stdout.txt",
        );
}

#[test]
fn fmt_fallback_for_non_int_specs_and_ignores_extra_args() {
    ProjectBuilder::new("t-lib-api-fmt-fallback-and-extra-args")
        .test_file(
            "fmt_env",
            r#"
            import "../../lib/fmt"
            import "../../lib/testing/expect"

            get fun `test-fmt-fallback-and-extra-args`() {
                val fallback = format2("{:x} {:ton}", "abc", "tonlike");
                expect(fallback).toEqual("abc tonlike");

                val extra = format2("{}", 255, 16);
                expect(extra).toEqual("255");
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/api_fmt_env/fmt_fallback_for_non_int_specs_and_ignores_extra_args.stdout.txt",
        );
}

#[test]
fn env_bool_parsing_handles_case_numeric_falsey_and_missing() {
    ProjectBuilder::new("t-lib-api-env-bool-edge-cases")
        .test_file(
            "fmt_env",
            r#"
            import "../../lib/env"
            import "../../lib/testing/expect"

            get fun `test-env-bool-edge-cases`() {
                expect(env<bool>("T_BOOL_TRUE_MIXED")).toEqual(true);
                expect(env<bool>("T_BOOL_ONE")).toEqual(true);
                expect(env<bool>("T_BOOL_FALSE_WORD")).toEqual(false);
                expect(env<bool>("T_BOOL_GARBAGE")).toEqual(false);
                expect(env<bool>("T_BOOL_MISSING")).toBeNull();
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .env("T_BOOL_TRUE_MIXED", "TrUe")
        .env("T_BOOL_ONE", "1")
        .env("T_BOOL_FALSE_WORD", "FALSE")
        .env("T_BOOL_GARBAGE", "definitely-not-bool")
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/api_fmt_env/env_bool_parsing_handles_case_numeric_falsey_and_missing.stdout.txt",
        );
}

#[test]
fn env_or_uses_defaults_for_invalid_int_address_and_cell_values() {
    ProjectBuilder::new("t-lib-api-env-or-invalid-fallbacks")
        .test_file(
            "fmt_env",
            r#"
            import "../../lib/env"
            import "../../lib/testing/expect"

            get fun `test-env-or-invalid-fallbacks`() {
                val fallbackAddress = address("EQBvDB_H7FFBs0nF4ap_DBdcOrwY_rMIpNVVOR6SWYFHByMJ");
                val fallbackCell = beginCell().storeUint(777, 32).endCell();

                expect(env<int>("T_BAD_INT")).toBeNull();
                expect(env<address>("T_BAD_ADDRESS")).toBeNull();
                expect(env<cell>("T_BAD_CELL")).toBeNull();

                expect(envOr<int>("T_BAD_INT", 42)).toEqual(42);
                expect(envOr<address>("T_BAD_ADDRESS", fallbackAddress)).toEqual(fallbackAddress);

                val resolvedCell = envOr<cell>("T_BAD_CELL", fallbackCell);
                var slice = resolvedCell.beginParse();
                expect(slice.loadUint(32)).toEqual(777);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .env("T_BAD_INT", "not-a-number")
        .env("T_BAD_ADDRESS", "definitely-not-an-address")
        .env("T_BAD_CELL", "not-a-boc")
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/api_fmt_env/env_or_uses_defaults_for_invalid_int_address_and_cell_values.stdout.txt",
        );
}

#[test]
fn env_rejects_unsupported_target_types_with_clear_error() {
    ProjectBuilder::new("t-lib-api-env-unsupported-type")
        .test_file(
            "fmt_env",
            r#"
            import "../../lib/env"

            struct Unsupported {
                value: int,
            }

            get fun `test-env-unsupported-type`() {
                env<Unsupported>("T_ENV_UNSUPPORTED");
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
            "env() supports only int, coins, bool, string, slice, address and cell types, but got Unsupported",
        )
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/api_fmt_env/env_rejects_unsupported_target_types_with_clear_error.stdout.txt",
        );
}
