import {describe, expect, test} from "bun:test"

import type {JettonWallet} from "../src/api/types"
import {sortJettonWalletsByAmount} from "../src/api/jettonWallets"

describe("Jetton wallet amounts", () => {
  test("sorts large balances exactly across token decimal scales", () => {
    const wallets = [
      wallet("smaller", "900719925474099299999999", "9", "SMALL"),
      wallet("one-b", "1000000", "6", "B"),
      wallet("larger", "900719925474099300000000", "9", "LARGE"),
      wallet("one-a", "1", "0", "A"),
    ]

    expect(sortJettonWalletsByAmount(wallets).map(item => item.address)).toMatchInlineSnapshot(`
      [
        "larger",
        "smaller",
        "one-a",
        "one-b",
      ]
    `)
  })
})

function wallet(address: string, balance: string, decimals: string, symbol: string): JettonWallet {
  return {
    address,
    balance,
    code_hash: "",
    data_hash: "",
    jetton: `master-${address}`,
    last_transaction_lt: "0",
    master: {
      address: `master-${address}`,
      jetton_content: {decimals, symbol},
    },
    owner: "owner",
  }
}
