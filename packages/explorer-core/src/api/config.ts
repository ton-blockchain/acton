import {Cell, Dictionary} from "@ton/core"
import {formatDateTime, type ParsedValue} from "@acton/ui"

import {loadConfigParam, loadConfigParams} from "../cell-inspector/block.tlb.generated"

export const TON_CONFIG_DOCS_URL = "https://docs.ton.org/foundations/config"

export interface FundamentalSmartContract {
  readonly address: string
  readonly codeHash: string
}

export interface PrecompiledContractConfiguration {
  readonly index: number
  readonly codeHash: string
  readonly gasUsage: bigint
}

export interface BurningConfiguration {
  readonly blackholeAddress?: string
  readonly feeBurnNum: number
  readonly feeBurnDenom: number
}

export interface ExtraCurrency {
  readonly id: number
  readonly amount: bigint
}

export interface GlobalVersionConfiguration {
  readonly version: number
  readonly capabilities: bigint
}

export interface ValidatorConfiguration {
  readonly index: number
  readonly publicKey: string
  readonly adnlAddress?: string
  readonly weight: bigint
}

export interface ValidatorSetConfiguration {
  readonly utimeSince: number
  readonly utimeUntil: number
  readonly total: number
  readonly main: number
  readonly totalWeight?: bigint
  readonly validators: readonly ValidatorConfiguration[]
}

export interface SuspendedAddressesConfiguration {
  readonly suspendedUntil: number
  readonly addresses: readonly string[]
}

export interface BridgeOracle {
  readonly index: number
  readonly key: string
  readonly value: string
}

export interface BridgeConfiguration {
  readonly kind: "oracle" | "jetton"
  readonly bridgeAddress: string
  readonly oracleAddress: string
  readonly oracles: readonly BridgeOracle[]
  readonly externalChainAddress?: string
  readonly stateFlags?: number
  readonly burnBridgeFee?: bigint
  readonly prices?: readonly NetworkConfigValue[]
}

export interface NetworkConfigValue {
  readonly label: string
  readonly value?: bigint | boolean | number
  readonly children?: readonly NetworkConfigValue[]
  readonly format?: NetworkConfigValueFormat
}

export type NetworkConfigValueFormat =
  | "bytes"
  | "date"
  | "duration"
  | "duration-ms"
  | "gram"
  | "gram-per-65536"

export interface NetworkConfigParameter {
  readonly id: number
  readonly title: string
  readonly description: string
  readonly rawHex: string
  readonly address?: string
  readonly parsedValue?: ParsedValue
  readonly burningConfiguration?: BurningConfiguration
  readonly extraCurrencies?: readonly ExtraCurrency[]
  readonly globalVersion?: GlobalVersionConfiguration
  readonly configurationValues?: readonly NetworkConfigValue[]
  readonly globalId?: number
  readonly parameterIds?: readonly number[]
  readonly fundamentalSmartContracts?: readonly FundamentalSmartContract[]
  readonly precompiledContracts?: readonly PrecompiledContractConfiguration[]
  readonly validatorSet?: ValidatorSetConfiguration
  readonly suspendedAddresses?: SuspendedAddressesConfiguration
  readonly bridgeConfiguration?: BridgeConfiguration
  readonly parseError?: string
}

export interface NetworkConfig {
  readonly configAddress?: string
  readonly parameters: readonly NetworkConfigParameter[]
  readonly rawHex: string
}

interface ConfigParameterMetadata {
  readonly title: string
  readonly description: string
}

const CONFIG_PARAMETER_METADATA: Readonly<Record<number, ConfigParameterMetadata>> = {
  // The collection code identifies itself as "TON Config Parameter Ownership":
  // https://actonscan.com/tx/59ac226ac63ba4ecfcf4b9da1a02510fc787cc1f52fdfddb3a2b8f1bf54bbd77?network=testnet
  // Its NFT for parameter 10000 created this slot through the Config contract:
  // https://actonscan.com/tx/54507a083797977b25d761d9a80b0892a3d12f1f958b225dcbd64fba96d6d6f0?network=testnet
  [-10000]: {
    title: "NFT-owned test slot",
    description: "Test configuration slot controlled by a TON Config Parameter Ownership NFT",
  },
  // The current value was proposed as part of a live governance test on testnet:
  // https://actonscan.com/tx/94541e28b2099038c06f23da9ab6fb8cefe6687d1e42bfdda083c1f726f99fda?network=testnet
  [-1337]: {
    title: "Governance test slot",
    description: "Testnet scratch slot used to exercise configuration proposal and voting behavior",
  },
  // Created as one batch of independent testnet governance proposals:
  // https://actonscan.com/tx/24b6a0d289f8acb6e86b69269916e164dd6d993c07917571cf7ee4cde193d3d7?network=testnet
  // Their values copy the ConfigParam 16 layout while varying max_validators from 31 to 38:
  // https://github.com/ton-blockchain/ton/blob/686b56a9b4f0b905386ad2a5ff865eca2506457e/crypto/block/block.tlb#L700-L704
  [-1306]: {
    title: "Validator proposal test 31",
    description:
      "Testnet governance scratch slot containing validator limits with max validators set to 31",
  },
  [-1305]: {
    title: "Validator proposal test 32",
    description:
      "Testnet governance scratch slot containing validator limits with max validators set to 32",
  },
  [-1304]: {
    title: "Validator proposal test 33",
    description:
      "Testnet governance scratch slot containing validator limits with max validators set to 33",
  },
  [-1303]: {
    title: "Validator proposal test 34",
    description:
      "Testnet governance scratch slot containing validator limits with max validators set to 34",
  },
  [-1302]: {
    title: "Validator proposal test 35",
    description:
      "Testnet governance scratch slot containing validator limits with max validators set to 35",
  },
  [-1301]: {
    title: "Validator proposal test 36",
    description:
      "Testnet governance scratch slot containing validator limits with max validators set to 36",
  },
  [-1300]: {
    title: "Validator proposal test 37",
    description:
      "Testnet governance scratch slot containing validator limits with max validators set to 37",
  },
  [-1299]: {
    title: "Validator proposal test 38",
    description:
      "Testnet governance scratch slot containing validator limits with max validators set to 38",
  },
  // https://github.com/ton-blockchain/config-with-ownable-params/blob/4d942616389a7327f8ee40b3664b1b08a457a340/config-code.fc#L9-L10
  [-1025]: {
    title: "Custom config slot 2",
    description: "Second owner-controlled custom configuration slot",
  },
  [-1024]: {
    title: "Custom config slot 1",
    description: "First owner-controlled custom configuration slot",
  },
  // https://github.com/ton-blockchain/governance-contract/blob/a9e0d23eab49f08b3fdbb9b6fe5c2847d05b4d12/constants.fc#L62-L66
  [-1003]: {
    title: "Config multikey",
    description: "Additional public keys used to authorize configuration contract actions",
  },
  [-1002]: {
    title: "Signed votes disabled",
    description: "Disables signed external votes for configuration proposals when present",
  },
  [-1001]: {
    title: "Update elector code",
    description: "Contains code used to upgrade the Elector smart contract",
  },
  [-1000]: {
    title: "Update config code",
    description: "Contains code used to upgrade the configuration smart contract",
  },
  [-999]: {
    title: "Set config key",
    description: "Sets the master public key used to authorize configuration contract actions",
  },
  // Installs currency ID 100 and the faucet contract address:
  // https://actonscan.com/tx/4ca7d1700b1a36d5ffe94cfbe42fae219d8ed83475cca4c5c470879b46ac72c6?network=testnet
  // Deploys the configured contract, which is subsequently funded with ECHIDNA and dispenses it:
  // https://actonscan.com/tx/632ac6146ac1fa3a207d92d77075ca55b19775f3098676f5c3219460a1791b20?network=testnet
  [-236]: {
    title: "ECHIDNA faucet",
    description: "Configures the testnet faucet contract for ECHIDNA extra currency",
  },
  // Set to 0xffffffff, then 1, and finally 0 through direct signed configuration updates:
  // https://actonscan.com/tx/2f4ee3e8ccd5a8408b76c18e126c740adad4246a175885e6114d41811d4bbb69?network=testnet
  // https://actonscan.com/tx/f9c1fac16a8fbece8e1aaac502cfa454d8ec6c055ceee3a5efce6b78db8185c9?network=testnet
  // https://actonscan.com/tx/000be59ef8c5b2d1f6f76b977571626adabe44d64411c3294c851dc4fb367e3c?network=testnet
  [-137]: {
    title: "Direct config update test",
    description: "Testnet scratch slot used to exercise signed configuration updates",
  },
  // Created through testnet governance voting:
  // https://actonscan.com/tx/6c675b35b70653a497580add73f12dd67d72d88063982c62193d11d0fe8485e2?network=testnet
  [-133]: {
    title: "Governance test slot",
    description:
      "Testnet scratch slot containing a ConfigParam 16-shaped value with max validators set to zero",
  },
  // https://t.me/tonstatus/175
  // https://actonscan.com/tx/2d5738520c63a6aeddc8d7ff7f52a19c70b11b9ecbca81acc1408488262358c2?network=mainnet
  // https://actonscan.com/tx/0ef291123b4ffc82db9b1eea82f675389151d676084a9885df42d2e7bd3ac2fe?network=testnet
  [-90]: {
    title: "BTC Teleport governance",
    description: "Configures validator participation in governance for the BTC Teleport bridge",
  },
  // https://github.com/ton-blockchain/ton/blob/686b56a9b4f0b905386ad2a5ff865eca2506457e/crypto/func/auto-tests/legacy_tests/dns-collection/dns-utils.fc#L7
  [-80]: {
    title: "DNS blacklist",
    description: "Testnet configuration slot containing the TON DNS domain blacklist",
  },
  // https://github.com/ton-blockchain/token-bridge-func/blob/4e7ec44a651e6b455ce5a09ed1383535fae3a637/src/func/jetton-bridge/config.fc#L7-L10
  // https://github.com/ton-blockchain/token-bridge-func/blob/4e7ec44a651e6b455ce5a09ed1383535fae3a637/src/func/jetton-bridge/params.fc#L1
  [-79]: {
    title: "Legacy Ethereum jetton bridge",
    description: "Legacy configuration slot used as a fallback by Ethereum jetton bridge contracts",
  },
  // https://github.com/ton-blockchain/ton/blob/6b49d6a382e30cf7f248a93d448726145c052300/crypto/func/auto-tests/legacy_tests/bsc-bridge-collector/bridge-config.fc#L1-L12
  [-72]: {
    title: "Legacy Binance Smart Chain bridge",
    description: "Legacy configuration slot used as a fallback by BSC bridge contracts",
  },
  // https://github.com/ton-blockchain/ton/blob/686b56a9b4f0b905386ad2a5ff865eca2506457e/crypto/func/auto-tests/legacy_tests/eth-bridge-multisig/multisig-code.fc#L5-L10
  [-71]: {
    title: "Legacy Ethereum bridge",
    description: "Legacy configuration slot used as a fallback by Ethereum bridge contracts",
  },
  // https://github.com/ton-blockchain/ton/blob/011e97f53c1610ead70e59f662f14d4a7be268d6/crypto/block/block.tlb#L737-L739
  [-41]: {
    title: "Legacy collator configuration",
    description: "Legacy full-collated-data flag and list of configured collator nodes",
  },
  // https://github.com/ton-blockchain/ton/blob/b978e27b2f2ca6e4403843b9d6bf3f407bf63499/crypto/smartcont/restricted-wallet-code.fc#L1-L10
  [-13]: {
    title: "Restricted wallet start time",
    description: "Default activation timestamp used by legacy restricted wallet contracts",
  },
  // Installs the address of the collection deployed in the transaction linked above:
  // https://actonscan.com/tx/52a0273685e28eba6511a9b5dcc1b62ae80f46ebaf7413ccb1b88043b7b892b9?network=testnet
  [-1]: {
    title: "Config ownership collection",
    description:
      "Address of the testnet NFT collection representing ownership of configuration slots",
  },
  0: {
    title: "Config address",
    description:
      "Address of the masterchain smart contract that stores the active network configuration",
  },
  1: {
    title: "Elector address",
    description: "Address of the Elector smart contract used for validator elections",
  },
  2: {
    title: "GRAM minting address",
    description: "Address of the smart contract that controls GRAM coin minting",
  },
  3: {
    title: "Fee collector address",
    description: "Address that receives protocol fees collected by the network",
  },
  4: {
    title: "Root DNS address",
    description: "Address of the root TON DNS smart contract",
  },
  5: {
    title: "Burning configuration",
    description: "Controls how transaction fees are burned and where remaining fees are sent",
  },
  6: {
    title: "Extra currency minting prices",
    description: "Prices for creating and extending extra-currency balances",
  },
  7: {
    title: "Extra currency volume",
    description: "Total supply of extra currencies currently recorded by the network",
  },
  8: {
    title: "Network version and capabilities",
    description: "Protocol version and feature flags supported by the network",
  },
  9: {
    title: "Mandatory parameters",
    description: "Configuration parameters that every validator must understand",
  },
  10: {
    title: "Critical parameters",
    description: "Configuration parameters that require a network-wide coordinated update",
  },
  11: {
    title: "Configuration voting",
    description: "Rules for proposals that change normal and critical configuration parameters",
  },
  12: {
    title: "Workchains",
    description: "Workchain definitions, split limits, activation state, and zero-state hashes",
  },
  13: {
    title: "Complaint pricing",
    description: "Deposit and processing prices for validator complaints",
  },
  14: {
    title: "Block creation fees",
    description: "Fees paid for creating masterchain and basechain blocks",
  },
  15: {
    title: "Election timing",
    description: "Durations of validator elections, election windows, and stake holding",
  },
  16: {
    title: "Validator limits",
    description: "Minimum and maximum validator counts for the network",
  },
  17: {
    title: "Stake limits",
    description: "Minimum, maximum, and aggregate stake limits for validator elections",
  },
  18: {
    title: "Storage prices",
    description: "Prices for storing bits and cells, including separate masterchain prices",
  },
  19: {
    title: "Global ID",
    description: "Signed network identifier used by the protocol and tooling",
  },
  20: {
    title: "Masterchain gas prices",
    description: "Gas prices and limits applied to smart contracts in the masterchain",
  },
  21: {
    title: "Workchain gas prices",
    description: "Gas prices and limits applied to smart contracts in workchains",
  },
  22: {
    title: "Masterchain block limits",
    description: "Byte, gas, and logical-time limits for masterchain blocks",
  },
  23: {
    title: "Workchain block limits",
    description: "Byte, gas, and logical-time limits for workchain blocks",
  },
  24: {
    title: "Masterchain message prices",
    description: "Forwarding prices for messages routed through the masterchain",
  },
  25: {
    title: "Workchain message prices",
    description: "Forwarding prices for messages routed through workchains",
  },
  28: {
    title: "Catchain configuration",
    description: "Validator session lifetimes and validator-count settings for Catchain",
  },
  29: {
    title: "Consensus configuration",
    description: "Consensus round timing and candidate limits",
  },
  30: {
    title: "Consensus extension",
    description: "Additional consensus settings used by newer protocol versions",
  },
  31: {
    title: "Fundamental smart contracts",
    description:
      "Addresses of system smart contracts exempt from gas and storage fees and eligible for tick-tock transactions",
  },
  32: {
    title: "Previous validator set",
    description: "Validator set used by the previous validator election",
  },
  33: {
    title: "Previous temporary validator set",
    description: "Temporary validator set from the previous election",
  },
  34: {
    title: "Current validator set",
    description: "Validator set currently responsible for producing blocks",
  },
  35: {
    title: "Current temporary validator set",
    description: "Temporary validator set used during the current election",
  },
  36: {
    title: "Next validator set",
    description: "Validator set selected for the next election",
  },
  37: {
    title: "Next temporary validator set",
    description: "Temporary validator set selected for the next election",
  },
  39: {
    title: "Validator temporary keys",
    description: "Temporary validator keys used to sign validator-set changes",
  },
  40: {
    title: "Misbehavior punishment",
    description: "Penalties and limits applied when validators misbehave",
  },
  43: {
    title: "Size limits",
    description: "Limits for messages, account state, libraries, and VM data",
  },
  44: {
    title: "Suspended addresses",
    description: "Addresses temporarily suspended by validator voting and the suspension deadline",
  },
  45: {
    title: "Precompiled contracts",
    description: "Contracts executed by protocol precompiles with configured gas costs",
  },
  71: {
    title: "Ethereum bridge",
    description: "Oracle bridge parameters for the Ethereum bridge",
  },
  72: {
    title: "Binance Smart Chain bridge",
    description: "Oracle bridge parameters for the BSC bridge",
  },
  73: {
    title: "Polygon bridge",
    description: "Oracle bridge parameters for the Polygon bridge",
  },
  79: {
    title: "TON jetton bridge",
    description: "Jetton bridge parameters for the legacy TON bridge",
  },
  81: {
    title: "Ethereum jetton bridge",
    description: "Jetton bridge parameters for the Ethereum bridge",
  },
  82: {
    title: "Polygon jetton bridge",
    description: "Jetton bridge parameters for the Polygon bridge",
  },
}

const UNKNOWN_PARAMETER_METADATA: ConfigParameterMetadata = {
  title: "Undocumented parameter",
  description:
    "No public description or typed parser is available for this parameter; inspect its raw cell",
}

const EXTENSION_PARAMETER_METADATA: ConfigParameterMetadata = {
  title: "Extension parameter",
  description:
    "Negative configuration identifiers are reserved for implementation-specific extension data",
}

export function parseNetworkConfig(rawBoc: string): NetworkConfig {
  const rootCell = Cell.fromBase64(rawBoc)
  const {config, configAddress} = readConfigState(rootCell)
  const parameters = [...config]
    .map(([unsignedId, cell]) => parseConfigParameter(toSignedInt32(unsignedId), cell))
    .sort(compareConfigParameters)

  return {
    configAddress,
    parameters,
    rawHex: rootCell.toBoc().toString("hex"),
  }
}

function compareConfigParameters(
  left: NetworkConfigParameter,
  right: NetworkConfigParameter,
): number {
  return compareConfigIds(left.id, right.id)
}

function compareConfigIds(left: number, right: number): number {
  const leftIsExtension = left < 0
  const rightIsExtension = right < 0
  if (leftIsExtension !== rightIsExtension) return leftIsExtension ? 1 : -1
  return left - right
}

function readConfigState(rootCell: Cell): {
  readonly config: Dictionary<number, Cell>
  readonly configAddress?: string
} {
  try {
    const config = loadConfigDictionary(rootCell)
    const configAddressCell = config.get(0)
    if (!configAddressCell) throw new Error("Configuration does not contain parameter 0")

    return {
      config,
      configAddress: masterchainAddress(configAddressCell.beginParse(true).loadBuffer(32)),
    }
  } catch (directDictionaryError) {
    try {
      const configParams = loadConfigParams(rootCell.beginParse(true))
      return {
        config: configParams.config,
        configAddress: masterchainAddress(configParams.config_addr),
      }
    } catch {
      throw directDictionaryError
    }
  }
}

function loadConfigDictionary(rootCell: Cell): Dictionary<number, Cell> {
  return Dictionary.loadDirect(
    Dictionary.Keys.Uint(32),
    {
      serialize: () => {
        throw new Error("Serialization is not available for network configuration")
      },
      parse: slice => slice.loadRef().beginParse(true).asCell(),
    },
    rootCell.beginParse(true),
  )
}

function parseConfigParameter(id: number, cell: Cell): NetworkConfigParameter {
  const metadata =
    CONFIG_PARAMETER_METADATA[id] ??
    (id < 0 ? EXTENSION_PARAMETER_METADATA : UNKNOWN_PARAMETER_METADATA)
  const parameter: NetworkConfigParameter = {
    id,
    title: metadata.title,
    description: metadata.description,
    rawHex: cell.toBoc().toString("hex"),
  }

  if (id < 0) return parameter

  try {
    const parsed = loadConfigParam(cell.beginParse(true), id)
    const parsedValue = toParsedValue(parsed)
    const address = isConfigAddressParameter(id) ? parseConfigAddress(parsed) : undefined
    const burningConfiguration = id === 5 ? parseBurningConfiguration(parsed) : undefined
    const extraCurrencies = id === 7 ? parseExtraCurrencies(parsed) : undefined
    const globalVersion = id === 8 ? parseGlobalVersion(parsed) : undefined
    const configurationValues = parseConfigurationValues(id, parsed)
    const globalId = id === 19 ? parseGlobalId(parsed) : undefined
    const parameterIds =
      id === 9
        ? parseParameterIds(parsed, "mandatory_params")
        : id === 10
          ? parseParameterIds(parsed, "critical_params")
          : undefined
    const precompiledContracts = id === 45 ? parsePrecompiledContracts(parsed) : undefined
    const validatorSet = parseValidatorSet(id, parsed)
    const suspendedAddresses = id === 44 ? parseSuspendedAddresses(parsed) : undefined
    const bridgeConfiguration = parseBridgeConfiguration(id, parsed)

    return {
      ...parameter,
      parsedValue,
      ...(address === undefined ? {} : {address}),
      ...(burningConfiguration === undefined ? {} : {burningConfiguration}),
      ...(extraCurrencies === undefined ? {} : {extraCurrencies}),
      ...(globalVersion === undefined ? {} : {globalVersion}),
      ...(configurationValues === undefined ? {} : {configurationValues}),
      ...(globalId === undefined ? {} : {globalId}),
      ...(parameterIds === undefined ? {} : {parameterIds}),
      ...(id === 31 ? {fundamentalSmartContracts: parseFundamentalSmartContracts(parsed)} : {}),
      ...(precompiledContracts === undefined ? {} : {precompiledContracts}),
      ...(validatorSet === undefined ? {} : {validatorSet}),
      ...(suspendedAddresses === undefined ? {} : {suspendedAddresses}),
      ...(bridgeConfiguration === undefined ? {} : {bridgeConfiguration}),
    }
  } catch (error) {
    return {
      ...parameter,
      parseError: error instanceof Error ? error.message : String(error),
    }
  }
}

function isConfigAddressParameter(id: number): boolean {
  return id >= 0 && id <= 4
}

function parseConfigAddress(value: unknown): string | undefined {
  if (typeof value !== "object" || value === null) return undefined

  for (const [key, candidate] of Object.entries(value)) {
    if (key !== "kind" && candidate instanceof Uint8Array && candidate.length === 32) {
      return masterchainAddress(candidate)
    }
  }

  return undefined
}

function parseBurningConfiguration(value: unknown): BurningConfiguration | undefined {
  if (typeof value !== "object" || value === null) return undefined

  const parameter = value as {readonly anon0?: unknown}
  const candidate = parameter.anon0 ?? value
  if (typeof candidate !== "object" || candidate === null) return undefined

  const config = candidate as {
    readonly kind?: unknown
    readonly blackhole_addr?: unknown
    readonly fee_burn_num?: unknown
    readonly fee_burn_denom?: unknown
  }
  if (
    config.kind !== "BurningConfig" ||
    typeof config.fee_burn_num !== "number" ||
    typeof config.fee_burn_denom !== "number"
  ) {
    return undefined
  }

  return {
    blackholeAddress: parseMaybeAddress(config.blackhole_addr),
    feeBurnNum: config.fee_burn_num,
    feeBurnDenom: config.fee_burn_denom,
  }
}

function parseMaybeAddress(value: unknown): string | undefined {
  if (typeof value !== "object" || value === null) return undefined

  const maybeAddress = value as {readonly kind?: unknown; readonly value?: unknown}
  if (maybeAddress.kind !== "Maybe_just") return undefined

  const address = maybeAddress.value
  return address instanceof Uint8Array && address.length === 32
    ? masterchainAddress(address)
    : undefined
}

function parseExtraCurrencies(value: unknown): readonly ExtraCurrency[] | undefined {
  if (typeof value !== "object" || value === null) return undefined

  const parameter = value as {readonly to_mint?: unknown}
  const collection = parameter.to_mint
  if (typeof collection !== "object" || collection === null) return undefined

  const dictionary = (collection as {readonly dict?: unknown}).dict
  if (!(dictionary instanceof Dictionary)) return undefined

  return [...dictionary]
    .map(([id, amount]) => ({id: toSignedInt32(id), amount}))
    .sort((left, right) => left.id - right.id)
}

function parseGlobalVersion(value: unknown): GlobalVersionConfiguration | undefined {
  if (typeof value !== "object" || value === null) return undefined

  const parameter = value as {readonly anon0?: unknown}
  const candidate = parameter.anon0 ?? value
  if (typeof candidate !== "object" || candidate === null) return undefined

  const config = candidate as {
    readonly kind?: unknown
    readonly version?: unknown
    readonly capabilities?: unknown
  }
  if (
    config.kind !== "GlobalVersion" ||
    typeof config.version !== "number" ||
    typeof config.capabilities !== "bigint"
  ) {
    return undefined
  }

  return {
    version: config.version,
    capabilities: config.capabilities,
  }
}

function parseConfigurationValues(
  id: number,
  value: unknown,
): readonly NetworkConfigValue[] | undefined {
  switch (id) {
    case 6:
      return parseTypedConfigurationValues(value, "ConfigParam__6", [
        {key: "mint_new_price", label: "Mint new price", type: "bigint", format: "gram"},
        {key: "mint_add_price", label: "Mint add price", type: "bigint", format: "gram"},
      ])
    case 11:
      return parseConfigVotingConfigurationValues(value)
    case 18:
      return parseStoragePricesConfigurationValues(value)
    case 13:
      return parseTypedConfigurationValues(value, "ComplaintPricing", [
        {key: "deposit", label: "Deposit", type: "bigint", format: "gram"},
        {key: "bit_price", label: "Bit price", type: "bigint", format: "gram"},
        {key: "_cell_price", label: "Cell price", type: "bigint", format: "gram"},
      ])
    case 14:
      return parseTypedConfigurationValues(value, "BlockCreateFees", [
        {
          key: "masterchain_block_fee",
          label: "Masterchain block fee",
          type: "bigint",
          format: "gram",
        },
        {key: "basechain_block_fee", label: "Basechain block fee", type: "bigint", format: "gram"},
      ])
    case 15:
      return parseTypedConfigurationValues(value, "ConfigParam__15", [
        {
          key: "validators_elected_for",
          label: "Validators elected for",
          type: "number",
          format: "duration",
        },
        {
          key: "elections_start_before",
          label: "Elections start before",
          type: "number",
          format: "duration",
        },
        {
          key: "elections_end_before",
          label: "Elections end before",
          type: "number",
          format: "duration",
        },
        {key: "stake_held_for", label: "Stake held for", type: "number", format: "duration"},
      ])
    case 16:
      return parseTypedConfigurationValues(value, "ConfigParam__16", [
        {key: "max_validators", label: "Max validators", type: "number"},
        {key: "max_main_validators", label: "Max main validators", type: "number"},
        {key: "min_validators", label: "Min validators", type: "number"},
      ])
    case 17:
      return parseTypedConfigurationValues(value, "ConfigParam__17", [
        {key: "min_stake", label: "Min stake", type: "bigint", format: "gram"},
        {key: "max_stake", label: "Max stake", type: "bigint", format: "gram"},
        {key: "min_total_stake", label: "Min total stake", type: "bigint", format: "gram"},
        {key: "max_stake_factor", label: "Max stake factor", type: "number"},
      ])
    case 43:
      return parseSizeLimitsConfigurationValues(value)
    case 20:
    case 21:
      return parseNestedConfigurationValues(value, "GasLimitsPrices")
    case 22:
    case 23:
      return parseNestedConfigurationValues(value, "BlockLimits")
    case 24:
    case 25:
      return parseNestedConfigurationValues(value, "MsgForwardPrices")
    case 28:
      return parseNestedConfigurationValues(value, "CatchainConfig")
    case 29:
      return parseNestedConfigurationValues(value, "ConsensusConfig")
    default:
      return undefined
  }
}

function parseTypedConfigurationValues(
  value: unknown,
  kind: string,
  fields: readonly ConfigValueField[],
): readonly NetworkConfigValue[] | undefined {
  const candidate = unwrapAnonymousConfigValue(value)
  if (!candidate || candidate.kind !== kind) return undefined

  const values: NetworkConfigValue[] = []
  for (const field of fields) {
    const fieldValue = candidate[field.key]
    if (
      (field.type === "bigint" && typeof fieldValue !== "bigint") ||
      (field.type === "number" && typeof fieldValue !== "number")
    ) {
      return undefined
    }
    if (typeof fieldValue !== "bigint" && typeof fieldValue !== "number") return undefined
    values.push(createNetworkConfigValue(field.label, fieldValue, field.format))
  }

  return values
}

function parseConfigVotingConfigurationValues(
  value: unknown,
): readonly NetworkConfigValue[] | undefined {
  const candidate = unwrapAnonymousConfigValue(value)
  if (
    candidate?.kind !== "ConfigVotingSetup" ||
    typeof candidate.normal_params !== "object" ||
    candidate.normal_params === null ||
    typeof candidate.critical_params !== "object" ||
    candidate.critical_params === null
  ) {
    return undefined
  }

  const normalParams = configProposalSetupValues(candidate.normal_params)
  const criticalParams = configProposalSetupValues(candidate.critical_params)
  if (!normalParams || !criticalParams) return undefined

  return [
    {
      label: "Normal params",
      children: [{label: "Config proposal setup", children: normalParams}],
    },
    {
      label: "Critical params",
      children: [{label: "Config proposal setup", children: criticalParams}],
    },
  ]
}

function configProposalSetupValues(value: object): readonly NetworkConfigValue[] | undefined {
  return parseRequiredConfigurationFields(value as Record<string, unknown>, [
    {key: "min_tot_rounds", label: "Min total rounds", type: "number"},
    {key: "max_tot_rounds", label: "Max total rounds", type: "number"},
    {key: "min_wins", label: "Min wins", type: "number"},
    {key: "max_losses", label: "Max losses", type: "number"},
    {key: "min_store_sec", label: "Min store sec", type: "number", format: "duration"},
    {key: "max_store_sec", label: "Max store sec", type: "number", format: "duration"},
    {key: "bit_price", label: "Bit price", type: "number", format: "gram"},
    {key: "_cell_price", label: "Cell price", type: "number", format: "gram"},
  ])
}

function parseStoragePricesConfigurationValues(
  value: unknown,
): readonly NetworkConfigValue[] | undefined {
  if (typeof value !== "object" || value === null) return undefined

  const dictionary = (value as Record<string, unknown>).anon0
  if (!(dictionary instanceof Dictionary)) return undefined

  const values: NetworkConfigValue[] = []
  for (const [key, storagePrices] of dictionary) {
    if (typeof storagePrices !== "object" || storagePrices === null) return undefined

    const fields = parseRequiredConfigurationFields(storagePrices as Record<string, unknown>, [
      {key: "utime_since", label: "Utime since", type: "number", format: "date"},
      {key: "bit_price_ps", label: "Bit price ps", type: "bigint", format: "gram"},
      {key: "_cell_price_ps", label: "Cell price ps", type: "bigint", format: "gram"},
      {key: "mc_bit_price_ps", label: "MC bit price ps", type: "bigint", format: "gram"},
      {key: "mc_cell_price_ps", label: "MC cell price ps", type: "bigint", format: "gram"},
    ])
    if (!fields) return undefined

    values.push({
      label:
        Number(key) === 0
          ? "Initial storage prices"
          : `Storage prices from ${formatDateTime(Number(key), {
              display: "date",
              locale: "en-US",
              timeZone: "UTC",
              unit: "seconds",
            })}`,
      children: fields,
    })
  }

  return values
}

function parseRequiredConfigurationFields(
  candidate: Record<string, unknown>,
  fields: readonly ConfigValueField[],
): readonly NetworkConfigValue[] | undefined {
  const values: NetworkConfigValue[] = []
  for (const field of fields) {
    const fieldValue = candidate[field.key]
    if (
      (field.type === "bigint" && typeof fieldValue !== "bigint") ||
      (field.type === "number" && typeof fieldValue !== "number")
    ) {
      return undefined
    }
    if (typeof fieldValue !== "bigint" && typeof fieldValue !== "number") return undefined
    values.push(createNetworkConfigValue(field.label, fieldValue, field.format))
  }
  return values
}

function parseSizeLimitsConfigurationValues(
  value: unknown,
): readonly NetworkConfigValue[] | undefined {
  const candidate = unwrapAnonymousConfigValue(value)
  if (
    !candidate ||
    typeof candidate.kind !== "string" ||
    !candidate.kind.startsWith("SizeLimitsConfig_")
  ) {
    return undefined
  }

  const values: NetworkConfigValue[] = []
  const fields: readonly ConfigValueField[] = [
    {key: "max_msg_bits", label: "Max msg bits", type: "number"},
    {key: "max_msg_cells", label: "Max msg cells", type: "number"},
    {key: "max_library_cells", label: "Max library cells", type: "number"},
    {key: "max_vm_data_depth", label: "Max VM data depth", type: "number"},
    {key: "max_ext_msg_size", label: "Max ext msg size", type: "number"},
    {key: "max_ext_msg_depth", label: "Max ext msg depth", type: "number"},
    {key: "max_acc_state_cells", label: "Max acc state cells", type: "number"},
    {key: "max_mc_acc_state_cells", label: "Max MC acc state cells", type: "number"},
    {key: "max_acc_public_libraries", label: "Max acc public libraries", type: "number"},
    {key: "defer_out_queue_size_limit", label: "Defer out queue size limit", type: "number"},
    {key: "max_msg_extra_currencies", label: "Max msg extra currencies", type: "number"},
    {key: "max_acc_fixed_prefix_length", label: "Max acc fixed prefix length", type: "number"},
    {
      key: "acc_state_cells_for_storage_dict",
      label: "Acc state cells for storage dict",
      type: "number",
    },
    {key: "max_total_msg_bits", label: "Max total msg bits", type: "number"},
    {key: "max_total_msg_cells", label: "Max total msg cells", type: "number"},
  ]

  for (const field of fields) {
    if (!(field.key in candidate)) continue
    const fieldValue = candidate[field.key]
    if (typeof fieldValue !== "number") return undefined
    values.push({label: field.label, value: fieldValue})
  }

  const transactionLibraryLoads = candidate.max_transaction_library_loads
  if (typeof transactionLibraryLoads === "object" && transactionLibraryLoads !== null) {
    const maybeValue = transactionLibraryLoads as Record<string, unknown>
    if (maybeValue.kind === "Maybe_just" && typeof maybeValue.value === "number") {
      values.push({label: "Max transaction library loads", value: maybeValue.value})
    }
  }

  return values.length > 0 ? values : undefined
}

function parseNestedConfigurationValues(
  value: unknown,
  kindPrefix: string,
): readonly NetworkConfigValue[] | undefined {
  const candidate = unwrapAnonymousConfigValue(value)
  if (!candidate || typeof candidate.kind !== "string" || !candidate.kind.startsWith(kindPrefix)) {
    return undefined
  }

  const values = nestedConfigurationValues(candidate)
  return values.length > 0 ? values : undefined
}

function nestedConfigurationValues(
  value: unknown,
  inheritedFormat?: NetworkConfigValueFormat,
): NetworkConfigValue[] {
  if (typeof value !== "object" || value === null) return []

  const record = value as Record<string, unknown>
  const kind = typeof record.kind === "string" ? record.kind : undefined
  const values: NetworkConfigValue[] = []
  for (const [key, child] of Object.entries(record)) {
    if (key === "flags" || key === "kind") continue

    const label = humanizeFieldName(key)
    const fieldFormat = configurationFieldFormat(key)
    const scalar = toNetworkConfigScalar(child)
    if (scalar !== undefined) {
      const format = configurationValueFormat(kind, key) ?? inheritedFormat
      values.push(createNetworkConfigValue(label, scalar, format))
      continue
    }

    const children = nestedConfigurationValues(child, fieldFormat ?? inheritedFormat)
    if (children.length > 0) values.push({label, children})
  }
  return values
}

function toNetworkConfigScalar(value: unknown): bigint | boolean | number | undefined {
  if (typeof value === "bigint" || typeof value === "boolean" || typeof value === "number") {
    return value
  }
  if (typeof value !== "object" || value === null) return undefined

  const record = value as Record<string, unknown>
  return record.kind === "Bool" && typeof record.value === "boolean" ? record.value : undefined
}

interface ConfigValueField {
  readonly key: string
  readonly label: string
  readonly type: "bigint" | "number"
  readonly format?: NetworkConfigValueFormat
}

function createNetworkConfigValue(
  label: string,
  value: bigint | boolean | number,
  format?: NetworkConfigValueFormat,
): NetworkConfigValue {
  return {label, value, ...(format === undefined ? {} : {format})}
}

function configurationValueFormat(
  kind: string | undefined,
  key: string,
): NetworkConfigValueFormat | undefined {
  if (
    kind?.startsWith("GasLimitsPrices") &&
    ["gas_price", "flat_gas_price", "freeze_due_limit", "delete_due_limit"].includes(key)
  ) {
    return "gram"
  }

  if (kind === "MsgForwardPrices" && key === "lump_price") {
    return "gram"
  }

  if (kind === "MsgForwardPrices" && ["bit_price", "_cell_price"].includes(key)) {
    return "gram-per-65536"
  }

  return configurationFieldFormat(key)
}

function configurationFieldFormat(key: string): NetworkConfigValueFormat | undefined {
  const normalizedKey = key.toLowerCase()
  if (normalizedKey.includes("bytes")) return "bytes"
  if (normalizedKey.endsWith("_ms")) return "duration-ms"
  if (normalizedKey.includes("sec")) return "duration"
  return undefined
}

function unwrapAnonymousConfigValue(value: unknown): Record<string, unknown> | undefined {
  if (typeof value !== "object" || value === null) return undefined

  const record = value as Record<string, unknown>
  const anonymousValue = record.anon0
  if (typeof anonymousValue === "object" && anonymousValue !== null) {
    return anonymousValue as Record<string, unknown>
  }

  return record
}

function parseGlobalId(value: unknown): number | undefined {
  if (typeof value !== "object" || value === null) return undefined

  const record = value as Record<string, unknown>
  return record.kind === "ConfigParam__19" && typeof record.global_id === "number"
    ? record.global_id
    : undefined
}

function parseParameterIds(value: unknown, fieldName: string): readonly number[] | undefined {
  if (typeof value !== "object" || value === null) return undefined

  const dictionary = (value as Record<string, unknown>)[fieldName]
  if (!(dictionary instanceof Dictionary)) return undefined

  return [...dictionary].map(([id]) => toSignedInt32(id)).sort(compareConfigIds)
}

function parseFundamentalSmartContracts(
  value: unknown,
): readonly FundamentalSmartContract[] | undefined {
  if (typeof value !== "object" || value === null) return undefined

  const dictionary = (value as {readonly fundamental_smc_addr?: unknown}).fundamental_smc_addr
  if (!(dictionary instanceof Dictionary)) return undefined

  return [...dictionary]
    .map(([hash]) => {
      const codeHash = hash.toString(16).padStart(64, "0")
      return {
        address: `-1:${codeHash}`,
        codeHash: `0x${codeHash}`,
      }
    })
    .sort((left, right) => left.codeHash.localeCompare(right.codeHash))
}

function parsePrecompiledContracts(
  value: unknown,
): readonly PrecompiledContractConfiguration[] | undefined {
  const candidate = unwrapAnonymousConfigValue(value)
  if (candidate?.kind !== "PrecompiledContractsConfig" || !(candidate.list instanceof Dictionary)) {
    return undefined
  }

  const contracts: PrecompiledContractConfiguration[] = []
  for (const [codeHash, rawContract] of candidate.list) {
    if (typeof rawContract !== "object" || rawContract === null) return undefined

    const contract = rawContract as Record<string, unknown>
    const formattedCodeHash = fixedBigintHex(codeHash)
    if (
      contract.kind !== "PrecompiledSmc" ||
      !formattedCodeHash ||
      typeof contract.gas_usage !== "bigint"
    ) {
      return undefined
    }

    contracts.push({
      index: contracts.length,
      codeHash: formattedCodeHash,
      gasUsage: contract.gas_usage,
    })
  }

  return contracts
}

const VALIDATOR_SET_FIELDS: Readonly<Record<number, string>> = {
  32: "prev_validators",
  33: "prev_temp_validators",
  34: "cur_validators",
  35: "cur_temp_validators",
  36: "next_validators",
  37: "next_temp_validators",
}

function parseValidatorSet(id: number, value: unknown): ValidatorSetConfiguration | undefined {
  const fieldName = VALIDATOR_SET_FIELDS[id]
  if (!fieldName || typeof value !== "object" || value === null) return undefined

  const rawValidatorSet = (value as Record<string, unknown>)[fieldName]
  if (typeof rawValidatorSet !== "object" || rawValidatorSet === null) return undefined

  const validatorSet = rawValidatorSet as Record<string, unknown>
  if (
    typeof validatorSet.utime_since !== "number" ||
    typeof validatorSet.utime_until !== "number" ||
    typeof validatorSet.total !== "number" ||
    typeof validatorSet.main !== "number" ||
    !(validatorSet.list instanceof Dictionary)
  ) {
    return undefined
  }

  const validators: ValidatorConfiguration[] = []
  for (const [index, rawValidator] of validatorSet.list) {
    if (typeof rawValidator !== "object" || rawValidator === null) return undefined

    const validator = rawValidator as Record<string, unknown>
    const rawPublicKey = validator.public_key
    if (typeof rawPublicKey !== "object" || rawPublicKey === null) return undefined

    const publicKey = (rawPublicKey as Record<string, unknown>).pubkey
    if (!(publicKey instanceof Uint8Array) || publicKey.length !== 32) return undefined

    const adnlAddress = validator.adnl_addr
    if (
      adnlAddress !== undefined &&
      (!(adnlAddress instanceof Uint8Array) || adnlAddress.length !== 32)
    ) {
      return undefined
    }
    if (typeof validator.weight !== "bigint") return undefined

    validators.push({
      index,
      publicKey: bytesToHex(publicKey),
      ...(adnlAddress === undefined ? {} : {adnlAddress: bytesToHex(adnlAddress)}),
      weight: validator.weight,
    })
  }

  validators.sort((left, right) => left.index - right.index)

  return {
    utimeSince: validatorSet.utime_since,
    utimeUntil: validatorSet.utime_until,
    total: validatorSet.total,
    main: validatorSet.main,
    ...(typeof validatorSet.total_weight === "bigint"
      ? {totalWeight: validatorSet.total_weight}
      : {}),
    validators,
  }
}

function parseSuspendedAddresses(value: unknown): SuspendedAddressesConfiguration | undefined {
  const candidate = unwrapAnonymousConfigValue(value)
  if (
    candidate?.kind !== "SuspendedAddressList" ||
    typeof candidate.suspended_until !== "number" ||
    !(candidate.addresses instanceof Dictionary)
  ) {
    return undefined
  }

  return {
    suspendedUntil: candidate.suspended_until,
    addresses: [...candidate.addresses]
      .map(([address]) => addressFromUint288(address))
      .sort((left, right) => left.localeCompare(right)),
  }
}

function parseBridgeConfiguration(id: number, value: unknown): BridgeConfiguration | undefined {
  const candidate = unwrapAnonymousConfigValue(value)
  if (!candidate) return undefined

  if (id >= 71 && id <= 73 && candidate.kind === "OracleBridgeParams") {
    const bridgeAddress = parseHashAddress(candidate.bridge_address)
    const oracleAddress = parseHashAddress(candidate.oracle_mutlisig_address)
    const oracles = parseBridgeOracles(candidate.oracles)
    const externalChainAddress = parseFixedHex(candidate.external_chain_address)
    if (!bridgeAddress || !oracleAddress || !oracles || !externalChainAddress) return undefined

    return {
      kind: "oracle",
      bridgeAddress,
      oracleAddress,
      oracles,
      externalChainAddress,
    }
  }

  if (id !== 79 && id !== 81 && id !== 82) return undefined
  if (typeof candidate.kind !== "string" || !candidate.kind.startsWith("JettonBridgeParams_")) {
    return undefined
  }

  const bridgeAddress = parseHashAddress(candidate.bridge_address)
  const oracleAddress = parseHashAddress(candidate.oracles_address)
  const oracles = parseBridgeOracles(candidate.oracles)
  if (!bridgeAddress || !oracleAddress || !oracles) return undefined

  const externalChainAddress =
    candidate.external_chain_address === undefined
      ? undefined
      : parseFixedHex(candidate.external_chain_address)
  if (candidate.external_chain_address !== undefined && !externalChainAddress) return undefined

  const stateFlags = candidate.state_flags
  if (typeof stateFlags !== "number") return undefined

  const burnBridgeFee = candidate.burn_bridge_fee
  const prices = parseJettonBridgePrices(candidate.prices)
  if (burnBridgeFee !== undefined && typeof burnBridgeFee !== "bigint") return undefined
  if (candidate.prices !== undefined && !prices) return undefined

  return {
    kind: "jetton",
    bridgeAddress,
    oracleAddress,
    oracles,
    ...(externalChainAddress === undefined ? {} : {externalChainAddress}),
    stateFlags,
    ...(burnBridgeFee === undefined ? {} : {burnBridgeFee}),
    ...(prices === undefined ? {} : {prices}),
  }
}

function parseBridgeOracles(value: unknown): readonly BridgeOracle[] | undefined {
  if (!(value instanceof Dictionary)) return undefined

  const oracles: BridgeOracle[] = []
  for (const [index, [key, oracleValue]] of [...value].entries()) {
    const formattedKey = fixedBigintHex(key)
    const formattedValue = typeof oracleValue === "bigint" ? fixedBigintHex(oracleValue) : undefined
    if (!formattedKey || !formattedValue) return undefined
    oracles.push({index, key: formattedKey, value: formattedValue})
  }
  return oracles
}

function parseJettonBridgePrices(value: unknown): readonly NetworkConfigValue[] | undefined {
  if (typeof value !== "object" || value === null) return undefined

  const prices = value as Record<string, unknown>
  if (prices.kind !== "JettonBridgePrices") return undefined

  const fields: readonly ConfigValueField[] = [
    {key: "bridge_burn_fee", label: "Bridge burn fee", type: "bigint", format: "gram"},
    {key: "bridge_mint_fee", label: "Bridge mint fee", type: "bigint", format: "gram"},
    {
      key: "wallet_min_tons_for_storage",
      label: "Wallet min GRAM for storage",
      type: "bigint",
      format: "gram",
    },
    {
      key: "wallet_gas_consumption",
      label: "Wallet gas consumption",
      type: "bigint",
      format: "gram",
    },
    {
      key: "minter_min_tons_for_storage",
      label: "Minter min GRAM for storage",
      type: "bigint",
      format: "gram",
    },
    {
      key: "discover_gas_consumption",
      label: "Discover gas consumption",
      type: "bigint",
      format: "gram",
    },
  ]

  const values: NetworkConfigValue[] = []
  for (const field of fields) {
    const fieldValue = prices[field.key]
    if (typeof fieldValue !== "bigint") return undefined
    values.push(createNetworkConfigValue(field.label, fieldValue, field.format))
  }
  return values
}

function toSignedInt32(value: number): number {
  return value > 0x7f_ff_ff_ff ? value - 0x1_00_00_00_00 : value
}

function masterchainAddress(bytes: Uint8Array): string {
  return `-1:${bytesToHex(bytes)}`
}

function parseHashAddress(value: unknown): string | undefined {
  return value instanceof Uint8Array && value.length === 32 ? masterchainAddress(value) : undefined
}

function parseFixedHex(value: unknown): string | undefined {
  return value instanceof Uint8Array && value.length === 32 ? `0x${bytesToHex(value)}` : undefined
}

function fixedBigintHex(value: bigint): string | undefined {
  const hex = value.toString(16)
  return hex.length <= 64 ? `0x${hex.padStart(64, "0")}` : undefined
}

function addressFromUint288(value: bigint): string {
  const hex = value.toString(16).padStart(72, "0")
  const workchain = toSignedInt32(Number.parseInt(hex.slice(0, 8), 16))
  return `${workchain}:${hex.slice(8)}`
}

function toParsedValue(value: unknown, fieldName?: string): ParsedValue {
  if (value === null) return {kind: "null"}
  if (value === undefined) return {kind: "void"}

  if (typeof value === "boolean") return {kind: "boolean", value}

  if (typeof value === "bigint" || typeof value === "number" || typeof value === "string") {
    return {
      kind: "scalar",
      typeName: typeof value === "bigint" ? "uint" : undefined,
      value: String(value),
    }
  }

  if (value instanceof Cell) {
    return {
      kind: "scalar",
      typeName: "Cell",
      value: `${value.bits.length} bits, ${value.refs.length} refs`,
      rawValue: value.toBoc().toString("base64"),
    }
  }

  if (value instanceof Uint8Array) {
    const hex = bytesToHex(value)
    if (isAddressField(fieldName) && value.length === 32) {
      return {kind: "address", value: masterchainAddress(value)}
    }

    return {
      kind: "scalar",
      typeName: value.length === 32 ? "bits256" : "bytes",
      value: `0x${hex}`,
      rawValue: hex,
    }
  }

  if (value instanceof Dictionary) {
    return {
      kind: "map",
      typeName: "dictionary",
      entries: [...value].map(([key, item]) => ({
        key: toParsedValue(key),
        value: toParsedValue(item, fieldName),
      })),
    }
  }

  if (Array.isArray(value)) {
    return {kind: "array", items: value.map(item => toParsedValue(item, fieldName))}
  }

  if (typeof value === "object") {
    const record = value as Record<string, unknown>
    const kind = typeof record.kind === "string" ? record.kind : undefined
    const entries = Object.entries(record)
      .filter(([key]) => key !== "kind")
      .map(([key, item]) => ({
        key: humanizeFieldName(key),
        value: toParsedValue(item, key),
      }))

    if (kind?.startsWith("ConfigParam") && "anon0" in record) {
      return toParsedValue(record.anon0)
    }

    return {
      kind: "object",
      typeName: kind && !kind.startsWith("ConfigParam") ? humanizeTypeName(kind) : undefined,
      entries,
    }
  }

  return {kind: "scalar", value: String(value)}
}

function isAddressField(fieldName: string | undefined): boolean {
  if (!fieldName || fieldName === "external_chain_address") return false

  return (
    /(?:^|_)(?:addr|address)$/.test(fieldName) ||
    /(?:elector|minter|collector|dns_root|blackhole|oracle_mutlisig)/.test(fieldName)
  )
}

function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes, byte => byte.toString(16).padStart(2, "0")).join("")
}

function humanizeFieldName(value: string): string {
  const words = value
    .replace(/^_+/, "")
    .replace(/([a-z])([A-Z])/g, "$1 $2")
    .split(/[_\s]+/)
    .filter(Boolean)

  const acronyms: Readonly<Record<string, string>> = {
    bsc: "BSC",
    dns: "DNS",
    id: "ID",
    lt: "LT",
    mc: "MC",
    smc: "SMC",
    ton: "TON",
    tot: "total",
    vm: "VM",
  }
  const readable = words.map(word => acronyms[word.toLowerCase()] ?? word.toLowerCase())
  return readable.length === 0
    ? value
    : `${readable[0][0].toUpperCase()}${readable[0].slice(1)}${readable
        .slice(1)
        .map(word => ` ${word}`)
        .join("")}`
}

function humanizeTypeName(value: string): string {
  return humanizeFieldName(value.replace(/_v\d+$/, ""))
}
