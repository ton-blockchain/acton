use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

#[test]
fn treasury_workchain_controls_address_deployment_and_transfers() {
    ProjectBuilder::new("treasury-workchains")
        .test_file(
            "treasury_workchains",
            r#"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/io"
import "../../lib/testing/expect"
import "../../lib/types/big_array"

get fun `test treasury addresses and initial funding`() {
    val base = testing.treasury("deployer");
    val baseBalance = testing.getAccountBalance(base.address);
    val explicitBase = testing.treasury("deployer", BASECHAIN);
    val master = testing.treasury("deployer", MASTERCHAIN);
    val masterBalance = testing.getAccountBalance(master.address);
    val (baseWorkchain, baseHash) = base.address.getWorkchainAndHash();
    val (masterWorkchain, masterHash) = master.address.getWorkchainAndHash();

    // Keep the established address for existing one-argument calls.
    expect(base.address).toEqual(address("0:6f0c1fc7ec5141b349c5e1aa7f0c175c3abc18feb308a4d555391e9259814707"));
    expect(explicitBase.address).toEqual(base.address);
    expect(baseWorkchain).toEqual(BASECHAIN);
    expect(masterWorkchain).toEqual(MASTERCHAIN);
    expect(master.address).toNotEqual(base.address);
    expect(masterHash).toEqual(baseHash);
    expect(testing.isDeployed(base.address)).toBeTrue();
    expect(testing.isDeployed(master.address)).toBeTrue();
    expect(baseBalance > ton("9999")).toBeTrue();
    expect(masterBalance > ton("9999")).toBeTrue();
    println("basechain: workchain={}, hash={:x}, balance={}", baseWorkchain, baseHash, baseBalance);
    println("masterchain: workchain={}, hash={:x}, balance={}", masterWorkchain, masterHash, masterBalance);
}

get fun `test repeated treasury calls fund the same address in each workchain`() {
    val base = testing.treasury("owner", BASECHAIN);
    val master = testing.treasury("owner", MASTERCHAIN);
    val baseBefore = testing.getAccountBalance(base.address);
    val masterBefore = testing.getAccountBalance(master.address);

    expect(testing.treasury("owner", BASECHAIN).address).toEqual(base.address);
    expect(testing.getAccountBalance(master.address)).toEqual(masterBefore);
    val baseAfter = testing.getAccountBalance(base.address);
    expect(testing.treasury("owner", MASTERCHAIN).address).toEqual(master.address);
    expect(testing.getAccountBalance(base.address)).toEqual(baseAfter);
    val masterAfter = testing.getAccountBalance(master.address);
    expect(baseAfter > baseBefore).toBeTrue();
    expect(masterAfter > masterBefore).toBeTrue();
    println("basechain: before={}, after={}", baseBefore, baseAfter);
    println("masterchain: before={}, after={}", masterBefore, masterAfter);
}

get fun `test treasury transfers between basechain and masterchain`() {
    val base = testing.treasury("sender", BASECHAIN);
    val master = testing.treasury("recipient", MASTERCHAIN);
    val masterBefore = testing.getAccountBalance(master.address);
    val toMaster = net.send(base.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: master.address,
    }));
    expect(toMaster).toHaveAllSuccessfulTxs();
    expect(toMaster).toHaveSuccessfulTx({ from: base.address, to: master.address });
    expect(testing.getAccountBalance(master.address) > masterBefore).toBeTrue();
    println("0 -> -1: transactions={}, balance={} -> {}", toMaster.size(), masterBefore, testing.getAccountBalance(master.address));

    val baseBefore = testing.getAccountBalance(base.address);
    val toBase = net.send(master.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: base.address,
    }));
    expect(toBase).toHaveAllSuccessfulTxs();
    expect(toBase).toHaveSuccessfulTx({ from: master.address, to: base.address });
    expect(testing.getAccountBalance(base.address) > baseBefore).toBeTrue();
    println("-1 -> 0: transactions={}, balance={} -> {}", toBase.size(), baseBefore, testing.getAccountBalance(base.address));
}
"#,
        )
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/treasury_workchains.stdout.txt",
        );
}
