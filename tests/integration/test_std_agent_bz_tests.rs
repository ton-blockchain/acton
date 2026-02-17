//! Reserved integration test module for subagent BZ.
//!
//! Ownership boundary for agent BZ:
//! - tests/integration/test_std_agent_bz_tests.rs
//! - tests/integration/snapshots/test_std_agent_bz/**
//! - tests/integration/testdata/test_std_agent_bz/**
//! - tests/support/test_std_agent_bz/** (optional)
//!
//! Required test name prefix:
//! - bz_stdlib_

use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

const BZ_EXPECT_IMPORTS: &str = r#"
import "../../lib/testing/expect"

fun bzExternalAddress(tag: uint32): any_address {
    return beginCell()
        .storeUint(0b01, 2)
        .storeUint(32, 9)
        .storeUint(tag, 32)
        .endCell()
        .beginParse()
        .loadAddressAny();
}
"#;

fn with_bz_source(test_body: &str) -> String {
    format!("{BZ_EXPECT_IMPORTS}\n{test_body}\n")
}

fn run_bz_success_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = with_bz_source(test_body);
    ProjectBuilder::new(project_name)
        .test_file("expect_address_behavior", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn bz_stdlib_expect_address_matchers_accept_internal_none_and_external_addresses() {
    run_bz_success_case(
        "bz-stdlib-expect-address-matchers-success",
        r#"
get fun `test-bz-stdlib-expect-address-matchers-success`() {
    val internalAddress = address("0:00000000000000000000000000000000000000000000000000000000000000AA")
        as any_address;
    val noneAddress = createAddressNone();
    val externalAddress = bzExternalAddress(0x10203040);

    expect(internalAddress).toBeInternalAddress();
    expect(noneAddress).toBeNoneAddress();
    expect(externalAddress).toBeExternalAddress();
}
"#,
        "integration/snapshots/test_std_agent_bz/bz_stdlib_expect_address_matchers_accept_internal_none_and_external_addresses.stdout.txt",
    );
}

#[test]
fn bz_stdlib_expect_address_matchers_report_assertion_failures_for_wrong_kinds() {
    let fixture = FixtureProject::load("basic");
    let test_path = "tests/bz_stdlib_expect_address_matchers_wrong_kind_failures.test.tolk";
    let source = with_bz_source(
        r#"
get fun `test-bz-stdlib-to-be-internal-address-fails-for-none`() {
    expectToEndWithExitCode(567);
    expect(createAddressNone()).toBeInternalAddress();
}

get fun `test-bz-stdlib-to-be-none-address-fails-for-internal`() {
    expectToEndWithExitCode(567);
    expect(address("0:00000000000000000000000000000000000000000000000000000000000000AA") as any_address)
        .toBeNoneAddress();
}

get fun `test-bz-stdlib-to-be-external-address-fails-for-internal`() {
    expectToEndWithExitCode(567);
    expect(address("0:00000000000000000000000000000000000000000000000000000000000000AA") as any_address)
        .toBeExternalAddress();
}
"#,
    );

    fs::write(fixture.path().join(test_path), source)
        .expect("failed to write BZ fixture expect address matcher failures test");

    fixture
        .acton()
        .test()
        .path(test_path)
        .run()
        .success()
        .assert_passed(3)
        .assert_snapshot_matches(
            "integration/snapshots/test_std_agent_bz/bz_stdlib_expect_address_matchers_report_assertion_failures_for_wrong_kinds.stdout.txt",
        );
}
