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
fn config_storage_prices_multi_entry_roundtrip_keeps_initial_and_future_entries() {
    run_config_success_case(
        "bc-stdlib-config-storage-prices-multi-entry-roundtrip",
        r#"
get fun `test-bc-stdlib-config-storage-prices-multi-entry-roundtrip`() {
    val extraTs: uint32 = 1000000;
    var config = net.getConfig();

    var prices = createEmptyMap<uint32, StoragePrices>();
    prices.setInitial(StoragePrices {
        initialUnixTime: 0,
        bitPrice: 11,
        cellPrice: 22,
        masterchainBitPrice: 33,
        masterchainCellPrice: 44
    });
    prices.set(extraTs, StoragePrices {
        initialUnixTime: extraTs,
        bitPrice: 55,
        cellPrice: 66,
        masterchainBitPrice: 77,
        masterchainCellPrice: 88
    });

    config.setStoragePrices(prices);
    expect(net.setConfig(config)).toBeTrue();

    val updated = net.getConfig().getStoragePrices();
    expect(updated).toHaveLength(2);
    expect(updated).toContainKey(0);
    expect(updated).toContainKey(extraTs);

    val initial = updated.getInitial();
    val future = updated.get(extraTs).loadValue();

    expect(initial.initialUnixTime).toEqual(0);
    expect(initial.bitPrice).toEqual(11);
    expect(initial.cellPrice).toEqual(22);
    expect(initial.masterchainBitPrice).toEqual(33);
    expect(initial.masterchainCellPrice).toEqual(44);

    expect(future.initialUnixTime).toEqual(extraTs);
    expect(future.bitPrice).toEqual(55);
    expect(future.cellPrice).toEqual(66);
    expect(future.masterchainBitPrice).toEqual(77);
    expect(future.masterchainCellPrice).toEqual(88);
}
"#,
        "integration/snapshots/test-runner/config_storage_prices_multi_entry_roundtrip_keeps_initial_and_future_entries/config_storage_prices_multi_entry_roundtrip_keeps_initial_and_future_entries.stdout.txt",
    );
}

#[test]
fn config_storage_prices_set_initial_overwrites_index_zero_only() {
    run_config_success_case(
        "bc-stdlib-config-storage-prices-set-initial-overwrites-index-zero",
        r#"
get fun `test-bc-stdlib-config-storage-prices-set-initial-overwrites-index-zero`() {
    val extraTs: uint32 = 2000000;
    var config = net.getConfig();

    var prices = createEmptyMap<uint32, StoragePrices>();
    prices.setInitial(StoragePrices {
        initialUnixTime: 0,
        bitPrice: 101,
        cellPrice: 202,
        masterchainBitPrice: 303,
        masterchainCellPrice: 404
    });
    prices.set(extraTs, StoragePrices {
        initialUnixTime: extraTs,
        bitPrice: 505,
        cellPrice: 606,
        masterchainBitPrice: 707,
        masterchainCellPrice: 808
    });

    prices.setInitial(StoragePrices {
        initialUnixTime: 0,
        bitPrice: 909,
        cellPrice: 1001,
        masterchainBitPrice: 1102,
        masterchainCellPrice: 1203
    });

    config.setStoragePrices(prices);
    expect(net.setConfig(config)).toBeTrue();

    val updated = net.getConfig().getStoragePrices();
    expect(updated).toHaveLength(2);
    expect(updated).toContainKey(0);
    expect(updated).toContainKey(extraTs);

    val initial = updated.getInitial();
    val future = updated.get(extraTs).loadValue();

    expect(initial.bitPrice).toEqual(909);
    expect(initial.cellPrice).toEqual(1001);
    expect(initial.masterchainBitPrice).toEqual(1102);
    expect(initial.masterchainCellPrice).toEqual(1203);
    expect(initial.bitPrice).toNotEqual(101);
    expect(initial.cellPrice).toNotEqual(202);

    expect(future.initialUnixTime).toEqual(extraTs);
    expect(future.bitPrice).toEqual(505);
    expect(future.cellPrice).toEqual(606);
    expect(future.masterchainBitPrice).toEqual(707);
    expect(future.masterchainCellPrice).toEqual(808);
}
"#,
        "integration/snapshots/test-runner/config_storage_prices_multi_entry_roundtrip_keeps_initial_and_future_entries/config_storage_prices_set_initial_overwrites_index_zero_only.stdout.txt",
    );
}
