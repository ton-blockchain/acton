use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use tycho_types::boc::Boc;
use tycho_types::cell::CellBuilder;

const EH_VALID_RAW_ADDR: &str =
    "0:8356d05f87ec5141b349c5e1aa7f0c175c3abc18feb308a4d555391e92598147";

#[test]
fn env_or_address_falls_back_for_invalid_present_and_uses_valid_present_value() {
    ProjectBuilder::new("eh-stdlib-env-or-address-fallback-vs-valid-precedence")
        .test_file(
            "env_or_address",
            r#"
            import "../../lib/env"
            import "../../lib/testing/expect"

            get fun `test eh stdlib env or address fallback vs valid precedence`() {
                val fallbackAddress = address("0:1111111111111111111111111111111111111111111111111111111111111111");
                val expectedAddress = address("0:8356d05f87ec5141b349c5e1aa7f0c175c3abc18feb308a4d555391e92598147");

                val resolvedFromInvalidPresent = env<address>("EH_ENV_OR_ADDRESS_INVALID") ?? fallbackAddress;
                val resolvedFromValidPresent = env<address>("EH_ENV_OR_ADDRESS_VALID") ?? fallbackAddress;

                expect(resolvedFromInvalidPresent).toEqual(fallbackAddress);
                expect(resolvedFromValidPresent).toEqual(expectedAddress);
            }
            "#,
        )
        .build()
        .acton()
        .test()
        .env("EH_ENV_OR_ADDRESS_INVALID", "definitely-not-address")
        .env("EH_ENV_OR_ADDRESS_VALID", EH_VALID_RAW_ADDR)
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/env_or_address_falls_back_for_invalid_present_and_uses_valid_present_value/env_or_address_falls_back_for_invalid_present_and_uses_valid_present_value.stdout.txt",
        );
}

#[test]
fn env_or_cell_falls_back_for_invalid_present_and_uses_valid_present_value() {
    let mut valid_cell_builder = CellBuilder::new();
    valid_cell_builder.store_uint(0xBEEF, 16).ok();
    let valid_cell = valid_cell_builder.build().ok().unwrap_or_default();
    let valid_cell_b64 = Boc::encode_base64(valid_cell);

    ProjectBuilder::new("eh-stdlib-env-or-cell-fallback-vs-valid-precedence")
        .test_file(
            "env_or_cell",
            r#"
            import "../../lib/env"
            import "../../lib/testing/expect"

            get fun `test eh stdlib env or cell fallback vs valid precedence`() {
                val fallbackCell = beginCell().storeUint(0xCAFE, 16).endCell();

                val resolvedFromInvalidPresent = env<cell>("EH_ENV_OR_CELL_INVALID") ?? fallbackCell;
                var invalidSlice = resolvedFromInvalidPresent.beginParse();
                expect(invalidSlice.loadUint(16)).toEqual(0xCAFE);

                val resolvedFromValidPresent = env<cell>("EH_ENV_OR_CELL_VALID") ?? fallbackCell;
                var validSlice = resolvedFromValidPresent.beginParse();
                expect(validSlice.loadUint(16)).toEqual(0xBEEF);
            }
            "#,
        )
        .build()
        .acton()
        .test()
        .env("EH_ENV_OR_CELL_INVALID", "definitely-not-boc")
        .env("EH_ENV_OR_CELL_VALID", &valid_cell_b64)
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/env_or_address_falls_back_for_invalid_present_and_uses_valid_present_value/env_or_cell_falls_back_for_invalid_present_and_uses_valid_present_value.stdout.txt",
        );
}
