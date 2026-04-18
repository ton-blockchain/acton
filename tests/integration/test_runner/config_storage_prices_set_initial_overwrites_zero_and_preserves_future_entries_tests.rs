use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const CONFIG_IMPORTS: &str = r#"
import "../../lib/emulation/config"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
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
fn config_storage_prices_set_initial_overwrites_zero_and_preserves_future_entries() {
    run_config_success_case(
        "dq-stdlib-config-storage-prices-set-initial-overwrite-preserves-future",
        r"
get fun `test dq stdlib config storage prices set initial overwrite preserves future`() {
    val futureTsA: uint32 = 1000000;
    val futureTsB: uint32 = 2000000;
    var config = testing.getConfig();

    var prices = createEmptyMap<uint32, StoragePrices>();
    prices.setInitial(StoragePrices {
        initialUnixTime: 0,
        bitPrice: 11,
        cellPrice: 22,
        masterchainBitPrice: 33,
        masterchainCellPrice: 44
    });
    prices.set(futureTsA, StoragePrices {
        initialUnixTime: futureTsA,
        bitPrice: 55,
        cellPrice: 66,
        masterchainBitPrice: 77,
        masterchainCellPrice: 88
    });
    prices.set(futureTsB, StoragePrices {
        initialUnixTime: futureTsB,
        bitPrice: 99,
        cellPrice: 111,
        masterchainBitPrice: 122,
        masterchainCellPrice: 133
    });

    prices.setInitial(StoragePrices {
        initialUnixTime: 0,
        bitPrice: 901,
        cellPrice: 902,
        masterchainBitPrice: 903,
        masterchainCellPrice: 904
    });

    config.setStoragePrices(prices);
    expect(testing.setConfig(config)).toBeTrue();

    val updated = testing.getConfig().getStoragePrices();
    expect(updated).toHaveLength(3);
    expect(updated).toContainKey(0);
    expect(updated).toContainKey(futureTsA);
    expect(updated).toContainKey(futureTsB);

    val initial = updated.getInitial();
    val futureA = updated.get(futureTsA).loadValue();
    val futureB = updated.get(futureTsB).loadValue();

    expect(initial.initialUnixTime).toEqual(0);
    expect(initial.bitPrice).toEqual(901);
    expect(initial.cellPrice).toEqual(902);
    expect(initial.masterchainBitPrice).toEqual(903);
    expect(initial.masterchainCellPrice).toEqual(904);
    expect(initial.bitPrice).toNotEqual(11);
    expect(initial.cellPrice).toNotEqual(22);

    expect(futureA.initialUnixTime).toEqual(futureTsA);
    expect(futureA.bitPrice).toEqual(55);
    expect(futureA.cellPrice).toEqual(66);
    expect(futureA.masterchainBitPrice).toEqual(77);
    expect(futureA.masterchainCellPrice).toEqual(88);

    expect(futureB.initialUnixTime).toEqual(futureTsB);
    expect(futureB.bitPrice).toEqual(99);
    expect(futureB.cellPrice).toEqual(111);
    expect(futureB.masterchainBitPrice).toEqual(122);
    expect(futureB.masterchainCellPrice).toEqual(133);
}
",
        "integration/snapshots/test-runner/config_storage_prices_set_initial_overwrites_zero_and_preserves_future_entries/config_storage_prices_set_initial_overwrites_zero_and_preserves_future_entries.stdout.txt",
    );
}
