import {describe, expect, test} from "bun:test"

import type {JettonWallet} from "../src/api/types"
import {sortJettonWalletsForDisplay} from "../src/api/jettonWallets"

const USDT_MASTER = "EQCxE6mUtQJKFnGfaROTKOt1lZbDiiX1kCixRv7Nw2Id_sDs"

describe("Jetton wallet amounts", () => {
  test("sorts large balances exactly across token decimal scales", () => {
    const wallets = [
      wallet("smaller", "900719925474099299999999", "9", "SMALL"),
      wallet("one-b", "1000000", "6", "B"),
      wallet("larger", "900719925474099300000000", "9", "LARGE"),
      wallet("one-a", "1", "0", "A"),
    ]

    expect(sortJettonWalletsForDisplay(wallets).map(item => item.address)).toMatchInlineSnapshot(`
      [
        "larger",
        "smaller",
        "one-a",
        "one-b",
      ]
    `)
  })

  test("keeps mainnet USD₮ first when it is present", () => {
    const wallets = [
      wallet("large", "1000000000000", "6", "LARGE"),
      wallet("usdt", "1", "6", "USD₮", USDT_MASTER),
      wallet("medium", "1000000", "6", "MEDIUM"),
    ]

    expect(sortJettonWalletsForDisplay(wallets).map(item => item.address)).toMatchInlineSnapshot(`
      [
        "usdt",
        "large",
        "medium",
      ]
    `)
  })
})

function wallet(
  address: string,
  balance: string,
  decimals: string,
  symbol: string,
  jetton = `master-${address}`,
): JettonWallet {
  return {
    address,
    balance,
    code_hash: "",
    data_hash: "",
    jetton,
    last_transaction_lt: "0",
    master: {
      address: jetton,
      jetton_content: {decimals, symbol},
    },
    owner: "owner",
  }
}
