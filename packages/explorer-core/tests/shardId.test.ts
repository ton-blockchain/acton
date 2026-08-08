import {describe, expect, test} from "bun:test"

import {v2ShardToV3Shard, v3ShardToV2Shard} from "../src/api/shardId"

describe("Toncenter shard ID conversion", () => {
  test("preserves canonical V3 shard IDs after a V3 -> V2 -> V3 round trip", () => {
    const shardIds = [
      "0000000000000000",
      "0000000000000001",
      "4000000000000000",
      "7FFFFFFFFFFFFFFF",
      "8000000000000000",
      "C000000000000000",
      "FFFFFFFFFFFFFFFF",
    ]

    for (const shardId of shardIds) {
      expect(v2ShardToV3Shard(v3ShardToV2Shard(shardId))).toBe(shardId)
    }
  })

  test("preserves canonical V2 shard IDs after a V2 -> V3 -> V2 round trip", () => {
    const shardIds = [
      "-9223372036854775808",
      "-4611686018427387904",
      "-1",
      "0",
      "1",
      "4611686018427387904",
      "9223372036854775807",
    ]

    for (const shardId of shardIds) {
      expect(v3ShardToV2Shard(v2ShardToV3Shard(shardId))).toBe(shardId)
    }
  })
})
