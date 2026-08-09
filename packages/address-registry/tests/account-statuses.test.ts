import {expect, test} from "bun:test"

import {
  findMatchingStatusAddresses,
  type AccountStatusEntry,
} from "../scripts/network/account-statuses.ts"

const address = (digit: string): string => `0:${digit.repeat(64)}`
const state = (digit: string, status: string): AccountStatusEntry => ({
  address: address(digit),
  status,
})

test("keeps only addresses with the same status in mainnet and testnet", () => {
  const mainnetStates = [
    state("0", "active"),
    state("1", "uninit"),
    state("2", "frozen"),
    state("3", "nonexist"),
    state("4", "active"),
    state("5", "nonexist"),
  ]
  const testnetStates = [
    state("0", "active"),
    state("1", "uninit"),
    state("2", "frozen"),
    state("3", "nonexist"),
    state("4", "frozen"),
  ]

  expect([...findMatchingStatusAddresses(mainnetStates, testnetStates)]).toEqual([
    address("0"),
    address("1"),
    address("2"),
    address("3"),
  ])
})
