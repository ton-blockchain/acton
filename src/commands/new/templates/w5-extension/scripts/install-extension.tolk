import "@acton/env"
import "@acton/io"
import "@acton/prompts"
import "@acton/emulation/network"
import "@acton/emulation/scripts"
import "@wrappers/SimpleExtension.gen"
import "@wrappers/WalletV5.gen"
import "@wrappers/utils"
import "@contracts/walletv5/messages"
import "utils/common"

fun main() {
    val deployer = promptW5Wallet("W5_DEPLOYER", "Select Wallet V5");

    val wallet = WalletV5 { address: deployer.address };
    val extensionAddress = env<address>("EXT_ADDRESS") ??
    promptAddress("Enter the deployed extension address to install");

    val extension = SimpleExtension.fromAddress(extensionAddress);

    println("Deployer wallet: {}", deployer.address);
    println("Extension address: {}", extension.address);

    val existingExtensions = wallet.getExtensions();

    if (existingExtensions.exists(extension.address.getWorkchainAndHash().1)) {
        println("Extension is already installed");
        return;
    }

    var extraActions: array<ExtraAction> = [];
    extraActions.push(AddExtensionExtraAction { addr: extension.address });

    val signedBody = createExternalSignedBody(
        deployer.toKeyPair(),
        deployer.walletId(),
        wallet.getSeqno(),
        ActionsList {
            outActions: [],
            extraActions,
        },
    );

    val external = net.createExternalMessage(deployer.address, signedBody);

    val extResult = net.sendExternal(external);
    if (extResult == null) {
        println("External message was not accepted");
        return;
    }

    val result = extResult.waitForFirstTransaction();
    if (result == null) {
        println("External message did not produce a transaction");
        return;
    }

    println("Extension installed successfully");
}
