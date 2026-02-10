import "@acton/io"
import "@acton/fmt"
import "@acton/env"
import "@acton/build/build"
import "@acton/emulation/network"

import "@wrappers/JettonMinter"

const ONCHAIN_CONTENT_PREFIX = 0x00
const SNAKE_PREFIX = 0x00

struct JettonInfo {
    name: string,
    description: string,
    symbol: string,
    image: string,
}

fun string.toSnakeString(self): cell {
    return beginCell().storeUint(SNAKE_PREFIX, 8).storeSlice(self.beginParse()).endCell()
}

fun buildOnchainMetadata(info: JettonInfo): cell {
    println1("Deploying Jetton with {}", info);

    var dict = createEmptyMap<uint256, cell>();

    dict.set(stringSha256("name"), info.name.toSnakeString());
    dict.set(stringSha256("description"), info.description.toSnakeString());
    dict.set(stringSha256("symbol"), info.symbol.toSnakeString());
    dict.set(stringSha256("image"), info.image.toSnakeString());

    return beginCell().storeUint(ONCHAIN_CONTENT_PREFIX, 8).storeDict(dict.toLowLevelDict()).endCell()
}

fun main() {
    val content = buildOnchainMetadata({
        name: envOr<string>("JETTON_NAME", "SomeJetton"),
        description: envOr<string>("JETTON_DESCRIPTION", "Sone jetton"),
        symbol: envOr<string>("JETTON_SYMBOL", "JTTT"),
        image: envOr<string>("JETTON_IMAGE", "https://ton.org/download/ton_symbol.svg"),
    });

    val supply = envOr<int>("JETTON_SUPPLY", 1000000000);

    val deployer = net.wallet("deployer");
    val minter = JettonMinter.fromStorage({
        totalSupply: supply,
        adminAddress: deployer.address,
        content: content,
        jettonWalletCode: build("jetton_wallet"),
    });

    println("Deploying Jetton contract...");
    val deployRes = minter.sendDeploy(deployer.address, { value: ton("0.03") });
    deployRes.wait();

    println1("Minting tokens: {}", supply);
    val mintRes = minter.sendMint(deployer.address, deployer.address, supply, ton("0.01"), ton("0.02"));
    if (!mintRes.wait()) {
        return;
    }

    println(format2("Succesfully minted and transfered {} tokens to {}", supply, deployer.address));
}
