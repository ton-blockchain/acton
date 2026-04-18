import "@acton/build"
import "@acton/env"
import "@acton/io"
import "@acton/emulation/network"
import "@acton/emulation/scripts"

import "@wrappers/JettonMinter"

const ONCHAIN_CONTENT_PREFIX = 0x00
const SNAKE_PREFIX = 0x00

struct JettonInfo {
    name: string
    description: string
    symbol: string
    image: string
}

fun string.toSnakeString(self): cell {
    return beginCell().storeUint(SNAKE_PREFIX, 8).storeSlice(self.beginParse()).endCell();
}

fun buildOnchainMetadata(info: JettonInfo): cell {
    println("Deploying Jetton with {}", info);

    var dict: map<uint256, cell> = [];

    dict.set("name".sha256(), info.name.toSnakeString());
    dict.set("description".sha256(), info.description.toSnakeString());
    dict.set("symbol".sha256(), info.symbol.toSnakeString());
    dict.set("image".sha256(), info.image.toSnakeString());

    return beginCell()
        .storeUint(ONCHAIN_CONTENT_PREFIX, 8)
        .storeDict(dict.toLowLevelDict())
        .endCell();
}

fun main() {
    val content = buildOnchainMetadata({
        name: env<string>("JETTON_NAME") ?? "StarterJetton",
        description: env<string>("JETTON_DESCRIPTION") ?? "A starter jetton template",
        symbol: env<string>("JETTON_SYMBOL") ?? "SJTN",
        image: env<string>("JETTON_IMAGE") ?? "https://ton.org/download/ton_symbol.svg",
    });

    val supply = env<int>("JETTON_SUPPLY") ?? 1000000000;

    val deployer = scripts.wallet("deployer");
    val minter = JettonMinter.fromStorage({
        totalSupply: 0,
        adminAddress: deployer.address,
        content,
        jettonWalletCode: build("JettonWallet"),
    });

    println("Deploying Jetton contract...");
    val deployRes = minter.sendDeploy(deployer.address, { value: ton("0.03") });
    if (!deployRes.wait()) {
        return;
    }

    println("Deployed Jetton minter to {}", minter.address);

    println("Minting tokens: {}", supply);
    val mintRes = minter.sendMint(
        deployer.address,
        deployer.address,
        supply,
        ton("0.01"),
        ton("0.02"),
    );
    if (!mintRes.wait()) {
        return;
    }

    println("Successfully minted and transferred {} tokens to {}", supply, deployer.address);
    println("Total supply is {}", minter.getTotalSupply());
}
