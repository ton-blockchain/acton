import "@acton/env"
import "@acton/io"
import "@acton/emulation/network"
import "@wrappers/SimpleExtension.gen"
import "utils/common"

const DEFAULT_SUB_AMOUNT = grams("0.27")
const DEFAULT_PAYMENT_INTERVAL = 3600

fun main() {
    val deployer = promptW5Wallet("W5_DEPLOYER", "Select Wallet V5 Deployer");

    val extension = SimpleExtension.fromStorage({
        walletAddress: deployer.address,
        admin: deployer.address,
        subscriptionAmount: env("SUB_AMOUNT") ?? DEFAULT_SUB_AMOUNT,
        lastPaymentTime: null,
        paymentTimeInterval: env<int>("DEFAULT_PAYMENT_INTERVAL") ?? DEFAULT_PAYMENT_INTERVAL,
    });

    val res = extension.deploy(deployer.address, { value: grams("0.5"), bounce: false });

    if (res.waitForFirstTransaction() == null) {
        return;
    }

    println("Deployed simple extension to {}", extension.address);
    println("Configured wallet address {}", extension.extensionInfo().walletAddress);
}
