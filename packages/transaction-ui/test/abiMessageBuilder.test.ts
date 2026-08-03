import {describe, expect, test} from "bun:test"
import {Cell, loadMessage, type Message} from "@ton/core"

import {
  SAMPLE_ADDRESS,
  abiValueToFormValue,
  buildAbiMessageBoc,
  buildEmptyMessageBoc,
  decodeAbiMessageBuilderDraft,
  decodeAbiValueFromCell,
  formatAbiMessageOptionSummary,
  listAbiMessageBuilderOptions,
  type AbiMessageBuilderOption,
  type ContractABI,
} from "../src/abi"

const DESTINATION_ADDRESS = "EQAREREREREREREREREREREREREREREREREREREREREREeYT"

const messageAbi = {
  alias_instantiations: [],
  compiler_name: "tolk",
  compiler_version: "test",
  contract_name: "MessageBuilder",
  declarations: [
    {
      fields: [{name: "value", ty_idx: 8}],
      kind: "struct",
      name: "Increment",
      prefix: {prefix_len: 32, prefix_num: 1},
      ty_idx: 12,
    },
  ],
  emitted_events: [],
  get_methods: [],
  incoming_external: [{body_ty_idx: 12}],
  incoming_messages: [{body_ty_idx: 12}],
  outgoing_messages: [{body_ty_idx: 12}],
  storage: {},
  struct_instantiations: [],
  thrown_errors: [],
  unique_types: [
    {kind: "void"},
    {kind: "int"},
    {kind: "slice"},
    {kind: "cell"},
    {kind: "builder"},
    {kind: "bool"},
    {kind: "coins"},
    {kind: "address"},
    {kind: "intN", n: 32},
    {kind: "uintN", n: 32},
    {kind: "intN", n: 64},
    {kind: "uintN", n: 64},
    {kind: "StructRef", struct_name: "Increment"},
    {kind: "void"},
    {kind: "nullLiteral"},
    {
      kind: "union",
      variants: [
        {variant_ty_idx: 14, prefix_num: 0, prefix_len: 1, is_prefix_implicit: true},
        {variant_ty_idx: 12, prefix_num: 1, prefix_len: 1, is_prefix_implicit: true},
      ],
    },
  ],
} satisfies ContractABI

const unionMessageAbi = {
  ...messageAbi,
  incoming_external: [{body_ty_idx: 15}],
  incoming_messages: [{body_ty_idx: 15}],
} satisfies ContractABI

describe("ABI message builder", () => {
  test("lists internal and external message options with serializable defaults", () => {
    const internal = listAbiMessageBuilderOptions(messageAbi, "internal")
    const external = listAbiMessageBuilderOptions(messageAbi, "external")
    const outgoing = listAbiMessageBuilderOptions(messageAbi, "internal", "outgoing")
    const union = listAbiMessageBuilderOptions(unionMessageAbi, "internal")

    expect({
      internal: internal.map(summarizeOption),
      external: external.map(summarizeOption),
      outgoing: outgoing.map(summarizeOption),
      union: union.map(summarizeOption),
    }).toMatchSnapshot()
  })

  test("builds an internal message with source, value, bounce, and ABI body", () => {
    const option = requireOption(listAbiMessageBuilderOptions(messageAbi, "internal"), 0)
    const boc = buildAbiMessageBoc({
      abi: messageAbi,
      option,
      destination: DESTINATION_ADDRESS,
      source: SAMPLE_ADDRESS,
      value: "1.25",
      bounce: false,
      argsJson: '{"value":"7"}',
    })
    const message = loadMessage(Cell.fromHex(boc).beginParse())

    expect({
      boc,
      message: summarizeMessage(message, messageAbi, option.bodyTyIdx),
    }).toMatchSnapshot()
  })

  test("builds external and union messages", () => {
    const externalOption = requireOption(listAbiMessageBuilderOptions(messageAbi, "external"), 0)
    const unionOption = requireOption(listAbiMessageBuilderOptions(unionMessageAbi, "external"), 1)
    const externalBoc = buildAbiMessageBoc({
      abi: messageAbi,
      option: externalOption,
      destination: DESTINATION_ADDRESS,
      argsJson: '{"value":"11"}',
    })
    const unionBoc = buildAbiMessageBoc({
      abi: unionMessageAbi,
      option: unionOption,
      destination: DESTINATION_ADDRESS,
      argsJson: '{"value":"13"}',
    })

    expect({
      external: summarizeMessage(
        loadMessage(Cell.fromHex(externalBoc).beginParse()),
        messageAbi,
        externalOption.bodyTyIdx,
      ),
      union: summarizeMessage(
        loadMessage(Cell.fromHex(unionBoc).beginParse()),
        unionMessageAbi,
        unionOption.bodyTyIdx,
      ),
    }).toMatchSnapshot()
  })

  test("decodes ABI message bodies back into editable builder drafts", () => {
    const internalOption = requireOption(listAbiMessageBuilderOptions(messageAbi, "internal"), 0)
    const unionOption = requireOption(listAbiMessageBuilderOptions(unionMessageAbi, "external"), 1)
    const internalBoc = buildAbiMessageBoc({
      abi: messageAbi,
      option: internalOption,
      destination: DESTINATION_ADDRESS,
      source: SAMPLE_ADDRESS,
      value: "1.25",
      bounce: false,
      argsJson: '{"value":"7"}',
    })
    const unionBoc = buildAbiMessageBoc({
      abi: unionMessageAbi,
      option: unionOption,
      destination: DESTINATION_ADDRESS,
      argsJson: '{"value":"13"}',
    })
    const internalMessage = loadMessage(Cell.fromHex(internalBoc).beginParse())
    const unionMessage = loadMessage(Cell.fromHex(unionBoc).beginParse())
    const internalDraft = decodeAbiMessageBuilderDraft(messageAbi, "internal", internalMessage.body)
    const outgoingDraft = decodeAbiMessageBuilderDraft(
      messageAbi,
      "internal",
      internalMessage.body,
      "outgoing",
    )
    const unionDraft = decodeAbiMessageBuilderDraft(unionMessageAbi, "external", unionMessage.body)

    expect({
      internal: internalDraft
        ? {
            option: summarizeOption(internalDraft.option),
            args: JSON.parse(internalDraft.argsJson),
          }
        : undefined,
      outgoing: outgoingDraft
        ? {
            option: summarizeOption(outgoingDraft.option),
            args: JSON.parse(outgoingDraft.argsJson),
          }
        : undefined,
      union: unionDraft
        ? {
            option: summarizeOption(unionDraft.option),
            args: JSON.parse(unionDraft.argsJson),
          }
        : undefined,
      emptyBody: decodeAbiMessageBuilderDraft(messageAbi, "internal", Cell.EMPTY),
    }).toMatchSnapshot()
  })

  test("builds internal and external messages with empty bodies", () => {
    const internalBoc = buildEmptyMessageBoc({
      transport: "internal",
      destination: DESTINATION_ADDRESS,
      source: SAMPLE_ADDRESS,
      value: "0.25",
      bounce: false,
    })
    const externalBoc = buildEmptyMessageBoc({
      transport: "external",
      destination: DESTINATION_ADDRESS,
    })

    expect({
      internal: {
        boc: internalBoc,
        message: summarizeEmptyMessage(loadMessage(Cell.fromHex(internalBoc).beginParse())),
      },
      external: {
        boc: externalBoc,
        message: summarizeEmptyMessage(loadMessage(Cell.fromHex(externalBoc).beginParse())),
      },
    }).toMatchSnapshot()
  })

  test("rejects missing internal fields and malformed values", () => {
    const option = requireOption(listAbiMessageBuilderOptions(messageAbi, "internal"), 0)
    const base = {
      abi: messageAbi,
      option,
      destination: DESTINATION_ADDRESS,
      argsJson: '{"value":"7"}',
    }

    expect(() => buildAbiMessageBoc(base)).toThrow("Source address is required")
    expect(() => buildAbiMessageBoc({...base, source: SAMPLE_ADDRESS, value: "invalid"})).toThrow()
    expect(() => buildAbiMessageBoc({...base, source: SAMPLE_ADDRESS, argsJson: "{"})).toThrow()
  })
})

function summarizeOption(option: AbiMessageBuilderOption) {
  return {
    id: option.id,
    transport: option.transport,
    label: option.label,
    summary: formatAbiMessageOptionSummary(option),
    bodyTyIdx: option.bodyTyIdx,
    valueTyIdx: option.valueTyIdx,
    sample: JSON.parse(option.sampleJson),
    union: option.union,
  }
}

function summarizeMessage(message: Message, abi: ContractABI, bodyTyIdx: number) {
  const info =
    message.info.type === "internal"
      ? {
          type: message.info.type,
          source: message.info.src.toString(),
          destination: message.info.dest.toString(),
          value: message.info.value.coins.toString(),
          bounce: message.info.bounce,
        }
      : {
          type: message.info.type,
          destination: message.info.dest?.toString(),
        }

  return {
    info,
    body: abiValueToFormValue(decodeAbiValueFromCell(abi, bodyTyIdx, message.body)),
  }
}

function summarizeEmptyMessage(message: Message) {
  const info =
    message.info.type === "internal"
      ? {
          type: message.info.type,
          source: message.info.src.toString(),
          destination: message.info.dest.toString(),
          value: message.info.value.coins.toString(),
          bounce: message.info.bounce,
        }
      : {
          type: message.info.type,
          destination: message.info.dest?.toString(),
        }

  return {
    info,
    body: {
      bits: message.body.bits.length,
      refs: message.body.refs.length,
    },
  }
}

function requireOption(
  options: readonly AbiMessageBuilderOption[],
  index: number,
): AbiMessageBuilderOption {
  const option = options[index]
  if (!option) throw new Error(`Missing ABI message option ${index}`)
  return option
}
