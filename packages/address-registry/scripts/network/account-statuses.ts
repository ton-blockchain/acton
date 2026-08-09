import {Address} from "@ton/core"

export interface AccountStatusEntry {
  readonly address: string
  readonly status: string
}

export const findMatchingStatusAddresses = (
  mainnetStates: readonly AccountStatusEntry[],
  testnetStates: readonly AccountStatusEntry[],
): ReadonlySet<string> => {
  const testnetStatuses = new Map(
    testnetStates.map(state => [Address.parse(state.address).toRawString(), state.status]),
  )

  return new Set(
    mainnetStates.flatMap(state => {
      const address = Address.parse(state.address).toRawString()
      return testnetStatuses.get(address) === state.status ? [address] : []
    }),
  )
}
