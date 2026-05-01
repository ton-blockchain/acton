import "@acton/env"
import "@acton/io"
import "@acton/emulation/network"
import "@acton/emulation/scripts"
import "@wrappers/SimpleExtension.gen"

const DEFAULT_SUB_AMOUNT = ton("0.27")
const DEFAULT_PAYMENT_INTERVAL = 3600

fun main() {
    val deployer = scripts.wallet("deployer-2");

    val extension = SimpleExtension.fromStorage({
        walletAddress: deployer.address,
        admin: deployer.address,
        subscriptionAmount: env("SUB_AMOUNT") ?? DEFAULT_SUB_AMOUNT,
        lastPaymentTime: null,
        paymentTimeInterval: env<int>("DEFAULT_PAYMENT_INTERVAL") ?? DEFAULT_PAYMENT_INTERVAL,
    });

    val res = extension.deploy(deployer.address, { value: ton("0.5"), bounce: false });

    if (res.waitForFirstTransaction() == null) {
        return;
    }

    println("Deployed simple extension to {}", extension.address);
    println("Configured wallet address {}", extension.extensionInfo().walletAddress);
}
