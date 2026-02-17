//! Reserved integration test module for subagent DP.
//!
//! Ownership boundary for agent DP:
//! - tests/integration/test_std_agent_dp_tests.rs
//! - tests/integration/snapshots/test_std_agent_dp/**
//! - tests/integration/testdata/test_std_agent_dp/**
//! - tests/support/test_std_agent_dp/** (optional)
//!
//! Required test name prefix:
//! - dp_stdlib_

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const CONFIG_IMPORTS: &str = r#"
import "../../lib/emulation/config"
import "../../lib/testing/expect"
"#;

fn run_config_success_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{CONFIG_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("config_behavior", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn dp_stdlib_config_storage_prices_get_initial_empty_dict_falls_back_to_zero_prices() {
    run_config_success_case(
        "dp-stdlib-config-storage-prices-get-initial-empty-dict-fallback",
        r#"
get fun `test-dp-stdlib-config-storage-prices-get-initial-empty-dict-fallback`() {
    val prices = createEmptyMap<uint32, StoragePrices>();
    // BUG: StoragePricesDict.getInitial should return zeroed StoragePrices for empty dict, expected zeros, got VM exit_code=7 ("not a cell slice").
    val initial = prices.getInitial();

    expect(initial.initialUnixTime).toEqual(0);
    expect(initial.bitPrice).toEqual(0);
    expect(initial.cellPrice).toEqual(0);
    expect(initial.masterchainBitPrice).toEqual(0);
    expect(initial.masterchainCellPrice).toEqual(0);
}
"#,
        "integration/snapshots/test_std_agent_dp/dp_stdlib_config_storage_prices_get_initial_empty_dict_falls_back_to_zero_prices.stdout.txt",
    );
}
