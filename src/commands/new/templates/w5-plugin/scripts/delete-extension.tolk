import "@acton/env"
import "@acton/io"
import "@acton/emulation/network"
import "@acton/emulation/scripts"
import "@wrappers/SimpleExtension.gen"
import "@wrappers/WalletV5Contract"
import "@contracts/walletv5/messages"

fun main() {
    val deployer = scripts.wallet("deployer-2");

    val wallet = WalletV5Contract { address: deployer.address };
    val extensionAddress = env<address>("EXT_ADDRESS") ??
    address("kQA-genT40TdeI85bXtJ-tXN9y-wqHQ3MWIRQIh2CCTJVWdi");

    val extension = SimpleExtension.fromAddress(extensionAddress);

    println("Deployer wallet: {}", deployer.address);
    println("Extension address: {}", extension.address);

    val existingExtensions = wallet.getExtensions();

    if (!existingExtensions.exists(extension.address.getWorkchainAndHash().1)) {
        println("Extension is already not installed");
        return;
    }

    var extraActions: array<ExtraAction> = [];
    extraActions.push(RemoveExtensionExtraAction { addr: extension.address });

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
    val result = extResult!.waitForFirstTransaction();

    if (result == null) {
        println("External message was not accepted");
        return;
    }

    println("Extension was successfully deleted");
}
