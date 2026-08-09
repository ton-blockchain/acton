import {describe, expect, test} from "bun:test"
import {beginCell, Dictionary, type Cell} from "@ton/core"

import {parseNetworkConfig} from "../src/api/config"
import {
  storeConfigVotingSetup,
  storeJettonBridgeParams,
  storeOracleBridgeParams,
  storePrecompiledContractsConfig,
  storeSizeLimitsConfig,
  storeStoragePrices,
  storeSuspendedAddressList,
  storeValidatorSet,
  type ConfigVotingSetup,
  type JettonBridgeParams,
  type OracleBridgeParams,
  type PrecompiledContractsConfig,
  type PrecompiledSmc,
  type SizeLimitsConfig,
  type StoragePrices,
  type SuspendedAddressList,
  type ValidatorDescr,
  type ValidatorSet,
} from "../src/cell-inspector/block.tlb.generated"

describe("network configuration parser", () => {
  test("parses a direct getConfigAll dictionary and keeps signed extension IDs", () => {
    const config = Dictionary.empty<number, Cell>()
    config.set(0, beginCell().storeBuffer(Buffer.alloc(32, 0x55)).endCell())
    config.set(
      5,
      beginCell()
        .storeUint(1, 8)
        .storeBit(1)
        .storeBuffer(Buffer.alloc(32, 0x66))
        .storeUint(1, 32)
        .storeUint(2, 32)
        .endCell(),
    )
    const extraCurrencies = Dictionary.empty<number, bigint>()
    extraCurrencies.set(239, 666_666_666_666n)
    extraCurrencies.set(4_294_967_279, 1_000_000_000_000n)
    config.set(
      7,
      beginCell()
        .storeDict(extraCurrencies, Dictionary.Keys.Uint(32), {
          serialize: (value, builder) => builder.storeVarUint(value, 5),
          parse: () => {
            throw new Error("not used in test")
          },
        })
        .endCell(),
    )
    config.set(8, beginCell().storeUint(0xc4, 8).storeUint(15, 32).storeUint(1006n, 64).endCell())
    const mandatoryParams = Dictionary.empty<number, true>()
    mandatoryParams.set(0, true)
    mandatoryParams.set(5, true)
    config.set(
      9,
      beginCell()
        .storeDictDirect(mandatoryParams, Dictionary.Keys.Uint(32), {
          serialize: () => undefined,
          parse: () => {
            throw new Error("not used in test")
          },
        })
        .endCell(),
    )
    const criticalParams = Dictionary.empty<number, true>()
    criticalParams.set(8, true)
    criticalParams.set(31, true)
    criticalParams.set(0xff_ff_ff_ff, true)
    config.set(
      10,
      beginCell()
        .storeDictDirect(criticalParams, Dictionary.Keys.Uint(32), {
          serialize: () => undefined,
          parse: () => {
            throw new Error("not used in test")
          },
        })
        .endCell(),
    )
    const configVotingSetup: ConfigVotingSetup = {
      kind: "ConfigVotingSetup",
      normal_params: {
        kind: "ConfigProposalSetup",
        min_tot_rounds: 2,
        max_tot_rounds: 6,
        min_wins: 2,
        max_losses: 5,
        min_store_sec: 1_000_000,
        max_store_sec: 10_000_000,
        bit_price: 1,
        _cell_price: 500,
      },
      critical_params: {
        kind: "ConfigProposalSetup",
        min_tot_rounds: 4,
        max_tot_rounds: 7,
        min_wins: 3,
        max_losses: 5,
        min_store_sec: 5_000_000,
        max_store_sec: 20_000_000,
        bit_price: 2,
        _cell_price: 1000,
      },
    }
    config.set(11, beginCell().store(storeConfigVotingSetup(configVotingSetup)).endCell())
    const storagePriceEntries = Dictionary.empty<number, StoragePrices>()
    storagePriceEntries.set(0, {
      kind: "StoragePrices",
      utime_since: 0,
      bit_price_ps: 1n,
      _cell_price_ps: 500n,
      mc_bit_price_ps: 1_000n,
      mc_cell_price_ps: 500_000n,
    })
    storagePriceEntries.set(1_777_500_000, {
      kind: "StoragePrices",
      utime_since: 1_777_500_000,
      bit_price_ps: 0n,
      _cell_price_ps: 135n,
      mc_bit_price_ps: 1_000n,
      mc_cell_price_ps: 500_000n,
    })
    config.set(
      18,
      beginCell()
        .storeDictDirect(storagePriceEntries, Dictionary.Keys.Uint(32), {
          serialize: (value, builder) => storeStoragePrices(value)(builder),
          parse: () => {
            throw new Error("not used in test")
          },
        })
        .endCell(),
    )
    config.set(
      13,
      beginCell()
        .storeUint(0x1a, 8)
        .storeCoins(1_000_000_000n)
        .storeCoins(1n)
        .storeCoins(500n)
        .endCell(),
    )
    config.set(
      14,
      beginCell()
        .storeUint(0x6b, 8)
        .storeCoins(1_700_000_000n)
        .storeCoins(1_000_000_000n)
        .endCell(),
    )
    config.set(
      15,
      beginCell()
        .storeUint(65_536, 32)
        .storeUint(32_768, 32)
        .storeUint(8192, 32)
        .storeUint(32_768, 32)
        .endCell(),
    )
    config.set(16, beginCell().storeUint(400, 16).storeUint(100, 16).storeUint(75, 16).endCell())
    config.set(
      17,
      beginCell()
        .storeCoins(300_000_000_000_000n)
        .storeCoins(10_000_000_000_000_000n)
        .storeCoins(75_000_000_000_000n)
        .storeUint(294_912, 32)
        .endCell(),
    )
    config.set(19, beginCell().storeInt(-239, 32).endCell())
    config.set(
      20,
      beginCell()
        .storeUint(0xdd, 8)
        .storeUint(1n, 64)
        .storeUint(2n, 64)
        .storeUint(3n, 64)
        .storeUint(4n, 64)
        .storeUint(5n, 64)
        .storeUint(6n, 64)
        .endCell(),
    )
    config.set(
      21,
      beginCell()
        .storeUint(0xde, 8)
        .storeUint(7n, 64)
        .storeUint(8n, 64)
        .storeUint(9n, 64)
        .storeUint(10n, 64)
        .storeUint(11n, 64)
        .storeUint(12n, 64)
        .storeUint(13n, 64)
        .endCell(),
    )
    config.set(
      22,
      beginCell()
        .storeUint(0x5e, 8)
        .storeUint(0xc3, 8)
        .storeUint(1, 32)
        .storeUint(2, 32)
        .storeUint(3, 32)
        .storeUint(0xc3, 8)
        .storeUint(4, 32)
        .storeUint(5, 32)
        .storeUint(6, 32)
        .storeUint(0xc3, 8)
        .storeUint(7, 32)
        .storeUint(8, 32)
        .storeUint(9, 32)
        .storeUint(0xc3, 8)
        .storeUint(10, 32)
        .storeUint(11, 32)
        .storeUint(12, 32)
        .storeUint(0xd3, 8)
        .storeUint(13, 32)
        .storeUint(14, 32)
        .endCell(),
    )
    config.set(
      23,
      beginCell()
        .storeUint(0x5d, 8)
        .storeUint(0xc3, 8)
        .storeUint(15, 32)
        .storeUint(16, 32)
        .storeUint(17, 32)
        .storeUint(0xc3, 8)
        .storeUint(18, 32)
        .storeUint(19, 32)
        .storeUint(20, 32)
        .storeUint(0xc3, 8)
        .storeUint(21, 32)
        .storeUint(22, 32)
        .storeUint(23, 32)
        .endCell(),
    )
    config.set(
      24,
      beginCell()
        .storeUint(0xea, 8)
        .storeUint(24n, 64)
        .storeUint(25n, 64)
        .storeUint(26n, 64)
        .storeUint(27, 32)
        .storeUint(28, 16)
        .storeUint(29, 16)
        .endCell(),
    )
    config.set(
      25,
      beginCell()
        .storeUint(0xea, 8)
        .storeUint(30n, 64)
        .storeUint(31n, 64)
        .storeUint(32n, 64)
        .storeUint(33, 32)
        .storeUint(34, 16)
        .storeUint(35, 16)
        .endCell(),
    )
    config.set(
      28,
      beginCell()
        .storeUint(0xc2, 8)
        .storeUint(0, 7)
        .storeBit(1)
        .storeUint(36, 32)
        .storeUint(37, 32)
        .storeUint(38, 32)
        .storeUint(39, 32)
        .endCell(),
    )
    config.set(
      29,
      beginCell()
        .storeUint(0xd9, 8)
        .storeUint(0, 6)
        .storeBit(0)
        .storeBit(1)
        .storeUint(4, 8)
        .storeUint(5, 32)
        .storeUint(6, 32)
        .storeUint(7, 32)
        .storeUint(8, 32)
        .storeUint(9, 32)
        .storeUint(10, 32)
        .storeUint(11, 32)
        .storeUint(12, 16)
        .storeUint(13, 32)
        .endCell(),
    )
    const previousValidators = Dictionary.empty<number, ValidatorDescr>()
    previousValidators.set(0, {
      kind: "ValidatorDescr_validator_addr",
      public_key: {kind: "SigPubKey", pubkey: Buffer.alloc(32, 0x81)},
      weight: 100n,
      adnl_addr: Buffer.alloc(32, 0x01),
    })
    previousValidators.set(1, {
      kind: "ValidatorDescr_validator_addr",
      public_key: {kind: "SigPubKey", pubkey: Buffer.alloc(32, 0x82)},
      weight: 200n,
      adnl_addr: Buffer.alloc(32, 0x02),
    })
    const previousValidatorSet: ValidatorSet = {
      kind: "ValidatorSet_validators_ext",
      utime_since: 1_786_007_304,
      utime_until: 1_786_072_840,
      total: 2,
      main: 1,
      total_weight: 300n,
      list: previousValidators,
    }
    config.set(32, beginCell().store(storeValidatorSet(previousValidatorSet)).endCell())

    const currentValidators = Dictionary.empty<number, ValidatorDescr>()
    currentValidators.set(0, {
      kind: "ValidatorDescr_validator",
      public_key: {kind: "SigPubKey", pubkey: Buffer.alloc(32, 0x91)},
      weight: 400n,
    })
    const currentValidatorSet: ValidatorSet = {
      kind: "ValidatorSet_validators",
      utime_since: 1_786_072_840,
      utime_until: 1_786_138_376,
      total: 1,
      main: 1,
      list: currentValidators,
    }
    config.set(34, beginCell().store(storeValidatorSet(currentValidatorSet)).endCell())
    const sizeLimits: SizeLimitsConfig = {
      kind: "SizeLimitsConfig_size_limits_config_v2",
      max_msg_bits: 2_097_152,
      max_msg_cells: 8192,
      max_library_cells: 200,
      max_vm_data_depth: 512,
      max_ext_msg_size: 65_535,
      max_ext_msg_depth: 512,
      max_acc_state_cells: 65_536,
      max_mc_acc_state_cells: 2048,
      max_acc_public_libraries: 256,
      defer_out_queue_size_limit: 256,
      max_msg_extra_currencies: 0,
      max_acc_fixed_prefix_length: 8,
      acc_state_cells_for_storage_dict: 26,
      max_transaction_library_loads: {kind: "Maybe_just", value: 3},
    }
    config.set(43, beginCell().store(storeSizeLimitsConfig(sizeLimits)).endCell())

    const suspendedAddresses = Dictionary.empty<bigint, {readonly kind: "Unit"}>()
    suspendedAddresses.set(BigInt(`0x00000000${"44".repeat(32)}`), {kind: "Unit"})
    suspendedAddresses.set(BigInt(`0xffffffff${"55".repeat(32)}`), {kind: "Unit"})
    const suspendedAddressList: SuspendedAddressList = {
      kind: "SuspendedAddressList",
      addresses: suspendedAddresses,
      suspended_until: 1_803_189_600,
    }
    config.set(44, beginCell().store(storeSuspendedAddressList(suspendedAddressList)).endCell())

    const precompiledContracts = Dictionary.empty<bigint, PrecompiledSmc>()
    precompiledContracts.set(0x11n, {kind: "PrecompiledSmc", gas_usage: 1_000n})
    precompiledContracts.set(0x22n, {kind: "PrecompiledSmc", gas_usage: 2_000n})
    const precompiledContractsConfig: PrecompiledContractsConfig = {
      kind: "PrecompiledContractsConfig",
      list: precompiledContracts,
    }
    config.set(
      45,
      beginCell().store(storePrecompiledContractsConfig(precompiledContractsConfig)).endCell(),
    )

    const oracleEntries = Dictionary.empty<bigint, bigint>()
    oracleEntries.set(0x11n, 0x22n)
    const oracleBridge: OracleBridgeParams = {
      kind: "OracleBridgeParams",
      bridge_address: Buffer.alloc(32, 0xa1),
      oracle_mutlisig_address: Buffer.alloc(32, 0xa2),
      oracles: oracleEntries,
      external_chain_address: Buffer.alloc(32, 0xa3),
    }
    config.set(71, beginCell().store(storeOracleBridgeParams(oracleBridge)).endCell())

    const jettonOracleEntries = Dictionary.empty<bigint, bigint>()
    jettonOracleEntries.set(0x33n, 0x44n)
    const jettonBridge: JettonBridgeParams = {
      kind: "JettonBridgeParams_jetton_bridge_params_v1",
      bridge_address: Buffer.alloc(32, 0xb1),
      oracles_address: Buffer.alloc(32, 0xb2),
      oracles: jettonOracleEntries,
      state_flags: 1,
      prices: {
        kind: "JettonBridgePrices",
        bridge_burn_fee: 1n,
        bridge_mint_fee: 2n,
        wallet_min_tons_for_storage: 3n,
        wallet_gas_consumption: 4n,
        minter_min_tons_for_storage: 5n,
        discover_gas_consumption: 6n,
      },
      external_chain_address: Buffer.alloc(32, 0xb3),
    }
    config.set(79, beginCell().store(storeJettonBridgeParams(jettonBridge)).endCell())
    const fundamentalContracts = Dictionary.empty<bigint, true>()
    fundamentalContracts.set(0x1234n, true)
    config.set(
      31,
      beginCell()
        .storeDict(fundamentalContracts, Dictionary.Keys.BigUint(256), {
          serialize: () => undefined,
          parse: () => {
            throw new Error("not used in test")
          },
        })
        .endCell(),
    )
    config.set(0xff_ff_ff_ff, beginCell().storeUint(0, 1).endCell())

    const root = beginCell()
    root.storeDictDirect(config, Dictionary.Keys.Uint(32), {
      serialize: (value, builder) => builder.storeRef(value),
      parse: () => {
        throw new Error("not used in test")
      },
    })

    const parsed = parseNetworkConfig(root.endCell().toBoc().toString("base64"))
    const version = parsed.parameters.find(parameter => parameter.id === 8)
    const configAddressParameter = parsed.parameters.find(parameter => parameter.id === 0)
    const burning = parsed.parameters.find(parameter => parameter.id === 5)
    const extraCurrencyParameter = parsed.parameters.find(parameter => parameter.id === 7)
    const globalVersion = parsed.parameters.find(parameter => parameter.id === 8)
    const configVoting = parsed.parameters.find(parameter => parameter.id === 11)
    const storagePrices = parsed.parameters.find(parameter => parameter.id === 18)
    const mandatory = parsed.parameters.find(parameter => parameter.id === 9)
    const critical = parsed.parameters.find(parameter => parameter.id === 10)
    const complaintPricing = parsed.parameters.find(parameter => parameter.id === 13)
    const blockCreateFees = parsed.parameters.find(parameter => parameter.id === 14)
    const electionTiming = parsed.parameters.find(parameter => parameter.id === 15)
    const validatorLimits = parsed.parameters.find(parameter => parameter.id === 16)
    const stakeLimits = parsed.parameters.find(parameter => parameter.id === 17)
    const globalId = parsed.parameters.find(parameter => parameter.id === 19)
    const masterchainGasPrices = parsed.parameters.find(parameter => parameter.id === 20)
    const workchainGasPrices = parsed.parameters.find(parameter => parameter.id === 21)
    const masterchainBlockLimits = parsed.parameters.find(parameter => parameter.id === 22)
    const blockLimits = parsed.parameters.find(parameter => parameter.id === 23)
    const masterchainForwardPrices = parsed.parameters.find(parameter => parameter.id === 24)
    const forwardPrices = parsed.parameters.find(parameter => parameter.id === 25)
    const catchain = parsed.parameters.find(parameter => parameter.id === 28)
    const consensus = parsed.parameters.find(parameter => parameter.id === 29)
    const previousValidatorSetParameter = parsed.parameters.find(parameter => parameter.id === 32)
    const currentValidatorSetParameter = parsed.parameters.find(parameter => parameter.id === 34)
    const sizeLimitsParameter = parsed.parameters.find(parameter => parameter.id === 43)
    const suspendedAddressParameter = parsed.parameters.find(parameter => parameter.id === 44)
    const precompiledParameter = parsed.parameters.find(parameter => parameter.id === 45)
    const oracleBridgeParameter = parsed.parameters.find(parameter => parameter.id === 71)
    const jettonBridgeParameter = parsed.parameters.find(parameter => parameter.id === 79)
    const fundamental = parsed.parameters.find(parameter => parameter.id === 31)
    const extension = parsed.parameters.find(parameter => parameter.id === -1)

    expect({
      address: parsed.configAddress,
      ids: parsed.parameters.map(parameter => parameter.id),
      configAddressParameter: configAddressParameter?.address,
      burning: burning?.burningConfiguration,
      extraCurrencies: extraCurrencyParameter?.extraCurrencies,
      globalVersion: globalVersion?.globalVersion,
      configVoting: configVoting?.configurationValues,
      storagePrices: storagePrices?.configurationValues,
      mandatory: mandatory?.parameterIds,
      critical: critical?.parameterIds,
      complaintPricing: complaintPricing?.configurationValues,
      blockCreateFees: blockCreateFees?.configurationValues,
      electionTiming: electionTiming?.configurationValues,
      validatorLimits: validatorLimits?.configurationValues,
      stakeLimits: stakeLimits?.configurationValues,
      globalId: globalId?.globalId,
      masterchainGasPrices: masterchainGasPrices?.configurationValues,
      workchainGasPrices: workchainGasPrices?.configurationValues,
      masterchainBlockLimits: masterchainBlockLimits?.configurationValues,
      blockLimits: blockLimits?.configurationValues,
      masterchainForwardPrices: masterchainForwardPrices?.configurationValues,
      forwardPrices: forwardPrices?.configurationValues,
      catchain: catchain?.configurationValues,
      consensus: consensus?.configurationValues,
      previousValidatorSet: previousValidatorSetParameter?.validatorSet,
      currentValidatorSet: currentValidatorSetParameter?.validatorSet,
      sizeLimits: sizeLimitsParameter?.configurationValues,
      suspendedAddresses: suspendedAddressParameter?.suspendedAddresses,
      precompiledContracts: precompiledParameter?.precompiledContracts,
      oracleBridge: oracleBridgeParameter?.bridgeConfiguration,
      jettonBridge: jettonBridgeParameter?.bridgeConfiguration,
      fundamental: fundamental?.fundamentalSmartContracts,
      version: version?.parsedValue,
      extension: extension
        ? {
            title: extension.title,
            description: extension.description,
            hasRawHex: extension.rawHex.length > 0,
          }
        : undefined,
      hasRootRawHex: parsed.rawHex.length > 0,
    }).toMatchSnapshot()
  })
})
