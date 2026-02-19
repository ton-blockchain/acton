//! Reserved integration test module for subagent EC.
//!
//! Ownership boundary for agent EC:
//! - tests/integration/test-runner/test_runner_stdlib_ec_expect_tests.rs
//! - tests/integration/snapshots/test-runner/test_runner_stdlib_ec_expect_tests/**
//! - tests/integration/testdata/test_std_agent_ec/**
//! - tests/support/test_std_agent_ec/** (optional)
//!
//! Required test name prefix:
//! - ec_stdlib_

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const EXPECT_IMPORTS: &str = r#"
import "../../lib/testing/expect"
"#;

fn run_ec_expect_success(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{EXPECT_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("expect_map_address_keys", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn expect_map_to_contain_key_supports_address_keys() {
    run_ec_expect_success(
        "ec-stdlib-map-to-contain-key-address-pass",
        r#"
get fun `test-ec-stdlib-map-to-contain-key-address-pass`() {
    val ownerRaw = address("0:8356d05f87ec5141b349c5e1aa7f0c175c3abc18feb308a4d555391e92598147");
    val outsider = address("0:00000000000000000000000000000000000000000000000000000000000000aa");

    var balances = createEmptyMap<address, int32>();
    balances.set(ownerRaw, 700);

    expect(balances).toContainKey(ownerRaw);
    expect(balances).toNotContainKey(outsider);
    expect(balances).toHaveLength(1);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_ec_expect_tests/ec_stdlib_expect_map_to_contain_key_supports_address_keys.stdout.txt",
    );
}

#[test]
fn expect_map_to_contain_key_supports_equivalent_friendly_address() {
    run_ec_expect_success(
        "ec-stdlib-map-to-contain-key-friendly-equivalent",
        r#"
get fun `test-ec-stdlib-map-to-contain-key-friendly-equivalent`() {
    val ownerRaw = address("0:8356d05f87ec5141b349c5e1aa7f0c175c3abc18feb308a4d555391e92598147");
    val ownerFriendly = address("EQCDVtBfh-xRQbNJxeGqfwwXXDq8GP6zCKTVVTkeklmBR6sT");

    var balances = createEmptyMap<address, int32>();
    balances.set(ownerRaw, 1);

    expect(ownerRaw).toEqual(ownerFriendly);
    expect(balances).toContainKey(ownerFriendly);
    expect(balances).toNotContainKey(address("0:00000000000000000000000000000000000000000000000000000000000000bb"));
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_ec_expect_tests/ec_stdlib_expect_map_to_contain_key_fails_for_equivalent_friendly_address_bug.stdout.txt",
    );
}
