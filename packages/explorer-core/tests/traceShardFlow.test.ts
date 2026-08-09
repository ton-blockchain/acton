import {describe, expect, test} from "bun:test"

import {buildTraceShardFlow} from "../src/components/traceShardFlow"

describe("buildTraceShardFlow", () => {
  test("shows shard transitions in logical-time order", () => {
    const flow = buildTraceShardFlow([
      {
        id: "tx-5",
        lt: "5",
        blockRef: {workchain: 0, shard: "4000000000000000", seqno: 12},
      },
      {
        id: "tx-1",
        lt: "1",
        blockRef: {workchain: 0, shard: "4000000000000000", seqno: 10},
      },
      {
        id: "tx-3",
        lt: "3",
        blockRef: {workchain: 0, shard: "C000000000000000", seqno: 8},
      },
      {
        id: "tx-2",
        lt: "2",
        blockRef: {workchain: 0, shard: "C000000000000000", seqno: 8},
      },
      {
        id: "tx-4",
        lt: "4",
        blockRef: {workchain: 0, shard: "4000000000000000", seqno: 11},
      },
      {
        id: "tx-6",
        lt: "6",
        blockRef: {workchain: 0, shard: "4000000000000000", seqno: 12},
      },
    ])

    expect(flow).toEqual({
      shardCount: 2,
      transactionCount: 6,
      segments: [
        {
          workchain: 0,
          shard: "4000000000000000",
          transactionCount: 1,
          transactionIds: ["tx-1"],
          blocks: [{workchain: 0, shard: "4000000000000000", seqno: 10}],
        },
        {
          workchain: 0,
          shard: "C000000000000000",
          transactionCount: 2,
          transactionIds: ["tx-2", "tx-3"],
          blocks: [{workchain: 0, shard: "C000000000000000", seqno: 8}],
        },
        {
          workchain: 0,
          shard: "4000000000000000",
          transactionCount: 3,
          transactionIds: ["tx-4", "tx-5", "tx-6"],
          blocks: [
            {workchain: 0, shard: "4000000000000000", seqno: 11},
            {workchain: 0, shard: "4000000000000000", seqno: 12},
          ],
        },
      ],
    })
  })

  test("ignores transactions without a block reference", () => {
    const flow = buildTraceShardFlow([
      {id: "missing-block", lt: "1", blockRef: undefined},
      {
        id: "masterchain-tx",
        lt: "2",
        blockRef: {workchain: -1, shard: "8000000000000000", seqno: 42},
      },
    ])

    expect(flow.shardCount).toBe(1)
    expect(flow.transactionCount).toBe(1)
    expect(flow.segments).toHaveLength(1)
  })
})
