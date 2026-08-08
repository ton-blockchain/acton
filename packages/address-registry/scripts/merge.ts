import {Address} from "@ton/core"

import type {AddressSource, AddressSourceId, SourceAddress} from "./sources/shared.ts"

export interface AddressCandidate {
  readonly source: AddressSourceId
  readonly name: string
}

export interface AddressConflict {
  readonly address: string
  readonly candidates: readonly AddressCandidate[]
}

export interface MergeResult {
  readonly addresses: readonly SourceAddress[]
  readonly conflicts: readonly AddressConflict[]
}

export const mergeSources = (sources: readonly AddressSource[]): MergeResult => {
  const groups = new Map<string, AddressCandidate[]>()

  for (const source of sources) {
    for (const {address: sourceAddress, name} of source.addresses) {
      const address = Address.parse(sourceAddress).toRawString()
      const candidates = groups.get(address) ?? []
      const hasCandidate = candidates.some(
        candidate => candidate.source === source.id && candidate.name === name,
      )

      if (!hasCandidate) {
        candidates.push({source: source.id, name})
        groups.set(address, candidates)
      }
    }
  }

  const addresses: SourceAddress[] = []
  const conflicts: AddressConflict[] = []

  for (const [address, candidates] of groups) {
    const names = new Set(candidates.map(({name}) => name))
    if (names.size === 1) {
      addresses.push({address, name: candidates[0].name})
    } else {
      conflicts.push({address, candidates})
    }
  }

  return {addresses, conflicts}
}
