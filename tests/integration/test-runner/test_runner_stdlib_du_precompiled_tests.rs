//! Reserved integration test module for subagent DU.
//!
//! Ownership boundary for agent DU:
//! - tests/integration/test-runner/test_runner_stdlib_du_precompiled_tests.rs
//! - tests/integration/snapshots/test-runner/test_runner_stdlib_du_precompiled_tests/**
//! - tests/integration/testdata/test_std_agent_du/** (optional)
//! - tests/support/test_std_agent_du/** (optional)
//!
//! Required test name prefix:
//! - du_stdlib_

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const CONFIG_IMPORTS: &str = r#"
import "../../lib/emulation/config"
import "../../lib/emulation/network"
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
fn precompiled_add_contract_gas_duplicate_hash_does_not_overwrite_original_entry() {
    run_config_success_case(
        "du-stdlib-precompiled-duplicate-hash-no-overwrite",
        r#"
get fun `test-du-stdlib-precompiled-duplicate-hash-no-overwrite`() {
    var precompiled = PrecompiledContractsConfig {
        list: createEmptyMap<uint256, PrecompiledSmartContract>(),
    };

    val duplicateHash: uint256 = 0xD00D;
    val otherHash: uint256 = 0xBEEF;

    expect(precompiled.addContractGas(duplicateHash, 111)).toBeTrue();
    expect(precompiled.addContractGas(duplicateHash, 999)).toBeFalse();
    expect(precompiled.addContractGas(otherHash, 333)).toBeTrue();

    val localDuplicate = precompiled.list.get(duplicateHash).loadValue();
    val localOther = precompiled.list.get(otherHash).loadValue();
    expect(localDuplicate.gasUsage).toEqual(111);
    expect(localDuplicate.gasUsage).toNotEqual(999);
    expect(localOther.gasUsage).toEqual(333);

    var config = net.getConfig();
    config.setPrecompiledContractsConfig(precompiled);
    expect(net.setConfig(config)).toBeTrue();

    val persisted = net.getConfig().getPrecompiledContractsConfig();
    expect(persisted.list).toHaveLength(2);

    val persistedDuplicate = persisted.list.get(duplicateHash).loadValue();
    val persistedOther = persisted.list.get(otherHash).loadValue();
    expect(persistedDuplicate.gasUsage).toEqual(111);
    expect(persistedDuplicate.gasUsage).toNotEqual(999);
    expect(persistedOther.gasUsage).toEqual(333);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_du_precompiled_tests/du_stdlib_precompiled_add_contract_gas_duplicate_hash_does_not_overwrite_original_entry.stdout.txt",
    );
}
