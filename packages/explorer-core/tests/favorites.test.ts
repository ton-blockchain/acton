import {Buffer} from "node:buffer"

import {describe, expect, test} from "bun:test"

import {parseFavoriteAccounts} from "../src/hooks/useFavoriteAccounts"
import {parseFavoriteBlocks} from "../src/hooks/useFavoriteBlocks"
import {parseFavoriteTransactions} from "../src/hooks/useFavoriteTransactions"

describe("favorites persistence", () => {
  test("keeps the existing account favorites format compatible", () => {
    expect(
      parseFavoriteAccounts(
        JSON.stringify([
          {
            address: `0:${"1".repeat(64)}`,
            savedAt: 1_700_000_000_000,
          },
          {
            address: "invalid but preserved account identifier",
            savedAt: -1,
          },
          {
            address: `0:${"1".repeat(64)}`,
            savedAt: 1_800_000_000_000,
          },
          {address: 42, savedAt: 1},
        ]),
      ),
    ).toMatchSnapshot()
  })

  test("normalizes and validates saved block metadata", () => {
    expect(
      parseFavoriteBlocks(
        JSON.stringify([
          {
            workchain: -1,
            shard: "8000000000000000",
            seqno: 123,
            generatedAt: 1_700_000_000,
            savedAt: 1_700_000_000_000,
          },
          {
            workchain: 0,
            shard: "ABCDEF0000000000",
            seqno: 456,
            savedAt: 1_800_000_000_000,
          },
          {
            workchain: -1,
            shard: "8000000000000000",
            seqno: 123,
            savedAt: 1_900_000_000_000,
          },
          {workchain: 0, shard: "1", seqno: -1, savedAt: 1},
          {workchain: 0, shard: "1", seqno: 1, generatedAt: "now", savedAt: 1},
        ]),
      ),
    ).toMatchSnapshot()
  })

  test("normalizes and validates saved transaction metadata", () => {
    const firstHash = "ab".repeat(32)
    const secondHash = Buffer.alloc(32, 0xcd).toString("base64url")

    expect(
      parseFavoriteTransactions(
        JSON.stringify([
          {
            hash: firstHash.toUpperCase(),
            account: `0:${"2".repeat(64)}`,
            lt: " 123456 ",
            timestamp: 1_700_000_000,
            savedAt: 1_700_000_000_000,
          },
          {
            hash: secondHash,
            savedAt: 1_800_000_000_000,
          },
          {
            hash: firstHash,
            account: "duplicate",
            savedAt: 1_900_000_000_000,
          },
          {hash: "not a transaction hash", savedAt: 1},
          {hash: secondHash, account: 42, savedAt: 1},
        ]),
      ),
    ).toMatchSnapshot()
  })

  test("ignores malformed storage values", () => {
    expect({
      invalidJson: parseFavoriteTransactions("{"),
      object: parseFavoriteTransactions("{}"),
      empty: parseFavoriteTransactions(null),
    }).toMatchSnapshot()
  })
})
