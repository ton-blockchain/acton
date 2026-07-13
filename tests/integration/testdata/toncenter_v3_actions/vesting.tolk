import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/scripts"
import "../../lib/io"
import "../types/wallet_vesting.types"
import "../wrappers/WalletVesting.gen"

const WHITELISTED = "0:3333333333333333333333333333333333333333333333333333333333333333"
const OWNER = "0:4444444444444444444444444444444444444444444444444444444444444444"

fun waitForTransaction(result: SendResultList) {
    if (result.waitForFirstTransaction() == null) {
        throw 1000;
    }
}

fun main() {
    val deployer = scripts.wallet("deployer");
    val owner = address(OWNER);
    val whitelisted = address(WHITELISTED);
    val vestingParameters = WalletVestingParameters {
        vestingStartTime: 1700000000,
        vestingTotalDuration: 31536000,
        unlockPeriod: 2592000,
        cliffDuration: 5184000,
        vestingTotalAmount: ton("10"),
        vestingSenderAddress: deployer.address,
        ownerAddress: owner,
    }.toCell();
    val vesting = WalletVesting.fromStorage({
        seqno: 0,
        subwalletId: 123,
        publicKey: 0,
        whitelist: [],
        vesting: vestingParameters,
    });
    val vestingSecond = WalletVesting.fromStorage({
        seqno: 0,
        subwalletId: 124,
        publicKey: 0,
        whitelist: [],
        vesting: vestingParameters,
    });
    waitForTransaction(vesting.deploy(deployer.address, { value: ton("11"), bounce: false }));
    waitForTransaction(vestingSecond.deploy(deployer.address, { value: ton("11"), bounce: false }));
    waitForTransaction(vesting.sendWalletVestingAddWhitelist(
        deployer.address,
        400,
        beginCell().storeAddress(whitelisted).endCell().beginParse(),
        { value: ton("0.2"), bounce: false },
    ));

    println("OWNER={}", owner);
    println("SENDER={}", deployer.address);
    println("WHITELISTED={}", whitelisted);
    println("VESTING={}", vesting.address);
    println("VESTING_SECOND={}", vestingSecond.address);
}
