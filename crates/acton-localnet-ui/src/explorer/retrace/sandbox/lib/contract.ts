import {Address, type ShardAccount, type StateInit} from "@ton/core"

import type {TestData} from "./test-data"

type ContractMeta = {
  readonly abi?: {
    readonly types?: readonly {readonly header?: number}[]
  }
  readonly treasurySeed?: unknown
}

export type ContractData = {
  readonly address: Address
  readonly meta: ContractMeta | undefined
  readonly stateInit: StateInit | undefined
  readonly account: ShardAccount
  readonly letter: string
  readonly displayName: string
  readonly kind: "treasury" | "user-contract"
}

function areCodeEqual(it: ContractData, init: StateInit) {
  return it.stateInit?.code?.toBoc()?.toString("hex") === init?.code?.toBoc()?.toString("hex")
}

export function isContractDeployedInside(
  test: TestData,
  contracts: Map<string, ContractData>,
): boolean {
  for (const tx of test.transactions) {
    const src = tx.transaction?.inMessage?.info?.src
    if (!src) continue

    const srcContract = contracts.get(src.toString())
    if (srcContract?.meta?.treasurySeed) {
      continue // deployed by the treasury, not very interesting
    }

    for (const [_, value] of tx.transaction.outMessages) {
      const init = value.init
      if (!init) continue // not a deployment

      // search for contract with the same code
      const contract = [...contracts.values()].find(contract => areCodeEqual(contract, init))
      if (contract) {
        return true
      }
    }
  }
  return false
}
