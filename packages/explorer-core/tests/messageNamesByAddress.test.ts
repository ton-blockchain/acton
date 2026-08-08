import {expect, test} from "bun:test"

import type {V3Message, V3TransactionListItem} from "../src/api/types"
import {collectTransactionListAddresses} from "../src/hooks/useMessageNamesByAddress"

const message = ({
  source,
  destination,
  opcode,
}: {
  readonly source?: string
  readonly destination?: string
  readonly opcode?: string
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
