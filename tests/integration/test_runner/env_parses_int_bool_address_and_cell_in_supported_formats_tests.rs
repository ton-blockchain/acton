use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use tycho_types::boc::Boc;
use tycho_types::cell::CellBuilder;

const RAW_ADDR: &str = "0:8356d05f87ec5141b349c5e1aa7f0c175c3abc18feb308a4d555391e92598147";
const FRIENDLY_ZERO_ADDR: &str = "EQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAM9c";

#[test]
fn env_parses_int_bool_address_and_cell_in_supported_formats() {
    let mut builder = CellBuilder::new();
    builder.store_uint(0xBEEF, 16).ok();
    let cell = builder.build().ok().unwrap_or_default();
    let cell_hex = Boc::encode_hex(cell.clone());
    let cell_b64 = Boc::encode_base64(cell);

    ProjectBuilder::new("z-stdlib-env-supported-formats")
        .test_file(
            "env_supported",
            r#"
            import "../../lib/env"
            import "../../lib/testing/expect"

            get fun `test-z-stdlib-env-supported-formats`() {
                expect(env<int>("Z_ENV_INT_DEC")).toEqual(-17);
                expect(env<int>("Z_ENV_INT_HEX")).toEqual(26);

                expect(env<bool>("Z_ENV_BOOL_MIXED")).toEqual(true);
                expect(env<bool>("Z_ENV_BOOL_ONE")).toEqual(true);
                expect(env<bool>("Z_ENV_BOOL_ZERO")).toEqual(false);

                val fallbackAddress = address("EQBvDB_H7FFBs0nF4ap_DBdcOrwY_rMIpNVVOR6SWYFHByMJ");
                val parsedAddress = envOr<address>("Z_ENV_ADDRESS_RAW", fallbackAddress);
                expect(parsedAddress).toEqual(address("0:8356d05f87ec5141b349c5e1aa7f0c175c3abc18feb308a4d555391e92598147"));

                val fallbackCell = beginCell().storeUint(999, 16).endCell();

                val cellFromHex = envOr<cell>("Z_ENV_CELL_HEX", fallbackCell);
                var hexSlice = cellFromHex.beginParse();
                expect(hexSlice.loadUint(16)).toEqual(0xBEEF);

                val cellFromB64 = envOr<cell>("Z_ENV_CELL_B64", fallbackCell);
                var b64Slice = cellFromB64.beginParse();
                expect(b64Slice.loadUint(16)).toEqual(0xBEEF);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .env("Z_ENV_INT_DEC", "-17")
        .env("Z_ENV_INT_HEX", "0x1a")
        .env("Z_ENV_BOOL_MIXED", "TrUe")
        .env("Z_ENV_BOOL_ONE", "1")
        .env("Z_ENV_BOOL_ZERO", "0")
        .env("Z_ENV_ADDRESS_RAW", RAW_ADDR)
        .env("Z_ENV_CELL_HEX", &cell_hex)
        .env("Z_ENV_CELL_B64", &cell_b64)
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/env_parses_int_bool_address_and_cell_in_supported_formats/env_parses_int_bool_address_and_cell_in_supported_formats.stdout.txt",
        );
}

#[test]
fn env_returns_null_for_invalid_inputs_and_missing_values() {
    ProjectBuilder::new("z-stdlib-env-invalid-inputs")
        .test_file(
            "env_invalid",
            r#"
            import "../../lib/env"
            import "../../lib/testing/expect"

            get fun `test-z-stdlib-env-invalid-inputs`() {
                expect(env<int>("Z_BAD_INT")).toBeNull();
                expect(env<int>("Z_MISSING_INT")).toBeNull();

                expect(env<address>("Z_BAD_ADDRESS")).toBeNull();
                expect(env<cell>("Z_BAD_CELL")).toBeNull();

                expect(env<bool>("Z_BAD_BOOL")).toEqual(false);
                expect(env<bool>("Z_MISSING_BOOL")).toBeNull();
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .env("Z_BAD_INT", "0X1A")
        .env("Z_BAD_ADDRESS", "not-a-ton-address")
        .env("Z_BAD_CELL", "definitely-not-boc")
        .env("Z_BAD_BOOL", "true ")
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/env_parses_int_bool_address_and_cell_in_supported_formats/env_returns_null_for_invalid_inputs_and_missing_values.stdout.txt",
        );
}

#[test]
fn env_or_uses_defaults_only_for_null_paths() {
    ProjectBuilder::new("z-stdlib-env-or-defaults")
        .test_file(
            "env_or",
            r#"
            import "../../lib/env"
            import "../../lib/testing/expect"

            get fun `test-z-stdlib-env-or-defaults`() {
                expect(envOr<int>("Z_OR_MISSING_INT", 42)).toEqual(42);
                expect(envOr<int>("Z_OR_BAD_INT", 42)).toEqual(42);

                expect(envOr<bool>("Z_OR_MISSING_BOOL", true)).toEqual(true);
                expect(envOr<bool>("Z_OR_BAD_BOOL", true)).toEqual(false);

                val fallbackAddress = address("EQBvDB_H7FFBs0nF4ap_DBdcOrwY_rMIpNVVOR6SWYFHByMJ");
                expect(envOr<address>("Z_OR_MISSING_ADDRESS", fallbackAddress)).toEqual(fallbackAddress);
                expect(envOr<address>("Z_OR_BAD_ADDRESS", fallbackAddress)).toEqual(fallbackAddress);

                val fallbackCell = beginCell().storeUint(777, 16).endCell();
                val resolvedMissingCell = envOr<cell>("Z_OR_MISSING_CELL", fallbackCell);
                var missingSlice = resolvedMissingCell.beginParse();
                expect(missingSlice.loadUint(16)).toEqual(777);

                val resolvedCell = envOr<cell>("Z_OR_BAD_CELL", fallbackCell);
                var slice = resolvedCell.beginParse();
                expect(slice.loadUint(16)).toEqual(777);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .env("Z_OR_BAD_INT", "not-an-int")
        .env("Z_OR_BAD_BOOL", "definitely-not-bool")
        .env("Z_OR_BAD_ADDRESS", "bad-address")
        .env("Z_OR_BAD_CELL", "bad-cell")
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/env_parses_int_bool_address_and_cell_in_supported_formats/env_or_uses_defaults_only_for_null_paths.stdout.txt",
        );
}

#[test]
fn env_parses_user_friendly_address_form() {
    ProjectBuilder::new("z-stdlib-env-address-friendly-form")
        .test_file(
            "env_friendly_address",
            r#"
            import "../../lib/env"
            import "../../lib/fmt"
            import "../../lib/testing/expect"

            get fun `test-z-stdlib-env-address-friendly-form`() {
                val parsed = env<address>("Z_ENV_ADDRESS_FRIENDLY");
                expect(parsed).toBeNotNull();

                val renderedParsed = format1("{}", parsed!);
                val renderedExpected = format1("{}", address("0:0000000000000000000000000000000000000000000000000000000000000000"));
                expect(renderedParsed).toEqual(renderedExpected);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .env("Z_ENV_ADDRESS_FRIENDLY", FRIENDLY_ZERO_ADDR)
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/env_parses_int_bool_address_and_cell_in_supported_formats/env_parses_user_friendly_address_form.stdout.txt",
        );
}
