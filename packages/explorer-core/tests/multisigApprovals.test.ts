import {describe, expect, test} from "bun:test"

import type {V3Action, V3MultisigOrder} from "../src/api/types"
import {
  collectMultisigApprovalTimes,
  compareMultisigApprovalTimes,
  multisigAddressKey,
} from "../src/components/multisigApprovals"

const signers = ["0:SIGNER_0", "0:SIGNER_1", "0:SIGNER_2", "0:SIGNER_3", "0:SIGNER_4"]
const order = {
  address: "0:ORDER",
  multisig_address: "0:WALLET",
  order_seqno: "1",
  threshold: 3,
  sent_for_execution: true,
  approvals_mask: "25",
  approvals_num: 3,
  expiration_date: 2_000_000_000,
  order_boc: null,
  signers,
  last_transaction_lt: "1",
  code_hash: null,
  data_hash: null,
  actions: [],
} satisfies V3MultisigOrder

describe("multisig approval times", () => {
  test("combines explicit approvals with the creator's initial approval", () => {
    const times = collectMultisigApprovalTimes(order, [
      createOrderAction(signers[4], 4, 100),
      approveAction(signers[3], 200),
      approveAction(signers[0], 300),
    ])

    expect(times).toEqual(
      new Map([
        [multisigAddressKey(signers[4]), 100],
        [multisigAddressKey(signers[3]), 200],
        [multisigAddressKey(signers[0]), 300],
      ]),
    )
  })

  test("prefers an explicit approval and ignores failed or unapproved signers", () => {
    const times = collectMultisigApprovalTimes(order, [
      createOrderAction(signers[4], 4, 100),
      approveAction(signers[4], 250),
      approveAction(signers[1], 300),
      approveAction(signers[0], 350, {success: false}),
      approveAction(signers[0], 400, {exitCode: 1}),
    ])

    expect(times).toEqual(new Map([[multisigAddressKey(signers[4]), 250]]))
  })

  test("sorts known times in ascending order and keeps unknown times last", () => {
    const values = [200, undefined, 100]

    expect([...values].sort((left, right) => compareMultisigApprovalTimes(left, right))).toEqual([
      100,
      200,
      undefined,
    ])
  })
})

function createOrderAction(source: string, creatorIndex: number, timestamp: number): V3Action {
  return {
    ...actionBase("create", timestamp),
    type: "multisig_create_order",
    details: {
      query_id: null,
      order_seqno: "1",
      is_created_by_signer: true,
      is_signed_by_creator: false,
      creator_index: creatorIndex,
      expiration_date: order.expiration_date,
      order_boc: null,
      source,
      destination: order.multisig_address,
      destination_order: order.address,
    },
  }
}

function approveAction(
  source: string,
  timestamp: number,
  overrides: {readonly success?: boolean; readonly exitCode?: number} = {},
): V3Action {
  return {
    ...actionBase(`approve:${source}:${timestamp}`, timestamp, overrides.success ?? true),
    type: "multisig_approve",
    details: {
      signer_index: -1,
      exit_code: overrides.exitCode ?? 0,
      source,
      destination: order.address,
    },
  }
}

function actionBase(actionId: string, timestamp: number, success = true) {
  return {
    action_id: actionId,
    end_lt: "1",
    end_utime: timestamp,
    finality: "finalized",
    start_lt: "1",
    start_utime: timestamp,
    success,
    trace_end_lt: "1",
    trace_end_utime: timestamp,
    trace_id: "trace",
    trace_mc_seqno_end: 1,
    transactions: [],
  }
}
