import {describe, expect, test} from "bun:test"

import type {V3MultisigOrder} from "../src/api/types"
import {compareOrdersDescending} from "../src/components/multisig-details"

describe("multisig order sorting", () => {
  test("sorts orders by expiration date descending instead of order seqno", () => {
    const orders = [
      createOrder("1758625901954", 1_762_949_154),
      createOrder("2026062011", 1_753_011_805),
      createOrder("2026042701", 1_779_877_079),
    ]

    expect([...orders].sort(compareOrdersDescending)).toEqual([orders[2], orders[0], orders[1]])
  })

  test("keeps orders without an expiration date last", () => {
    const orders = [createOrder("2", null), createOrder("1", 1_800_000_000)]

    expect([...orders].sort(compareOrdersDescending)).toEqual([orders[1], orders[0]])
  })
})

function createOrder(orderSeqno: string, expirationDate: number | null): V3MultisigOrder {
  return {
    address: `0:ORDER_${orderSeqno}`,
    multisig_address: "0:WALLET",
    order_seqno: orderSeqno,
    threshold: 2,
    sent_for_execution: true,
    approvals_mask: "3",
    approvals_num: 2,
    expiration_date: expirationDate,
    order_boc: null,
    signers: [],
    last_transaction_lt: "1",
    code_hash: null,
    data_hash: null,
    actions: [],
  }
}
