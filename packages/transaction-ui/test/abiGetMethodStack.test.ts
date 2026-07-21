import {describe, expect, test} from "bun:test"
import {Address, beginCell} from "@ton/core"
import {callGetMethodDynamic, DynamicCtx, type ContractABI} from "@ton/tolk-abi-to-typescript"

import {
  createAbiGetMethodProvider,
  type AbiGetMethodStackEntry,
} from "../src/components/AbiViewer/abiGetMethodStack"

const address = Address.parse("EQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAM9c")

const abi = {
  contract_name: "Pool",
  compiler_name: "tolk",
  compiler_version: "1.4.2",
  declarations: [],
  unique_types: [
    {kind: "void"},
    {kind: "int"},
    {kind: "slice"},
    {kind: "cell"},
    {kind: "builder"},
    {kind: "bool"},
    {kind: "coins"},
    {kind: "address"},
  ],
  struct_instantiations: [],
  alias_instantiations: [],
  storage: {},
  incoming_messages: [],
  incoming_external: [],
  outgoing_messages: [],
  emitted_events: [],
  get_methods: [
    {
      name: "get_wallet_address",
      parameters: [{name: "ownerAddress", ty_idx: 7}],
      return_ty_idx: 7,
      tvm_method_id: 97_036,
    },
  ],
  thrown_errors: [],
} satisfies ContractABI

describe("ABI get-method stack", () => {
  test("accepts a Toncenter cell result for an ABI address", async () => {
    const ctx = new DynamicCtx(abi)
    let requestStack: readonly AbiGetMethodStackEntry[] = []
    const provider = createAbiGetMethodProvider(
      async (_method, stack) => {
        requestStack = stack
        return {
          gas_used: 6093,
          exit_code: 0,
          stack: [
            {
              type: "cell",
              value: beginCell().storeAddress(address).endCell().toBoc().toString("base64"),
            },
          ],
          vm_log: "",
        }
      },
      () => undefined,
      {symbols: ctx.symbols, returnTyIdx: 7},
    )

    const result = await callGetMethodDynamic(provider, ctx, "get_wallet_address", [address])

    expect(requestStack).toMatchSnapshot()
    expect((result as Address).toString()).toBe(address.toString())
  })

  test("reports a non-zero exit before decoding its result stack", async () => {
    const ctx = new DynamicCtx(abi)
    const provider = createAbiGetMethodProvider(
      async () => ({
        gas_used: 3943,
        exit_code: 292,
        stack: [{type: "num", value: "0x0"}],
      }),
      () => undefined,
      {symbols: ctx.symbols, returnTyIdx: 7},
    )

    expect(callGetMethodDynamic(provider, ctx, "get_wallet_address", [address])).rejects.toThrow(
      "Get method exited with code 292.",
    )
  })

  test("decodes Toncenter list entries as tuples without inventing a linked list", async () => {
    const provider = createAbiGetMethodProvider(
      async () => ({
        gas_used: 1,
        exit_code: 0,
        stack: [
          {
            type: "list",
            value: [
              {type: "num", value: "7"},
              {type: "num", value: "8"},
            ],
          },
          {type: "list", value: []},
        ],
      }),
      () => undefined,
    )

    const result = await provider.get("list_result", [])

    expect(result.stack.pop()).toEqual({
      type: "tuple",
      items: [
        {type: "int", value: 7n},
        {type: "int", value: 8n},
      ],
    })
    expect(result.stack.pop()).toEqual({type: "null"})
  })

  test("normalizes a cell-like value at the aligned offset of a wide union", async () => {
    const unionTyIdx = 10
    const unionAbi = {
      ...abi,
      unique_types: [
        ...abi.unique_types,
        {kind: "nullLiteral"},
        {kind: "tensor", items_ty_idx: [1, 1]},
        {
          kind: "union",
          variants: [
            {
              variant_ty_idx: 8,
              prefix_num: 0,
              prefix_len: 2,
              is_prefix_implicit: null,
              stack_type_id: 130,
              stack_width: 0,
            },
            {
              variant_ty_idx: 9,
              prefix_num: 1,
              prefix_len: 2,
              is_prefix_implicit: null,
              stack_type_id: 131,
              stack_width: 2,
            },
            {
              variant_ty_idx: 7,
              prefix_num: 2,
              prefix_len: 2,
              is_prefix_implicit: null,
              stack_type_id: 132,
              stack_width: 1,
            },
          ],
          stack_width: 3,
        },
      ],
    } satisfies ContractABI
    const ctx = new DynamicCtx(unionAbi)
    const provider = createAbiGetMethodProvider(
      async () => ({
        gas_used: 1,
        exit_code: 0,
        stack: [
          {type: "list", value: []},
          {
            type: "cell",
            value: beginCell().storeAddress(address).endCell().toBoc().toString("base64"),
          },
          {type: "num", value: "132"},
        ],
      }),
      () => undefined,
      {symbols: ctx.symbols, returnTyIdx: unionTyIdx},
    )

    const result = await provider.get("union_result", [])

    expect(result.stack.pop()).toEqual({type: "null"})
    expect(result.stack.pop().type).toBe("slice")
    expect(result.stack.pop()).toEqual({type: "int", value: 132n})
  })

  test("rejects numeric stack values that would lose precision", async () => {
    const provider = createAbiGetMethodProvider(
      async () => ({
        gas_used: 1,
        exit_code: 0,
        stack: [{type: "num", value: 1.5}],
      }),
      () => undefined,
    )

    expect(provider.get("unsafe_number", [])).rejects.toThrow(
      "Numeric stack value must be an integer or hex string.",
    )
  })
})
