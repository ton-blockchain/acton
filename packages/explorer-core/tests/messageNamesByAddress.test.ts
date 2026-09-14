import {expect, test} from "bun:test"

import type {V3Message, V3TransactionListItem} from "../src/api/types"
import {
  collectTransactionListAddresses,
  collectTransactionListAbiAddresses,
} from "../src/hooks/useMessageNamesByAddress"

const message = ({
  source,
  destination,
  opcode,
}: {
  readonly source?: string
  readonly destination?: string
  readonly opcode?: string | null
}): V3Message => ({source, destination, opcode}) as V3Message

test("transaction history ignores undisplayed bulk-send recipients", () => {
  const transaction = {
    account: "account",
    in_msg: message({source: "sender", destination: "account"}),
    out_msgs: [
      message({source: "account", destination: "first-recipient"}),
      ...Array.from({length: 249}, (_, index) =>
        message({source: "account", destination: `hidden-recipient-${index + 1}`}),
      ),
    ],
  } as V3TransactionListItem

  expect(collectTransactionListAddresses([transaction])).toEqual([
    "account",
    "sender",
    "first-recipient",
  ])
})

test.each([
  undefined,
  null,
  "",
  " ",
  "invalid",
  "0x100000000",
])("messages without a valid opcode (%s) need address names but no ABI lookups", opcode => {
  const transaction = {
    account: "account",
    in_msg: message({source: "sender", destination: "account", opcode}),
    out_msgs: [message({source: "account", destination: "recipient", opcode})],
  } as V3TransactionListItem

  expect(collectTransactionListAbiAddresses([transaction])).toEqual([])
  expect(collectTransactionListAddresses([transaction])).toEqual(["account", "sender", "recipient"])
})

test("ABI lookups include only endpoints of messages with opcodes, including zero", () => {
  const transactions = [
    {
      account: "account",
      in_msg: message({source: "sender", destination: "account", opcode: "0x12345678"}),
      out_msgs: [
        message({source: "account", destination: "no-opcode-recipient"}),
        message({source: "account", destination: "recipient", opcode: "0"}),
        message({source: "account", destination: "recipient", opcode: "0x76543210"}),
        message({source: "external-out-sender", opcode: "0x12345678"}),
      ],
    },
    {account: "system-account", in_msg: null, out_msgs: []},
  ] as V3TransactionListItem[]

  expect(collectTransactionListAbiAddresses(transactions)).toEqual([
    "account",
    "sender",
    "recipient",
    "external-out-sender",
  ])
})
