import {describe, expect, test} from "bun:test"
import {beginCell, Dictionary, type Builder} from "@ton/core"

import {parseConfigParameter} from "../src/api/config"
import {
  storeBlockLimits,
  storeCatchainConfig,
  storeConsensusConfig,
  storeGasLimitsPrices,
  storeJettonBridgeParams,
  storeNewConsensusConfigAll,
  storeSizeLimitsConfig,
  storeValidatorSet,
  storeWorkchainDescr,
  type BlockLimits,
  type CatchainConfig,
  type ConsensusConfig,
  type GasLimitsPrices,
  type JettonBridgeParams,
  type NewConsensusConfig,
  type NewConsensusConfigAll,
  type ParamLimits,
  type SizeLimitsConfig,
  type ValidatorDescr,
  type ValidatorSet,
  type WorkchainDescr,
} from "../src/cell-inspector/block.tlb.generated"

function parseVersions<T>(
  ids: readonly number[],
  value: T,
  store: (value: T) => (builder: Builder) => void,
) {
  const cell = beginCell().store(store(value)).endCell()
  return ids.map(id => parseConfigParameter(id, cell))
}

describe("historical network configuration constructors", () => {
  test("gas prices in parameters 20 and 21", () => {
    const versions: readonly GasLimitsPrices[] = [
      {
        kind: "GasLimitsPrices_gas_prices",
        gas_price: 1n,
        gas_limit: 2n,
        gas_credit: 3n,
        block_gas_limit: 4n,
        freeze_due_limit: 5n,
        delete_due_limit: 6n,
      },
      {
        kind: "GasLimitsPrices_gas_prices_ext",
        gas_price: 11n,
        gas_limit: 12n,
        special_gas_limit: 13n,
        gas_credit: 14n,
        block_gas_limit: 15n,
        freeze_due_limit: 16n,
        delete_due_limit: 17n,
      },
      {
        kind: "GasLimitsPrices_gas_flat_pfx",
        flat_gas_limit: 21n,
        flat_gas_price: 22n,
        other: {
          kind: "GasLimitsPrices_gas_prices_ext",
          gas_price: 23n,
          gas_limit: 24n,
          special_gas_limit: 25n,
          gas_credit: 26n,
          block_gas_limit: 27n,
          freeze_due_limit: 28n,
          delete_due_limit: 29n,
        },
      },
    ]
    for (const version of versions) {
      const parameters = parseVersions([20, 21], version, storeGasLimitsPrices)
      const values = parameters[0]?.configurationValues
      expect(values).toBeDefined()
      for (const parameter of parameters) {
        expect(parameter.parseError).toBeUndefined()
        expect(parameter.parsedValue).toBeDefined()
        expect(parameter.configurationValues).toEqual(values)
      }
      expect({rawHex: parameters[0]?.rawHex, values}).toMatchSnapshot(version.kind)
    }
  })

  test("block limits in parameters 22 and 23", () => {
    const limits = (start: number): ParamLimits => ({
      kind: "ParamLimits",
      underload: start,
      soft_limit: start + 1,
      hard_limit: start + 2,
    })
    const versions: readonly BlockLimits[] = [
      {
        kind: "BlockLimits_block_limits",
        bytes: limits(1),
        gas: limits(4),
        lt_delta: limits(7),
      },
      {
        kind: "BlockLimits_block_limits_v2",
        bytes: limits(11),
        gas: limits(14),
        lt_delta: limits(17),
        collated_data: limits(20),
        imported_msg_queue: {kind: "ImportedMsgQueueLimits", max_bytes: 23, max_msgs: 24},
      },
    ]
    for (const version of versions) {
      const parameters = parseVersions([22, 23], version, storeBlockLimits)
      const values = parameters[0]?.configurationValues
      expect(values).toBeDefined()
      for (const parameter of parameters) {
        expect(parameter.parseError).toBeUndefined()
        expect(parameter.parsedValue).toBeDefined()
        expect(parameter.configurationValues).toEqual(values)
      }
      expect({rawHex: parameters[0]?.rawHex, values}).toMatchSnapshot(version.kind)
    }
  })

  test("catchain configuration in parameter 28", () => {
    const versions: readonly CatchainConfig[] = [
      {
        kind: "CatchainConfig_catchain_config",
        mc_catchain_lifetime: 1,
        shard_catchain_lifetime: 2,
        shard_validators_lifetime: 3,
        shard_validators_num: 4,
      },
      {
        kind: "CatchainConfig_catchain_config_new",
        flags: 0,
        shuffle_mc_validators: {kind: "Bool", value: true},
        mc_catchain_lifetime: 5,
        shard_catchain_lifetime: 6,
        shard_validators_lifetime: 7,
        shard_validators_num: 8,
      },
    ]
    for (const version of versions) {
      const [parameter] = parseVersions([28], version, storeCatchainConfig)
      expect(parameter?.parseError).toBeUndefined()
      expect(parameter?.parsedValue).toBeDefined()
      expect({rawHex: parameter?.rawHex, values: parameter?.configurationValues}).toMatchSnapshot(
        version.kind,
      )
    }
  })

  test("consensus configuration in parameter 29", () => {
    const common = {
      round_candidates: 1,
      next_candidate_delay_ms: 2,
      consensus_timeout_ms: 3,
      fast_attempts: 4,
      attempt_duration: 5,
      catchain_max_deps: 6,
      max_block_bytes: 7,
      max_collated_bytes: 8,
    }
    const versions: readonly ConsensusConfig[] = [
      {kind: "ConsensusConfig_consensus_config", ...common},
      {
        kind: "ConsensusConfig_consensus_config_new",
        ...common,
        flags: 0,
        new_catchain_ids: {kind: "Bool", value: true},
      },
      {
        kind: "ConsensusConfig_consensus_config_v3",
        ...common,
        flags: 0,
        new_catchain_ids: {kind: "Bool", value: true},
        proto_version: 9,
      },
      {
        kind: "ConsensusConfig_consensus_config_v4",
        ...common,
        flags: 0,
        use_quic: {kind: "Bool", value: true},
        new_catchain_ids: {kind: "Bool", value: true},
        proto_version: 10,
        catchain_max_blocks_coeff: 11,
      },
    ]
    for (const version of versions) {
      const [parameter] = parseVersions([29], version, storeConsensusConfig)
      expect(parameter?.parseError).toBeUndefined()
      expect(parameter?.parsedValue).toBeDefined()
      expect({rawHex: parameter?.rawHex, values: parameter?.configurationValues}).toMatchSnapshot(
        version.kind,
      )
    }
  })

  test("size limits in parameter 43", () => {
    const common = {
      max_msg_bits: 1,
      max_msg_cells: 2,
      max_library_cells: 3,
      max_vm_data_depth: 4,
      max_ext_msg_size: 5,
      max_ext_msg_depth: 6,
    }
    const v2 = {
      ...common,
      max_acc_state_cells: 7,
      max_mc_acc_state_cells: 8,
      max_acc_public_libraries: 9,
      defer_out_queue_size_limit: 10,
      max_msg_extra_currencies: 11,
      max_acc_fixed_prefix_length: 12,
      acc_state_cells_for_storage_dict: 13,
      max_transaction_library_loads: {kind: "Maybe_just" as const, value: 14},
    }
    const versions: readonly SizeLimitsConfig[] = [
      {kind: "SizeLimitsConfig_size_limits_config", ...common},
      {kind: "SizeLimitsConfig_size_limits_config_v2", ...v2},
      {
        kind: "SizeLimitsConfig_size_limits_config_v3",
        ...v2,
        max_total_msg_bits: 15,
        max_total_msg_cells: 16,
        out_msg_queue_size_hard_limit: 17,
        out_msg_queue_size_soft_limit: 18,
      },
    ]
    for (const version of versions) {
      const [parameter] = parseVersions([43], version, storeSizeLimitsConfig)
      expect(parameter?.parseError).toBeUndefined()
      expect(parameter?.parsedValue).toBeDefined()
      expect({rawHex: parameter?.rawHex, values: parameter?.configurationValues}).toMatchSnapshot(
        version.kind,
      )
    }
  })

  test("validator sets in parameters 32, 34 and 36", () => {
    const list = Dictionary.empty<number, ValidatorDescr>()
    list.set(0, {
      kind: "ValidatorDescr_validator_addr",
      public_key: {kind: "SigPubKey", pubkey: Buffer.alloc(32, 0x11)},
      weight: 100n,
      adnl_addr: Buffer.alloc(32, 0x22),
    })
    const common = {utime_since: 1, utime_until: 2, total: 1, main: 1, list}
    const versions: readonly ValidatorSet[] = [
      {kind: "ValidatorSet_validators", ...common},
      {kind: "ValidatorSet_validators_ext", ...common, total_weight: 100n},
    ]
    for (const version of versions) {
      const parameters = parseVersions([32, 34, 36], version, storeValidatorSet)
      const validatorSet = parameters[0]?.validatorSet
      expect(validatorSet).toBeDefined()
      for (const parameter of parameters) {
        expect(parameter.parseError).toBeUndefined()
        expect(parameter.parsedValue).toBeDefined()
        expect(parameter.validatorSet).toEqual(validatorSet)
      }
      expect({rawHex: parameters[0]?.rawHex, validatorSet}).toMatchSnapshot(version.kind)
    }
  })

  test.each([null, Buffer.alloc(32), Buffer.from(`00${"ff".repeat(31)}`, "hex")])(
    "validator registry constructors in parameter 46 with code hash %p",
    newCodeHash => {
      const legacy = beginCell()
        .storeUint(0x36_01_16_3e, 32)
        .storeBuffer(Buffer.alloc(32, 0xe1))
        .storeUint(8, 32)
        .storeMaybeBuffer(newCodeHash, 32)
        .endCell()
      const legacyParameter = parseConfigParameter(46, legacy)
      expect(legacyParameter.parseError).toBeUndefined()
      expect(legacyParameter.validatorRegistry).toEqual({
        contractAddress: `-1:${"e1".repeat(32)}`,
        maxCollatorsPerValidator: 8,
        ...(newCodeHash === null ? {} : {newCodeHash: `0x${newCodeHash.toString("hex")}`}),
      })

      const roles = Dictionary.empty<number, readonly [number, number, number]>()
      roles.set(0, [1, 10_000, 5])
      roles.set(31, [10_000, 1, 0xff_ff_ff_ff])
      const current = beginCell()
        .storeUint(0x36_01_16_3f, 32)
        .storeBuffer(Buffer.alloc(32, 0xe1))
        .storeMaybeBuffer(newCodeHash, 32)
        .storeDict(roles, Dictionary.Keys.Uint(5), {
          serialize: ([add, remove, maxNodes], builder) =>
            builder
              .storeUint(0xe1_9f_2d_29, 32)
              .storeUint(add, 16)
              .storeUint(remove, 16)
              .storeUint(maxNodes, 32),
          parse: () => {
            throw new Error("not used in test")
          },
        })
        .storeUint(0xff_ff_ff_ff, 32)
        .storeCoins(9_007_199_254_740_993n)
        .storeUint(2, 32)
        .endCell()
      const currentParameter = parseConfigParameter(46, current)
      expect(currentParameter.parseError).toBeUndefined()
      // The new constructor uses the generic view so all role and voting fields remain visible.
      expect(currentParameter.validatorRegistry).toBeUndefined()
      expect(currentParameter.parsedValue).toMatchObject({
        kind: "object",
        entries: [
          {
            key: "Contract address",
            value: {kind: "address", value: `-1:${"e1".repeat(32)}`},
          },
          {
            key: "New code hash",
            value: {
              kind: "object",
              entries:
                newCodeHash === null
                  ? []
                  : [{key: "Value", value: {value: `0x${newCodeHash.toString("hex")}`}}],
            },
          },
          {
            key: "Roles",
            value: {
              kind: "map",
              entries: [
                {
                  key: {value: "0"},
                  value: {
                    entries: [
                      {key: "Threshold add", value: {value: "1"}},
                      {key: "Threshold remove", value: {value: "10000"}},
                      {key: "Max nodes", value: {value: "5"}},
                    ],
                  },
                },
                {
                  key: {value: "31"},
                  value: {
                    entries: [
                      {key: "Threshold add", value: {value: "10000"}},
                      {key: "Threshold remove", value: {value: "1"}},
                      {key: "Max nodes", value: {value: "4294967295"}},
                    ],
                  },
                },
              ],
            },
          },
          {key: "Voting timeout", value: {value: "4294967295"}},
          {key: "Voting price", value: {value: "9007199254740993"}},
          {key: "Node update period", value: {value: "2"}},
        ],
      })
    },
  )

  test("jetton bridges in parameters 79, 81 and 82", () => {
    const oracles = Dictionary.empty<bigint, bigint>()
    oracles.set(0x11n, 0x22n)
    const common = {
      bridge_address: Buffer.alloc(32, 0x33),
      oracles_address: Buffer.alloc(32, 0x44),
      oracles,
      state_flags: 1,
    }
    const versions: readonly JettonBridgeParams[] = [
      {kind: "JettonBridgeParams_jetton_bridge_params_v0", ...common, burn_bridge_fee: 5n},
      {
        kind: "JettonBridgeParams_jetton_bridge_params_v1",
        ...common,
        prices: {
          kind: "JettonBridgePrices",
          bridge_burn_fee: 6n,
          bridge_mint_fee: 7n,
          wallet_min_tons_for_storage: 8n,
          wallet_gas_consumption: 9n,
          minter_min_tons_for_storage: 10n,
          discover_gas_consumption: 11n,
        },
        external_chain_address: Buffer.concat([Buffer.alloc(12), Buffer.alloc(20, 0x55)]),
      },
    ]
    for (const version of versions) {
      const parameters = parseVersions([79, 81, 82], version, storeJettonBridgeParams)
      for (const parameter of parameters) {
        expect(parameter.parseError).toBeUndefined()
        expect(parameter.parsedValue).toBeDefined()
        expect(parameter.bridgeConfiguration).toBeDefined()
      }
      expect({
        rawHex: parameters[0]?.rawHex,
        bridges: parameters.map(parameter => parameter.bridgeConfiguration),
      }).toMatchSnapshot(version.kind)
    }
  })

  test("workchain descriptions inside parameter 12", () => {
    const common = {
      enabled_since: 1,
      monitor_min_split: 0,
      min_split: 0,
      max_split: 1,
      basic: 1,
      active: {kind: "Bool" as const, value: true},
      accept_msgs: {kind: "Bool" as const, value: true},
      flags: 0,
      zerostate_root_hash: Buffer.alloc(32, 0x11),
      zerostate_file_hash: Buffer.alloc(32, 0x22),
      version: 1,
      format: {kind: "WorkchainFormat_wfmt_basic" as const, vm_version: 1, vm_mode: 0n},
    }
    const versions: readonly WorkchainDescr[] = [
      {kind: "WorkchainDescr_workchain", ...common},
      {
        kind: "WorkchainDescr_workchain_v2",
        ...common,
        split_merge_timings: {
          kind: "WcSplitMergeTimings",
          split_merge_delay: 2,
          split_merge_interval: 3,
          min_split_merge_interval: 4,
          max_split_merge_delay: 5,
        },
        persistent_state_split_depth: 6,
      },
    ]
    for (const version of versions) {
      const workchains = Dictionary.empty<number, WorkchainDescr>()
      workchains.set(0, version)
      const cell = beginCell()
        .storeDict(workchains, Dictionary.Keys.Uint(32), {
          serialize: (value, builder) => storeWorkchainDescr(value)(builder),
          parse: () => {
            throw new Error("not used in test")
          },
        })
        .endCell()
      const parameter = parseConfigParameter(12, cell)
      expect(parameter.parseError).toBeUndefined()
      expect({rawHex: parameter.rawHex, parsedValue: parameter.parsedValue}).toMatchSnapshot(
        version.kind,
      )
    }
  })

  test("simplex versions inside parameter 30", () => {
    const versions: readonly NewConsensusConfig[] = [
      {
        kind: "NewConsensusConfig_simplex_config",
        flags: 0,
        use_quic: {kind: "Bool", value: true},
        target_rate_ms: 1,
        slots_per_leader_window: 2,
        first_block_timeout_ms: 3,
        max_leader_window_desync: 4,
      },
      {
        kind: "NewConsensusConfig_simplex_config_v2",
        flags: 0,
        protocol_version: 1,
        use_quic: {kind: "Bool", value: true},
        slots_per_leader_window: 2,
        noncritical_params: Dictionary.empty<number, number>(),
      },
    ]
    for (const version of versions) {
      const config: NewConsensusConfigAll = {
        kind: "NewConsensusConfigAll",
        mc: {kind: "Maybe_just", value: version},
        shard: {kind: "Maybe_nothing"},
      }
      const [parameter] = parseVersions([30], config, storeNewConsensusConfigAll)
      expect(parameter?.parseError).toBeUndefined()
      expect({rawHex: parameter?.rawHex, parsedValue: parameter?.parsedValue}).toMatchSnapshot(
        version.kind,
      )
    }
  })
})
