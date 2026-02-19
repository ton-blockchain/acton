/// Module for configuring the emulated blockchain environment.
///
/// This module provides utilities for managing TON Blockchain configuration parameters.
/// Configuration parameters control network behavior, including gas prices, storage costs,
/// validator settings, and system contract addresses. These parameters are stored in the
/// masterchain configuration and are used by the TON Virtual Machine to execute transactions.
///
/// For detailed information about each blockchain configuration parameter, see the
/// [TON Blockchain Configuration Reference](https://docs.ton.org/foundations/config) in the TON documentation.
///
/// Examples:
/// ```tolk
/// // Set custom gas prices
/// var config = net.blockchainConfig();
/// val gasPrice = GasPricesSimple {
///     gasPrice: 1000,
///     gasLimit: 1000000,
///     gasCredit: 10000,
///     blockGasLimit: 10000000,
///     freezeDueLimit: 1000000000,
///     deleteDueLimit: 5000000000
/// };
/// config.setGasPrices(simplePrices, MASTERCHAIN);
///
/// // Access storage prices
/// val storagePrices = config.getStoragePrices();
/// println1("Bit price: {}", storagePrices);
/// ```

import "../testing/assert"

/// Type representing the blockchain configuration.
///
/// The configuration is a key-value map where keys are parameter indices (0-255)
/// and values are cells containing the serialized configuration parameter data.
/// Configuration parameters are defined in the [`block.tlb`](https://github.com/ton-blockchain/ton/blob/master/crypto/block/block.tlb)
/// TL-B schema.
///
/// `Config.set*` methods mutate only this in-memory map.
/// To apply updates to the emulator globally, call [net.setConfig] with the mutated config.
type Config = map<uint32, cell>

/// Sets a raw configuration parameter at the given index.
///
/// This is a low-level function that directly stores a cell at the specified parameter index.
/// For type-safe parameter access, prefer using specialized setter functions such as
/// [Config.setGasPrices], [Config.setStoragePrices], or [Config.setGlobalVersion].
/// This mutates only the local [Config] value until you call [net.setConfig].
///
/// Parameters:
/// - `index`: The configuration parameter index (typically 0-90)
/// - `data`: The serialized cell containing the parameter value
///
/// Example:
/// ```tolk
/// config.setParamRaw(20, gasCell);
/// ```
fun Config.setParamRaw(mutate self, index: uint32, data: cell) {
    self.set(index, data);
}

/// Retrieves a raw configuration parameter at the given index.
///
/// This is a low-level function that returns a cell containing the serialized configuration data.
/// For type-safe parameter access, prefer using specialized getter functions such as
/// [Config.getGasPrices], [Config.getStoragePrices], or [Config.getGlobalVersion].
///
/// Parameters:
/// - `index`: The configuration parameter index (typically 0-90)
///
/// Returns: The raw cell containing the serialized parameter value.
///
/// Example:
/// ```tolk
/// val paramCell = config.getParamRaw(20);
/// ```
fun Config.getParamRaw(self, index: uint32) {
    return self.get(index).loadValue();
}

/// Configuration parameter index for global network version (Param 8).
///
/// This parameter contains the network version number and capability flags supported by validators.
/// It is used to coordinate network upgrades and enable/disable protocol features.
const GLOBAL_VERSION_INDEX = 8

/// Network version and capabilities information (Param 8).
///
/// This struct contains the current network version and a set of capability flags
/// that indicate which features are supported by the network.
struct (0xc4) GlobalVersion {
    /// The current network version number.
    version: uint32
    /// Bitmask of supported capabilities. Each bit represents a specific network feature.
    capabilities: uint64
}

/// Sets the global network version and capabilities (Param 8).
///
/// This function updates the network version and validator capabilities configuration.
/// When validators vote to update the network, they update this parameter, allowing
/// the TON Blockchain to upgrade without downtime.
/// This mutates only the local [Config] value until you call [net.setConfig].
///
/// Parameters:
/// - `version`: The new global network version struct containing version and capabilities
///
/// Example:
/// ```tolk
/// val newVersion = GlobalVersion {
///     version: 5,
///     capabilities: 0xFF
/// };
/// config.setGlobalVersion(newVersion);
/// if (!net.setConfig(config)) {
///     Assert.fail("Failed to apply config");
/// }
/// ```
fun Config.setGlobalVersion(mutate self, version: GlobalVersion) {
    self.set(GLOBAL_VERSION_INDEX, version.toCell());
}

/// Retrieves the global network version and capabilities (Param 8).
///
/// This function reads the current network version and capabilities supported by validators.
/// Use this to determine which network features are available.
///
/// Returns: [GlobalVersion] struct containing version number and capabilities bitmask.
///
/// Example:
/// ```tolk
/// val version = config.getGlobalVersion();
/// println(format2("Network version: {}, capabilities: {}", version.version, version.capabilities));
/// ```
fun Config.getGlobalVersion(self) {
    return GlobalVersion.fromCell(self.get(GLOBAL_VERSION_INDEX).loadValue());
}

/// Configuration parameter index for storage prices (Param 18).
///
/// Storage prices determine the cost of storing data on the TON Blockchain.
/// These costs are used to prevent spam and ensure network maintenance is properly funded.
const STORAGE_PRICES_INDEX = 18

/// Storage price configuration for the TON Blockchain (Param 18).
///
/// This struct defines the pricing for data storage in both the masterchain and regular workchains.
/// Prices are given per 65536 seconds of storage. Storage fees are calculated based on the number
/// of bits and cells used to represent contract code and data.
struct (0xcc) StoragePrices {
    /// Unix timestamp indicating when these prices became effective.
    initialUnixTime: uint32
    /// Price per bit of storage in basechain workchains for 65536 seconds, in nanotons.
    bitPrice: uint64
    /// Price per cell of storage in basechain workchains for 65536 seconds, in nanotons.
    cellPrice: uint64
    /// Price per bit of storage in the masterchain for 65536 seconds, in nanotons.
    masterchainBitPrice: uint64
    /// Price per cell of storage in the masterchain for 65536 seconds, in nanotons.
    masterchainCellPrice: uint64
}

/// Dictionary mapping Unix timestamps to storage price configurations.
///
/// This allows different storage prices to apply at different periods in time.
/// The dictionary is indexed by Unix timestamp values.
type StoragePricesDict = map<uint32, StoragePrices>

/// Configuration parameter index for the initial storage price (timestamp 0).
///
/// This index refers to the storage price entry at index 0, which represents the initial
/// storage price configuration that applies from the beginning of time.
const INITIAL_UNIXTIME_STORAGE_INDEX = 0

/// Retrieves the initial storage price configuration from the dictionary.
///
/// Gets the storage price entry at index 0, which contains the initial pricing that
/// applies from the beginning of time until the first timestamp-indexed price update.
///
/// Returns: [StoragePrices] configuration containing the initial storage price parameters.
///
/// Example:
/// ```tolk
/// val storagePrices = config.getStoragePrices();
/// val initialPrices = storagePrices.getInitial();
/// println1("Initial bit price: {}", initialPrices.bitPrice);
/// ```
fun StoragePricesDict.getInitial(self) {
    val entry = self.get(INITIAL_UNIXTIME_STORAGE_INDEX);
    if (!entry.isFound) {
        Assert.fail("No value found for INITIAL_UNIXTIME_STORAGE_INDEX");
    }

    return entry.loadValue();
}

/// Sets the initial storage price configuration in the dictionary.
///
/// Updates the storage price entry at index 0 with new pricing parameters.
/// This affects the base storage price that applies from the beginning of time.
///
/// Parameters:
/// - `prices`: The new [StoragePrices] configuration to set as the initial entry
///
/// Example:
/// ```tolk
/// var storagePrices = config.getStoragePrices();
/// val newInitialPrices = StoragePrices {
///     initialUnixTime: 0,
///     bitPrice: 1,
///     cellPrice: 1000,
///     masterchainBitPrice: 1,
///     masterchainCellPrice: 1000
/// };
/// storagePrices.setInitial(newInitialPrices);
/// config.setStoragePrices(storagePrices);
/// if (!net.setConfig(config)) {
///     Assert.fail("Failed to apply config");
/// }
/// ```
fun StoragePricesDict.setInitial(mutate self, prices: StoragePrices) {
    self.set(INITIAL_UNIXTIME_STORAGE_INDEX, prices);
}

/// Sets the storage price configuration (Param 18).
///
/// Storage prices are organized as a dictionary mapping timestamps to price structures,
/// allowing for price adjustments over time. This function updates the entire storage
/// price dictionary.
/// This mutates only the local [Config] value until you call [net.setConfig].
///
/// Parameters:
/// - `prices`: A dictionary mapping Unix timestamps to [StoragePrices] configurations
///
/// Example:
/// ```tolk
/// var pricesDict = StoragePricesDict.fromMap(createEmptyMap<uint32, StoragePrices>());
/// pricesDict.set(1000000, StoragePrices {
///     initialUnixTime: 1000000,
///     bitPrice: 1,
///     cellPrice: 1000,
///     masterchainBitPrice: 1,
///     masterchainCellPrice: 1000
/// });
/// config.setStoragePrices(pricesDict);
/// if (!net.setConfig(config)) {
///     Assert.fail("Failed to apply config");
/// }
/// ```
fun Config.setStoragePrices(mutate self, prices: StoragePricesDict) {
    val pricesDict = prices.toCell();
    val pricesRef = pricesDict.beginParse().loadRef();

    self.set(STORAGE_PRICES_INDEX, pricesRef);
}

/// Retrieves the storage price configuration (Param 18).
///
/// Returns a dictionary of storage prices indexed by timestamp. Use this to determine
/// the cost of storing data on the blockchain.
///
/// Returns: [StoragePricesDict] containing storage price configurations indexed by timestamp.
///
/// Example:
/// ```tolk
/// val storagePrices = config.getStoragePrices();
/// val prices = storagePrices.get(1000000);
/// println(format3("Bit price: {}, Cell price: {}, Masterchain bit price: {}",
///     prices.bitPrice, prices.cellPrice, prices.masterchainBitPrice));
/// ```
fun Config.getStoragePrices(self) {
    val rawDict = self.get(STORAGE_PRICES_INDEX).loadValue();

    return StoragePricesDict.fromCell(beginCell().storeBool(true).storeRef(rawDict).endCell());
}

/// Gas price configuration for extended gas schemes (Param 20/21 variant).
///
/// This variant includes a special gas limit for system contracts.
struct (0xde) GasPricesExtended {
    /// Price of 65536 gas units in nanotons.
    gasPrice: uint64
    /// Maximum amount of gas that can be consumed per regular transaction.
    gasLimit: uint64
    /// Maximum amount of gas that can be consumed per system contract transaction.
    specialGasLimit: uint64
    /// Gas credit provided to process external messages, in gas units.
    gasCredit: uint64
    /// Maximum amount of gas that can be consumed within a single block.
    blockGasLimit: uint64
    /// Accumulated storage fee limit (in nanotons) above which contracts are frozen.
    freezeDueLimit: uint64
    /// Accumulated storage fee limit (in nanotons) above which contracts are deleted.
    deleteDueLimit: uint64
}

/// Gas price configuration with flat pricing prefix.
///
/// This allows for two-tiered pricing where a flat fee and price are applied first,
/// followed by the standard pricing structure.
struct (0xd1) GasPrices {
    /// Initial gas amount provided at flat price before standard pricing applies.
    flatGasLimit: uint64
    /// Flat price applied to the initial [GasPricesPrefix.flatGasLimit] amount of gas.
    flatGasPrice: uint64
    /// Additional gas pricing configuration applied after the flat pricing.
    other: GasPricesExtended
}

/// Configuration parameter index for masterchain gas prices (Param 20).
const MASTERCHAIN_GAS_PRICES_INDEX = 20

/// Configuration parameter index for basechain gas prices (Param 21).
const BASECHAIN_GAS_PRICES_INDEX = 21

/// Sets gas price configuration for the masterchain or basechain (Param 20/21).
///
/// Gas prices define the computational cost of executing transactions and smart contracts
/// on the TON Blockchain. The complexity of any computation is measured in gas units,
/// and each gas unit has a price in nanotons.
///
/// Parameters:
/// - `prices`: Gas price configuration struct
/// - `chainType`: Chain type constant - either `BASECHAIN` for basechain prices (Param 21) or `MASTERCHAIN` for masterchain prices (Param 20)
///
/// This mutates only the local [Config] value until you call [net.setConfig].
fun Config.setGasPrices(mutate self, prices: GasPrices, chainType: int) {
    val index = chainType == MASTERCHAIN ? MASTERCHAIN_GAS_PRICES_INDEX : BASECHAIN_GAS_PRICES_INDEX;
    self.set(index, prices.toCell());
}

/// Retrieves gas price configuration for the masterchain or basechain (Param 20/21).
///
/// Returns the gas price configuration that determines transaction execution costs.
///
/// Parameters:
/// - `chainType`: If equal to `MASTERCHAIN`, reads Param 20; any other value (including `BASECHAIN`) reads Param 21
///
/// Returns: [GasPrices] configuration union containing the gas pricing parameters.
///
/// Example:
/// ```tolk
/// val gasPrices = config.getGasPrices(BASECHAIN);
/// println1("Gas limit basechain: {}", gasPrices);
/// ```
fun Config.getGasPrices(self, chainType: int) {
    val index = chainType == MASTERCHAIN ? MASTERCHAIN_GAS_PRICES_INDEX : BASECHAIN_GAS_PRICES_INDEX;
    return GasPrices.fromCell(self.get(index).loadValue());
}

/// Message forwarding price configuration (Param 24/25).
///
/// This struct defines the costs associated with forwarding messages between accounts and shards
/// in the TON Blockchain. Message routing uses these prices to calculate fees for message delivery.
struct (0xea) MsgForwardPrices {
    /// Base price for forwarding a message, independent of its size, in nanotons.
    lumpPrice: uint64
    /// Price per bit of the message being forwarded, in nanotons per bit.
    bitPrice: uint64
    /// Price per cell of the message being forwarded, in nanotons per cell.
    cellPrice: uint64
    /// Factor for calculating the cost of immediate hypercube routing (IHR).
    ihrPriceFactor: uint32
    /// Fraction of the remaining fee used for the first transition along the message route (in 1/65536ths).
    firstFrac: uint16
    /// Fraction of the remaining fee used for subsequent transitions along the message route (in 1/65536ths).
    nextFrac: uint16
}

/// Configuration parameter index for masterchain message forwarding prices (Param 24).
const MASTERCHAIN_MSG_FORWARD_PRICES_INDEX = 24

/// Configuration parameter index for basechain message forwarding prices (Param 25).
const BASECHAIN_MSG_FORWARD_PRICES_INDEX = 25

/// Sets message forwarding price configuration for the masterchain or basechain (Param 24/25).
///
/// Message forwarding prices determine the cost of sending messages between accounts in the TON Blockchain.
/// These prices are used by the message routing mechanism to calculate message forwarding fees.
///
/// Parameters:
/// - `prices`: Message forwarding price configuration
/// - `chainType`: If equal to `MASTERCHAIN`, uses Param 24; any other value (including `BASECHAIN`) uses Param 25
///
/// This mutates only the local [Config] value until you call [net.setConfig].
///
/// Example:
/// ```tolk
/// val msgPrices = MsgForwardPrices {
///     lumpPrice: 1000,
///     bitPrice: 1,
///     cellPrice: 100,
///     ihrPriceFactor: 3,
///     firstFrac: 21845,
///     nextFrac: 21845
/// };
/// config.setMsgForwardPrices(msgPrices, MASTERCHAIN);
/// if (!net.setConfig(config)) {
///     Assert.fail("Failed to apply config");
/// }
/// ```
fun Config.setMsgForwardPrices(mutate self, prices: MsgForwardPrices, chainType: int) {
    val index = chainType == MASTERCHAIN
        ? MASTERCHAIN_MSG_FORWARD_PRICES_INDEX
        : BASECHAIN_MSG_FORWARD_PRICES_INDEX;

    self.set(index, prices.toCell());
}

/// Retrieves message forwarding price configuration for the masterchain or basechain (Param 24/25).
///
/// Returns the message forwarding prices that determine message routing costs.
///
/// Parameters:
/// - `chainType`: If equal to `MASTERCHAIN`, reads Param 24; any other value (including `BASECHAIN`) reads Param 25
///
/// Returns: [MsgForwardPrices] configuration containing message forwarding price parameters.
///
/// Example:
/// ```tolk
/// val msgPrices = config.getMsgForwardPrices(BASECHAIN);
/// println(format3("Lump price: {}, Bit price: {}, Cell price: {}",
///     msgPrices.lumpPrice, msgPrices.bitPrice, msgPrices.cellPrice));
/// ```
fun Config.getMsgForwardPrices(self, chainType: int) {
    val index = chainType == MASTERCHAIN
        ? MASTERCHAIN_MSG_FORWARD_PRICES_INDEX
        : BASECHAIN_MSG_FORWARD_PRICES_INDEX;

    return MsgForwardPrices.fromCell(self.get(index).loadValue());
}

/// Configuration parameter index for precompiled contracts (Param 45).
///
/// Precompiled contracts are built-in smart contracts optimized for specific computations.
/// They consume a fixed amount of gas regardless of input size, making them more efficient
/// than general-purpose contracts for their specific functions.
const PRECOMPILED_CONTRACTS_CONFIG_INDEX = 45

/// Precompiled smart contract configuration.
///
/// Specifies the gas usage for a precompiled contract.
struct (0xb0) PrecompiledSmartContract {
    /// Fixed gas consumption for executing this precompiled contract.
    gasUsage: uint64
}

/// Configuration for all precompiled contracts (Param 45).
///
/// Contains a dictionary mapping contract addresses to their gas usage specifications.
/// Precompiled contracts are optimized implementations of common operations provided by the system.
struct (0xc0) PrecompiledContractsConfig {
    /// Dictionary mapping 256-bit contract addresses to their gas usage configurations.
    list: map<uint256, PrecompiledSmartContract>
}

/// Adds a precompiled contract gas entry if `hash` is not already present.
///
/// Returns `true` when a new entry is inserted.
/// Returns `false` for duplicate `hash` values, and keeps the original `gasUsage` unchanged.
fun PrecompiledContractsConfig.addContractGas(mutate self, hash: uint256, gasForComputePhase: uint64) {
    return self.list.addIfNotExists(hash, {
        gasUsage: gasForComputePhase,
    });
}

/// Sets the precompiled contracts configuration (Param 45).
///
/// Updates the list of precompiled smart contracts and their associated gas costs.
/// Precompiled contracts are system-provided optimizations for common cryptographic
/// and utility operations.
/// This mutates only the local [Config] value until you call [net.setConfig].
///
/// Parameters:
/// - `precompiledConfig`: Configuration containing the dictionary of precompiled contracts
///
/// Example:
/// ```tolk
/// var contractsList = createEmptyMap<uint256, PrecompiledSmartContract>();
/// contractsList.set(0x123456, PrecompiledSmartContract { gasUsage: 1000 });
/// val precompiledConfig = PrecompiledContractsConfig {
///     list: contractsList
/// };
/// config.setPrecompiledContractsConfig(precompiledConfig);
/// if (!net.setConfig(config)) {
///     Assert.fail("Failed to apply config");
/// }
/// ```
fun Config.setPrecompiledContractsConfig(mutate self, precompiledConfig: PrecompiledContractsConfig) {
    self.set(PRECOMPILED_CONTRACTS_CONFIG_INDEX, precompiledConfig.toCell());
}

/// Retrieves the precompiled contracts configuration (Param 45).
///
/// Returns the configuration of all precompiled contracts available in the network,
/// including their fixed gas costs.
/// In the default emulator config this dictionary starts with one built-in entry.
/// If you explicitly write an empty dictionary via [Config.setPrecompiledContractsConfig]
/// and apply it with [net.setConfig], subsequent reads return an empty dictionary.
///
/// Returns: [PrecompiledContractsConfig] containing the precompiled contracts dictionary.
///
/// Example:
/// ```tolk
/// val precompiledConfig = config.getPrecompiledContractsConfig();
/// val contractsList = precompiledConfig.list;
/// ```
fun Config.getPrecompiledContractsConfig(self) {
    return PrecompiledContractsConfig.fromCell(self.get(PRECOMPILED_CONTRACTS_CONFIG_INDEX)
        .loadValue());
}
