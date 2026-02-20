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
fn config_storage_prices_get_initial_prefers_zero_key_when_present() {
    run_config_success_case(
        "dp-stdlib-config-storage-prices-get-initial-prefers-zero-key",
        r#"
get fun `test-dp-stdlib-config-storage-prices-get-initial-prefers-zero-key`() {
    var prices = createEmptyMap<uint32, StoragePrices>();
    prices.set(0, StoragePrices {
        initialUnixTime: 0,
        bitPrice: 101,
        cellPrice: 202,
        masterchainBitPrice: 303,
        masterchainCellPrice: 404,
    });
    prices.set(10, StoragePrices {
        initialUnixTime: 10,
        bitPrice: 11,
        cellPrice: 12,
        masterchainBitPrice: 13,
        masterchainCellPrice: 14,
    });

    val initial = prices.getInitial();
    expect(initial.initialUnixTime).toEqual(0);
    expect(initial.bitPrice).toEqual(101);
    expect(initial.cellPrice).toEqual(202);
    expect(initial.masterchainBitPrice).toEqual(303);
    expect(initial.masterchainCellPrice).toEqual(404);
}
"#,
        "integration/snapshots/test-runner/config_storage_prices_get_initial_empty_dict_falls_back_to_zero_prices/config_storage_prices_get_initial_prefers_zero_key_when_present.stdout.txt",
    );
}
