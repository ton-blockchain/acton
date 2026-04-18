import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/testing/expect"

get fun `test get shard account`() {
    val deployer = testing.treasury("deployer");
    val shard = testing.getShardAccount(deployer.address);
    expect(shard != null).toBeTrue();
}

get fun `test set and get shard account`() {
    val deployer = testing.treasury("deployer");
    val shard = testing.getShardAccount(deployer.address);
    expect(shard != null).toBeTrue();

    val lt = shard!.lastTransLt;

    // Copy shard account to a different address and read it back
    val addr = address("0:0000000000000000000000000000000000000000000000000000000000000001");
    testing.setShardAccount(addr, shard);

    val restored = testing.getShardAccount(addr);
    expect(restored != null).toBeTrue();
    expect(restored!.lastTransLt as int).toEqual(lt as int);
}

get fun `test reset shard account`() {
    val deployer = testing.treasury("deployer");
    val before = testing.getShardAccount(deployer.address);
    expect(before != null).toBeTrue();
    expect((before!.lastTransLt as int) > 0).toBeTrue();

    testing.setShardAccount(deployer.address, null);

    // After reset, shard account should have zero lastTransLt and lastTransHash
    val after = testing.getShardAccount(deployer.address);
    expect(after != null).toBeTrue();
    expect(after!.lastTransLt as int).toEqual(0);
    expect(after!.lastTransHash as int).toEqual(0);
}
