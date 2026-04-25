import "@acton/env"
import "@acton/fmt"
import "@acton/io"
import "@acton/prompts"
import "@acton/emulation/network"
import "@acton/emulation/scripts"
import "@contracts/storage"
import "@wrappers/JettonMinter.gen"
import "@wrappers/JettonWallet.gen"
import "@wrappers/utils"

const JETTON_DEFAULT_NAME = "Acton Sample Jetton"
const JETTON_DEFAULT_SYMBOL = "ACTON"
const JETTON_DEFAULT_DESCRIPTION = "Sample jetton deployed from acton-contracts scripts"
const JETTON_DEFAULT_IMAGE = "https://ton.org/download/ton_symbol.png"
const JETTON_DEFAULT_DECIMALS = "9"
const JETTON_DEFAULT_UPDATED_DESCRIPTION = "Sample jetton (metadata updated from acton-contracts scripts)"

const JETTON_DEFAULT_INITIAL_SUPPLY = "0"
const JETTON_DEFAULT_MINT_AMOUNT = "100000000000"
const JETTON_DEFAULT_TRANSFER_AMOUNT = "10000000000"

struct JettonMetadata {
    name: string
    symbol: string
    description: string
    image: string
    decimals: string
}

fun promptJettonString(envName: string, message: string, default: string): string {
    val fromEnv = env<string>(envName);
    if (fromEnv != null) {
        return fromEnv;
    }
    return prompt(message, "", default);
}

fun promptJettonAddress(envName: string, message: string, default: address): address {
    val fromEnv = env<address>(envName);
    if (fromEnv != null) {
        return fromEnv;
    }
    return parseAddress(prompt(message, "", format("{}", default)));
}

fun promptJettonAddressRequired(envName: string, message: string): address {
    val fromEnv = env<address>(envName);
    if (fromEnv != null) {
        return fromEnv;
    }
    return parseAddress(prompt(message, "EQ... / kQ...", ""));
}

fun promptJettonCoins(envName: string, message: string, default: string): coins {
    val fromEnv = env<coins>(envName);
    if (fromEnv != null) {
        return fromEnv;
    }
    return parseInt(prompt(message, "in nanotons/nano-jettons", default));
}

fun promptJettonWallet(envName: string, message: string): ActonWallet {
    val nameFromEnv = env<string>(envName);
    if (nameFromEnv != null) {
        return scripts.wallet(nameFromEnv);
    }
    return scripts.wallet(promptWallet(message));
}

fun promptJettonMinter(message: string): JettonMinter {
    val addr = promptJettonAddressRequired("JETTON_MINTER_ADDRESS", message);
    return JettonMinter.fromAddress(addr);
}

fun promptJettonMetadata(descriptionDefault: string): JettonMetadata {
    return {
        name: promptJettonString("JETTON_NAME", "Enter jetton name", JETTON_DEFAULT_NAME),
        symbol: promptJettonString("JETTON_SYMBOL", "Enter jetton symbol", JETTON_DEFAULT_SYMBOL),
        description: promptJettonString(
            "JETTON_DESCRIPTION",
            "Enter jetton description",
            descriptionDefault,
        ),
        image: promptJettonString("JETTON_IMAGE", "Enter jetton image URL", JETTON_DEFAULT_IMAGE),
        decimals: promptJettonString(
            "JETTON_DECIMALS",
            "Enter jetton decimals",
            JETTON_DEFAULT_DECIMALS,
        ),
    };
}

fun buildJettonMetadataCell(info: JettonMetadata): cell {
    var metadata: OnchainMetadataReply = { contentDict: [] };
    metadata.contentDict.set("name".sha256(), SnakeDataReply { string: info.name }.toCell());
    metadata.contentDict.set("symbol".sha256(), SnakeDataReply { string: info.symbol }.toCell());
    metadata
        .contentDict
        .set("description".sha256(), SnakeDataReply { string: info.description }.toCell());
    metadata.contentDict.set("image".sha256(), SnakeDataReply { string: info.image }.toCell());
    metadata
        .contentDict
        .set("decimals".sha256(), SnakeDataReply { string: info.decimals }.toCell());
    return metadata.toCell();
}

fun promptJettonMinterStorage(admin: address): MinterStorage {
    return {
        totalSupply: 0,
        adminAddress: admin,
        nextAdminAddress: null,
        metadata: buildJettonMetadataCell(promptJettonMetadata(JETTON_DEFAULT_DESCRIPTION)),
    };
}

fun deployJettonMinter(deployer: ActonWallet, storage: MinterStorage): JettonMinter {
    val minter = JettonMinter.fromStorage(storage);
    val res = minter.sendDeploy(deployer.address, { value: ton("0.05") });
    res.waitForFirstTransaction();
    return minter;
}

fun readOnchainMetadataField(metadata: OnchainMetadataReply, key: uint256): string? {
    val valueCell = metadata.contentDict.get(key);
    if (!valueCell.isFound) {
        return null;
    }
    return valueCell.loadValue().load().string;
}

fun printMetadataField(label: string, fieldName: string, value: string?) {
    if (value == null) {
        println("{} METADATA_{}=<unavailable>", label, fieldName);
    } else {
        println("{} METADATA_{}={}", label, fieldName, value);
    }
}

fun printJettonMinterInfo(minter: JettonMinter, label: string) {
    val data = minter.getJettonData();
    println("{} MINTER_ADDRESS={}", label, minter.address);
    println("{} TOTAL_SUPPLY={}", label, data.totalSupply);
    println("{} MINTABLE={}", label, data.mintable);
    println("{} ADMIN_ADDRESS={}", label, data.adminAddress);
    val metadata = OnchainMetadataReply.fromCell(data.jettonContent);
    printMetadataField(label, "name", readOnchainMetadataField(metadata, "name".sha256()));
    printMetadataField(label, "symbol", readOnchainMetadataField(metadata, "symbol".sha256()));
    printMetadataField(
        label,
        "description",
        readOnchainMetadataField(metadata, "description".sha256()),
    );
    printMetadataField(label, "image", readOnchainMetadataField(metadata, "image".sha256()));
    printMetadataField(label, "decimals", readOnchainMetadataField(metadata, "decimals".sha256()));
}

fun printJettonWalletInfo(minter: JettonMinter, owner: address, label: string) {
    val wallet = JettonWallet { address: minter.getWalletAddress(owner) };
    val hasCode = scripts.isContractDeployed(wallet.address);
    println("{} OWNER_ADDRESS={}", label, owner);
    println("{} WALLET_ADDRESS={}", label, wallet.address);
    println("{} WALLET_DEPLOYED={}", label, hasCode);
    if (hasCode) {
        println("{} WALLET_BALANCE={}", label, wallet.getJettonBalance());
    } else {
        println("{} WALLET_BALANCE=<unavailable>", label);
    }
}
