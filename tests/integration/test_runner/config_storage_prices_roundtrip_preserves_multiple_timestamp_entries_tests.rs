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
        .test_file("dr_config_storage_prices", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn config_storage_prices_roundtrip_preserves_multiple_timestamp_entries() {
    run_config_success_case(
        "dr-stdlib-config-storage-prices-multi-entry-roundtrip",
        r"
get fun `test dr stdlib config storage prices multi entry roundtrip`() {
    val tsA: uint32 = 1700000000;
    val tsB: uint32 = 1700003600;

    var prices = createEmptyMap<uint32, StoragePrices>();
    prices.set(0, StoragePrices {
        initialUnixTime: 0,
        bitPrice: 11,
        cellPrice: 22,
        masterchainBitPrice: 33,
        masterchainCellPrice: 44
    });
    prices.set(tsA, StoragePrices {
        initialUnixTime: tsA,
        bitPrice: 101,
        cellPrice: 202,
        masterchainBitPrice: 303,
        masterchainCellPrice: 404
    });
    prices.set(tsB, StoragePrices {
        initialUnixTime: tsB,
        bitPrice: 505,
        cellPrice: 606,
        masterchainBitPrice: 707,
        masterchainCellPrice: 808
    });

    var config = testing.getConfig();
    config.setStoragePrices(prices);
    expect(testing.setConfig(config)).toBeTrue();

    val roundtrip = testing.getConfig().getStoragePrices();
    expect(roundtrip).toHaveLength(3);
    expect(roundtrip).toContainKey(0);
    expect(roundtrip).toContainKey(tsA);
    expect(roundtrip).toContainKey(tsB);

    val initial = roundtrip.getInitial();
    val entryA = roundtrip.get(tsA).loadValue();
    val entryB = roundtrip.get(tsB).loadValue();

    expect(initial.initialUnixTime).toEqual(0);
    expect(initial.bitPrice).toEqual(11);
    expect(initial.cellPrice).toEqual(22);
    expect(initial.masterchainBitPrice).toEqual(33);
    expect(initial.masterchainCellPrice).toEqual(44);

    expect(entryA.initialUnixTime).toEqual(tsA);
    expect(entryA.bitPrice).toEqual(101);
    expect(entryA.cellPrice).toEqual(202);
    expect(entryA.masterchainBitPrice).toEqual(303);
    expect(entryA.masterchainCellPrice).toEqual(404);

    expect(entryB.initialUnixTime).toEqual(tsB);
    expect(entryB.bitPrice).toEqual(505);
    expect(entryB.cellPrice).toEqual(606);
    expect(entryB.masterchainBitPrice).toEqual(707);
    expect(entryB.masterchainCellPrice).toEqual(808);
}
",
        "integration/snapshots/test-runner/config_storage_prices_roundtrip_preserves_multiple_timestamp_entries/config_storage_prices_roundtrip_preserves_multiple_timestamp_entries.stdout.txt",
    );
}

#[test]
fn config_storage_prices_roundtrip_replaces_old_dictionary_entries_on_second_write() {
    run_config_success_case(
        "dr-stdlib-config-storage-prices-second-write-replacement",
        r"
get fun `test dr stdlib config storage prices second write replacement`() {
    val oldTsA: uint32 = 1800000000;
    val oldTsB: uint32 = 1800003600;
    val newTs: uint32 = 1900000000;

    var first = createEmptyMap<uint32, StoragePrices>();
    first.set(0, StoragePrices {
        initialUnixTime: 0,
        bitPrice: 9,
        cellPrice: 19,
        masterchainBitPrice: 29,
        masterchainCellPrice: 39
    });
    first.set(oldTsA, StoragePrices {
        initialUnixTime: oldTsA,
        bitPrice: 49,
        cellPrice: 59,
        masterchainBitPrice: 69,
        masterchainCellPrice: 79
    });
    first.set(oldTsB, StoragePrices {
        initialUnixTime: oldTsB,
        bitPrice: 89,
        cellPrice: 99,
        masterchainBitPrice: 109,
        masterchainCellPrice: 119
    });

    var config = testing.getConfig();
    config.setStoragePrices(first);
    expect(testing.setConfig(config)).toBeTrue();

    var second = map<uint32, StoragePrices> [];
    second.set(0, StoragePrices {
        initialUnixTime: 0,
        bitPrice: 901,
        cellPrice: 902,
        masterchainBitPrice: 903,
        masterchainCellPrice: 904
    });
    second.set(newTs, StoragePrices {
        initialUnixTime: newTs,
        bitPrice: 1001,
        cellPrice: 1002,
        masterchainBitPrice: 1003,
        masterchainCellPrice: 1004
    });

    var rewritten = testing.getConfig();
    rewritten.setStoragePrices(second);
    expect(testing.setConfig(rewritten)).toBeTrue();

    val afterSecondWrite = testing.getConfig().getStoragePrices();
    expect(afterSecondWrite).toHaveLength(2);
    expect(afterSecondWrite).toContainKey(0);
    expect(afterSecondWrite).toContainKey(newTs);
    expect(afterSecondWrite).toNotContainKey(oldTsA);
    expect(afterSecondWrite).toNotContainKey(oldTsB);

    val initial = afterSecondWrite.getInitial();
    val replacement = afterSecondWrite.get(newTs).loadValue();

    expect(initial.bitPrice).toEqual(901);
    expect(initial.cellPrice).toEqual(902);
    expect(initial.masterchainBitPrice).toEqual(903);
    expect(initial.masterchainCellPrice).toEqual(904);

    expect(replacement.initialUnixTime).toEqual(newTs);
    expect(replacement.bitPrice).toEqual(1001);
    expect(replacement.cellPrice).toEqual(1002);
    expect(replacement.masterchainBitPrice).toEqual(1003);
    expect(replacement.masterchainCellPrice).toEqual(1004);
}
",
        "integration/snapshots/test-runner/config_storage_prices_roundtrip_preserves_multiple_timestamp_entries/config_storage_prices_roundtrip_replaces_old_dictionary_entries_on_second_write.stdout.txt",
    );
}
