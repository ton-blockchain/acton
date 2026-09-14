import {describe, expect, test} from "bun:test"
import {ExternalAddress} from "@ton/core"
import {callGetMethodDynamic, DynamicCtx, type ContractABI} from "@ton/tolk-abi-to-typescript"

import {formatAbiDecodedValue} from "../src/components/AbiViewer/abiDecodedValue"
import {
  createAbiGetMethodProvider,
  type AbiGetMethodStackEntry,
} from "../src/components/AbiViewer/abiGetMethodStack"

const replyTyIdx = 8

const abi = {
  contract_name: "Pool",
  compiler_name: "tolk",
  compiler_version: "1.4.2",
  declarations: [
    {
      kind: "struct",
      name: "GetTradeFeeReply",
      ty_idx: replyTyIdx,
      fields: [
        {name: "tradeFeeNumerator", ty_idx: 1},
        {name: "tradeFeeDenominator", ty_idx: 1},
        {name: "numericLabel", ty_idx: 9},
        {name: "externalAddress", ty_idx: 10},
      ],
    },
  ],
  unique_types: [
    {kind: "void"},
    {kind: "int"},
    {kind: "slice"},
    {kind: "cell"},
    {kind: "builder"},
    {kind: "bool"},
    {kind: "coins"},
    {kind: "address"},
    {kind: "StructRef", struct_name: "GetTradeFeeReply"},
    {kind: "string"},
    {kind: "addressExt"},
  ],
  struct_instantiations: [],
  alias_instantiations: [],
  storage: {},
  incoming_messages: [],
  incoming_external: [],
  outgoing_messages: [],
  emitted_events: [],
  get_methods: [],
  thrown_errors: [],
} satisfies ContractABI

const enumAbi = {
  ...abi,
  declarations: [
    ...abi.declarations,
    {
      kind: "enum",
      name: "Status",
      ty_idx: 11,
      encoded_as_ty_idx: 1,
      members: [
        {name: "Idle", value: "0"},
        {name: "Active", value: "1"},
        {name: "Failed", value: "-1"},
        {name: "Large", value: "9007199254740993"},
      ],
    },
    {kind: "alias", name: "StatusAlias", ty_idx: 13, target_ty_idx: 12},
    {
      kind: "struct",
      name: "StatusReply",
      ty_idx: 17,
      fields: [
        {name: "status", ty_idx: 13},
        {name: "count", ty_idx: 1},
      ],
    },
  ],
  unique_types: [
    ...abi.unique_types,
    {kind: "EnumRef", enum_name: "Status"},
    {kind: "nullable", inner_ty_idx: 11},
    {kind: "AliasRef", alias_name: "StatusAlias"},
    {kind: "arrayOf", inner_ty_idx: 11},
    {kind: "shapedTuple", items_ty_idx: [11, 1, 13]},
    {kind: "tensor", items_ty_idx: [11, 1, 13]},
    {kind: "StructRef", struct_name: "StatusReply"},
  ],
} satisfies ContractABI

describe("ABI decoded value", () => {
  test.each<{
    name: string
    returnTyIdx: number
    stack: readonly AbiGetMethodStackEntry[]
  }>([
    {name: "enum member", returnTyIdx: 11, stack: [{type: "num", value: "0x1"}]},
    {name: "zero enum member", returnTyIdx: 11, stack: [{type: "num", value: "0"}]},
    {name: "negative enum member", returnTyIdx: 11, stack: [{type: "num", value: "-0x1"}]},
    {
      name: "large enum member",
      returnTyIdx: 11,
      stack: [{type: "num", value: "9007199254740993"}],
    },
    {name: "unknown enum value", returnTyIdx: 11, stack: [{type: "num", value: "42"}]},
    {name: "nullable enum", returnTyIdx: 12, stack: [{type: "num", value: "1"}]},
    {name: "null enum", returnTyIdx: 12, stack: [{type: "null", value: null}]},
    {name: "nullable enum alias", returnTyIdx: 13, stack: [{type: "num", value: "1"}]},
    {name: "ordinary integer", returnTyIdx: 1, stack: [{type: "num", value: "1"}]},
    {
      name: "enum array",
      returnTyIdx: 14,
      stack: [
        {
          type: "tuple",
          value: [
            {type: "num", value: "0"},
            {type: "num", value: "1"},
          ],
        },
      ],
    },
    {
      name: "enum tuple",
      returnTyIdx: 15,
      stack: [
        {
          type: "tuple",
          value: [
            {type: "num", value: "1"},
            {type: "num", value: "1"},
            {type: "num", value: "0"},
          ],
        },
      ],
    },
    {
      name: "enum tensor",
      returnTyIdx: 16,
      stack: [
        {type: "num", value: "1"},
        {type: "num", value: "1"},
        {type: "num", value: "0"},
      ],
    },
    {
      name: "enum struct field",
      returnTyIdx: 17,
      stack: [
        {type: "num", value: "1"},
        {type: "num", value: "1"},
      ],
    },
  ])("formats a get-method result with $name", async ({returnTyIdx, stack}) => {
    const ctx = new DynamicCtx({
      ...enumAbi,
      get_methods: [
        {name: "get_status", parameters: [], return_ty_idx: returnTyIdx, tvm_method_id: 100_001},
      ],
    })
    const provider = createAbiGetMethodProvider(
      async () => ({gas_used: 1, exit_code: 0, stack}),
      () => undefined,
      {symbols: ctx.symbols, returnTyIdx},
    )
    const decoded: unknown = await callGetMethodDynamic(provider, ctx, "get_status", [])

    expect(formatAbiDecodedValue(decoded, ctx.symbols, returnTyIdx).value).toMatchSnapshot()
  })

  test("renders a decoded struct as Tolk initialization", () => {
    const symbols = new DynamicCtx(abi).symbols

    expect(
      formatAbiDecodedValue(
        {
          $: "GetTradeFeeReply",
          tradeFeeNumerator: 100n,
          tradeFeeDenominator: 10_000n,
          numericLabel: "123",
          externalAddress: new ExternalAddress(15n, 4),
        },
        symbols,
        replyTyIdx,
      ).value,
    ).toMatchSnapshot()
  })
})
