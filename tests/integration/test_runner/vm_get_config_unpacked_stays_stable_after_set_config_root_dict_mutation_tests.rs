use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const DK_VM_IMPORTS: &str = r#"
import "../../lib/emulation/config"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
import "../../lib/vm/vm"
"#;

fn run_success_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{DK_VM_IMPORTS}\n{test_body}\n");

    ProjectBuilder::new(project_name)
        .test_file("dk_vm_config_unpacked", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn vm_get_config_unpacked_stays_stable_after_set_config_root_dict_mutation() {
    run_success_case(
        "dk-stdlib-vm-config-unpacked-after-root-dict-mutation",
        r#"
get fun `test-dk-stdlib-vm-config-unpacked-after-root-dict-mutation`() {
    val unpackedBefore = vm.getConfigUnpacked();
    val beforeStorage = StoragePrices.fromSlice(unpackedBefore.get(0) as slice);

    var config = net.getConfig();
    var storagePrices = config.getStoragePrices();
    var updatedStorage = storagePrices.getInitial();

    updatedStorage.bitPrice += 777;
    updatedStorage.cellPrice += 111;
    updatedStorage.masterchainBitPrice += 222;
    updatedStorage.masterchainCellPrice += 333;
    storagePrices.setInitial(updatedStorage);
    config.setStoragePrices(storagePrices);

    vm.setConfigRootDict(config.toLowLevelDict());

    val c7 = vm.getC7();
    val params = c7.get(0) as tuple;
    val rootConfig = params.get(9) as Config;
    val rootStorage = rootConfig.getStoragePrices().getInitial();

    expect(rootStorage.bitPrice).toEqual(updatedStorage.bitPrice);
    expect(rootStorage.cellPrice).toEqual(updatedStorage.cellPrice);
    expect(rootStorage.masterchainBitPrice).toEqual(updatedStorage.masterchainBitPrice);
    expect(rootStorage.masterchainCellPrice).toEqual(updatedStorage.masterchainCellPrice);

    val unpackedAfter = vm.getConfigUnpacked();
    val afterStorage = StoragePrices.fromSlice(unpackedAfter.get(0) as slice);

    expect(unpackedAfter.size()).toEqual(unpackedBefore.size());
    expect(afterStorage.bitPrice).toEqual(beforeStorage.bitPrice);
    expect(afterStorage.cellPrice).toEqual(beforeStorage.cellPrice);
    expect(afterStorage.masterchainBitPrice).toEqual(beforeStorage.masterchainBitPrice);
    expect(afterStorage.masterchainCellPrice).toEqual(beforeStorage.masterchainCellPrice);

    expect(afterStorage.bitPrice).toNotEqual(rootStorage.bitPrice);
    expect(afterStorage.cellPrice).toNotEqual(rootStorage.cellPrice);
    expect(afterStorage.masterchainBitPrice).toNotEqual(rootStorage.masterchainBitPrice);
    expect(afterStorage.masterchainCellPrice).toNotEqual(rootStorage.masterchainCellPrice);
}
"#,
        "integration/snapshots/test-runner/vm_get_config_unpacked_stays_stable_after_set_config_root_dict_mutation/vm_get_config_unpacked_stays_stable_after_set_config_root_dict_mutation.stdout.txt",
    );
}
