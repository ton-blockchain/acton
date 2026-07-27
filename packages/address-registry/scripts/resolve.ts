import type {AddressConflict} from "./merge.ts"
import type {AddressSourceId, SourceAddress} from "./sources/shared.ts"

export interface ConflictResolution {
  readonly address: string
  readonly source: AddressSourceId
  readonly name: string
}

export interface ConflictResolutionResult {
  readonly addresses: readonly SourceAddress[]
  readonly unresolved: readonly AddressConflict[]
}

export const resolveConflicts = (
  conflicts: readonly AddressConflict[],
  resolutions: readonly ConflictResolution[],
): ConflictResolutionResult => {
  const conflictsByAddress = new Map(conflicts.map(conflict => [conflict.address, conflict]))
  const resolutionsByAddress = new Map<string, ConflictResolution>()

  for (const resolution of resolutions) {
    if (resolutionsByAddress.has(resolution.address)) {
      throw new Error(`Duplicate conflict resolution for ${resolution.address}`)
    }

    const conflict = conflictsByAddress.get(resolution.address)
    if (!conflict) {
      throw new Error(`Conflict resolution for ${resolution.address} is stale`)
    }

    const matchesCandidate = conflict.candidates.some(
      candidate => candidate.source === resolution.source && candidate.name === resolution.name,
    )
    if (!matchesCandidate) {
      throw new Error(
        `Conflict resolution for ${resolution.address} selects an unknown candidate: ${resolution.source} / ${resolution.name}`,
      )
    }

    resolutionsByAddress.set(resolution.address, resolution)
  }

  const addresses: SourceAddress[] = []
  const unresolved: AddressConflict[] = []

  for (const conflict of conflicts) {
    const resolution = resolutionsByAddress.get(conflict.address)
    if (resolution) {
      addresses.push({address: conflict.address, name: resolution.name})
    } else {
      unresolved.push(conflict)
    }
  }

  return {addresses, unresolved}
}
