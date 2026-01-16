type Config = map<uint32, cell>

fun Config.setParamRaw(mutate self, index: uint32, data: cell) {
    self.set(index, data);
}

fun Config.getParamRaw(self, index: uint32) {
    return self.get(index).loadValue();
}

const GLOBAL_VERSION_INDEX = 8
struct (0xc4) GlobalVersion {
    version: uint32
    capabilities: uint64
}

fun Config.setGlobalVersion(mutate self, version: GlobalVersion) {
    self.set(GLOBAL_VERSION_INDEX, version.toCell());
}

fun Config.getGlobalVersion(self) {
    return GlobalVersion.fromCell(self.get(GLOBAL_VERSION_INDEX).loadValue());
}

const STORAGE_PRICES_INDEX = 18
struct (0xcc) StoragePrices {
    initialUnixTime: uint32
    bitPrice: uint64
    cellPrice: uint64
    masterchainBitPrice: uint64
    masterchainCellPrice: uint64
}

type StoragePricesDict = map<uint32, StoragePrices>

fun Config.setStoragePrices(mutate self, prices: StoragePricesDict) {
    val pricesDict = prices.toCell();
    val pricesRef = pricesDict.beginParse().loadRef();

    self.set(STORAGE_PRICES_INDEX, pricesRef);
}

fun Config.getStoragePrices(self) {
    val rawDict = self.get(STORAGE_PRICES_INDEX).loadValue();

    return StoragePricesDict.fromCell(beginCell().storeBool(true).storeRef(rawDict).endCell());
}

struct (0xdd) GasPricesSimple {
    gasPrice: uint64
    gasLimit: uint64
    gasCredit: uint64
    blockGasLimit: uint64
    freezeDueLimit: uint64
    deleteDueLimit: uint64
}

struct (0xde) GasPricesExtended {
    gasPrice: uint64
    gasLimit: uint64
    specialGasLimit: uint64
    gasCredit: uint64
    blockGasLimit: uint64
    freezeDueLimit: uint64
    deleteDueLimit: uint64
}

type GasPricesLimited = GasPricesSimple | GasPricesExtended

struct (0xd1) GasPricesPrefix {
    flatGasLimit: uint64
    flatGasPrice: uint64
    suffix: GasPricesLimited
}

type GasPrices = GasPricesLimited | GasPricesPrefix

const MASTERCHAIN_GAS_PRICES_INDEX = 20
const BASECHAIN_GAS_PRICES_INDEX = 21

fun Config.setGasPrices(mutate self, prices: GasPrices, isMasterchain: bool) {
    val index = isMasterchain ? MASTERCHAIN_GAS_PRICES_INDEX : BASECHAIN_GAS_PRICES_INDEX;
    self.set(index, prices.toCell());
}

fun Config.getGasPrices(self, isMasterchain: bool) {
    val index = isMasterchain ? MASTERCHAIN_GAS_PRICES_INDEX : BASECHAIN_GAS_PRICES_INDEX;
    return GasPrices.fromCell(self.get(index).loadValue());
}

struct (0xea) MsgForwardPrices {
    lumpPrice: uint64
    bitPrice: uint64
    cellPrice: uint64
    ihrPriceFactor: uint32
    firstFrac: uint16
    nextFrac: uint16
}

const MASTERCHAIN_MSG_FORWARD_PRICES_INDEX = 24
const BASECHAIN_MSG_FORWARD_PRICES_INDEX = 25

fun Config.setMsgForwardPrices(mutate self, prices: MsgForwardPrices, isMasterchain: bool) {
    val index = isMasterchain
        ? MASTERCHAIN_MSG_FORWARD_PRICES_INDEX
        : BASECHAIN_MSG_FORWARD_PRICES_INDEX;

    self.set(index, prices.toCell());
}

fun Config.getMsgForwardPrices(self, isMasterchain: bool) {
    val index = isMasterchain
        ? MASTERCHAIN_MSG_FORWARD_PRICES_INDEX
        : BASECHAIN_MSG_FORWARD_PRICES_INDEX;

    return MsgForwardPrices.fromCell(self.get(index).loadValue());
}

const PRECOMPILED_CONTRACTS_CONFIG_INDEX = 45
struct (0xb0) PrecompiledSmartContract {
    gasUsage: uint64
}
struct (0xc0) PrecompiledContractsConfig {
    list: map<uint256, PrecompiledSmartContract>
}

fun Config.setPrecompiledContractsConfig(mutate self, precompiledConfig: PrecompiledContractsConfig) {
    self.set(PRECOMPILED_CONTRACTS_CONFIG_INDEX, precompiledConfig.toCell());
}

fun Config.getPrecompiledContractsConfig(self) {
    return PrecompiledContractsConfig.fromCell(self.get(PRECOMPILED_CONTRACTS_CONFIG_INDEX)
        .loadValue());
}
