import {describe, expect, test} from "bun:test"
import {ExternalAddress} from "@ton/core"
import {DynamicCtx, type ContractABI} from "@ton/tolk-abi-to-typescript"

import {formatAbiDecodedValue} from "../src/components/AbiViewer/abiDecodedValue"

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

describe("ABI decoded value", () => {
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
